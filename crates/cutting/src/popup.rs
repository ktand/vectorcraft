//! Fork: pop-up cards (C16.1). A card folds in half; the design, cut free of it except along two
//! straight folds, stands up when the card is opened to a right angle. The geometry is our own, the
//! parallel fold every paper pop-up of this kind uses:
//!
//! - The **face** is the design united with a **base**, a bar under it (its height a share of the
//!   design's), so that every part of the design stands on one straight fold, the base's bottom
//!   edge, where the face meets the card's floor.
//! - A **strut** rises from the top of each part of the design to a fold on the card's back, all at
//!   one height, so the face is held upright: the struts' fold with the face is the design's top,
//!   and their length is the face's depth in front of the back.
//! - The **card's fold** runs between them, halfway down the face. Then the face, standing on the
//!   floor at the base's fold, is as far in front of the back as the struts are long, and the
//!   struts are as high on the back as the face is tall: it stands upright at a right angle, and
//!   folds flat when the card is closed (the path from the struts' top to the base's bottom is as
//!   long along the card as through the face).
//!
//! The face's outline is cut except where it joins the card (the struts' tops, the base's bottom),
//! the folds are cut as dashes (perforated, so they bend), and the card is a rectangle round it all,
//! `margin` larger. Everything is in document points (y down); the engine adds it as a group.

use vectorcraft_doc::{Document, NodeId};
use vectorcraft_geom::{BezPath, FillRule, PathData, PathEl, Point, Rect, SubPath, shapes};
use vectorcraft_pathops::try_unite_all;

use crate::placement::MM_PER_PT;

/// The base's height, % of the design's: the range, and the usual.
pub const BASE_PCT: (f64, f64) = (2.0, 50.0);
pub const DEFAULT_BASE_PCT: f64 = 10.0;
/// A strut's width, mm.
pub const STRUT_MM: (f64, f64) = (0.5, 50.0);
pub const DEFAULT_STRUT_MM: f64 = 2.0;
/// The length of each dash of a fold, and of the gap after it, mm.
pub const DASH_MM: (f64, f64) = (0.25, 10.0);
pub const DEFAULT_DASH_MM: f64 = 1.0;
/// How much larger than the pop-up the card is on each side, mm.
pub const MARGIN_MM: (f64, f64) = (0.0, 200.0);
pub const DEFAULT_MARGIN_MM: f64 = 10.0;
/// Most parts of a design one pop-up holds up (a strut each).
const MAX_PARTS: usize = 2_000;
/// Most dashes the folds have.
const MAX_DASHES: usize = 100_000;
/// Outlines are followed to within this, points.
const FLAT_PT: f64 = 0.05;
/// A point this close to a fold's line is on it, points.
const ON_LINE: f64 = 1e-3;
/// How far a strut reaches down into its part of the design, at most, mm: enough to join it.
const STRUT_HOLD_MM: f64 = 2.0;
/// How far the base reaches up into the design, at most, mm.
const BASE_HOLD_MM: f64 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Options {
    /// The base's height, % of the design's ([`BASE_PCT`]).
    pub base_pct: f64,
    /// A strut's width, mm ([`STRUT_MM`]).
    pub strut_mm: f64,
    /// A dash's length and its gap's, mm ([`DASH_MM`]).
    pub dash_mm: f64,
    /// The card's margin round the pop-up, mm ([`MARGIN_MM`]).
    pub margin_mm: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options { base_pct: DEFAULT_BASE_PCT, strut_mm: DEFAULT_STRUT_MM, dash_mm: DEFAULT_DASH_MM, margin_mm: DEFAULT_MARGIN_MM }
    }
}

/// A pop-up card: what is cut, document points.
#[derive(Clone, Debug, PartialEq)]
pub struct PopUp {
    /// The card's outline.
    pub card: PathData,
    /// The face's outline, open where it joins the card.
    pub cuts: PathData,
    /// The folds, as dashes.
    pub folds: PathData,
    /// The card's fold: where it is, across the card.
    pub fold_y: f64,
    /// The struts holding the design up.
    pub struts: usize,
}

