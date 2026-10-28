//! The order paths are cut in (ARCHITECTURE.md › 5.1, step 7).
//!
//! Everything scored is scored before anything is cut: cutting a piece free first would let it
//! move. Within each operation, with [`Ordering::InsideFirst`], a path inside a closed path is cut
//! before it (the counter of an O before the O), so the piece around it is still held by the mat;
//! among the paths that may go next, the nearest one does, a closed path starting at its point
//! nearest the blade and an open one from whichever end is nearer. Past [`MAX_OPTIMIZED`] paths
//! the search would take too long: inner paths still go first, in the document's order.
//! [`Ordering::Document`] keeps the document's paint order.

use vectorcraft_cutjob::{Ordering, RectMm};

use crate::prepare::Polyline;

/// Most paths of one operation ordered by the nearest-path search (it compares every pair).
pub const MAX_OPTIMIZED: usize = 4_000;

/// `paths` in the order they are cut, starting from `origin` (mm, where the blade waits).
pub fn order(mut paths: Vec<Polyline>, ordering: Ordering, origin: [f64; 2]) -> Vec<Polyline> {
    // A stable sort: each operation's paths keep their order until ordered below.
    paths.sort_by_key(|p| p.op);
    if ordering == Ordering::Document {
        return paths;
    }
    let mut out = Vec::with_capacity(paths.len());
    let mut at = origin;
    let mut rest = paths.into_iter().peekable();
    while let Some(first) = rest.next() {
        let mut group = vec![first];
        while let Some(p) = rest.next_if(|p| p.op == group[0].op) {
            group.push(p);
        }
        let ordered = order_group(group, at);
        if let Some(end) = ordered.last().and_then(end_of) {
            at = end;
        }
        out.extend(ordered);
    }
    out
}

fn dist(a: [f64; 2], b: [f64; 2]) -> f64 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Where the blade lifts after `p`.
fn end_of(p: &Polyline) -> Option<[f64; 2]> {
    if p.closed { p.points.first().copied() } else { p.points.last().copied() }
}

fn bounds(p: &Polyline) -> RectMm {
    p.points.iter().fold(RectMm { x0: f64::INFINITY, y0: f64::INFINITY, x1: f64::NEG_INFINITY, y1: f64::NEG_INFINITY }, |b, q| RectMm {
        x0: b.x0.min(q[0]),
        y0: b.y0.min(q[1]),
        x1: b.x1.max(q[0]),
        y1: b.y1.max(q[1]),
    })
}

/// `q` is inside closed polygon `poly` (even-odd).
fn in_polygon(q: [f64; 2], poly: &[[f64; 2]]) -> bool {
    let mut inside = false;
    let mut j = poly.len().wrapping_sub(1);
    for (i, a) in poly.iter().enumerate() {
        let Some(b) = poly.get(j) else { continue };
        if (a[1] > q[1]) != (b[1] > q[1]) && q[0] < (b[0] - a[0]) * (q[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Path `inner` lies inside closed path `outer`: within its bounds, and a point of it inside.
fn contains(outer: &Polyline, ob: RectMm, inner: &Polyline, ib: RectMm) -> bool {
    outer.closed
        && ib.x0 >= ob.x0
        && ib.y0 >= ob.y0
        && ib.x1 <= ob.x1
        && ib.y1 <= ob.y1
        && (ib.x0, ib.y0, ib.x1, ib.y1) != (ob.x0, ob.y0, ob.x1, ob.y1)
        && inner.points.iter().any(|q| in_polygon(*q, &outer.points))
}

/// For each path, how many of the others lie inside it.
fn inner_counts(paths: &[Polyline], b: &[RectMm]) -> (Vec<usize>, Vec<Vec<usize>>) {
    let mut inside = vec![0; paths.len()];
    let mut containers = vec![vec![]; paths.len()];
    for (o, outer) in paths.iter().enumerate() {
        if !outer.closed {
            continue;
        }
        for (i, inner) in paths.iter().enumerate() {
            if i != o
                && let (Some(&ob), Some(&ib)) = (b.get(o), b.get(i))
                && contains(outer, ob, inner, ib)
            {
                if let Some(n) = inside.get_mut(o) {
                    *n += 1;
                }
                if let Some(c) = containers.get_mut(i) {
                    c.push(o);
                }
            }
        }
    }
    (inside, containers)
}

/// `p` started where it is nearest `at`: a closed path turned to its nearest point, an open one
/// reversed when its last point is nearer. With the distance to that start.
fn nearest_start(p: &Polyline, at: [f64; 2]) -> (f64, Option<usize>, bool) {
    if p.closed {
        let best = p.points.iter().enumerate().map(|(i, q)| (dist(at, *q), i)).min_by(|a, b| a.0.total_cmp(&b.0));
        best.map_or((f64::INFINITY, None, false), |(d, i)| (d, Some(i), false))
    } else {
        let first = p.points.first().map_or(f64::INFINITY, |q| dist(at, *q));
        let last = p.points.last().map_or(f64::INFINITY, |q| dist(at, *q));
        if last < first { (last, None, true) } else { (first, None, false) }
    }
}

/// One operation's paths in order, from `at`.
fn order_group(paths: Vec<Polyline>, at: [f64; 2]) -> Vec<Polyline> {
    let b: Vec<RectMm> = paths.iter().map(bounds).collect();
    if paths.len() > MAX_OPTIMIZED {
        // Too many to compare each pair: the innermost first by area, a cheap stand-in for depth.
        let mut v: Vec<(f64, Polyline)> = paths.into_iter().zip(&b).map(|(p, r)| ((r.x1 - r.x0) * (r.y1 - r.y0), p)).collect();
        v.sort_by(|a, b| a.0.total_cmp(&b.0));
        return v.into_iter().map(|(_, p)| p).collect();
    }
    let (mut waiting_inside, containers) = inner_counts(&paths, &b);
    let mut slots: Vec<Option<Polyline>> = paths.into_iter().map(Some).collect();
    let mut out = Vec::with_capacity(slots.len());
    let mut at = at;
    loop {
        // The nearest path with nothing left to cut inside it.
        let mut best: Option<(f64, usize, Option<usize>, bool)> = None;
        for (i, slot) in slots.iter().enumerate() {
            let Some(p) = slot else { continue };
            if waiting_inside.get(i).is_some_and(|n| *n > 0) {
                continue;
            }
            let (d, rotate, reverse) = nearest_start(p, at);
            if best.is_none_or(|b| d < b.0) {
                best = Some((d, i, rotate, reverse));
            }
        }
        let Some((_, i, rotate, reverse)) = best else { break };
        let Some(mut p) = slots.get_mut(i).and_then(Option::take) else { break };
        if let Some(r) = rotate {
            p.points.rotate_left(r);
        }
        if reverse {
            p.points.reverse();
        }
        for &c in containers.get(i).into_iter().flatten() {
            if let Some(n) = waiting_inside.get_mut(c) {
                *n = n.saturating_sub(1);
            }
        }
        if let Some(e) = end_of(&p) {
            at = e;
        }
        out.push(p);
    }
    // Containment needs a smaller box, so it can't go round in a circle and nothing is left;
    // anything that were would still be cut, last.
    out.extend(slots.into_iter().flatten());
    out
}

#[cfg(test)]
mod tests;
