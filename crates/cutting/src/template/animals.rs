//! Fork: the animal treat boxes (C15.5): an open tray glued at its four corners, a lid off the
//! back wall that tucks inside the front, and ears rising off the front wall, with a face to
//! print on it. Laid out with the front wall at the top, ears up:
//!
//! ```text
//!              ears
//!          [  front  ]
//! [left] [   base    ] [right]   corner flaps on the end walls, glued inside front and back
//!          [  back   ]
//!          [   lid   ]
//!          [  tuck   ]
//! ```
//!
//! Ported from Die Cut Studio (<https://github.com/pradeepsiddappa/diecutstudio>, commit
//! `3584baa4ac5d1e213a92a79b60622564518955ec`, `src/core/models/animalBox.ts`, `bunnyBox.ts`,
//! `catBox.ts`, `bearBox.ts`, `foxBox.ts`): the layout, the flaps' and the tuck's proportions,
//! each animal's ears and face and their colours. Ours: each panel the inside size plus a
//! thickness, the face as art printed on the net, and arcs for the round shapes.
//!
//! Copyright (c) 2026 Pradeep Siddappa
//!
//! Permission is hereby granted, free of charge, to any person obtaining a copy of this software
//! and associated documentation files (the "Software"), to deal in the Software without
//! restriction, including without limitation the rights to use, copy, modify, merge, publish,
//! distribute, sublicense, and/or sell copies of the Software, and to permit persons to whom the
//! Software is furnished to do so, subject to the following conditions:
//!
//! The above copyright notice and this permission notice shall be included in all copies or
//! substantial portions of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
//! BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
//! NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
//! DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
//! OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use std::f64::consts::TAU;

use vectorcraft_geom::{PathData, SubPath};

use super::{Art, Edge, Kind, Net, P, Param, THICKNESS, Unit, Values, polygon};

/// The sizes every animal box takes; at first a small treat box whose net, with the bunny's tall
/// ears, fits a 12 × 12 in mat.
const PARAMS: &[Param] = &[
    Param {
        key: "lengthMm",
        label: "Length",
        unit: Unit::Mm,
        default: 60.0,
        min: 30.0,
        max: 300.0,
        help: "inside, across the face (the ears' side)",
    },
    Param { key: "widthMm", label: "Width", unit: Unit::Mm, default: 40.0, min: 20.0, max: 300.0, help: "inside, front to back" },
    Param {
        key: "heightMm",
        label: "Height",
        unit: Unit::Mm,
        default: 50.0,
        min: 25.0,
        max: 300.0,
        help: "inside, the walls' height (the ears rise above it)",
    },
    THICKNESS,
];

const fn animal(id: &'static str, name: &'static str, make: fn(&Values) -> Result<Net, String>) -> Kind {
    Kind { id, name, family: "Party", params: PARAMS, sized: true, fit: Some(("lengthMm", "heightMm")), make }
}

pub(super) const BUNNY: Kind = animal("bunnyBox", "Bunny Treat Box", |v| build(v, &BUNNY_SPEC));
pub(super) const CAT: Kind = animal("catBox", "Cat Treat Box", |v| build(v, &CAT_SPEC));
pub(super) const BEAR: Kind = animal("bearBox", "Bear Treat Box", |v| build(v, &BEAR_SPEC));
pub(super) const FOX: Kind = animal("foxBox", "Fox Treat Box", |v| build(v, &FOX_SPEC));

/// Where the ears go on the front wall's top edge, and its face.
struct Ears {
    /// The front wall's left and right edges, and its top (the ears' base).
    x1: f64,
    x2: f64,
    y0: f64,
    /// Each ear's half width at its base and its height, and the two ears' centres.
    w: f64,
    h: f64,
    xl: f64,
    xr: f64,
}

/// The front wall, for the face: its sizes, its top and bottom, and its middle.
struct Face {
    l: f64,
    h: f64,
    y0: f64,
    cx: f64,
    ears: Ears,
}

/// What makes one animal: its ears' sizes and outline, and its face.
struct Spec {
    /// An ear's half width at its base, from the face's width.
    ear_w: fn(f64) -> f64,
    /// An ear's height, from the wall's height.
    ear_h: fn(f64) -> f64,
    /// The ears' centres, as shares of the face's width from its left edge.
    ear_at: (f64, f64),
    /// The outline across the ears, from the front wall's top left (already drawn to) to its top
    /// right, the ears' tips at y = 0.
    ears: fn(&mut Edge, &Ears),
    face: fn(&Face) -> Vec<Art>,
}

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    v.min(hi).max(lo)
}