/// The pop-up card for the art of `ids` (and what is inside them), or why there is none.
pub fn popup(d: &Document, ids: &[NodeId], opt: Options) -> Result<PopUp, String> {
    check(opt)?;
    let area = crate::bridges::area(d, ids).map_err(|e| e.replace("to bridge", "to pop up"))?;
    popup_of(&area, opt)
}

fn check(opt: Options) -> Result<(), String> {
    let ok = |v: f64, (lo, hi): (f64, f64)| v.is_finite() && (lo..=hi).contains(&v);
    if !ok(opt.base_pct, BASE_PCT) {
        return Err(format!("the base is {} to {} % of the design's height", BASE_PCT.0, BASE_PCT.1));
    }
    if !ok(opt.strut_mm, STRUT_MM) {
        return Err(format!("a strut is {} to {} mm wide", STRUT_MM.0, STRUT_MM.1));
    }
    if !ok(opt.dash_mm, DASH_MM) {
        return Err(format!("the dashes are {} to {} mm", DASH_MM.0, DASH_MM.1));
    }
    if !ok(opt.margin_mm, MARGIN_MM) {
        return Err(format!("the margin is {} to {} mm", MARGIN_MM.0, MARGIN_MM.1));
    }
    Ok(())
}

/// The pop-up for a filled `area` (document points).
pub fn popup_of(area: &PathData, opt: Options) -> Result<PopUp, String> {
    check(opt)?;
    let b = area.bounds().filter(|b| b.width() > 0.0 && b.height() > 0.0 && b.width().is_finite() && b.height().is_finite());
    let Some(b) = b else { return Err("the design has no size to pop up".into()) };
    let rings = polylines(&area.to_bezpath());
    // The outsides of the design's parts: rings inside an even number of the others.
    let outsides: Vec<&Vec<Point>> = (0..rings.len()).filter(|&i| depth(&rings, i).is_multiple_of(2)).filter_map(|i| rings.get(i)).collect();
    if outsides.len() > MAX_PARTS {
        return Err(format!("at most {MAX_PARTS} parts can be held up"));
    }
    let base = b.height() * opt.base_pct / 100.0;
    let floor_y = b.y1 + base;
    let top_y = b.y0;
    let depth_pt = (floor_y - top_y) / 2.0;
    let fold_y = floor_y - depth_pt;
    let back_y = top_y - depth_pt;
    // No wider than the design, so a strut stays over it (and on the card).
    let half = (opt.strut_mm / MM_PER_PT / 2.0).min(b.width() / 2.0);

    let mut pieces = vec![area.clone(), rect(Rect::new(b.x0, b.y1 - (BASE_HOLD_MM / MM_PER_PT).min(base / 2.0), b.x1, floor_y))];
    let mut strut_x: Vec<(f64, f64)> = vec![];
    for ring in &outsides {
        let Some(top) = ring.iter().copied().min_by(|p, q| p.y.total_cmp(&q.y)) else { continue };
        let tall = ring.iter().map(|p| p.y).fold(f64::NEG_INFINITY, f64::max) - top.y;
        let hold = (STRUT_HOLD_MM / MM_PER_PT).min(tall / 2.0).max(0.0);
        // Kept over the design, so the strut's fold with the card is within the card's width.
        let x = top.x.clamp(b.x0 + half, (b.x1 - half).max(b.x0 + half));
        strut_x.push((x - half, x + half));
        pieces.push(rect(Rect::new(x - half, back_y, x + half, top.y + hold)));
    }
    let struts = strut_x.len();
    let strut_x = merged(strut_x);
    let inputs: Vec<(&PathData, FillRule)> = pieces.iter().map(|p| (p, FillRule::NonZero)).collect();
    let face = try_unite_all(&inputs).map_err(|e| format!("the pop-up couldn't be made: {e}"))?;
    let face_rings = polylines(&face.to_bezpath());

    // Cut round the face, but not where it joins the card.
    let base_x = [(b.x0, b.x1)];
    let joined = |a: Point, c: Point| on(a, c, back_y, &strut_x) || on(a, c, floor_y, &base_x);
    let mut cuts = vec![];
    for ring in &face_rings {
        cut_except(ring, &joined, &mut cuts);
    }

    let margin = opt.margin_mm / MM_PER_PT;
    let reach = (fold_y - back_y).max(floor_y - fold_y) + margin;
    let card = Rect::new(b.x0 - margin, fold_y - reach, b.x1 + margin, fold_y + reach);
    let dash = opt.dash_mm / MM_PER_PT;
    let mut folds = vec![];
    for (y, spans) in
        [(back_y, strut_x.clone()), (top_y, strut_x.clone()), (floor_y, base_x.to_vec()), (fold_y, outside(&face_rings, fold_y, card.x0, card.x1))]
    {
        for (x0, x1) in spans {
            dashes(x0, x1, y, dash, &mut folds)?;
        }
    }
    Ok(PopUp { card: rect(card), cuts: PathData::new(cuts), folds: PathData::new(folds), fold_y, struts })
}

