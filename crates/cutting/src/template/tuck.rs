//! Fork: tuck-end cartons (C15.2): the straight tuck end (both tucks off the back panel) and the
//! reverse tuck end (the top off the back, the bottom off the front).
//!
//! Ported from Die Cut Studio (<https://github.com/pradeepsiddappa/diecutstudio>, commit
//! `3584baa4ac5d1e213a92a79b60622564518955ec`, files `src/core/models/tuckEnd.ts` and
//! `src/core/models/reverseTuckEnd.ts`): the panel layout and the flap proportions (tongue,
//! dust flaps, shoulders, rounded tongue corners, glue-flap taper, thumb notch). Ours: the
//! material's thickness taken up at each fold (each panel the inside size plus one thickness), a
//! thumb notch wherever a tongue tucks in, and the reverse tuck's tongue for both.
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

use vectorcraft_geom::PathData;

use super::{Edge, Kind, Net, P, Param, THICKNESS, Unit, Values, polygon};

const PARAMS: &[Param] = &[
    Param { key: "lengthMm", label: "Length", unit: Unit::Mm, default: 63.0, min: 10.0, max: 600.0, help: "inside, across the front" },
    Param { key: "widthMm", label: "Width", unit: Unit::Mm, default: 22.0, min: 8.0, max: 300.0, help: "inside, front to back (the sides' width)" },
    Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 89.0, min: 15.0, max: 600.0, help: "inside, top to bottom" },
    THICKNESS,
];

pub(super) const STRAIGHT: Kind =
    Kind { id: "tuckEnd", name: "Tuck End Box", family: "Boxes", params: PARAMS, sized: true, fit: Some(("lengthMm", "heightMm")), make: straight };

pub(super) const REVERSE: Kind = Kind {
    id: "reverseTuckEnd",
    name: "Reverse Tuck End Box",
    family: "Boxes",
    params: PARAMS,
    sized: true,
    fit: Some(("lengthMm", "heightMm")),
    make: reverse,
};

fn straight(v: &Values) -> Result<Net, String> {
    tuck_end(v, false)
}

fn reverse(v: &Values) -> Result<Net, String> {
    tuck_end(v, true)
}

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    v.min(hi).max(lo)
}

/// What closes one end over one panel.
#[derive(Clone, Copy, PartialEq)]
enum Flap {
    /// A tuck panel the box's depth deep, then the tongue that tucks in.
    Tuck,
    /// A dust flap, folded in under the tuck.
    Dust,
    /// No flap: the edge a tongue tucks in behind, with a thumb notch.
    Open,
}

/// The sizes every flap is drawn from, mm.
struct Sizes {
    /// Body top and bottom.
    top: f64,
    bottom: f64,
    tuck: f64,
    tongue: f64,
    dust: f64,
    dust_corner: f64,
    shoulder: f64,
    tongue_corner: f64,
    notch: f64,
}

/// One end's edge over `[x0, x1]`, drawn as the top end (y up is away from the body); the bottom is
/// the same mirrored.
fn flap(e: &mut Edge, s: &Sizes, x0: f64, x1: f64, kind: Flap) {
    let tb = s.top;
    match kind {
        Flap::Tuck => {
            let crease = tb - s.tuck;
            let tip = crease - s.tongue;
            let (sh, r) = (s.shoulder, s.tongue_corner);
            e.to((x0, tb)).to((x0, crease)).to((x0 + sh, crease - sh)).to((x0 + sh, tip + r));
            e.quad((x0 + sh, tip), (x0 + sh + r, tip)).to((x1 - sh - r, tip));
            e.quad((x1 - sh, tip), (x1 - sh, tip + r)).to((x1 - sh, crease - sh)).to((x1, crease)).to((x1, tb));
        }
        Flap::Dust => {
            let (top, c) = (tb - s.dust, s.dust_corner);
            e.to((x0, tb)).to((x0, top + c)).to((x0 + c, top)).to((x1 - c, top)).to((x1, top + c)).to((x1, tb));
        }
        Flap::Open => {
            let (mid, r) = ((x0 + x1) / 2.0, s.notch);
            e.to((x0, tb)).to((mid - r, tb));
            // A half circle dipping into the panel (y down: from π back to 0 through π/2).
            e.arc((mid, tb), r, std::f64::consts::PI, 0.0);
            e.to((x1, tb));
        }
    }
}

