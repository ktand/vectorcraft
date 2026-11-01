//! Fork: sketch fill (C13.3, C14.5). A pen draws lines, not areas: a sketch fill gives a shape's
//! inside lines a set distance apart, for the machine to draw with a pen: straight lines at a set
//! angle (and across them too, for cross-hatching), rings each the spacing in from the last
//! (concentric), or a spiral from the middle out, each clipped to the shape. The art is taken as filled, as it is
//! cut ([`crate::bridges`] takes it the same way); the lines are in document points, and the
//! engine adds them as a path.

use vectorcraft_doc::{Document, NodeId};
use vectorcraft_geom::{BezPath, FillRule, PathData, Point, Shape};

use crate::placement::MM_PER_PT;

/// The distance between lines, mm: the range, and the usual.
pub const SPACING_MM: (f64, f64) = (0.3, 20.0);
pub const DEFAULT_SPACING_MM: f64 = 2.0;
/// The lines' angle by default, degrees anticlockwise from across.
pub const DEFAULT_ANGLE_DEG: f64 = 45.0;
/// Most lines one fill draws before they are clipped.
const MAX_LINES: usize = 20_000;

/// What a sketch fill draws (the maker's software's patterns, protocol notes [SM]).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pattern {
    /// Straight lines at the angle; crossed with [`Options::cross`].
    #[default]
    Lines,
    /// Rings, each the spacing in from the one outside it.
    Concentric,
    /// A spiral from the middle out, its turns the spacing apart.
    Spiral,
}

impl Pattern {
    pub fn id(self) -> &'static str {
        match self {
            Pattern::Lines => "lines",
            Pattern::Concentric => "concentric",
            Pattern::Spiral => "spiral",
        }
    }

    pub fn parse(s: &str) -> Option<Self> {
        [Pattern::Lines, Pattern::Concentric, Pattern::Spiral].into_iter().find(|p| p.id() == s)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    pub pattern: Pattern,
    /// The distance between lines, mm ([`SPACING_MM`]).
    pub spacing_mm: f64,
    /// The lines' angle, degrees anticlockwise from across (any; taken modulo 180).
    pub angle_deg: f64,
    /// A second set of lines at right angles to the first.
    pub cross: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { pattern: Pattern::Lines, spacing_mm: DEFAULT_SPACING_MM, angle_deg: DEFAULT_ANGLE_DEG, cross: false }
    }
}

/// The sketch fill of the art of `ids` (and what is inside them): open lines, document points, or
/// why there are none.
pub fn hatch(d: &Document, ids: &[NodeId], opt: Options) -> Result<PathData, String> {
    if !(opt.spacing_mm.is_finite() && (SPACING_MM.0..=SPACING_MM.1).contains(&opt.spacing_mm)) {
        return Err(format!("the spacing is {} to {} mm", SPACING_MM.0, SPACING_MM.1));
    }
    if !opt.angle_deg.is_finite() {
        return Err("the angle isn't a number".into());
    }
    let area = crate::bridges::area(d, ids).map_err(|e| e.replace("to bridge", "to fill"))?;
    let spacing = opt.spacing_mm / MM_PER_PT;
    match opt.pattern {
        Pattern::Lines => {}
        Pattern::Concentric => return concentric(&area, spacing),
        Pattern::Spiral => return spiral(&area, spacing),
    }
    let region: crate::clip::Region = (area.to_bezpath(), FillRule::NonZero);
    let b = region.0.bounding_box();
    let (c, r) = (b.center(), 0.5 * (b.width().hypot(b.height())) + 1.0);
    let half = (r / spacing).ceil();
    let angles: &[f64] = if opt.cross { &[0.0, 90.0] } else { &[0.0] };
    if !(half.is_finite() && (2.0 * half + 1.0) * angles.len() as f64 <= MAX_LINES as f64) {
        return Err(format!("that would be more than {MAX_LINES} lines: use a wider spacing"));
    }
    // Whole and small after the check: the cast is exact.
    let half = half as i64;
    let mut out: Vec<BezPath> = vec![];
    for extra in angles {
        let (sin, cos) = (opt.angle_deg + extra).rem_euclid(180.0).to_radians().sin_cos();
        // Along the line (y down on the page: anticlockwise is up), and across it.
        let (u, n) = ((cos, -sin), (sin, cos));
        for k in -half..=half {
            let o = k as f64 * spacing;
            let mid = Point::new(c.x + n.0 * o, c.y + n.1 * o);
            let mut line = BezPath::new();
            line.move_to(Point::new(mid.x - u.0 * r, mid.y - u.1 * r));
            line.line_to(Point::new(mid.x + u.0 * r, mid.y + u.1 * r));
            let pieces = crate::clip::clip(&line, false, &region).ok_or("a line couldn't be clipped to the art")?;
            out.extend(pieces.into_iter().map(|(p, _)| p));
        }
    }
    if out.is_empty() {
        return Err("the art is too small for lines that far apart".into());
    }
    Ok(PathData::new(out.iter().flat_map(|p| PathData::from_bezpath(p).subpaths).collect()))
}

