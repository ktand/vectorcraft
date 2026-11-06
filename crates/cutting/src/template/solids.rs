//! Fork: solids (C15.4), all our own geometry: a cylinder and a cone (round, drawn directly), and
//! pyramids, prisms and the five Platonic solids (unfolded from their corners by `unfold`).

use std::f64::consts::{PI, TAU};

use vectorcraft_geom::PathData;

use super::unfold::{Solid, V3, unfold};
use super::{Edge, Kind, Net, P, Param, Unit, Values, polygon};

const FAMILY: &str = "Solids";

const TAB: Param = Param { key: "tabMm", label: "Glue Tab", unit: Unit::Mm, default: 8.0, min: 2.0, max: 40.0, help: "how deep the glue tabs are" };

const EDGE: Param = Param { key: "edgeMm", label: "Edge", unit: Unit::Mm, default: 50.0, min: 5.0, max: 500.0, help: "each edge's length" };

/// The golden ratio.
fn phi() -> f64 {
    (1.0 + 5.0_f64.sqrt()) / 2.0
}

fn scale(v: &[V3], k: f64) -> Vec<V3> {
    v.iter().map(|p| [p[0] * k, p[1] * k, p[2] * k]).collect()
}

fn dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// A convex solid from its corners and its faces' outward directions: each face is the corners
/// furthest along its direction, put in order counter-clockwise seen from outside.
fn convex(verts: Vec<V3>, dirs: &[V3]) -> Solid {
    let mut faces = vec![];
    for &d in dirs {
        let top = verts.iter().map(|&v| dot(v, d)).fold(f64::NEG_INFINITY, f64::max);
        let on: Vec<usize> = (0..verts.len()).filter(|&i| dot(verts[i], d) > top - 1e-6 * top.abs().max(1.0)).collect();
        let n = d;
        let c = on.iter().fold([0.0; 3], |a, &i| [a[0] + verts[i][0], a[1] + verts[i][1], a[2] + verts[i][2]]);
        let c = [c[0] / on.len() as f64, c[1] / on.len() as f64, c[2] / on.len() as f64];
        // A basis in the face's plane: e1 toward the first corner, e2 = n × e1.
        let Some(&first) = on.first() else { continue };
        let e1 = [verts[first][0] - c[0], verts[first][1] - c[1], verts[first][2] - c[2]];
        let e2 = [n[1] * e1[2] - n[2] * e1[1], n[2] * e1[0] - n[0] * e1[2], n[0] * e1[1] - n[1] * e1[0]];
        let mut ring = on.clone();
        ring.sort_by(|&a, &b| {
            let ang = |i: usize| {
                let r = [verts[i][0] - c[0], verts[i][1] - c[1], verts[i][2] - c[2]];
                dot(r, e2).atan2(dot(r, e1))
            };
            ang(a).total_cmp(&ang(b))
        });
        faces.push(ring);
    }
    Solid { verts, faces }
}

/// The cube's corners, (±1, ±1, ±1).
fn cube_corners() -> Vec<V3> {
    let mut out = vec![];
    for x in [-1.0, 1.0] {
        for y in [-1.0, 1.0] {
            for z in [-1.0, 1.0] {
                out.push([x, y, z]);
            }
        }
    }
    out
}

/// The points (0, ±a, ±b), (±a, ±b, 0) and (±b, 0, ±a): a rectangle's three cyclic placings.
fn cyclic(a: f64, b: f64) -> Vec<V3> {
    let mut out = vec![];
    for s in [-1.0, 1.0] {
        for t in [-1.0, 1.0] {
            out.extend([[0.0, s * a, t * b], [s * a, t * b, 0.0], [t * b, 0.0, s * a]]);
        }
    }
    out
}