/// A tuck-end carton: `[glue] [back] [side] [front] [side]`, each panel the body's height; the top
/// closed by a tuck off the back, the bottom by one off the back (straight) or the front
/// (reverse), dust flaps on the sides.
fn tuck_end(v: &Values, reverse: bool) -> Result<Net, String> {
    let t = v.get("thicknessMm");
    let (mut l, mut w, mut h) = (v.get("lengthMm"), v.get("widthMm"), v.get("heightMm"));
    if v.outside {
        (l, w, h) = (l - 2.0 * t, w - 2.0 * t, h - 2.0 * t);
        if l <= 0.0 || w <= 0.0 || h <= 0.0 {
            return Err("the outside sizes are too small for the thickness".to_string());
        }
    }
    // Each panel takes up half a thickness at each of its two folds.
    let (l, w, h) = (l + t, w + t, h + t);
    let glue = clamp(w * 0.8, 8.0, 20.0).min(w);
    let tuck = w;
    let notch = (l * 0.18).min(10.16).min(h * 0.3);
    // The tongue backs the thumb notch it tucks in behind, with a little to spare.
    let tongue = clamp(w * 0.5, 8.0, h * 0.5).max(notch + 3.8);
    let tongue_corner = clamp((w * 0.4).min(12.0), 1.5, tongue * 0.45);
    let shoulder = (w * 0.3).min(8.0).min(l * 0.25).min((tongue - tongue_corner) * 0.9);
    // On a narrow box the rounded corners must leave the tongue a straight top.
    let tongue_corner = tongue_corner.min((l - 2.0 * shoulder) * 0.45);
    let dust = w - 2.0_f64.min(w * 0.2);
    let dust_corner = (w * 0.25).min(4.0);
    let taper = (h * 0.08).min(4.0);
    let top = tuck + tongue;
    let s = Sizes { top, bottom: top + h, tuck, tongue, dust, dust_corner, shoulder, tongue_corner, notch };
    let xs = [0.0, glue, glue + l, glue + l + w, glue + 2.0 * l + w, glue + 2.0 * l + 2.0 * w];
    let [_, xb0, xb1, xs1, xf1, xs2] = xs;
    let tops = [Flap::Tuck, Flap::Dust, Flap::Open, Flap::Dust];
    let bottoms = if reverse { [Flap::Open, Flap::Dust, Flap::Tuck, Flap::Dust] } else { tops };
    let spans = [(xb0, xb1), (xb1, xs1), (xs1, xf1), (xf1, xs2)];
    let end = |kinds: [Flap; 4]| {
        let mut e = Edge::from((0.0, top + taper));
        e.to((taper, top)).to((xb0, top));
        for (&(a, b), &k) in spans.iter().zip(kinds.iter()) {
            flap(&mut e, &s, a, b, k);
        }
        e
    };
    // The bottom: drawn as a top, mirrored about the body's middle and walked back.
    let mirror = |p: P| (p.0, s.top + s.bottom - p.1);
    let mut outline = end(tops);
    outline.then(&end(bottoms).map(mirror).reversed());
    let mut folds = vec![];
    let line = |a: P, b: P| {
        let mut e = Edge::from(a);
        e.to(b);
        e.open()
    };
    for x in [xb0, xb1, xs1, xf1] {
        folds.push(line((x, s.top), (x, s.bottom)));
    }
    for (kinds, y, sign) in [(tops, s.top, -1.0), (bottoms, s.bottom, 1.0)] {
        for (&(a, b), &k) in spans.iter().zip(kinds.iter()) {
            if k != Flap::Open {
                folds.push(line((a, y), (b, y)));
            }
            if k == Flap::Tuck {
                let crease = y + sign * s.tuck;
                folds.push(line((a, crease), (b, crease)));
            }
        }
    }
    let glue_area = polygon(&[(0.0, top + taper), (taper, top), (xb0, top), (xb0, s.bottom), (taper, s.bottom), (0.0, s.bottom - taper)]);
    Ok(Net {
        cut: PathData::single(outline.closed()),
        folds: PathData::new(folds),
        glue: PathData::single(glue_area),
        face: Some([xs1, s.top, xf1, s.bottom]),
    })
}