/// Most rings a concentric fill draws.
const MAX_RINGS: usize = 1_000;
/// Most points a spiral is drawn with.
const MAX_SPIRAL_POINTS: usize = 500_000;
/// How many of a spiral's points are clipped at a time (clipping compares every pair of segments).
const SPIRAL_CHUNK: usize = 400;

/// Rings `spacing` (points) apart inside `area`, from the outside in: its outlines shrunk by
/// one spacing, two, … until nothing is left.
fn concentric(area: &PathData, spacing: f64) -> Result<PathData, String> {
    let mut out = vec![];
    for k in 1..=MAX_RINGS {
        let ring = crate::bridges::shrunk(area, spacing * k as f64)?;
        if ring.is_empty() {
            break;
        }
        out.extend(ring.subpaths);
        if out.len() > MAX_LINES {
            return Err(format!("that would be more than {MAX_LINES} lines: use a wider spacing"));
        }
    }
    if out.is_empty() {
        return Err("the art is too small for lines that far apart".into());
    }
    Ok(PathData::new(out))
}

/// A spiral from the middle of `area`'s bounds out past its corners, its turns `spacing`
/// (points) apart, clipped to `area`.
fn spiral(area: &PathData, spacing: f64) -> Result<PathData, String> {
    let region: crate::clip::Region = (area.to_bezpath(), FillRule::NonZero);
    let b = region.0.bounding_box();
    let (c, reach) = (b.center(), 0.5 * b.width().hypot(b.height()) + spacing);
    // r = a·θ: a turn (2π) out by the spacing; a point each quarter spacing along it, at most.
    let a = spacing / std::f64::consts::TAU;
    let turns = reach / spacing;
    if !(turns.is_finite() && turns * turns * 16.0 * std::f64::consts::PI <= MAX_SPIRAL_POINTS as f64) {
        return Err(format!("that would be more than {MAX_SPIRAL_POINTS} points: use a wider spacing"));
    }
    let mut pts = vec![c];
    let mut theta = 0.0f64;
    while a * theta < reach && pts.len() < MAX_SPIRAL_POINTS {
        let r = (a * theta).max(spacing * 0.25);
        theta += (spacing * 0.25 / r).min(0.3);
        let (sin, cos) = theta.sin_cos();
        pts.push(Point::new(c.x + a * theta * cos, c.y + a * theta * sin));
    }
    let mut out: Vec<BezPath> = vec![];
    for chunk in pts.chunks(SPIRAL_CHUNK).enumerate().map(|(i, _)| {
        // Each chunk starts at the last one's end, so the pieces join.
        let from = (i * SPIRAL_CHUNK).saturating_sub(usize::from(i > 0));
        pts.get(from..((i + 1) * SPIRAL_CHUNK).min(pts.len())).unwrap_or_default()
    }) {
        let mut line = BezPath::new();
        let mut it = chunk.iter();
        let Some(first) = it.next() else { continue };
        line.move_to(*first);
        for p in it {
            line.line_to(*p);
        }
        let pieces = crate::clip::clip(&line, false, &region).ok_or("the spiral couldn't be clipped to the art")?;
        out.extend(pieces.into_iter().map(|(p, _)| p));
    }
    if out.is_empty() {
        return Err("the art is too small for lines that far apart".into());
    }
    Ok(PathData::new(out.iter().flat_map(|p| PathData::from_bezpath(p).subpaths).collect()))
}

#[cfg(test)]
mod tests {
    use vectorcraft_doc::{Appearance, Node};
    use vectorcraft_geom::{Rect, shapes};

    use super::*;

