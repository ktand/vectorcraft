//! Fork: unfolding a polyhedron into a flat net (C15.4; the start of C15.6's papercraft). Our own
//! method: the faces are laid flat one by one along a spanning tree of the faces (a face joins
//! the net across an edge it shares with one already laid, turned about that edge into the
//! plane), so the tree's edges become folds and every other edge is cut. Spanning trees are tried
//! from each face in turn, breadth first and depth first, until one lays no face over another.
//! Each cut edge then gets one glue tab, on whichever of its two sides the tab clears the rest.
//!
//! Faces are convex, planar and listed counter-clockwise seen from outside; the solid is closed
//! (every edge in two faces). Anything else is refused, not unfolded wrongly.

use std::collections::{BTreeMap, VecDeque};

use vectorcraft_geom::PathData;

use super::{Edge, Net, P, polygon};

/// A 3D point.
pub(super) type V3 = [f64; 3];

/// A polyhedron: its corners and its faces (corner indices, counter-clockwise from outside).
pub(super) struct Solid {
    pub verts: Vec<V3>,
    pub faces: Vec<Vec<usize>>,
}

/// At most this many faces are unfolded (a cap on work for untrusted meshes).
pub(super) const MAX_FACES: usize = 2_000;

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}
fn norm(a: V3) -> Option<V3> {
    let l = dot(a, a).sqrt();
    (l > 1e-12 && l.is_finite()).then(|| [a[0] / l, a[1] / l, a[2] / l])
}

/// A face's corners in its own plane: the first corner at the origin, the first edge along x,
/// counter-clockwise seen from outside.
pub(super) fn flat(s: &Solid, f: &[usize]) -> Option<Vec<P>> {
    let p: Vec<V3> = f.iter().map(|&i| s.verts.get(i).copied()).collect::<Option<_>>()?;
    let (o, a) = (*p.first()?, *p.get(1)?);
    let u = norm(sub(a, o))?;
    // Newell's normal, robust for any planar polygon.
    let mut n = [0.0; 3];
    for (i, q) in p.iter().enumerate() {
        let r = p[(i + 1) % p.len()];
        n[0] += (q[1] - r[1]) * (q[2] + r[2]);
        n[1] += (q[2] - r[2]) * (q[0] + r[0]);
        n[2] += (q[0] - r[0]) * (q[1] + r[1]);
    }
    let n = norm(n)?;
    let w = cross(n, u);
    Some(p.iter().map(|&q| (dot(sub(q, o), u), dot(sub(q, o), w))).collect())
}

/// The rigid motion (turn and shift, no mirror) taking `a0`→`b0` and `a1`→`b1` (the same length).
pub(super) fn align(a0: P, a1: P, b0: P, b1: P) -> impl Fn(P) -> P {
    let ta = (a1.1 - a0.1).atan2(a1.0 - a0.0);
    let tb = (b1.1 - b0.1).atan2(b1.0 - b0.0);
    let (s, c) = (tb - ta).sin_cos();
    move |p: P| {
        let (x, y) = (p.0 - a0.0, p.1 - a0.1);
        (b0.0 + c * x - s * y, b0.1 + s * x + c * y)
    }
}

/// Whether two convex polygons overlap by more than touching (separating axes, with a small
/// tolerance so faces sharing an edge or a corner don't count).
pub(super) fn overlap(a: &[P], b: &[P]) -> bool {
    const EPS: f64 = 1e-6;
    for poly in [a, b] {
        for i in 0..poly.len() {
            let (p, q) = (poly[i], poly[(i + 1) % poly.len()]);
            let n = (q.1 - p.1, p.0 - q.0);
            let l = n.0.hypot(n.1);
            if l < 1e-12 {
                continue;
            }
            let proj = |pts: &[P]| {
                pts.iter().map(|r| (r.0 * n.0 + r.1 * n.1) / l).fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), v| (lo.min(v), hi.max(v)))
            };
            let (a0, a1) = proj(a);
            let (b0, b1) = proj(b);
            if a1 <= b0 + EPS || b1 <= a0 + EPS {
                return false;
            }
        }
    }
    true
}

/// A face laid flat: its corners in the net, and the face it joined across (none for the first).
struct Laid {
    pts: Vec<P>,
    parent: Option<usize>,
}