fn rect(r: Rect) -> PathData {
    shapes::rectangle(r)
}

/// `p` flattened into closed rings (the first point again at the end).
fn polylines(p: &BezPath) -> Vec<Vec<Point>> {
    let mut out: Vec<Vec<Point>> = vec![];
    vectorcraft_geom::flatten(p.elements().iter().copied(), FLAT_PT, |el| match el {
        PathEl::MoveTo(a) => out.push(vec![a]),
        PathEl::LineTo(a) => {
            if let Some(last) = out.last_mut() {
                last.push(a);
            }
        }
        PathEl::ClosePath => {
            if let Some(last) = out.last_mut()
                && let Some(&first) = last.first()
                && last.last() != Some(&first)
            {
                last.push(first);
            }
        }
        _ => {}
    });
    out.retain(|r| r.len() >= 3);
    out
}

/// Whether `q` is inside the closed ring `r` (even-odd).
fn inside(r: &[Point], q: Point) -> bool {
    let mut odd = false;
    for w in r.windows(2) {
        let (Some(a), Some(c)) = (w.first(), w.get(1)) else { continue };
        if (a.y > q.y) != (c.y > q.y) && q.x < a.x + (q.y - a.y) * (c.x - a.x) / (c.y - a.y) {
            odd = !odd;
        }
    }
    odd
}

/// How many of the other rings ring `i` lies inside.
fn depth(rings: &[Vec<Point>], i: usize) -> usize {
    let Some(q) = rings.get(i).and_then(|r| r.first()).copied() else { return 0 };
    rings.iter().enumerate().filter(|(j, r)| *j != i && inside(r, q)).count()
}

/// `spans` sorted, the overlapping ones joined.
fn merged(mut spans: Vec<(f64, f64)>) -> Vec<(f64, f64)> {
    spans.sort_by(|a, b| a.0.total_cmp(&b.0));
    let mut out: Vec<(f64, f64)> = vec![];
    for s in spans {
        match out.last_mut() {
            Some(last) if s.0 <= last.1 => last.1 = last.1.max(s.1),
            _ => out.push(s),
        }
    }
    out
}

/// Whether the edge `a`–`c` lies along the line `y` within one of `spans`.
fn on(a: Point, c: Point, y: f64, spans: &[(f64, f64)]) -> bool {
    let (lo, hi) = (a.x.min(c.x), a.x.max(c.x));
    (a.y - y).abs() < ON_LINE && (c.y - y).abs() < ON_LINE && spans.iter().any(|(x0, x1)| lo >= x0 - ON_LINE && hi <= x1 + ON_LINE)
}