    fn square(side_mm: f64) -> (Document, Vec<NodeId>) {
        let mut d = Document::new(500.0, 500.0);
        let layer = d.layers[0].id;
        let s = side_mm / MM_PER_PT;
        let n = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(10.0, 10.0, 10.0 + s, 10.0 + s)), Appearance::default());
        let id = d.insert(Some(layer), usize::MAX, n).unwrap();
        (d, vec![id])
    }

    fn inside(p: &PathData, r: Rect) -> bool {
        p.subpaths
            .iter()
            .flat_map(|s| s.anchors.iter())
            .all(|a| a.p.x >= r.x0 - 1e-6 && a.p.x <= r.x1 + 1e-6 && a.p.y >= r.y0 - 1e-6 && a.p.y <= r.y1 + 1e-6)
    }

    /// Lines across a 50 mm square 5 mm apart: about ten, open, inside the square; across it at
    /// 0°, every line level.
    #[test]
    fn a_square_is_filled_with_lines() {
        let (d, ids) = square(50.0);
        let r = Rect::new(10.0, 10.0, 10.0 + 50.0 / MM_PER_PT, 10.0 + 50.0 / MM_PER_PT);
        let p = hatch(&d, &ids, Options { spacing_mm: 5.0, angle_deg: 0.0, cross: false, ..Options::default() }).unwrap();
        assert!((9..=11).contains(&p.subpaths.len()), "{}", p.subpaths.len());
        assert!(p.subpaths.iter().all(|s| !s.closed && s.anchors.len() == 2 && (s.anchors[0].p.y - s.anchors[1].p.y).abs() < 1e-6));
        assert!(inside(&p, r));
        let cross = hatch(&d, &ids, Options { spacing_mm: 5.0, angle_deg: 0.0, cross: true, ..Options::default() }).unwrap();
        assert!((18..=22).contains(&cross.subpaths.len()), "both ways: {}", cross.subpaths.len());
        let slanted = hatch(&d, &ids, Options { spacing_mm: 5.0, angle_deg: 45.0, cross: false, ..Options::default() }).unwrap();
        let s = &slanted.subpaths[slanted.subpaths.len() / 2].anchors;
        let (dx, dy) = (s[1].p.x - s[0].p.x, s[1].p.y - s[0].p.y);
        assert!((dx.abs() - dy.abs()).abs() < 1e-6 && dx * dy < 0.0, "up to the right at 45°: {dx} {dy}");
        assert!(inside(&slanted, r));
    }

    /// A ring's hole stays empty: no line point lies in it.
    #[test]
    fn holes_stay_empty() {
        let mut d = Document::new(500.0, 500.0);
        let layer = d.layers[0].id;
        let mut p = shapes::rectangle(Rect::new(0.0, 0.0, 200.0, 200.0));
        p.subpaths.extend(shapes::rectangle(Rect::new(50.0, 50.0, 150.0, 150.0)).subpaths);
        let ring = Node::path(d.alloc_id(), p, Appearance::default());
        let id = d.insert(Some(layer), usize::MAX, ring).unwrap();
        let h = hatch(&d, &[id], Options { spacing_mm: 3.0, angle_deg: 30.0, cross: false, ..Options::default() }).unwrap();
        let in_hole = |q: Point| q.x > 50.5 && q.x < 149.5 && q.y > 50.5 && q.y < 149.5;
        assert!(h.subpaths.iter().all(|s| s.anchors.iter().all(|a| !in_hole(a.p))));
        // Lines through the middle stop at the hole: two pieces each.
        assert!(h.subpaths.len() > 40);
    }

    #[test]
    fn bad_options_and_lines_are_refused() {
        let (d, ids) = square(50.0);
        assert!(hatch(&d, &ids, Options { spacing_mm: 0.1, ..Options::default() }).is_err());
        assert!(hatch(&d, &ids, Options { angle_deg: f64::NAN, ..Options::default() }).is_err());
        let (d, ids) = square(20_000.0);
        assert!(hatch(&d, &ids, Options { spacing_mm: 0.3, ..Options::default() }).unwrap_err().contains("wider spacing"));
        let mut d = Document::new(100.0, 100.0);
        let layer = d.layers[0].id;
        let line = Node::path(d.alloc_id(), shapes::line(Point::new(0.0, 0.0), Point::new(50.0, 50.0)), Appearance::default());
        let id = d.insert(Some(layer), usize::MAX, line).unwrap();
        assert!(hatch(&d, &[id], Options::default()).unwrap_err().contains("to fill"));
    }

    /// C14.5: concentric rings round a 50 mm square 5 mm apart: closed, each inside the last
    /// (about five); a spiral, open pieces inside the square.
    #[test]
    fn concentric_rings_and_a_spiral() {
        let (d, ids) = square(50.0);
        let r = Rect::new(10.0, 10.0, 10.0 + 50.0 / MM_PER_PT, 10.0 + 50.0 / MM_PER_PT);
        let rings = hatch(&d, &ids, Options { pattern: Pattern::Concentric, spacing_mm: 5.0, ..Options::default() }).unwrap();
        assert!((4..=5).contains(&rings.subpaths.len()), "{}", rings.subpaths.len());
        assert!(rings.subpaths.iter().all(|s| s.closed));
        let first = PathData::single(rings.subpaths[0].clone()).bounds().unwrap();
        assert!((first.x0 - (10.0 + 5.0 / MM_PER_PT)).abs() < 0.5, "the first a spacing in: {first:?}");
        assert!(inside(&rings, r));
        let spiral = hatch(&d, &ids, Options { pattern: Pattern::Spiral, spacing_mm: 5.0, ..Options::default() }).unwrap();
        assert!(!spiral.subpaths.is_empty() && spiral.subpaths.iter().all(|s| !s.closed));
        assert!(inside(&spiral, r));
        assert_eq!(Pattern::parse("spiral"), Some(Pattern::Spiral));
        assert_eq!(Pattern::parse("zigzag"), None);
    }
}