/// The edges as corner pairs (lower first) → the faces and the edge's position in each.
/// Edge (corner pair, lower first) → the faces that have it and the edge's place in each.
type EdgeFaces = BTreeMap<(usize, usize), Vec<(usize, usize)>>;

fn edges(s: &Solid) -> Result<EdgeFaces, String> {
    let mut m: EdgeFaces = BTreeMap::new();
    for (fi, f) in s.faces.iter().enumerate() {
        if f.len() < 3 {
            return Err("a face has fewer than three corners".to_string());
        }
        for i in 0..f.len() {
            let (a, b) = (f[i], f[(i + 1) % f.len()]);
            if a == b {
                return Err("a face repeats a corner".to_string());
            }
            m.entry((a.min(b), a.max(b))).or_default().push((fi, i));
        }
    }
    if m.values().any(|v| v.len() != 2) {
        return Err("the solid isn't closed: every edge must join exactly two faces".to_string());
    }
    Ok(m)
}

/// Lays the faces out along a spanning tree from `root`, breadth first or depth first; `None` if
/// two faces would overlap.
fn lay(s: &Solid, flats: &[Vec<P>], adj: &[Vec<(usize, usize, usize)>], root: usize, depth_first: bool) -> Option<Vec<Laid>> {
    let mut laid: Vec<Option<Laid>> = (0..s.faces.len()).map(|_| None).collect();
    laid[root] = Some(Laid { pts: flats[root].clone(), parent: None });
    let mut queue = VecDeque::from([root]);
    while let Some(f) = if depth_first { queue.pop_back() } else { queue.pop_front() } {
        for &(i, g, j) in &adj[f] {
            if laid[g].is_some() {
                continue;
            }
            let here = laid[f].as_ref()?.pts.clone();
            let n = s.faces[f].len();
            let m = s.faces[g].len();
            // Edge i of f runs a→b; in g the same edge, edge j, runs b→a.
            let (a, b) = (here[i], here[(i + 1) % n]);
            let fg = &flats[g];
            let to = align(fg[(j + 1) % m], fg[j], a, b);
            let pts: Vec<P> = fg.iter().map(|&p| to(p)).collect();
            if laid.iter().flatten().any(|l| overlap(&l.pts, &pts)) {
                return None;
            }
            laid[g] = Some(Laid { pts, parent: Some(f) });
            queue.push_back(g);
        }
    }
    laid.into_iter().collect()
}

/// The solid unfolded: the outline with a glue tab `tab` mm deep (at most a third of its edge) on
/// one side of every cut edge, the folds, and the tabs as glue.
pub(super) fn unfold(s: &Solid, tab: f64) -> Result<Net, String> {
    if s.faces.is_empty() || s.faces.len() > MAX_FACES {
        return Err(format!("a solid of 1 to {MAX_FACES} faces"));
    }
    let edge_map = edges(s)?;
    let flats: Vec<Vec<P>> = s.faces.iter().map(|f| flat(s, f).ok_or("a face has no area")).collect::<Result<_, _>>()?;
    // Each face's neighbours: (its edge, the other face, that face's edge).
    let mut adj: Vec<Vec<(usize, usize, usize)>> = vec![vec![]; s.faces.len()];
    for v in edge_map.values() {
        if let [(f, i), (g, j)] = v.as_slice() {
            adj[*f].push((*i, *g, *j));
            adj[*g].push((*j, *f, *i));
        }
    }
    for a in &mut adj {
        a.sort_unstable();
    }
    let laid = (0..s.faces.len().min(64))
        .flat_map(|root| [false, true].map(|d| (root, d)))
        .find_map(|(root, d)| lay(s, &flats, &adj, root, d))
        .ok_or("no way was found to lay this solid flat without faces overlapping")?;
    // The folds: each face's edge to its parent. Every other edge occurrence is cut.
    let mut folds = vec![];
    let mut is_fold = vec![vec![false; 0]; s.faces.len()];
    for (f, l) in laid.iter().enumerate() {
        is_fold[f] = vec![false; l.pts.len()];
    }
    for (f, l) in laid.iter().enumerate() {
        let Some(p) = l.parent else { continue };
        if let Some(&(i, _, j)) = adj[f].iter().find(|(_, g, _)| *g == p) {
            is_fold[f][i] = true;
            is_fold[p][j] = true;
            let n = l.pts.len();
            let mut e = Edge::from(l.pts[i]);
            e.to(l.pts[(i + 1) % n]);
            folds.push(e.open());
        }
    }
    // A tab for each cut edge, on the first of its two sides where it clears the faces and the
    // tabs already placed.
    let faces: Vec<Vec<P>> = laid.iter().map(|l| l.pts.clone()).collect();
    let mut tabs: Vec<Vec<P>> = vec![];
    let mut tab_on: BTreeMap<(usize, usize), Vec<P>> = BTreeMap::new();
    for v in edge_map.values() {
        let [(f, i), (g, j)] = v.as_slice() else { continue };
        if is_fold[*f][*i] {
            continue;
        }
        // Full depth with 45° ends first; then steeper ends (a narrow gap between two faces), then
        // shallower, on either side.
        'shapes: for &(deep, slope) in &[(1.0, 1.0), (1.0, 1.8), (0.6, 1.8), (0.6, 3.0), (0.35, 4.0)] {
            for &(face, k) in &[(*f, *i), (*g, *j)] {
                let pts = &faces[face];
                let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
                let t = tab_shape(a, b, tab * deep, slope);
                let clear = faces.iter().all(|q| !overlap(q, &t)) && tabs.iter().all(|q| !overlap(q, &t));
                if clear {
                    tabs.push(t.clone());
                    tab_on.insert((face, k), t);
                    break 'shapes;
                }
            }
        }
    }
    let outline = boundary(&faces, &is_fold, &tab_on)?;
    let glue: Vec<_> = tabs.iter().map(|t| polygon(t)).collect();
    Ok(Net { cut: PathData::single(outline.closed()), folds: PathData::new(folds), glue: PathData::new(glue), face: None, art: vec![] })
}

