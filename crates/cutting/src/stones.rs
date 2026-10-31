//! Fork: rhinestone templates (C13.4). Stones are set through holes cut in template material: the
//! art becomes round holes the stones' size, evenly spaced along each of its outlines, and with
//! `fill` a staggered grid of them inside, kept clear of the outline's row. The art is taken as
//! filled, as it is cut ([`crate::bridges`] takes it the same way). The holes are in document
//! points; the engine adds them as a path.

use vectorcraft_doc::{Document, NodeId};
use vectorcraft_geom::{BezPath, FillRule, PathData, PathEl, Point, Rect, Shape, shapes};
use vectorcraft_pathops::{BoolOp, Cap, DEFAULT_PRECISION, Join, outline_stroke, try_boolean};

use crate::placement::MM_PER_PT;

/// A hole's diameter, mm: the range, and the usual (for SS10 stones, about 2.8 mm across).
pub const HOLE_MM: (f64, f64) = (1.0, 10.0);
pub const DEFAULT_HOLE_MM: f64 = 3.0;
/// The material between two holes, mm: the range, and the usual.
pub const GAP_MM: (f64, f64) = (0.3, 20.0);
pub const DEFAULT_GAP_MM: f64 = 1.0;
/// Most holes one template has.
const MAX_STONES: usize = 20_000;
/// Outlines are followed to within this, points.
const FLAT_PT: f64 = 0.05;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// A hole's diameter, mm ([`HOLE_MM`]).
    pub hole_mm: f64,
    /// The material between two holes, mm ([`GAP_MM`]).
    pub gap_mm: f64,
    /// Holes inside the shape as well as along its outlines.
    pub fill: bool,
}

impl Default for Options {
    fn default() -> Self {
        Options { hole_mm: DEFAULT_HOLE_MM, gap_mm: DEFAULT_GAP_MM, fill: false }
    }
}

/// The template for the art of `ids` (and what is inside them): its holes' centres, document
/// points, or why there are none.
pub fn centres(d: &Document, ids: &[NodeId], opt: Options) -> Result<Vec<Point>, String> {
    let ok = |v: f64, (lo, hi): (f64, f64)| v.is_finite() && (lo..=hi).contains(&v);
    if !ok(opt.hole_mm, HOLE_MM) || !ok(opt.gap_mm, GAP_MM) {
        return Err(format!("holes are {} to {} mm, {} to {} mm apart", HOLE_MM.0, HOLE_MM.1, GAP_MM.0, GAP_MM.1));
    }
    let area = crate::bridges::area(d, ids).map_err(|e| e.replace("to bridge", "to set stones in"))?;
    let pitch = (opt.hole_mm + opt.gap_mm) / MM_PER_PT;
    let mut out = vec![];
    for s in &area.subpaths {
        along(&PathData::single(s.clone()).to_bezpath(), pitch, &mut out)?;
    }
    if opt.fill {
        // Inside, a row's width in from the outline: a staggered grid, rows `pitch`·√3/2 apart.
        // The area less a band `pitch` wide each side of its outlines (whichever way they wind).
        let band = outline_stroke(&area, 2.0 * pitch, Cap::Round, Join::Round, 4.0);
        let inner = try_boolean(&area, FillRule::NonZero, &band, FillRule::NonZero, BoolOp::Difference, DEFAULT_PRECISION)
            .map_err(|e| format!("the inside couldn't be worked out: {e}"))?;
        if !inner.is_empty() {
            grid(&inner.to_bezpath(), pitch, &mut out)?;
        }
    }
    if out.is_empty() {
        return Err("the art is too small for holes that size".into());
    }
    Ok(out)
}

/// Centres evenly spaced along closed outline `ring`, about `pitch` apart, added to `out`.
fn along(ring: &BezPath, pitch: f64, out: &mut Vec<Point>) -> Result<(), String> {
    let mut pts: Vec<Point> = vec![];
    vectorcraft_geom::flatten(ring.elements().iter().copied(), FLAT_PT, |el| {
        if let PathEl::MoveTo(p) | PathEl::LineTo(p) = el {
            pts.push(p);
        }
    });
    if let Some(first) = pts.first().copied() {
        pts.push(first);
    }
    let len: f64 = pts.windows(2).map(|w| (w[1] - w[0]).hypot()).sum();
    let n = (len / pitch).floor();
    if !(n.is_finite() && n >= 1.0) {
        return Ok(());
    }
    if out.len() as f64 + n > MAX_STONES as f64 {
        return Err(format!("that would be more than {MAX_STONES} holes: use larger ones"));
    }
    // Whole and small after the check: the cast is exact.
    let (n, step) = (n as usize, len / n);
    let (mut placed, mut walked) = (0usize, 0.0);
    for w in pts.windows(2) {
        let seg = (w[1] - w[0]).hypot();
        while placed < n && placed as f64 * step <= walked + seg {
            let t = if seg > 0.0 { (placed as f64 * step - walked) / seg } else { 0.0 };
            out.push(w[0].lerp(w[1], t));
            placed += 1;
        }
        walked += seg;
    }
    Ok(())
}