fn build(v: &Values, spec: &Spec) -> Result<Net, String> {
    let t = v.get("thicknessMm");
    let (mut l, mut w, mut h) = (v.get("lengthMm"), v.get("widthMm"), v.get("heightMm"));
    if v.outside {
        (l, w, h) = (l - 2.0 * t, w - 2.0 * t, h - t);
        if l <= 0.0 || w <= 0.0 || h <= 0.0 {
            return Err("the outside sizes are too small for the thickness".to_string());
        }
    }
    let (l, w, h) = (l + t, w + t, h + t / 2.0);
    // The corner flaps: a relief from the walls, reaching most of the way along the front and back.
    let r = t.max(1.0);
    let fd = clamp(h * 0.8, 5.0_f64.min(l / 2.0 - r), l / 2.0 - r);
    let c = (fd * 0.4).min(6.0);
    let tuck = clamp(h * 0.7, 8.0_f64.min(h - r), h - r);
    let tc = (tuck * 0.5).min(l * 0.1);
    if fd <= 0.0 || tuck <= 0.0 {
        return Err("the box is too small for its flaps".to_string());
    }
    let ear_w = (spec.ear_w)(l);
    let ear_h = (spec.ear_h)(h);
    let (x0, xr, x1, x2, x3, x4) = (0.0, r, h, h + l, 2.0 * h + l - r, 2.0 * h + l);
    let ears = Ears { x1, x2, y0: ear_h, w: ear_w, h: ear_h, xl: x1 + l * spec.ear_at.0, xr: x1 + l * spec.ear_at.1 };
    let (y0, y1, y2, y4) = (ear_h, ear_h + h, ear_h + h + w, ear_h + 2.0 * h + w);
    let (y_lid, y_tuck) = (y4 + w, y4 + w + tuck);
    let (ytf, ybf) = (y1 - fd, y2 + fd);
    let mut o = Edge::from((x1, y0));
    (spec.ears)(&mut o, &ears);
    o.to((x2, y0)).to((x2, y1));
    o.to((x2, ytf)).to((x3 - c, ytf)).to((x3, ytf + c)).to((x3, y1));
    o.to((x4, y1)).to((x4, y2));
    o.to((x3, y2)).to((x3, ybf - c)).to((x3 - c, ybf)).to((x2, ybf)).to((x2, y2));
    o.to((x2, y4)).to((x2, y_lid));
    o.to((x2 - tc, y_tuck)).to((x1 + tc, y_tuck)).to((x1, y_lid));
    o.to((x1, y4)).to((x1, y2)).to((x1, ybf));
    o.to((xr + c, ybf)).to((xr, ybf - c)).to((xr, y2)).to((x0, y2)).to((x0, y1)).to((xr, y1));
    o.to((xr, ytf + c)).to((xr + c, ytf)).to((x1, ytf)).to((x1, y1));
    let line = |a: P, b: P| SubPath::polyline(&[vectorcraft_geom::Point::new(a.0, a.1), vectorcraft_geom::Point::new(b.0, b.1)], false);
    let folds = vec![
        line((x1, y1), (x2, y1)),
        line((x1, y2), (x2, y2)),
        line((x1, y1), (x1, y2)),
        line((x2, y1), (x2, y2)),
        line((xr, y1), (x1, y1)),
        line((xr, y2), (x1, y2)),
        line((x2, y1), (x3, y1)),
        line((x2, y2), (x3, y2)),
        line((x1, y4), (x2, y4)),
        line((x1, y_lid), (x2, y_lid)),
    ];
    // Each corner flap is glued inside the front or back wall.
    let glue = vec![
        polygon(&[(xr, y1), (xr, ytf + c), (xr + c, ytf), (x1, ytf), (x1, y1)]),
        polygon(&[(xr, y2), (xr, ybf - c), (xr + c, ybf), (x1, ybf), (x1, y2)]),
        polygon(&[(x2, y1), (x2, ytf), (x3 - c, ytf), (x3, ytf + c), (x3, y1)]),
        polygon(&[(x2, y2), (x3, y2), (x3, ybf - c), (x3 - c, ybf), (x2, ybf)]),
    ];
    let art = (spec.face)(&Face { l, h, y0, cx: x1 + l / 2.0, ears });
    Ok(Net { cut: PathData::single(o.closed()), folds: PathData::new(folds), glue: PathData::new(glue), face: Some([x1, y0, x2, y1]), art })
}

// ---- the face's shapes ----