#[cfg(test)]
mod tests {
    use super::super::tests::{simple, values};
    use super::*;

    /// The panels are the inside sizes plus a thickness; the net is the glue flap, two fronts and
    /// two sides wide, and two tucks with their tongues taller than the body.
    #[test]
    fn panels_take_up_the_thickness() {
        let v = values(&STRAIGHT, &[("lengthMm", 60.0), ("widthMm", 20.0), ("heightMm", 90.0), ("thicknessMm", 0.5)], false).unwrap();
        let net = STRAIGHT.make(&v).unwrap();
        let (w, h) = net.size();
        let glue = clamp(20.5 * 0.8, 8.0, 20.0);
        assert!((w - (glue + 2.0 * 60.5 + 2.0 * 20.5)).abs() < 1e-9, "{w}");
        let tongue = clamp(20.5 * 0.5, 8.0, 45.25).max((60.5_f64 * 0.18).min(10.16) + 3.8);
        assert!((h - (90.5 + 2.0 * (20.5 + tongue))).abs() < 1e-6, "{h}");
        // Four panel folds, three flap folds at each end, a tongue fold at each end.
        assert_eq!(net.folds.subpaths.len(), 4 + 2 * 3 + 2);
    }

    /// A straight tuck has both tongue folds over the back; a reverse one has the bottom's over
    /// the front.
    #[test]
    fn the_reverse_tuck_closes_off_the_front() {
        let v = values(&STRAIGHT, &[], false).unwrap();
        // The tongue folds: the horizontal folds beyond the body (the vertical folds' span).
        let tongue_folds = |net: &Net| -> Vec<(f64, f64)> {
            let lines: Vec<_> = net.folds.subpaths.iter().map(|s| (s.anchors[0].p, s.anchors[1].p)).collect();
            let top = lines.iter().filter(|(a, b)| (a.x - b.x).abs() < 1e-9).map(|(a, b)| a.y.min(b.y)).fold(f64::INFINITY, f64::min);
            let bottom = lines.iter().filter(|(a, b)| (a.x - b.x).abs() < 1e-9).map(|(a, b)| a.y.max(b.y)).fold(0.0, f64::max);
            lines.iter().filter(|(a, _)| a.y < top - 1e-6 || a.y > bottom + 1e-6).map(|(a, _)| (a.x, a.y)).collect()
        };
        let back = tongue_folds(&STRAIGHT.make(&v).unwrap());
        let front = tongue_folds(&REVERSE.make(&v).unwrap());
        assert_eq!((back.len(), front.len()), (2, 2));
        assert!((back[0].0 - back[1].0).abs() < 1e-9, "both over the back: {back:?}");
        assert!(front[1].0 > front[0].0 + 1.0, "the bottom over the front: {front:?}");
    }

    /// Sizes across their ranges give outlines that don't cross themselves.
    #[test]
    fn outlines_are_simple_across_the_range() {
        for k in [&STRAIGHT, &REVERSE] {
            for l in [10.0, 25.0, 63.0, 200.0, 600.0] {
                for w in [8.0, 15.0, 40.0, 300.0] {
                    for h in [15.0, 40.0, 150.0, 600.0] {
                        let v = values(k, &[("lengthMm", l), ("widthMm", w), ("heightMm", h)], false).unwrap();
                        let net = k.make(&v).unwrap_or_else(|e| panic!("{} {l} {w} {h}: {e}", k.id));
                        assert!(simple(&net.cut), "{} {l}×{w}×{h} crosses itself", k.id);
                    }
                }
            }
        }
    }
}
