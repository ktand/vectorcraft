//! Fork: the gable box (C15.5): a peaked carton with a carry handle. `[glue] [front] [side]
//! [back] [side]` round the box; over the front and back a roof panel as tall as the roof's slant,
//! tilting in to meet at the ridge, a handle slot in each (the two line up into one handle) and a
//! lock ear at each top corner; over each side a gable end, a triangle with a slit the ears tuck
//! into; a tuck bottom off the front.
//!
//! Ported from Die Cut Studio (<https://github.com/pradeepsiddappa/diecutstudio>, commit
//! `3584baa4ac5d1e213a92a79b60622564518955ec`, `src/core/models/gableBox.ts`): the layout and
//! the proportions (a 45° gable, the ears, the handle, the slits, the bottom's tuck and dust
//! flaps). Ours: each panel the inside size plus a thickness, and the curve builder's slots.
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

use std::f64::consts::{FRAC_PI_2, PI};

use vectorcraft_geom::{PathData, SubPath};

use super::{Edge, Kind, Net, P, Param, THICKNESS, Unit, Values, polygon};

pub(super) const GABLE: Kind = Kind {
    id: "gable",
    name: "Gable Box",
    family: "Boxes",
    params: &[
        Param {
            key: "lengthMm",
            label: "Length",
            unit: Unit::Mm,
            default: 90.0,
            min: 40.0,
            max: 500.0,
            help: "inside, across the front (the handle's side)",
        },
        Param { key: "widthMm", label: "Width", unit: Unit::Mm, default: 60.0, min: 25.0, max: 400.0, help: "inside, front to back" },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 90.0, min: 25.0, max: 600.0, help: "inside, the walls up to the roof" },
        THICKNESS,
    ],
    sized: true,
    fit: Some(("lengthMm", "heightMm")),
    make: gable,
};

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    v.min(hi).max(lo)
}

fn line(a: P, b: P) -> SubPath {
    let mut e = Edge::from(a);
    e.to(b);
    e.open()
}

/// A slot with round ends `w` long and `h` high, centred on `c`.
fn pill(c: P, w: f64, h: f64) -> SubPath {
    let r = h / 2.0;
    let half = (w / 2.0 - r).max(0.0);
    let mut e = Edge::from((c.0 - half, c.1 - r));
    e.to((c.0 + half, c.1 - r));
    e.arc((c.0 + half, c.1), r, -FRAC_PI_2, FRAC_PI_2);
    e.to((c.0 - half, c.1 + r));
    e.arc((c.0 - half, c.1), r, FRAC_PI_2, PI + FRAC_PI_2);
    e.closed()
}