fn fill(name: &'static str, color: u32, shapes: Vec<SubPath>) -> Art {
    Art { name, color, path: PathData::new(shapes), stroke_mm: None }
}

fn lines(name: &'static str, color: u32, width: f64, shapes: Vec<SubPath>) -> Art {
    Art { name, color, path: PathData::new(shapes), stroke_mm: Some(width) }
}

/// A circle `d` across round `c`.
fn disc(c: P, d: f64) -> SubPath {
    oval(c, d / 2.0, d / 2.0)
}

fn oval(c: P, rx: f64, ry: f64) -> SubPath {
    // A unit circle's arcs, stretched to the oval.
    let mut e = Edge::from((1.0, 0.0));
    e.arc((0.0, 0.0), 1.0, 0.0, TAU);
    e.map(|p| (c.0 + p.0 * rx, c.1 + p.1 * ry)).closed()
}

fn tri(a: P, b: P, c: P) -> SubPath {
    polygon(&[a, b, c])
}

/// A lens from `base` up to `tip` at `x`, bulging `w` either side half way.
fn leaf(x: f64, base: f64, tip: f64, w: f64) -> SubPath {
    let mid = (base + tip) / 2.0;
    let mut e = Edge::from((x, tip));
    e.quad((x + w, mid), (x, base)).quad((x - w, mid), (x, tip));
    e.closed()
}

/// A line from `a` to `b`, bowed toward `ctrl` when given.
fn stroke(a: P, b: P, ctrl: Option<P>) -> SubPath {
    let mut e = Edge::from(a);
    match ctrl {
        Some(c) => e.quad(c, b),
        None => e.to(b),
    };
    e.open()
}

fn line_w(l: f64) -> f64 {
    (l * 0.012).max(0.6)
}

const EYES: u32 = 0x2b2622;

const BUNNY_SPEC: Spec = Spec {
    ear_w: |l| l * 0.12,
    ear_h: |h| clamp(h * 1.2, 16.0, h * 2.4),
    ear_at: (0.32, 0.68),
    // Two tall leaves.
    ears: |o, g| {
        for x in [g.xl, g.xr] {
            o.to((x - g.w, g.y0));
            o.quad((x - g.w * 1.12, g.y0 - g.h * 0.62), (x, 0.0));
            o.quad((x + g.w * 1.12, g.y0 - g.h * 0.62), (x + g.w, g.y0));
        }
    },
    face: |f| {
        let (l, cx, e) = (f.l, f.cx, &f.ears);
        let (eye_y, cheek_y, nose_y, mouth_y) = (f.y0 + f.h * 0.30, f.y0 + f.h * 0.44, f.y0 + f.h * 0.46, f.y0 + f.h * 0.60);
        let (nw, nh, mw, md) = (l * 0.038, l * 0.05, l * 0.085, l * 0.05);
        vec![
            fill("Inner ears", 0xf3a9c0, [e.xl, e.xr].map(|x| leaf(x, e.y0 - e.h * 0.12, e.y0 - e.h * 0.86, e.w * 0.52)).to_vec()),
            fill("Cheeks", 0xf8c4d2, vec![disc((cx - l * 0.27, cheek_y), l * 0.11), disc((cx + l * 0.27, cheek_y), l * 0.11)]),
            fill("Eyes", EYES, vec![disc((cx - l * 0.17, eye_y), l * 0.1), disc((cx + l * 0.17, eye_y), l * 0.1)]),
            fill("Nose", 0xe26d8e, vec![tri((cx - nw, nose_y), (cx + nw, nose_y), (cx, nose_y + nh))]),
            lines(
                "Mouth",
                0x7a5560,
                line_w(l),
                vec![stroke((cx, nose_y + nh), (cx, mouth_y), None), stroke((cx - mw, mouth_y), (cx + mw, mouth_y), Some((cx, mouth_y + md)))],
            ),
        ]
    },
};

