//! Fork: papercraft from a 3D model (C15.6). Our own method, built on `unfold`'s steps: the
//! model's triangles turned to face one way (each shared edge running opposite ways in its two
//! faces, the whole outward when it encloses a volume), scaled so its longest side is the size
//! asked for, then laid flat in pieces: a piece grows from a face across its edges, a face that
//! would overlap the piece is left for another. Each fold between faces that aren't flat to each
//! other is a mountain fold (the faces bend away from the printed side) or a valley fold; each
//! edge that's cut has one glue tab where it clears its piece and a number, the same on both
//! sides, so the pieces are put together in pairs. The pieces are laid out in rows.

use std::cmp::Reverse;
use std::collections::{BTreeMap, BinaryHeap, VecDeque};

use vectorcraft_geom::{PathData, SubPath};

use super::unfold::{MAX_FACES, Solid, V3, align, boundary, flat, overlap, tab_shape};
use super::{Edge, Net, P, polygon};

/// The space between pieces, mm.
const GAP_MM: f64 = 6.0;
/// Faces closer to flat than this (degrees) get no fold line.
const FLAT_DEG: f64 = 1.0;

/// A number to print by a cut edge: where, along which direction (radians), and the number.
#[derive(Clone, Debug, PartialEq)]
pub struct Label {
    pub at: P,
    pub angle: f64,
    pub number: u32,
}

/// A papercraft net: the outline of every piece, the mountain and valley folds, the tabs, and the
/// edges' numbers, mm.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Craft {
    pub net: Net,
    pub valleys: PathData,
    pub labels: Vec<Label>,
    pub pieces: usize,
    /// The model's faces (triangles) laid flat.
    pub faces: usize,
}

/// The model in `bytes` (an OBJ or STL file named `name`) unfolded: see [`papercraft`].
pub fn from_file(name: &str, bytes: &[u8], size_mm: f64, tab: f64, row_mm: f64) -> Result<Craft, String> {
    papercraft(super::mesh::read(name, bytes)?, size_mm, tab, row_mm)
}

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn cross(a: V3, b: V3) -> V3 {
    [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]]
}

fn normal(s: &Solid, f: &[usize]) -> V3 {
    let (a, b, c) = (s.verts[f[0]], s.verts[f[1]], s.verts[f[2]]);
    let n = cross(sub(b, a), sub(c, a));
    let l = dot(n, n).sqrt().max(1e-300);
    [n[0] / l, n[1] / l, n[2] / l]
}

fn centroid(s: &Solid, f: &[usize]) -> V3 {
    let k = f.len() as f64;
    f.iter().fold([0.0; 3], |a, &i| [a[0] + s.verts[i][0] / k, a[1] + s.verts[i][1] / k, a[2] + s.verts[i][2] / k])
}

/// Edge (corner pair, lower first) → the faces that have it and the edge's place in each.
type Edges = BTreeMap<(usize, usize), Vec<(usize, usize)>>;

fn edge_map(s: &Solid) -> Result<Edges, String> {
    let mut m: Edges = BTreeMap::new();
    for (fi, f) in s.faces.iter().enumerate() {
        for i in 0..f.len() {
            let (a, b) = (f[i], f[(i + 1) % f.len()]);
            m.entry((a.min(b), a.max(b))).or_default().push((fi, i));
        }
    }
    if m.values().any(|v| v.len() > 2) {
        return Err("the model has an edge shared by more than two faces: it can't be folded from paper".to_string());
    }
    Ok(m)
}

/// Turns faces so each shared edge runs opposite ways in its two faces, then all of them outward
/// if the model encloses a volume (its signed volume negative otherwise).
fn orient(s: &mut Solid) -> Result<(), String> {
    let m = edge_map(s)?;
    let mut done = vec![false; s.faces.len()];
    for start in 0..s.faces.len() {
        if done[start] {
            continue;
        }
        done[start] = true;
        let mut queue = VecDeque::from([start]);
        while let Some(f) = queue.pop_front() {
            let face = s.faces[f].clone();
            for i in 0..face.len() {
                let (a, b) = (face[i], face[(i + 1) % face.len()]);
                let Some(both) = m.get(&(a.min(b), a.max(b))) else { continue };
                for &(g, j) in both {
                    if g == f || done[g] {
                        continue;
                    }
                    let gf = &s.faces[g];
                    // The same way round in both: g is flipped.
                    if gf[j] == a {
                        s.faces[g].reverse();
                    }
                    done[g] = true;
                    queue.push_back(g);
                }
            }
        }
    }
    let volume: f64 = s.faces.iter().map(|f| dot(s.verts[f[0]], cross(s.verts[f[1]], s.verts[f[2]]))).sum();
    if volume < 0.0 {
        for f in &mut s.faces {
            f.reverse();
        }
    }
    Ok(())
}