/// A tab on edge `a`→`b` of a face whose corners run with a positive signed area (as every laid
/// face does: they are only turned and shifted, never mirrored), so the face is on the edge's
/// left and the tab goes on its right: a trapezoid `depth` deep (at most a third of the edge), its
/// ends sloping in by `slope` times the depth along the edge (1: 45°).
pub(super) fn tab_shape(a: P, b: P, depth: f64, slope: f64) -> Vec<P> {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let l = dx.hypot(dy);
    // Its top at least a fifth of the edge long.
    let d = depth.min(l / 3.0).min(0.4 * l / slope);
    let (ux, uy) = (dx / l, dy / l);
    let (nx, ny) = (uy, -ux);
    let i = d * slope;
    vec![a, b, (b.0 - ux * i + nx * d, b.1 - uy * i + ny * d), (a.0 + ux * i + nx * d, a.1 + uy * i + ny * d)]
}

/// The net's outline: the faces' cut edges joined end to end, a tab's three sides in place of
/// each edge that carries one.
pub(super) fn boundary(faces: &[Vec<P>], is_fold: &[Vec<bool>], tabs: &BTreeMap<(usize, usize), Vec<P>>) -> Result<Edge, String> {
    let key = |p: P| ((p.0 * 1e6).round() as i64, (p.1 * 1e6).round() as i64);

    // Directed cut edges, start → (end, the points between).
    type Outgoing = BTreeMap<(i64, i64), Vec<(P, Vec<P>)>>;
    let mut next: Outgoing = BTreeMap::new();
    let mut count = 0usize;
    for (f, pts) in faces.iter().enumerate() {
        for k in 0..pts.len() {
            if is_fold[f][k] {
                continue;
            }
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            let via = tabs.get(&(f, k)).map(|t| vec![t[3], t[2]]).unwrap_or_default();
            next.entry(key(a)).or_default().push((b, via));
            count += 1;
        }
    }
    let start =
        faces.iter().enumerate().find_map(|(f, pts)| (0..pts.len()).find(|&k| !is_fold[f][k]).map(|k| pts[k])).ok_or("the net has no outline")?;
    let mut e = Edge::from(start);
    let mut at = start;
    for _ in 0..count {
        let Some(outs) = next.get_mut(&key(at)) else { return Err("the net's outline doesn't join up".to_string()) };
        let Some((to, via)) = outs.pop() else { return Err("the net's outline doesn't join up".to_string()) };
        for p in via {
            e.to(p);
        }
        e.to(to);
        at = to;
        if key(at) == key(start) && next.values().all(Vec::is_empty) {
            break;
        }
    }
    if key(at) != key(start) || next.values().any(|v| !v.is_empty()) {
        return Err("the net's outline doesn't close".to_string());
    }
    Ok(e)
}