/// The ring `ring` as open lines, leaving out the edges `joined` says join the card.
fn cut_except(ring: &[Point], joined: &dyn Fn(Point, Point) -> bool, out: &mut Vec<SubPath>) {
    let edges: Vec<(Point, Point)> = ring.windows(2).filter_map(|w| Some((*w.first()?, *w.get(1)?))).collect();
    let Some(start) = edges.iter().position(|(a, c)| joined(*a, *c)) else {
        // Nothing joins: the whole ring is cut (a hole in the design).
        out.push(SubPath::polyline(ring, false));
        return;
    };
    // Walk from just after a joined edge, so each run of cut edges is one line.
    let mut line: Vec<Point> = vec![];
    for k in 1..=edges.len() {
        let Some(&(a, c)) = edges.get((start + k) % edges.len()) else { continue };
        if joined(a, c) {
            if line.len() >= 2 {
                out.push(SubPath::polyline(&line, false));
            }
            line.clear();
        } else {
            if line.is_empty() {
                line.push(a);
            }
            line.push(c);
        }
    }
    if line.len() >= 2 {
        out.push(SubPath::polyline(&line, false));
    }
}

/// The spans of `x0`–`x1` along the line `y` outside the rings.
fn outside(rings: &[Vec<Point>], y: f64, x0: f64, x1: f64) -> Vec<(f64, f64)> {
    let mut xs: Vec<f64> = vec![];
    for r in rings {
        for w in r.windows(2) {
            let (Some(a), Some(c)) = (w.first(), w.get(1)) else { continue };
            if (a.y > y) != (c.y > y) {
                xs.push(a.x + (y - a.y) * (c.x - a.x) / (c.y - a.y));
            }
        }
    }
    xs.sort_by(f64::total_cmp);
    let mut out = vec![];
    let mut from = x0;
    for pair in xs.chunks(2) {
        let [a, c] = pair else { continue };
        if *a > from {
            out.push((from, a.min(x1)));
        }
        from = from.max(*c);
    }
    if from < x1 {
        out.push((from, x1));
    }
    out.retain(|(a, c)| c > a);
    out
}

