//! Clipping masks applied to the cut (C3.3b): art inside a clip group or a clipping layer is cut
//! only where it shows. A closed outline becomes the outline of its visible part (the shape and
//! the mask's region intersected), so a piece half under a mask is cut out as the half that
//! shows; an open path is cut along the runs of it inside the region. The region is the one the
//! renderer clips to ([`vectorcraft_effects::clip_outline`]), under its fill rule.

use vectorcraft_doc::{Document, NodeId};
use vectorcraft_geom::{BezPath, FillRule, PathData, PathEl, Point};
use vectorcraft_pathops::{BoolOp, DEFAULT_PRECISION, try_boolean};

use crate::geometry::subpaths;

/// Open paths and regions are flattened to within this, points (finer than any cut tolerance).
const FLAT_PT: f64 = 0.01;
/// Most segment pairs one open path's clipping compares: past it, it isn't clipped.
const MAX_PAIRS: usize = 20_000_000;

/// A region to clip to: a path and its fill rule.
pub type Region = (BezPath, FillRule);

/// The region clip group (or clipping layer) `clip` clips its other members to; `None` when it
/// clips to nothing, which hides them.
pub fn region(d: &Document, clip: NodeId) -> Option<Region> {
    let first = d.node(clip)?.children()?.first()?;
    vectorcraft_effects::clip_outline(first)
}

/// Subpath `path` (closed or open) cut to `region`: the pieces of it to cut, each with whether it
/// is closed (none when nothing of it shows). `None` when it can't be clipped (the booleans
/// refused it, or it is too large to compare): the caller cuts it whole and says so.
pub fn clip(path: &BezPath, closed: bool, region: &Region) -> Option<Vec<(BezPath, bool)>> {
    if closed {
        let shape = PathData::from_bezpath(path);
        let mask = PathData::from_bezpath(&region.0);
        let visible = try_boolean(&shape, FillRule::NonZero, &mask, region.1, BoolOp::Intersect, DEFAULT_PRECISION).ok()?;
        return Some(subpaths(&visible.to_bezpath()).into_iter().map(|(p, _)| (p, true)).collect());
    }
    clip_open(path, region)
}

/// `p` flattened: its points, and for each subpath whether it is closed.
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
            {
                last.push(first);
            }
        }
        _ => {}
    });
    out
}

/// The winding number of the region's flattened outlines `rings` round `q`.
fn winding(q: Point, rings: &[Vec<Point>]) -> i32 {
    let mut w = 0;
    for ring in rings {
        for s in ring.windows(2) {
            let [a, b] = [s[0], s[1]];
            if a.y <= q.y {
                if b.y > q.y && (b.x - a.x) * (q.y - a.y) - (q.x - a.x) * (b.y - a.y) > 0.0 {
                    w += 1;
                }
            } else if b.y <= q.y && (b.x - a.x) * (q.y - a.y) - (q.x - a.x) * (b.y - a.y) < 0.0 {
                w -= 1;
            }
        }
    }
    w
}

fn inside(q: Point, rings: &[Vec<Point>], rule: FillRule) -> bool {
    let w = winding(q, rings);
    match rule {
        FillRule::EvenOdd => w % 2 != 0,
        _ => w != 0,
    }
}

/// Where segment `a`–`b` crosses segment `c`–`d`, as a fraction along `a`–`b`.
fn crossing(a: Point, b: Point, c: Point, d: Point) -> Option<f64> {
    let (r, s) = (b - a, d - c);
    let den = r.x * s.y - r.y * s.x;
    if den.abs() < 1e-12 {
        return None;
    }
    let q = c - a;
    let t = (q.x * s.y - q.y * s.x) / den;
    let u = (q.x * r.y - q.y * r.x) / den;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then_some(t)
}

/// Open subpath `path` cut to `region`: its runs inside.
fn clip_open(path: &BezPath, region: &Region) -> Option<Vec<(BezPath, bool)>> {
    let rings = polylines(&region.0);
    let edges: usize = rings.iter().map(|r| r.len().saturating_sub(1)).sum();
    let mut out = vec![];
    for line in polylines(path) {
        if line.len().saturating_sub(1).saturating_mul(edges) > MAX_PAIRS {
            return None;
        }
        let mut run: Vec<Point> = vec![];
        let flush = |run: &mut Vec<Point>, out: &mut Vec<(BezPath, bool)>| {
            if run.len() >= 2 {
                let mut bp = BezPath::new();
                for (i, q) in run.iter().enumerate() {
                    if i == 0 { bp.move_to(*q) } else { bp.line_to(*q) }
                }
                out.push((bp, false));
            }
            run.clear();
        };
        for seg in line.windows(2) {
            let [a, b] = [seg[0], seg[1]];
            let mut ts: Vec<f64> = vec![0.0, 1.0];
            for ring in &rings {
                for e in ring.windows(2) {
                    ts.extend(crossing(a, b, e[0], e[1]));
                }
            }
            ts.sort_by(f64::total_cmp);
            ts.dedup_by(|x, y| (*x - *y).abs() < 1e-12);
            let at = |t: f64| a + (b - a) * t;
            for w in ts.windows(2) {
                let (p0, p1) = (at(w[0]), at(w[1]));
                if inside(p0.midpoint(p1), &rings, region.1) {
                    if run.last().is_none_or(|q| (*q - p0).hypot() > 1e-9) {
                        flush(&mut run, &mut out);
                        run.push(p0);
                    }
                    run.push(p1);
                } else {
                    flush(&mut run, &mut out);
                }
            }
        }
        flush(&mut run, &mut out);
    }
    Some(out)
}

#[cfg(test)]
mod tests;