/// The model unfolded at `size_mm` along its longest side, tabs `tab` deep, the pieces laid out in
/// rows at most `row_mm` wide.
pub(super) fn papercraft(mut s: Solid, size_mm: f64, tab: f64, row_mm: f64) -> Result<Craft, String> {
    if s.faces.len() > MAX_FACES {
        return Err(format!("the model has {} faces; papercraft takes at most {MAX_FACES}: simplify it first", s.faces.len()));
    }
    // Scale to size, the lowest corner at the origin.
    let (mut lo, mut hi) = ([f64::INFINITY; 3], [f64::NEG_INFINITY; 3]);
    for v in &s.verts {
        for k in 0..3 {
            lo[k] = lo[k].min(v[k]);
            hi[k] = hi[k].max(v[k]);
        }
    }
    let span = (0..3).map(|k| hi[k] - lo[k]).fold(0.0, f64::max);
    if !(span.is_finite() && span > 0.0) {
        return Err("the model has no size".to_string());
    }
    let k = size_mm / span;
    for v in &mut s.verts {
        *v = [(v[0] - lo[0]) * k, (v[1] - lo[1]) * k, (v[2] - lo[2]) * k];
    }
    // Faces with no area are dropped (a sliver can't be laid flat).
    s.faces.retain(|f| {
        f.len() >= 3
            && dot(
                cross(sub(s.verts[f[1]], s.verts[f[0]]), sub(s.verts[f[2]], s.verts[f[0]])),
                cross(sub(s.verts[f[1]], s.verts[f[0]]), sub(s.verts[f[2]], s.verts[f[0]])),
            ) > 1e-12
    });
    if s.faces.is_empty() {
        return Err("the model has no faces with an area".to_string());
    }
    orient(&mut s)?;
    let edges = edge_map(&s)?;
    let flats: Vec<Vec<P>> = s.faces.iter().map(|f| flat(&s, f).ok_or("a face has no area")).collect::<Result<_, _>>()?;
    let mut adj: Vec<Vec<(usize, usize, usize)>> = vec![vec![]; s.faces.len()];
    for v in edges.values() {
        if let [(f, i), (g, j)] = v.as_slice() {
            adj[*f].push((*i, *g, *j));
            adj[*g].push((*j, *f, *i));
        }
    }
    // Grow pieces, joining the flattest neighbours first (Prim's order over the faces): faces in
    // one plane (a square split into two triangles) are laid together before any fold is made.
    let flatness = |f: usize, g: usize| (dot(normal(&s, &s.faces[f]), normal(&s, &s.faces[g])) * 1e9) as i64;
    let mut placed: Vec<Option<(usize, Vec<P>)>> = vec![None; s.faces.len()];
    let mut parent: Vec<Option<(usize, usize)>> = vec![None; s.faces.len()];
    let mut pieces: Vec<Vec<usize>> = vec![];
    for root in 0..s.faces.len() {
        if placed[root].is_some() {
            continue;
        }
        let pi = pieces.len();
        placed[root] = Some((pi, flats[root].clone()));
        let mut members = vec![root];
        let mut frontier: BinaryHeap<(i64, Reverse<usize>, usize, usize, usize)> = BinaryHeap::new();
        let push = |frontier: &mut BinaryHeap<_>, f: usize| {
            for &(i, g, j) in &adj[f] {
                frontier.push((flatness(f, g), Reverse(g), f, i, j));
            }
        };
        push(&mut frontier, root);
        while let Some((_, Reverse(g), f, i, j)) = frontier.pop() {
            if placed[g].is_some() {
                continue;
            }
            let Some((_, here)) = &placed[f] else { continue };
            let (a, b) = (here[i], here[(i + 1) % here.len()]);
            let fg = &flats[g];
            let to = align(fg[(j + 1) % fg.len()], fg[j], a, b);
            let pts: Vec<P> = fg.iter().map(|&p| to(p)).collect();
            if members.iter().any(|&m| placed[m].as_ref().is_some_and(|(_, q)| overlap(q, &pts))) {
                continue;
            }
            placed[g] = Some((pi, pts));
            parent[g] = Some((f, i));
            members.push(g);
            push(&mut frontier, g);
        }
        pieces.push(members);
    }
    // Folds, mountain or valley; which face edges are folds.
    let mut is_fold: Vec<Vec<bool>> = s.faces.iter().map(|f| vec![false; f.len()]).collect();
    let (mut mountains, mut valleys) = (vec![], vec![]);
    let cos_flat = FLAT_DEG.to_radians().cos();
    for g in 0..s.faces.len() {
        let Some((f, i)) = parent[g] else { continue };
        let Some(&(_, _, j)) = adj[f].iter().find(|&&(ii, gg, _)| ii == i && gg == g) else { continue };
        is_fold[f][i] = true;
        is_fold[g][j] = true;
        let (nf, ng) = (normal(&s, &s.faces[f]), normal(&s, &s.faces[g]));
        if dot(nf, ng) > cos_flat {
            continue;
        }
        let Some((_, pts)) = &placed[f] else { continue };
        let mut e = Edge::from(pts[i]);
        e.to(pts[(i + 1) % pts.len()]);
        // g bends away from f's outside: a mountain seen from the printed side.
        if dot(sub(centroid(&s, &s.faces[g]), centroid(&s, &s.faces[f])), nf) < 0.0 {
            mountains.push((f, e.open()));
        } else {
            valleys.push((f, e.open()));
        }
    }
    // Tabs and numbers on the cut edges that join two faces.
    let mut tab_on: BTreeMap<(usize, usize), Vec<P>> = BTreeMap::new();
    let mut tabs_in: Vec<Vec<Vec<P>>> = vec![vec![]; pieces.len()];
    let mut labels: Vec<(usize, P, f64, u32)> = vec![];
    let mut number = 0u32;
    for v in edges.values() {
        let [(f, i), (g, j)] = v.as_slice() else { continue };
        if is_fold[*f][*i] {
            continue;
        }
        number += 1;
        'shapes: for &(deep, slope) in &[(1.0, 1.0), (1.0, 1.8), (0.6, 1.8), (0.6, 3.0), (0.35, 4.0)] {
            for &(face, k) in &[(*f, *i), (*g, *j)] {
                let Some((pi, pts)) = &placed[face] else { continue };
                let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
                let t = tab_shape(a, b, tab * deep, slope);
                let clear = pieces[*pi].iter().all(|&m| placed[m].as_ref().is_none_or(|(_, q)| !overlap(q, &t)))
                    && tabs_in[*pi].iter().all(|q| !overlap(q, &t));
                if clear {
                    tabs_in[*pi].push(t.clone());
                    tab_on.insert((face, k), t);
                    break 'shapes;
                }
            }
        }
        for &(face, k) in &[(*f, *i), (*g, *j)] {
            let Some((_, pts)) = &placed[face] else { continue };
            let (a, b) = (pts[k], pts[(k + 1) % pts.len()]);
            let l = (b.0 - a.0).hypot(b.1 - a.1).max(1e-9);
            // Inside the face, a little in from the edge's middle.
            let inset = (l * 0.12).min(3.0);
            let (nx, ny) = (-(b.1 - a.1) / l, (b.0 - a.0) / l);
            let at = ((a.0 + b.0) / 2.0 + nx * inset, (a.1 + b.1) / 2.0 + ny * inset);
            labels.push((face, at, (b.1 - a.1).atan2(b.0 - a.0), number));
        }
    }
    // Each piece's outline, then the pieces laid out in rows.
    let mut craft = Craft { pieces: pieces.len(), faces: s.faces.len(), ..Craft::default() };
    let (mut x, mut y, mut row_h) = (0.0f64, 0.0f64, 0.0f64);
    let mut shift: Vec<(f64, f64)> = vec![(0.0, 0.0); pieces.len()];
    let mut outlines: Vec<SubPath> = vec![];
    for (pi, members) in pieces.iter().enumerate() {
        let faces: Vec<Vec<P>> = members.iter().filter_map(|&m| placed[m].as_ref().map(|(_, q)| q.clone())).collect();
        let folds: Vec<Vec<bool>> = members.iter().map(|&m| is_fold[m].clone()).collect();
        let tabs: BTreeMap<(usize, usize), Vec<P>> = members
            .iter()
            .enumerate()
            .flat_map(|(local, &m)| tab_on.iter().filter(move |((f, _), _)| *f == m).map(move |((_, k), t)| ((local, *k), t.clone())))
            .collect();
        let outline = boundary(&faces, &folds, &tabs)?.closed();
        let Some(b) = PathData::single(outline.clone()).bounds() else { continue };
        if x > 0.0 && x + b.width() > row_mm {
            (x, y) = (0.0, y + row_h + GAP_MM);
            row_h = 0.0;
        }
        shift[pi] = (x - b.x0, y - b.y0);
        x += b.width() + GAP_MM;
        row_h = row_h.max(b.height());
        outlines.push(outline);
    }
    let piece_of = |face: usize| placed[face].as_ref().map_or(0, |(pi, _)| *pi);
    let moved = |sp: &SubPath, d: (f64, f64)| PathData::single(sp.clone()).transformed(vectorcraft_geom::Affine::translate(d)).subpaths;
    let mut cut = vec![];
    for (pi, o) in outlines.iter().enumerate() {
        cut.extend(moved(o, shift[pi]));
    }
    let mut folds = vec![];
    for (f, sp) in &mountains {
        folds.extend(moved(sp, shift[piece_of(*f)]));
    }
    let mut vs = vec![];
    for (f, sp) in &valleys {
        vs.extend(moved(sp, shift[piece_of(*f)]));
    }
    let mut glue = vec![];
    for ((f, _), t) in &tab_on {
        let d = shift[piece_of(*f)];
        glue.push(polygon(&t.iter().map(|p| (p.0 + d.0, p.1 + d.1)).collect::<Vec<_>>()));
    }
    craft.labels = labels
        .into_iter()
        .map(|(f, at, angle, number)| {
            let d = shift[piece_of(f)];
            Label { at: (at.0 + d.0, at.1 + d.1), angle, number }
        })
        .collect();
    craft.net = Net { cut: PathData::new(cut), folds: PathData::new(folds), glue: PathData::new(glue) };
    craft.valleys = PathData::new(vs);
    Ok(craft)
}