const CAT_SPEC: Spec = Spec {
    ear_w: |l| l * 0.17,
    ear_h: |h| clamp(h * 0.7, 12.0, h * 1.3),
    ear_at: (0.22, 0.78),
    // Upright pointed triangles.
    ears: |o, g| {
        for x in [g.xl, g.xr] {
            o.to((x - g.w, g.y0)).to((x, 0.0)).to((x + g.w, g.y0));
        }
    },
    face: |f| {
        let (l, cx, e) = (f.l, f.cx, &f.ears);
        let (eye_y, nose_y, mouth_y) = (f.y0 + f.h * 0.32, f.y0 + f.h * 0.50, f.y0 + f.h * 0.64);
        let (nw, nh, aw, ad) = (l * 0.04, l * 0.045, l * 0.07, l * 0.045);
        let inner = |x: f64| tri((x - e.w * 0.5, e.y0 - e.h * 0.06), (x + e.w * 0.5, e.y0 - e.h * 0.06), (x, e.y0 - e.h * 0.74));
        let mut whiskers = vec![];
        for s in [-1.0, 1.0] {
            whiskers.push(stroke((cx + s * l * 0.07, nose_y + nh * 0.4), (cx + s * l * 0.46, nose_y - l * 0.06), None));
            whiskers.push(stroke((cx + s * l * 0.07, nose_y + nh * 0.9), (cx + s * l * 0.47, nose_y + nh * 0.9), None));
            whiskers.push(stroke((cx + s * l * 0.07, nose_y + nh * 1.4), (cx + s * l * 0.46, nose_y + nh * 1.9), None));
        }
        vec![
            fill("Inner ears", 0xf3a9c0, vec![inner(e.xl), inner(e.xr)]),
            fill("Eyes", EYES, vec![disc((cx - l * 0.18, eye_y), l * 0.11), disc((cx + l * 0.18, eye_y), l * 0.11)]),
            fill("Nose", 0xe07a96, vec![tri((cx - nw, nose_y), (cx + nw, nose_y), (cx, nose_y + nh))]),
            lines(
                "Mouth",
                0x5b5550,
                line_w(l),
                vec![
                    stroke((cx, nose_y + nh), (cx, mouth_y), None),
                    stroke((cx, mouth_y), (cx - aw, mouth_y + ad), Some((cx - aw * 0.5, mouth_y + ad))),
                    stroke((cx, mouth_y), (cx + aw, mouth_y + ad), Some((cx + aw * 0.5, mouth_y + ad))),
                ],
            ),
            lines("Whiskers", 0x9a948c, line_w(l), whiskers),
        ]
    },
};

const BEAR_SPEC: Spec = Spec {
    ear_w: |l| l * 0.18,
    ear_h: |h| clamp(h * 0.34, 11.0, h * 0.6),
    ear_at: (0.24, 0.76),
    // Round domes.
    ears: |o, g| {
        for x in [g.xl, g.xr] {
            o.to((x - g.w, g.y0));
            o.quad((x - g.w, 0.0), (x, 0.0));
            o.quad((x + g.w, 0.0), (x + g.w, g.y0));
        }
    },
    face: |f| {
        let (l, cx, e) = (f.l, f.cx, &f.ears);
        let (eye_y, muzzle_y, nose_y) = (f.y0 + f.h * 0.30, f.y0 + f.h * 0.62, f.y0 + f.h * 0.52);
        vec![
            fill("Inner ears", 0xc9966a, vec![disc((e.xl, e.y0 - e.h * 0.4), e.w * 0.9), disc((e.xr, e.y0 - e.h * 0.4), e.w * 0.9)]),
            fill("Muzzle", 0xead9c6, vec![disc((cx, muzzle_y), l * 0.52)]),
            fill("Eyes", EYES, vec![disc((cx - l * 0.16, eye_y), l * 0.1), disc((cx + l * 0.16, eye_y), l * 0.1)]),
            fill("Nose", 0x3f3128, vec![disc((cx, nose_y), l * 0.13)]),
            lines(
                "Mouth",
                0x6b5444,
                line_w(l),
                vec![
                    stroke((cx, nose_y + l * 0.07), (cx, muzzle_y + l * 0.04), None),
                    stroke((cx - l * 0.1, muzzle_y + l * 0.04), (cx + l * 0.1, muzzle_y + l * 0.04), Some((cx, muzzle_y + l * 0.11))),
                ],
            ),
        ]
    },
};