fn platonic(v: &Values, faces: usize) -> Result<Net, String> {
    let (a, tab) = (v.get("edgeMm"), v.get("tabMm"));
    let p = phi();
    let axes = vec![[1.0, 0.0, 0.0], [-1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, -1.0, 0.0], [0.0, 0.0, 1.0], [0.0, 0.0, -1.0]];
    let solid = match faces {
        4 => {
            let vs = vec![[1.0, 1.0, 1.0], [1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [-1.0, -1.0, 1.0]];
            let dirs: Vec<V3> = vs.iter().map(|q: &V3| [-q[0], -q[1], -q[2]]).collect();
            convex(scale(&vs, a / 8.0_f64.sqrt()), &dirs)
        }
        6 => convex(scale(&cube_corners(), a / 2.0), &axes),
        8 => convex(scale(&axes, a / 2.0_f64.sqrt()), &cube_corners()),
        12 | 20 => {
            // The icosahedron's corners (0, ±1, ±φ)…, edge 2; its faces point to (±1, ±1, ±1) and
            // (0, ±φ, ±1/φ)…. The dodecahedron's corners (±1, ±1, ±1) and (0, ±1/φ, ±φ)…, edge
            // 2/φ; its faces point to (0, ±φ, ±1)….
            if faces == 20 {
                let mut dirs = cube_corners();
                dirs.extend(cyclic(p, 1.0 / p));
                convex(scale(&cyclic(1.0, p), a / 2.0), &dirs)
            } else {
                let mut dod = cube_corners();
                dod.extend(cyclic(1.0 / p, p));
                convex(scale(&dod, a * p / 2.0), &cyclic(p, 1.0))
            }
        }
        _ => return Err("a Platonic solid has 4, 6, 8, 12 or 20 faces".to_string()),
    };
    unfold(&solid, tab)
}

macro_rules! platonic_kind {
    ($name:ident, $id:literal, $label:literal, $faces:literal, $f:ident) => {
        fn $f(v: &Values) -> Result<Net, String> {
            platonic(v, $faces)
        }
        pub(super) const $name: Kind = Kind { id: $id, name: $label, family: FAMILY, params: &[EDGE, TAB], sized: false, fit: None, make: $f };
    };
}

platonic_kind!(TETRAHEDRON, "tetrahedron", "Tetrahedron", 4, tetrahedron);
platonic_kind!(CUBE, "cube", "Cube", 6, cube);
platonic_kind!(OCTAHEDRON, "octahedron", "Octahedron", 8, octahedron);
platonic_kind!(DODECAHEDRON, "dodecahedron", "Dodecahedron", 12, dodecahedron);
platonic_kind!(ICOSAHEDRON, "icosahedron", "Icosahedron", 20, icosahedron);

const SIDES: Param =
    Param { key: "sides", label: "Sides", unit: Unit::Count, default: 4.0, min: 3.0, max: 24.0, help: "how many sides the base has" };

pub(super) const PYRAMID: Kind = Kind {
    id: "pyramid",
    name: "Pyramid",
    family: FAMILY,
    params: &[
        SIDES,
        Param { key: "baseEdgeMm", label: "Base Edge", unit: Unit::Mm, default: 60.0, min: 5.0, max: 500.0, help: "each side of the base" },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 60.0, min: 2.0, max: 800.0, help: "base to tip" },
        TAB,
    ],
    sized: false,
    fit: None,
    make: pyramid,
};

pub(super) const PRISM: Kind = Kind {
    id: "prism",
    name: "Prism",
    family: FAMILY,
    params: &[
        Param { key: "sides", label: "Sides", unit: Unit::Count, default: 6.0, min: 3.0, max: 24.0, help: "how many sides the ends have" },
        Param { key: "baseEdgeMm", label: "Base Edge", unit: Unit::Mm, default: 30.0, min: 5.0, max: 500.0, help: "each side of the ends" },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 80.0, min: 2.0, max: 800.0, help: "end to end" },
        TAB,
    ],
    sized: false,
    fit: None,
    make: prism,
};

/// A regular `n`-gon with edge `a`, at height `z`.
fn ring(n: usize, a: f64, z: f64) -> Vec<V3> {
    let r = a / (2.0 * (PI / n as f64).sin());
    (0..n)
        .map(|i| {
            let t = TAU * i as f64 / n as f64;
            [r * t.cos(), r * t.sin(), z]
        })
        .collect()
}

/// Faces listed any way round, each turned to run counter-clockwise from outside (the solid is
/// convex, so outside is away from its middle).
fn oriented(verts: Vec<V3>, faces: Vec<Vec<usize>>) -> Solid {
    let n = verts.len() as f64;
    let mid = verts.iter().fold([0.0; 3], |a, v| [a[0] + v[0] / n, a[1] + v[1] / n, a[2] + v[2] / n]);
    let faces = faces
        .into_iter()
        .map(|mut f| {
            let mut nrm = [0.0; 3];
            let mut c = [0.0; 3];
            for (i, &a) in f.iter().enumerate() {
                let (p, q) = (verts[a], verts[f[(i + 1) % f.len()]]);
                nrm[0] += (p[1] - q[1]) * (p[2] + q[2]);
                nrm[1] += (p[2] - q[2]) * (p[0] + q[0]);
                nrm[2] += (p[0] - q[0]) * (p[1] + q[1]);
                c = [c[0] + p[0], c[1] + p[1], c[2] + p[2]];
            }
            let out = [c[0] / f.len() as f64 - mid[0], c[1] / f.len() as f64 - mid[1], c[2] / f.len() as f64 - mid[2]];
            if dot(nrm, out) < 0.0 {
                f.reverse();
            }
            f
        })
        .collect();
    Solid { verts, faces }
}