/// Dashes `dash` long with gaps as long along `y` from `x0` to `x1`.
fn dashes(x0: f64, x1: f64, y: f64, dash: f64, out: &mut Vec<SubPath>) -> Result<(), String> {
    let mut x = x0;
    while x < x1 {
        if out.len() >= MAX_DASHES {
            return Err(format!("the folds would have more than {MAX_DASHES} dashes: make them longer"));
        }
        let end = (x + dash).min(x1);
        out.push(SubPath::polyline(&[Point::new(x, y), Point::new(end, y)], false));
        x += 2.0 * dash;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(x: f64, y: f64, s: f64) -> PathData {
        rect(Rect::new(x, y, x + s, y + s))
    }

    fn all_points(p: &PathData) -> Vec<Point> {
        polylines(&p.to_bezpath())
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .into_iter()
            .chain(p.subpaths.iter().flat_map(|s| s.anchors.iter().map(|a| a.p)))
            .collect()
    }

    /// A square stands on a base and is held by one strut; its folds are where the parallel fold
    /// puts them, and the card's fold is halfway down the face.
    #[test]
    fn a_square_pops_up() {
        let mm = 1.0 / MM_PER_PT;
        let p = popup_of(&square(100.0, 100.0, 50.0 * mm), Options::default()).unwrap();
        assert_eq!(p.struts, 1);
        let base = 5.0 * mm;
        let floor = 100.0 + 50.0 * mm + base;
        assert!((p.fold_y - (100.0 + floor) / 2.0).abs() < 1e-6, "{}", p.fold_y);
        let fold_lines: Vec<f64> = {
            let mut ys: Vec<f64> = p.folds.subpaths.iter().filter_map(|s| s.anchors.first().map(|a| a.p.y)).collect();
            ys.sort_by(f64::total_cmp);
            ys.dedup_by(|a, b| (*a - *b).abs() < 1e-6);
            ys
        };
        let back = 100.0 - (floor - 100.0) / 2.0;
        for want in [back, 100.0, p.fold_y, floor] {
            assert!(fold_lines.iter().any(|y| (y - want).abs() < 1e-6), "a fold at {want}: {fold_lines:?}");
        }
        // The cut is open: the struts' tops and the base's bottom join the card.
        assert!(!p.cuts.is_empty() && p.cuts.subpaths.iter().all(|s| !s.closed));
        for s in &p.cuts.subpaths {
            for w in s.anchors.windows(2) {
                let (a, c) = (w[0].p, w[1].p);
                assert!(!((a.y - floor).abs() < 1e-6 && (c.y - floor).abs() < 1e-6), "the base's bottom isn't cut");
                assert!(!((a.y - back).abs() < 1e-6 && (c.y - back).abs() < 1e-6), "the strut's top isn't cut");
            }
        }
        // The card's fold isn't dashed across the face.
        let across: Vec<&SubPath> = p.folds.subpaths.iter().filter(|s| s.anchors.first().is_some_and(|a| (a.p.y - p.fold_y).abs() < 1e-6)).collect();
        assert!(across.iter().all(|s| s.anchors.iter().all(|a| a.p.x <= 100.0 + 1e-6 || a.p.x >= 100.0 + 50.0 * mm - 1e-6)));
    }

    /// Two parts get a strut each; a part with a hole has the hole cut all round.
    #[test]
    fn each_part_is_held_and_holes_are_cut() {
        let mm = 1.0 / MM_PER_PT;
        let ring = vectorcraft_pathops::try_boolean(
            &square(0.0, 0.0, 30.0 * mm),
            FillRule::NonZero,
            &square(10.0 * mm, 10.0 * mm, 10.0 * mm),
            FillRule::NonZero,
            vectorcraft_pathops::BoolOp::Difference,
            vectorcraft_pathops::DEFAULT_PRECISION,
        )
        .unwrap();
        let mut both = ring.clone();
        both.subpaths.extend(square(50.0 * mm, 10.0 * mm, 20.0 * mm).subpaths);
        let p = popup_of(&both, Options::default()).unwrap();
        assert_eq!(p.struts, 2);
        assert!(p.cuts.subpaths.iter().any(|s| s.anchors.iter().all(|a| (10.0 * mm - 1e-6..=20.0 * mm + 1e-6).contains(&a.p.x))), "the hole");
    }

    /// Nothing is cut outside the card, whatever the settings.
    #[test]
    fn everything_is_on_the_card() {
        let mm = 1.0 / MM_PER_PT;
        for opt in [Options::default(), Options { base_pct: 50.0, strut_mm: 50.0, dash_mm: 0.25, margin_mm: 0.0 }] {
            let p = popup_of(&square(0.0, 0.0, 40.0 * mm), opt).unwrap();
            let card = p.card.bounds().unwrap().inflate(1e-6, 1e-6);
            for q in all_points(&p.cuts).into_iter().chain(all_points(&p.folds)) {
                assert!(card.contains(q), "{q:?} off {card:?} with {opt:?}");
            }
        }
    }

    #[test]
    fn bad_settings_are_refused() {
        let s = square(0.0, 0.0, 100.0);
        assert!(popup_of(&s, Options { base_pct: 0.0, ..Options::default() }).is_err());
        assert!(popup_of(&s, Options { strut_mm: f64::NAN, ..Options::default() }).is_err());
        assert!(popup_of(&s, Options { dash_mm: 100.0, ..Options::default() }).is_err());
        assert!(popup_of(&PathData::new(vec![]), Options::default()).is_err());
    }
}