#[cfg(test)]
mod tests {
    use super::super::mesh::read;
    use super::super::tests::simple;
    use super::*;

    const CUBE: &str = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nv 0 0 1\nv 1 0 1\nv 1 1 1\nv 0 1 1\n\
        f 1 4 3 2\nf 5 6 7 8\nf 1 2 6 5\nf 2 3 7 6\nf 3 4 8 7\nf 4 1 5 8\n";

    /// A cube read as twelve triangles unfolds into one piece: the diagonals inside each square
    /// are flat, so no fold line; the cube's edges all mountains; every cut edge numbered twice.
    #[test]
    fn a_triangulated_cube_folds_up() {
        let c = papercraft(read("c.obj", CUBE.as_bytes()).unwrap(), 50.0, 8.0, 300.0).unwrap();
        assert_eq!(c.pieces, 1);
        assert_eq!(c.net.folds.subpaths.len(), 5, "the squares kept whole: the cube's five folds");
        assert!(c.valleys.subpaths.is_empty());
        // 18 edges, 11 joining the 12 triangles: 7 cut, each numbered on both sides.
        assert_eq!(c.labels.len(), 14);
        assert!(simple(&c.net.cut));
    }

    /// Faces wound the wrong way are turned, so the net is the same.
    #[test]
    fn faces_are_turned_to_agree() {
        let flipped = CUBE.replace("f 1 4 3 2", "f 2 3 4 1").replace("f 2 3 7 6", "f 6 7 3 2");
        let a = papercraft(read("a.obj", CUBE.as_bytes()).unwrap(), 50.0, 8.0, 300.0).unwrap();
        let b = papercraft(read("b.obj", flipped.as_bytes()).unwrap(), 50.0, 8.0, 300.0).unwrap();
        assert_eq!((a.pieces, a.net.folds.subpaths.len()), (b.pieces, b.net.folds.subpaths.len()));
    }

