//! The order paths are cut in (ARCHITECTURE.md › 5.1, step 7).
//!
//! Everything scored is scored before anything is cut: cutting a piece free first would let it
//! move. Within each operation, with [`Ordering::InsideFirst`], a path inside a closed path is cut
//! before it (the counter of an O before the O), so the piece around it is still held by the mat;
//! among the paths that may go next, the nearest one does, a closed path starting at its point
//! nearest the blade and an open one from whichever end is nearer. That order is then shortened
//! (C5.6, [`improve`]): stretches of it turned round where the blade travels less up, and each
//! closed path started where it suits the paths either side, inner paths still first, within a
//! work budget so the same document always gives the same job. Past [`MAX_OPTIMIZED`] paths the
//! search would take too long: inner paths still go first, in the document's order.
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
        let ordered = order_group(group, at, MAX_IMPROVE_WORK);
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

/// One operation's paths in order, from `at`, shortened with at most `work` moves weighed.
fn order_group(paths: Vec<Polyline>, at: [f64; 2], work: usize) -> Vec<Polyline> {
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
    let mut ids = Vec::with_capacity(slots.len());
    let origin = at;
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
        ids.push(i);
    }
    // Containment needs a smaller box, so it can't go round in a circle and nothing is left;
    // anything that were would still be cut, last.
    for (i, p) in slots.into_iter().enumerate() {
        if let Some(p) = p {
            out.push(p);
            ids.push(i);
        }
    }
    let mut work = work;
    improve(&mut out, &mut ids, &containers, origin, &mut work);
    out
}

/// Most moves the travel improvement weighs for one operation's paths: a work budget rather than a
/// clock, so the same document always gives the same job (a few hundred milliseconds at most).
pub const MAX_IMPROVE_WORK: usize = 20_000_000;

/// Where the blade goes down for `p`.
fn start_of(p: &Polyline) -> Option<[f64; 2]> {
    p.points.first().copied()
}

/// Where `p` would start and end cut the other way round: an open path swaps its ends; a closed
/// one starts and ends where it did.
fn reversed_ends(p: &Polyline) -> Option<([f64; 2], [f64; 2])> {
    let (first, last) = (p.points.first().copied()?, p.points.last().copied()?);
    Some(if p.closed { (first, first) } else { (last, first) })
}

/// Shortens the blade's travel up through `paths` (in cutting order, from `origin`) without
/// cutting a path before one inside it: segments of the order turned round where that shortens it
/// (2-opt; an open path in one is cut the other way), and each closed path started at the point
/// that suits its neighbours, until neither helps or `work` runs out. `ids[k]` is the original
/// index of `paths[k]` and `containers[i]` the original indices of the paths around path `i`.
fn improve(paths: &mut [Polyline], ids: &mut [usize], containers: &[Vec<usize>], origin: [f64; 2], work: &mut usize) {
    let n = paths.len();
    let mut pos = vec![0usize; n];
    for (k, &id) in ids.iter().enumerate() {
        if let Some(p) = pos.get_mut(id) {
            *p = k;
        }
    }
    let end = |p: &Polyline| end_of(p).unwrap_or(origin);
    let start = |p: &Polyline| start_of(p).unwrap_or(origin);
    for _round in 0..8 {
        let mut improved = false;
        // Segment turns.
        let mut i = 0;
        while i < n {
            let prev = if i == 0 { origin } else { paths.get(i - 1).map_or(origin, end) };
            let Some((_, first_rev_end)) = paths.get(i).and_then(reversed_ends) else {
                i += 1;
                continue;
            };
            let Some(first_start) = paths.get(i).map(start) else { break };
            let mut turned = false;
            for j in i..n {
                if *work == 0 {
                    return;
                }
                *work -= 1;
                let (Some(last), Some((last_rev_start, _))) = (paths.get(j), paths.get(j).and_then(reversed_ends)) else { continue };
                let next = paths.get(j + 1).map(start);
                let old = dist(prev, first_start) + next.map_or(0.0, |s| dist(end(last), s));
                let new = dist(prev, last_rev_start) + next.map_or(0.0, |s| dist(first_rev_end, s));
                if new < old - 1e-9 && turn_keeps_inner_first(ids, containers, &pos, i, j) {
                    turn(paths, ids, &mut pos, i, j);
                    improved = true;
                    turned = true;
                    break;
                }
            }
            if !turned {
                i += 1;
            }
        }
        // Closed paths' starts.
        for k in 0..n {
            let prev = if k == 0 { origin } else { paths.get(k - 1).map_or(origin, end) };
            let next = paths.get(k + 1).map(start);
            let Some(p) = paths.get_mut(k) else { continue };
            if !p.closed {
                continue;
            }
            *work = work.saturating_sub(p.points.len());
            let cost = |q: [f64; 2]| dist(prev, q) + next.map_or(0.0, |s| dist(q, s));
            let best = p.points.iter().enumerate().map(|(i, q)| (cost(*q), i)).min_by(|a, b| a.0.total_cmp(&b.0));
            if let (Some((c, b)), Some(&now)) = (best, p.points.first())
                && b != 0
                && c < cost(now) - 1e-9
            {
                p.points.rotate_left(b);
                improved = true;
            }
        }
        if !improved || *work == 0 {
            return;
        }
    }
}

/// Turning `paths[i..=j]` round keeps every path before the ones around it: none of a path's
/// containers is in the segment too.
fn turn_keeps_inner_first(ids: &[usize], containers: &[Vec<usize>], pos: &[usize], i: usize, j: usize) -> bool {
    ids.get(i..=j)
        .unwrap_or_default()
        .iter()
        .all(|&id| containers.get(id).into_iter().flatten().all(|c| pos.get(*c).is_none_or(|p| *p > j || *p < i)))
}

/// `paths[i..=j]` cut in the reverse order, each open one the other way.
fn turn(paths: &mut [Polyline], ids: &mut [usize], pos: &mut [usize], i: usize, j: usize) {
    if let Some(seg) = paths.get_mut(i..=j) {
        seg.reverse();
        for p in seg.iter_mut().filter(|p| !p.closed) {
            p.points.reverse();
        }
    }
    if let Some(seg) = ids.get_mut(i..=j) {
        seg.reverse();
        for (k, &id) in seg.iter().enumerate() {
            if let Some(p) = pos.get_mut(id) {
                *p = i + k;
            }
        }
    }
}

#[cfg(test)]
mod tests;