fn pyramid(v: &Values) -> Result<Net, String> {
    let n = v.get("sides") as usize;
    let mut verts = ring(n, v.get("baseEdgeMm"), 0.0);
    verts.push([0.0, 0.0, v.get("heightMm")]);
    let mut faces = vec![(0..n).collect::<Vec<_>>()];
    faces.extend((0..n).map(|i| vec![i, (i + 1) % n, n]));
    unfold(&oriented(verts, faces), v.get("tabMm"))
}

fn prism(v: &Values) -> Result<Net, String> {
    let n = v.get("sides") as usize;
    let a = v.get("baseEdgeMm");
    let mut verts = ring(n, a, 0.0);
    verts.extend(ring(n, a, v.get("heightMm")));
    let mut faces = vec![(0..n).collect::<Vec<_>>(), (n..2 * n).collect()];
    faces.extend((0..n).map(|i| vec![i, (i + 1) % n, n + (i + 1) % n, n + i]));
    unfold(&oriented(verts, faces), v.get("tabMm"))
}

pub(super) const CYLINDER: Kind = Kind {
    id: "cylinder",
    name: "Cylinder",
    family: FAMILY,
    params: &[
        Param { key: "diameterMm", label: "Diameter", unit: Unit::Mm, default: 50.0, min: 10.0, max: 300.0, help: "across" },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 80.0, min: 5.0, max: 600.0, help: "end to end" },
        TAB,
    ],
    sized: false,
    fit: None,
    make: cylinder,
};

/// How many tabs go round a circle `circumference` mm round: about one each 10 mm, at least 8.
fn tab_count(circumference: f64) -> usize {
    ((circumference / 10.0).round() as usize).clamp(8, 96)
}

/// A regular `n`-gon of radius `r` round `c`, a tab on every side: its outline, its folds and its
/// tabs. An end piece for a cylinder or a cone; `n` large enough that it is round in use.
fn disc(c: P, r: f64, n: usize, tab: f64) -> (Edge, Vec<vectorcraft_geom::SubPath>, Vec<vectorcraft_geom::SubPath>) {
    let at = |i: usize| {
        let t = TAU * i as f64 / n as f64;
        (c.0 + r * t.cos(), c.1 + r * t.sin())
    };
    let side = 2.0 * r * (PI / n as f64).sin();
    let d = tab.min(side * 0.45);
    let mut e = Edge::from(at(0));
    let (mut folds, mut glue) = (vec![], vec![]);
    for i in 0..n {
        let (a, b) = (at(i), at(i + 1));
        // Outward: the corners go round with the angle (a positive signed area), so the inside is
        // on each side's left and the tab on its right; its ends slope in at 45°.
        let (ux, uy) = ((b.0 - a.0) / side, (b.1 - a.1) / side);
        let (nx, ny) = (uy, -ux);
        let a2 = (a.0 + ux * d + nx * d, a.1 + uy * d + ny * d);
        let b2 = (b.0 - ux * d + nx * d, b.1 - uy * d + ny * d);
        e.to(a2).to(b2).to(b);
        let mut f = Edge::from(a);
        f.to(b);
        folds.push(f.open());
        glue.push(polygon(&[a, a2, b2, b]));
    }
    (e, folds, glue)
}