fn gable(v: &Values) -> Result<Net, String> {
    let t = v.get("thicknessMm");
    let (mut l, mut w, mut h) = (v.get("lengthMm"), v.get("widthMm"), v.get("heightMm"));
    if v.outside {
        (l, w, h) = (l - 2.0 * t, w - 2.0 * t, h - t);
        if l <= 0.0 || w <= 0.0 || h <= 0.0 {
            return Err("the outside sizes are too small for the thickness".to_string());
        }
    }
    let (l, w, h) = (l + t, w + t, h + t);
    let g = 12.7_f64.min(w * 0.9);
    let peak = w / 2.0; // a 45° gable
    let slant = (peak * peak + w * w / 4.0).sqrt();
    let tongue = (w * 0.5).min(17.78);
    let dust = (w * 0.85).min(l * 0.45);
    let handle_w = clamp(l * 0.42, 40.0_f64.min(l - 12.0), l - 12.0);
    let handle_h = 14.0_f64.min(slant * 0.3);
    let ear_h = 11.0_f64.min(slant * 0.32);
    let ear_w = clamp(l * 0.13, 10.0_f64.min(l * 0.24), l * 0.24);
    let ce = (ear_w * 0.35).min(ear_h * 0.55);
    let tc = (tongue * 0.7).min(l * 0.25);
    let dc = (dust * 0.5).min(w * 0.25);
    let gt = (h * 0.1).min(5.08);
    let top = ear_h + slant; // the eave
    let bottom = top + h;
    let roof = top - slant;
    let apex = top - peak;
    let (xf0, xs1, xb0, xs2, xr) = (g, g + l, g + l + w, g + 2.0 * l + w, g + 2.0 * l + 2.0 * w);
    let (mid1, mid2) = ((xs1 + xb0) / 2.0, (xs2 + xr) / 2.0);
    let (dust_b, tuck_b, tongue_b) = (bottom + dust, bottom + w, bottom + w + tongue);
    let mut o = Edge::from((0.0, top + gt));
    o.to((gt, top)).to((xf0, top));
    // A roof panel's top: a lock ear at each corner, standing above the ridge.
    let roof_top = |o: &mut Edge, a: f64, c: f64| {
        o.to((a, roof)).to((a, roof - ear_h + ce)).to((a + ce, roof - ear_h)).to((a + ear_w, roof - ear_h)).to((a + ear_w, roof));
        o.to((c - ear_w, roof)).to((c - ear_w, roof - ear_h)).to((c - ce, roof - ear_h)).to((c, roof - ear_h + ce)).to((c, roof));
    };
    roof_top(&mut o, xf0, xs1);
    o.to((xs1, top)).to((mid1, apex)).to((xb0, top));
    roof_top(&mut o, xb0, xs2);
    o.to((xs2, top)).to((mid2, apex)).to((xr, top)).to((xr, bottom));
    // The bottom, right to left: dust flap, the back's plain edge, dust flap, the front's tuck.
    o.to((xr, dust_b - dc)).to((xr - dc, dust_b)).to((xs2 + dc, dust_b)).to((xs2, dust_b - dc)).to((xs2, bottom));
    o.to((xb0, bottom));
    o.to((xb0, dust_b - dc)).to((xb0 - dc, dust_b)).to((xs1 + dc, dust_b)).to((xs1, dust_b - dc)).to((xs1, bottom));
    o.to((xs1, tuck_b)).to((xs1 - tc, tongue_b)).to((xf0 + tc, tongue_b)).to((xf0, tuck_b)).to((xf0, bottom));
    o.to((gt, bottom)).to((0.0, bottom - gt));
    let handle_y = roof + 12.0_f64.min(slant * 0.25) + handle_h / 2.0;
    // Each gable end's slit: below a solid tip, clear of the base.
    let tip = clamp(peak * 0.28, 7.0_f64.min(peak * 0.5), peak * 0.5);
    let slit = (peak * 0.4).min((peak - tip - 5.0).max(1.0));
    let cut = vec![
        o.closed(),
        pill(((xf0 + xs1) / 2.0, handle_y), handle_w, handle_h),
        pill(((xb0 + xs2) / 2.0, handle_y), handle_w, handle_h),
        line((mid1, apex + tip), (mid1, apex + tip + slit)),
        line((mid2, apex + tip), (mid2, apex + tip + slit)),
    ];
    let mut folds = vec![];
    for x in [xf0, xs1, xb0, xs2] {
        folds.push(line((x, top), (x, bottom)));
    }
    for (a, b) in [(xf0, xs1), (xs1, xb0), (xb0, xs2), (xs2, xr)] {
        folds.push(line((a, top), (b, top)));
    }
    for (a, b) in [(xf0, xf0 + ear_w), (xs1 - ear_w, xs1), (xb0, xb0 + ear_w), (xs2 - ear_w, xs2)] {
        folds.push(line((a, roof), (b, roof)));
    }
    for (a, b) in [(xf0, xs1), (xs1, xb0), (xs2, xr)] {
        folds.push(line((a, bottom), (b, bottom)));
    }
    folds.push(line((xf0, tuck_b), (xs1, tuck_b)));
    let glue = polygon(&[(0.0, top + gt), (gt, top), (xf0, top), (xf0, bottom), (gt, bottom), (0.0, bottom - gt)]);
    Ok(Net { cut: PathData::new(cut), folds: PathData::new(folds), glue: PathData::single(glue), face: Some([xf0, top, xs1, bottom]) })
}

#[cfg(test)]
mod tests {
    use super::super::tests::{simple, values};
    use super::*;

    /// The roof panels are as tall as the slant of a 45° gable over the box's depth, so they meet
    /// at the ridge; the outline doesn't cross itself across the range.
    #[test]
    fn the_roofs_meet_at_the_ridge() {
        let v = values(&GABLE, &[("lengthMm", 90.0), ("widthMm", 60.0), ("heightMm", 90.0), ("thicknessMm", 0.0)], false).unwrap();
        let net = GABLE.make(&v).unwrap();
        assert_eq!(net.cut.subpaths.len(), 5, "outline, two handle slots, two slits");
        // The eave folds and the ears' folds: a slant of 30 √2 apart.
        let ys: Vec<f64> =
            net.folds.subpaths.iter().filter(|s| (s.anchors[0].p.y - s.anchors[1].p.y).abs() < 1e-9).map(|s| s.anchors[0].p.y).collect();
        let (lo, hi) = (ys.iter().copied().fold(f64::INFINITY, f64::min), ys.iter().copied().fold(0.0, f64::max));
        assert!(ys.iter().any(|y| ((y - lo) - 30.0 * 2.0_f64.sqrt()).abs() < 1e-6), "{ys:?} {hi}");
        for (l, w, h) in [(40.0, 25.0, 25.0), (90.0, 60.0, 90.0), (500.0, 400.0, 600.0), (40.0, 400.0, 25.0)] {
            let v = values(&GABLE, &[("lengthMm", l), ("widthMm", w), ("heightMm", h)], false).unwrap();
            let net = GABLE.make(&v).unwrap();
            assert!(simple(&PathData::single(net.cut.subpaths[0].clone())), "{l} {w} {h}");
        }
    }
}