/// A staggered grid of centres `pitch` apart inside `region` (non-zero), added to `out`.
fn grid(region: &BezPath, pitch: f64, out: &mut Vec<Point>) -> Result<(), String> {
    let b = region.bounding_box();
    let row = pitch * 3f64.sqrt() / 2.0;
    let (rows, cols) = ((b.height() / row).floor() + 1.0, (b.width() / pitch).floor() + 2.0);
    if !(rows.is_finite() && cols.is_finite()) || out.len() as f64 + rows * cols > MAX_STONES as f64 * 4.0 {
        return Err(format!("that would be more than {MAX_STONES} holes: use larger ones"));
    }
    // Whole and bounded after the check: the casts are exact.
    for r in 0..rows as usize {
        // Half a step in from the bounds: no centre on the region's own edge, where the winding
        // test can't say.
        let y = b.y0 + row / 2.0 + r as f64 * row;
        let shift = if r % 2 == 1 { pitch / 2.0 } else { 0.0 };
        for k in 0..cols as usize {
            let q = Point::new(b.x0 + pitch / 2.0 + shift + k as f64 * pitch, y);
            if region.winding(q) != 0 {
                if out.len() >= MAX_STONES {
                    return Err(format!("that would be more than {MAX_STONES} holes: use larger ones"));
                }
                out.push(q);
            }
        }
    }
    Ok(())
}

/// The holes: a circle `hole_mm` across round each centre.
pub fn holes(centres: &[Point], hole_mm: f64) -> PathData {
    let r = hole_mm / MM_PER_PT / 2.0;
    PathData::new(centres.iter().flat_map(|c| shapes::ellipse(Rect::new(c.x - r, c.y - r, c.x + r, c.y + r)).subpaths).collect())
}

#[cfg(test)]
mod tests {
    use vectorcraft_doc::{Appearance, Node};

    use super::*;

    fn square(side_mm: f64) -> (Document, Vec<NodeId>) {
        let mut d = Document::new(500.0, 500.0);
        let layer = d.layers[0].id;
        let s = side_mm / MM_PER_PT;
        let n = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(10.0, 10.0, 10.0 + s, 10.0 + s)), Appearance::default());
        let id = d.insert(Some(layer), usize::MAX, n).unwrap();
        (d, vec![id])
    }

    fn nearest(c: &[Point]) -> f64 {
        let mut best = f64::INFINITY;
        for (i, a) in c.iter().enumerate() {
            for b in c.iter().skip(i + 1) {
                best = best.min((*a - *b).hypot());
            }
        }
        best * MM_PER_PT
    }

    /// A 40 mm square, 3 mm holes 1 mm apart: 160 mm round it, 40 holes 4 mm apart on the line.
    #[test]
    fn holes_go_evenly_round_the_outline() {
        let (d, ids) = square(40.0);
        let c = centres(&d, &ids, Options::default()).unwrap();
        assert_eq!(c.len(), 40);
        assert!((nearest(&c) - 4.0).abs() < 0.05, "{}", nearest(&c));
        let on_edge = |p: &Point| {
            let s = 40.0 / MM_PER_PT;
            [p.x - 10.0, p.y - 10.0, 10.0 + s - p.x, 10.0 + s - p.y].iter().any(|v| v.abs() < 1e-3)
        };
        assert!(c.iter().all(on_edge));
        assert_eq!(holes(&c, 3.0).subpaths.len(), 40);
    }

    /// Filled: more holes inside, none closer than the pitch to another.
    #[test]
    fn a_fill_adds_a_grid_inside() {
        let (d, ids) = square(40.0);
        let c = centres(&d, &ids, Options { fill: true, ..Options::default() }).unwrap();
        assert!(c.len() > 100, "{}", c.len());
        assert!(nearest(&c) > 4.0 * 0.85, "the holes keep apart: {}", nearest(&c));
    }

    #[test]
    fn bad_options_and_art_are_refused() {
        let (d, ids) = square(40.0);
        assert!(centres(&d, &ids, Options { hole_mm: 0.5, ..Options::default() }).is_err());
        assert!(centres(&d, &ids, Options { gap_mm: f64::NAN, ..Options::default() }).is_err());
        let (d, ids) = square(2.0);
        assert!(centres(&d, &ids, Options { hole_mm: 10.0, ..Options::default() }).unwrap_err().contains("too small"));
        let (d, ids) = square(20_000.0);
        assert!(centres(&d, &ids, Options { hole_mm: 1.0, gap_mm: 0.3, fill: true }).unwrap_err().contains("more than"));
    }
}