/// A cylinder: the wall a strip as long as the circumference with a glue strip at one end and tabs
/// along both edges, and two ends.
fn cylinder(v: &Values) -> Result<Net, String> {
    let (d, h, tab) = (v.get("diameterMm"), v.get("heightMm"), v.get("tabMm"));
    let c = PI * d;
    let n = tab_count(c);
    let w = c / n as f64;
    let t = tab.min(w * 0.45);
    let g = tab.min(h / 3.0);
    // The wall, from its top left: tabs along the top, the glue strip, tabs back along the bottom.
    let mut o = Edge::from((0.0, t));
    let mut folds = vec![];
    let mut glue = vec![];
    for i in 0..n {
        let (x0, x1) = (w * i as f64, w * (i + 1) as f64);
        o.to((x0 + t, 0.0)).to((x1 - t, 0.0)).to((x1, t));
        glue.push(polygon(&[(x0, t), (x0 + t, 0.0), (x1 - t, 0.0), (x1, t)]));
    }
    o.to((c + g, t + g)).to((c + g, t + h - g)).to((c, t + h));
    glue.push(polygon(&[(c, t), (c + g, t + g), (c + g, t + h - g), (c, t + h)]));
    for i in (0..n).rev() {
        let (x0, x1) = (w * i as f64, w * (i + 1) as f64);
        o.to((x1 - t, 2.0 * t + h)).to((x0 + t, 2.0 * t + h)).to((x0, t + h));
        glue.push(polygon(&[(x1, t + h), (x1 - t, 2.0 * t + h), (x0 + t, 2.0 * t + h), (x0, t + h)]));
    }
    let line = |a: P, b: P| {
        let mut e = Edge::from(a);
        e.to(b);
        e.open()
    };
    folds.push(line((0.0, t), (c, t)));
    folds.push(line((0.0, t + h), (c, t + h)));
    folds.push(line((c, t), (c, t + h)));
    let mut cut = vec![o.closed()];
    // The two ends, plain circles (the wall's tabs glue under them), beside the wall.
    let r = d / 2.0;
    for k in 0..2 {
        let cx = c + g + 5.0 + r + k as f64 * (d + 5.0);
        let mut e = Edge::from((cx + r, t + r));
        e.arc((cx, t + r), r, 0.0, TAU);
        cut.push(e.closed());
    }
    Ok(Net { cut: PathData::new(cut), folds: PathData::new(folds), glue: PathData::new(glue), face: None, art: vec![] })
}

pub(super) const CONE: Kind = Kind {
    id: "cone",
    name: "Cone",
    family: FAMILY,
    params: &[
        Param { key: "diameterMm", label: "Base Diameter", unit: Unit::Mm, default: 60.0, min: 10.0, max: 400.0, help: "across the base" },
        Param {
            key: "topDiameterMm",
            label: "Top Diameter",
            unit: Unit::Mm,
            default: 0.0,
            min: 0.0,
            max: 399.0,
            help: "across the top (0: a point)",
        },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 70.0, min: 2.0, max: 600.0, help: "base to top" },
        TAB,
    ],
    sized: false,
    fit: None,
    make: cone,
};

/// A cone, or with a top a frustum: the wall a ring's sector (its radii from the slant), a glue
/// strip along one straight edge, and a base (a polygon fine enough to be round) with tabs that
/// fold up inside the wall; a top too when there is one.
fn cone(v: &Values) -> Result<Net, String> {
    let (d, dt, h, tab) = (v.get("diameterMm"), v.get("topDiameterMm"), v.get("heightMm"), v.get("tabMm"));
    if dt >= d {
        return Err("the top must be narrower than the base (for straight sides, make a cylinder)".to_string());
    }
    let (r1, r2) = (d / 2.0, dt / 2.0);
    let slant = h.hypot(r1 - r2);
    let outer = r1 * slant / (r1 - r2);
    let inner = r2 * slant / (r1 - r2);
    let sweep = TAU * r1 / outer;
    if sweep > TAU * 0.9 {
        return Err("the cone is too flat to wrap with its glue strip: make it taller".to_string());
    }
    // The sector opens downward, symmetric about the vertical through its centre.
    let (a0, a1) = (PI / 2.0 - sweep / 2.0, PI / 2.0 + sweep / 2.0);
    let centre = (outer + tab, tab);
    let at = |r: f64, a: f64| (centre.0 + r * a.cos(), centre.1 + r * a.sin());
    let mut o = Edge::from(at(inner, a0));
    o.to(at(outer, a0));
    o.arc(centre, outer, a0, a1);
    // The glue strip past the edge at a1: its outward side is the way the angle grows.
    let g = tab.min(slant / 3.0);
    let (nx, ny) = (-a1.sin(), a1.cos());
    let (rx, ry) = (a1.cos(), a1.sin());
    let s0 = at(outer, a1);
    let s1 = at(inner, a1);
    let p1 = (s0.0 + nx * g - rx * g, s0.1 + ny * g - ry * g);
    let p2 = (s1.0 + nx * g + rx * g.min(inner), s1.1 + ny * g + ry * g.min(inner));
    o.to(p1).to(p2).to(s1);
    if inner > 1e-9 {
        o.arc(centre, inner, a1, a0);
    }
    let mut glue = vec![polygon(&[s0, p1, p2, s1])];
    let mut folds = vec![{
        let mut e = Edge::from(s0);
        e.to(s1);
        e.open()
    }];
    let mut cut = vec![o.closed()];
    // The base (and top) beside the wall's bounds.
    let wall = o.closed();
    let right = wall.anchors.iter().map(|a| a.p.x).fold(0.0, f64::max) + 5.0;
    let mut x = right;
    for r in [r1, r2] {
        if r <= 1e-9 {
            continue;
        }
        let n = tab_count(TAU * r);
        let (e, f, gl) = disc((x + tab + r, tab + r), r, n, tab);
        cut.push(e.closed());
        folds.extend(f);
        glue.extend(gl);
        x += 2.0 * (r + tab) + 5.0;
    }
    Ok(Net { cut: PathData::new(cut), folds: PathData::new(folds), glue: PathData::new(glue), face: None, art: vec![] })
}