    /// A shape with a dent has valley folds; a model too big or with no size is refused.
    #[test]
    fn dents_fold_the_other_way() {
        // A square pyramid pushed in: its tip below the base, the base open.
        let dent = "v 0 0 0\nv 1 0 0\nv 1 1 0\nv 0 1 0\nv 0.5 0.5 -0.4\nf 1 2 5\nf 2 3 5\nf 3 4 5\nf 4 1 5\n";
        let c = papercraft(read("d.obj", dent.as_bytes()).unwrap(), 60.0, 6.0, 300.0).unwrap();
        assert_eq!(c.net.folds.subpaths.len() + c.valleys.subpaths.len(), 3);
        assert!(papercraft(read("z.obj", b"v 0 0 0\nv 0 0 0\nv 0 0 0\nf 1 2 3\n").unwrap(), 50.0, 6.0, 300.0).is_err());
    }

    proptest::proptest! {
        /// Random meshes (corners anywhere, triangles naming any corners, repeats, slivers, edges in
        /// three faces): a net or an error, never a panic, and a net's numbers are finite.
        #[test]
        fn any_mesh_is_safe(
            verts in proptest::collection::vec(proptest::array::uniform3(-100.0f64..100.0), 3..12),
            tris in proptest::collection::vec(proptest::array::uniform3(0usize..12), 1..20),
        ) {
            let n = verts.len();
            let faces = tris.iter().map(|t| t.iter().map(|i| i % n).collect()).collect();
            if let Ok(c) = papercraft(Solid { verts, faces }, 100.0, 6.0, 300.0) {
                let finite = |p: &PathData| p.subpaths.iter().flat_map(|s| &s.anchors).all(|a| a.p.x.is_finite() && a.p.y.is_finite());
                proptest::prop_assert!(finite(&c.net.cut) && finite(&c.net.folds) && finite(&c.valleys));
            }
        }
    }
}