const FOX_SPEC: Spec = Spec {
    ear_w: |l| l * 0.15,
    ear_h: |h| clamp(h * 0.9, 14.0, h * 1.6),
    ear_at: (0.2, 0.8),
    // Pointed, the tips leaning out.
    ears: |o, g| {
        o.to((g.xl - g.w, g.y0)).to((g.xl - g.w * 0.35, 0.0)).to((g.xl + g.w, g.y0));
        o.to((g.xr - g.w, g.y0)).to((g.xr + g.w * 0.35, 0.0)).to((g.xr + g.w, g.y0));
    },
    face: |f| {
        let (l, cx, e) = (f.l, f.cx, &f.ears);
        let (top, chin) = (f.y0 + f.h * 0.05, f.y0 + f.h * 0.95);
        let (eye_y, nose_y) = (f.y0 + f.h * 0.36, chin - l * 0.05);
        let inner = |x: f64, lean: f64| tri((x - e.w * 0.45, e.y0 - e.h * 0.04), (x + e.w * 0.45, e.y0 - e.h * 0.04), (x + lean, e.y0 - e.h * 0.72));
        vec![
            fill("Inner ears", 0xf6efe4, vec![inner(e.xl, -e.w * 0.25), inner(e.xr, e.w * 0.25)]),
            fill("Face patch", 0xe9803a, vec![tri((e.x1 + l * 0.08, top), (e.x2 - l * 0.08, top), (cx, chin))]),
            fill("Snout", 0xf6efe4, vec![tri((cx - l * 0.24, f.y0 + f.h * 0.52), (cx + l * 0.24, f.y0 + f.h * 0.52), (cx, chin))]),
            fill("Eyes", EYES, vec![disc((cx - l * 0.17, eye_y), l * 0.09), disc((cx + l * 0.17, eye_y), l * 0.09)]),
            fill("Nose", EYES, vec![tri((cx - l * 0.06, nose_y - l * 0.05), (cx + l * 0.06, nose_y - l * 0.05), (cx, nose_y + l * 0.04))]),
        ]
    },
};

#[cfg(test)]
mod tests {
    use super::super::tests::{simple, values};
    use super::*;

    /// Each animal: the net as wide as the base and two walls, as tall as the ears, front, base,
    /// back, lid and tuck; a face printed on the front wall; an outline that doesn't cross itself.
    #[test]
    fn the_animal_boxes_are_made_to_size() {
        for k in [&BUNNY, &CAT, &BEAR, &FOX] {
            let v = values(k, &[("lengthMm", 76.0), ("widthMm", 60.0), ("heightMm", 80.0), ("thicknessMm", 0.4)], false).unwrap();
            let net = k.make(&v).unwrap();
            let (w, h) = net.size();
            let (l, d, wall) = (76.4, 60.4, 80.2);
            assert!((w - (l + 2.0 * wall)).abs() < 1e-6, "{}: {w}", k.id);
            let ear = (net.face.unwrap()[1]).max(0.0);
            assert!(ear > 5.0, "{}: ears above the wall", k.id);
            let tuck = (wall * 0.7).clamp(8.0, wall - 1.0);
            assert!((h - (ear + 2.0 * wall + 2.0 * d + tuck)).abs() < 1e-6, "{}: {h}", k.id);
            assert!(simple(&net.cut), "{}", k.id);
            assert_eq!((net.folds.subpaths.len(), net.glue.subpaths.len()), (10, 4), "{}", k.id);
            let face = net.face.unwrap();
            assert!((face[2] - face[0] - l).abs() < 1e-6 && (face[3] - face[1] - wall).abs() < 1e-6, "{}", k.id);
            assert!(net.art.len() >= 4 && net.art.iter().any(|a| a.name == "Eyes"), "{}", k.id);
            // The face's art lies on the net.
            let b = net.cut.bounds().unwrap();
            for a in &net.art {
                let ab = a.path.bounds().unwrap();
                assert!(ab.x0 >= b.x0 - 1e-6 && ab.x1 <= b.x1 + 1e-6 && ab.y0 >= b.y0 - 1e-6 && ab.y1 <= b.y1 + 1e-6, "{}: {}", k.id, a.name);
            }
        }
    }

    /// At their first sizes, every animal's net fits a 12 × 12 in mat's cut area (about 290 mm).
    #[test]
    fn the_first_sizes_fit_a_mat() {
        for k in [&BUNNY, &CAT, &BEAR, &FOX] {
            let (w, h) = k.make(&values(k, &[], false).unwrap()).unwrap().size();
            assert!(w < 290.0 && h < 290.0, "{}: {w} × {h}", k.id);
        }
    }

    /// The smallest and largest sizes still make a net.
    #[test]
    fn the_animal_boxes_make_every_size() {
        for k in [&BUNNY, &CAT, &BEAR, &FOX] {
            for (l, d, h) in [(30.0, 20.0, 25.0), (300.0, 300.0, 300.0), (30.0, 300.0, 25.0)] {
                let v = values(k, &[("lengthMm", l), ("widthMm", d), ("heightMm", h), ("thicknessMm", 5.0)], false).unwrap();
                let net = k.make(&v).unwrap_or_else(|e| panic!("{} {l} {d} {h}: {e}", k.id));
                assert!(simple(&net.cut), "{} {l} {d} {h}", k.id);
            }
        }
    }
}