#[cfg(test)]
mod tests {
    use super::super::tests::{simple, values};
    use super::*;

    /// Each Platonic solid unfolds into one piece: its faces' count of folds is one less than its
    /// faces, and a tab on every edge that's cut.
    #[test]
    fn the_platonic_solids_unfold() {
        for (k, faces, edges) in [(&TETRAHEDRON, 4, 6), (&CUBE, 6, 12), (&OCTAHEDRON, 8, 12), (&DODECAHEDRON, 12, 30), (&ICOSAHEDRON, 20, 30)] {
            let net = k.make(&values(k, &[], false).unwrap()).unwrap_or_else(|e| panic!("{}: {e}", k.id));
            assert_eq!(net.cut.subpaths.len(), 1, "{}: one piece", k.id);
            assert_eq!(net.folds.subpaths.len(), faces - 1, "{}", k.id);
            assert_eq!(net.glue.subpaths.len(), edges - (faces - 1), "{}: a tab on every cut edge", k.id);
            assert!(simple(&net.cut), "{}", k.id);
        }
    }

    /// A cube of edge 50 unfolds with every fold 50 long.
    #[test]
    fn a_cubes_folds_are_its_edges() {
        let net = CUBE.make(&values(&CUBE, &[("edgeMm", 50.0)], false).unwrap()).unwrap();
        for f in &net.folds.subpaths {
            let (a, b) = (f.anchors[0].p, f.anchors[1].p);
            assert!(((a - b).hypot() - 50.0).abs() < 1e-6);
        }
    }

    /// Pyramids and prisms of every side count unfold into one piece.
    #[test]
    fn pyramids_and_prisms_unfold() {
        for n in 3..=24 {
            for (k, faces) in [(&PYRAMID, n + 1), (&PRISM, n + 2)] {
                let net = k.make(&values(k, &[("sides", n as f64)], false).unwrap()).unwrap_or_else(|e| panic!("{} {n}: {e}", k.id));
                assert_eq!(net.folds.subpaths.len(), faces - 1, "{} {n}", k.id);
                assert!(simple(&net.cut), "{} {n}", k.id);
            }
        }
    }

    /// A cone's wall: the outer arc is the base's circumference; a flat cone is refused when the
    /// top isn't narrower.
    #[test]
    fn the_cone_wraps_its_base() {
        let net = CONE.make(&values(&CONE, &[("diameterMm", 60.0), ("heightMm", 40.0)], false).unwrap()).unwrap();
        assert_eq!(net.cut.subpaths.len(), 2, "wall and base");
        assert!(simple(&PathData::single(net.cut.subpaths[0].clone())));
        let frustum = CONE.make(&values(&CONE, &[("diameterMm", 60.0), ("topDiameterMm", 30.0)], false).unwrap()).unwrap();
        assert_eq!(frustum.cut.subpaths.len(), 3, "wall, base and top");
        assert!(CONE.make(&values(&CONE, &[("diameterMm", 60.0), ("topDiameterMm", 60.0)], false).unwrap()).is_err());
        let cyl = CYLINDER.make(&values(&CYLINDER, &[], false).unwrap()).unwrap();
        assert_eq!(cyl.cut.subpaths.len(), 3);
        assert!(simple(&PathData::single(cyl.cut.subpaths[0].clone())));
        let (w, _) = cyl.size();
        assert!(w > PI * 50.0);
    }
}
