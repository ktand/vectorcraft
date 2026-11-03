//! Fork: boxes of our own geometry (C15.2): a tray with a separate lid that slides down over it,
//! and a pillow box. The open tray they build on is in the parent module.

use vectorcraft_geom::{Affine, PathData};

use super::{Edge, Kind, Net, P, Param, THICKNESS, Unit, Values, polygon, tray};

/// The space between the two pieces of a two-piece net, mm.
const PIECE_GAP_MM: f64 = 10.0;

pub(super) const TRAY_LID: Kind = Kind {
    id: "trayLid",
    name: "Tray with Lid",
    family: "Boxes",
    params: &[
        Param { key: "lengthMm", label: "Length", unit: Unit::Mm, default: 100.0, min: 10.0, max: 1000.0, help: "the tray's, inside" },
        Param { key: "widthMm", label: "Width", unit: Unit::Mm, default: 70.0, min: 10.0, max: 1000.0, help: "the tray's, inside" },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 30.0, min: 5.0, max: 500.0, help: "the tray's, inside" },
        Param {
            key: "lidHeightMm",
            label: "Lid Height",
            unit: Unit::Mm,
            default: 30.0,
            min: 5.0,
            max: 500.0,
            help: "how far the lid comes down: the tray's height covers it all",
        },
        Param {
            key: "clearanceMm",
            label: "Clearance",
            unit: Unit::Mm,
            default: 0.8,
            min: 0.0,
            max: 5.0,
            help: "the play between the tray and the lid, so the lid slides on",
        },
        THICKNESS,
        Param {
            key: "tabMm",
            label: "Glue Tab",
            unit: Unit::Mm,
            default: 12.0,
            min: 3.0,
            max: 100.0,
            help: "how far each corner's glue tab reaches along the side wall",
        },
    ],
    sized: true,
    make: tray_lid,
};

/// The tray, and under it a lid: another open tray whose inside is the tray's outside (two walls'
/// thickness more each way) plus the clearance, as deep as the lid height asked for.
fn tray_lid(v: &Values) -> Result<Net, String> {
    let t = v.get("thicknessMm");
    let (mut l, mut w, mut h) = (v.get("lengthMm"), v.get("widthMm"), v.get("heightMm"));
    if v.outside {
        (l, w, h) = (l - 2.0 * t, w - 2.0 * t, h - t);
        if l <= 0.0 || w <= 0.0 || h <= 0.0 {
            return Err("the outside sizes are too small for the thickness".to_string());
        }
    }
    let piece = |l: f64, w: f64, h: f64| {
        let nums = vec![("lengthMm", l), ("widthMm", w), ("heightMm", h), ("thicknessMm", t), ("tabMm", v.get("tabMm"))];
        tray(&Values { nums, outside: false })
    };
    let base = piece(l, w, h)?;
    let fit = 2.0 * t + v.get("clearanceMm");
    let lid = piece(l + fit, w + fit, v.get("lidHeightMm")).map_err(|e| format!("the lid: {e}"))?;
    let down = Affine::translate((0.0, base.size().1 + PIECE_GAP_MM));
    let join = |a: &PathData, b: &PathData| PathData::new(a.subpaths.iter().chain(b.transformed(down).subpaths.iter()).cloned().collect());
    Ok(Net { cut: join(&base.cut, &lid.cut), folds: join(&base.folds, &lid.folds), glue: join(&base.glue, &lid.glue) })
}

pub(super) const PILLOW: Kind = Kind {
    id: "pillow",
    name: "Pillow Box",
    family: "Boxes",
    params: &[
        Param {
            key: "lengthMm",
            label: "Length",
            unit: Unit::Mm,
            default: 120.0,
            min: 20.0,
            max: 600.0,
            help: "end to end, along the fold lines' middle",
        },
        Param { key: "widthMm", label: "Width", unit: Unit::Mm, default: 80.0, min: 20.0, max: 400.0, help: "across, flat" },
        Param {
            key: "depthMm",
            label: "Depth",
            unit: Unit::Mm,
            default: 25.0,
            min: 2.0,
            max: 300.0,
            help: "how far the ends' curves bow out: the box's thickness when squeezed into shape",
        },
        Param { key: "glueMm", label: "Glue Flap", unit: Unit::Mm, default: 12.0, min: 5.0, max: 40.0, help: "the flap the seam is glued along" },
    ],
    sized: false,
    make: pillow,
};

/// The circular arc through `(x0, y)` and `(x1, y)` bowing `sag` toward smaller y (`up`) or
/// larger y, from `x0` to `x1`. The radius from the chord `c` and the sagitta `s`:
/// `r = (c²/4 + s²) / 2s`.
fn bow(e: &mut Edge, x0: f64, x1: f64, y: f64, sag: f64, up: bool) {
    let c = x1 - x0;
    let r = (c * c / 4.0 + sag * sag) / (2.0 * sag);
    let mid = (x0 + x1) / 2.0;
    let sign = if up { 1.0 } else { -1.0 };
    let centre = (mid, y - sign * (sag - r));
    let a0 = (y - centre.1).atan2(x0 - mid);
    let a1 = (y - centre.1).atan2(x1 - mid);
    e.arc(centre, r, a0, a1);
}

/// A pillow box: `[glue] [panel] [panel]` round a flattened tube, each end of each panel closed by
/// a curved flap. Our own geometry: each end is a lens between the cut, an arc bowing out of the
/// panel, and the fold, the same arc bowing into it; folding both ends' lenses in bows the panels
/// into the pillow's shape. The arcs' sagitta is half the depth, at most 0.45 of the width (past
/// that the board can't take the bend).
fn pillow(v: &Values) -> Result<Net, String> {
    let (l, w, d, g) = (v.get("lengthMm"), v.get("widthMm"), v.get("depthMm"), v.get("glueMm"));
    let sag = d / 2.0;
    if sag > w * 0.45 {
        return Err(format!("the depth can be at most {:.0} mm for a width of {w} mm", w * 0.9));
    }
    if 2.0 * sag >= l {
        return Err("the depth must be less than the length".to_string());
    }
    let taper = (g * 0.35).min(4.0);
    let (top, bottom) = (sag, sag + l);
    let xs = [g, g + w, g + 2.0 * w];
    let end = |down: bool| {
        // Drawn as the top end; the bottom is the same mirrored.
        let mut e = Edge::from((0.0, top + taper));
        e.to((taper, top)).to((xs[0], top));
        bow(&mut e, xs[0], xs[1], top, sag, !down);
        bow(&mut e, xs[1], xs[2], top, sag, !down);
        e
    };
    let mirror = |p: P| (p.0, top + bottom - p.1);
    let mut outline = end(false);
    outline.then(&end(false).map(mirror).reversed());
    let line = |a: P, b: P| {
        let mut e = Edge::from(a);
        e.to(b);
        e.open()
    };
    let mut folds = vec![line((xs[0], top), (xs[0], bottom)), line((xs[1], top), (xs[1], bottom))];
    for (x0, x1) in [(xs[0], xs[1]), (xs[1], xs[2])] {
        for (y, up) in [(top, false), (bottom, true)] {
            let mut e = Edge::from((x0, y));
            bow(&mut e, x0, x1, y, sag, up);
            folds.push(e.open());
        }
    }
    let glue = polygon(&[(0.0, top + taper), (taper, top), (xs[0], top), (xs[0], bottom), (taper, bottom), (0.0, bottom - taper)]);
    Ok(Net { cut: PathData::single(outline.closed()), folds: PathData::new(folds), glue: PathData::single(glue) })
}

#[cfg(test)]
mod tests {
    use super::super::tests::{simple, values};
    use super::super::{KINDS, TRAY};
    use super::*;

    /// The lid's inside is the tray's outside plus the clearance; it lies under the tray.
    #[test]
    fn the_lid_slides_over_the_tray() {
        let given = [("lengthMm", 100.0), ("widthMm", 60.0), ("heightMm", 20.0), ("lidHeightMm", 20.0), ("clearanceMm", 1.0), ("thicknessMm", 0.5)];
        let net = TRAY_LID.make(&values(&TRAY_LID, &given, false).unwrap()).unwrap();
        assert_eq!(net.cut.subpaths.len(), 2, "two pieces");
        let base = TRAY.make(&values(&TRAY, &given[..3].iter().copied().chain([("thicknessMm", 0.5)]).collect::<Vec<_>>(), false).unwrap()).unwrap();
        let (bw, bh) = base.size();
        let lid = net.cut.subpaths[1].anchors.iter().map(|a| a.p).collect::<Vec<_>>();
        let (x0, x1) = lid.iter().fold((f64::INFINITY, 0.0f64), |(lo, hi), p| (lo.min(p.x), hi.max(p.x)));
        let y0 = lid.iter().map(|p| p.y).fold(f64::INFINITY, f64::min);
        assert!((y0 - (bh + PIECE_GAP_MM)).abs() < 1e-9, "under the tray: {y0}");
        // The lid's base: the tray's inside + a thickness, + 2 thicknesses + 1 mm; its walls 20.25.
        assert!(((x1 - x0) - (100.0 + 0.5 + 2.0 * 0.5 + 1.0 + 2.0 * 20.25)).abs() < 1e-9, "{}", x1 - x0);
        assert!(bw < x1 - x0);
    }

    /// Each end of each panel is a lens: the fold bows into the panel as far as the cut bows out.
    #[test]
    fn the_pillow_ends_are_lenses() {
        let v = values(&PILLOW, &[("lengthMm", 120.0), ("widthMm", 80.0), ("depthMm", 30.0)], false).unwrap();
        let net = PILLOW.make(&v).unwrap();
        let (w, h) = net.size();
        assert!((w - (12.0 + 160.0)).abs() < 1e-9 && (h - (120.0 + 30.0)).abs() < 1e-6, "{w} {h}");
        // The four curved folds reach 15 mm into the body, the cut 15 mm out of it.
        let curved: Vec<_> = net.folds.subpaths.iter().skip(2).collect();
        assert_eq!(curved.len(), 4);
        use vectorcraft_geom::ParamCurve;
        for s in curved {
            let y = s.anchors[0].p.y;
            let pts: Vec<_> =
                (0..s.segment_count()).flat_map(|i| (0..=8).map(move |k| (i, k))).map(|(i, k)| s.segment(i).eval(f64::from(k) / 8.0)).collect();
            // Into the body: down from the top end (y 15), up from the bottom (y 135).
            let into = pts.iter().map(|p| if y < 75.0 { p.y - y } else { y - p.y }).fold(f64::MIN, f64::max);
            assert!((into - 15.0).abs() < 0.01, "{y}: {into}");
            assert!(pts.iter().all(|p| if y < 75.0 { p.y >= y - 1e-9 } else { p.y <= y + 1e-9 }), "{y}");
        }
        assert!(PILLOW.make(&values(&PILLOW, &[("widthMm", 20.0), ("depthMm", 30.0)], false).unwrap()).is_err());
    }

    /// Sizes across their ranges give outlines that don't cross themselves.
    #[test]
    fn outlines_are_simple_across_the_range() {
        for (l, w, d) in [(20.0, 20.0, 2.0), (120.0, 20.0, 18.0), (120.0, 80.0, 25.0), (600.0, 400.0, 300.0)] {
            let v = values(&PILLOW, &[("lengthMm", l), ("widthMm", w), ("depthMm", d)], false).unwrap();
            assert!(simple(&PILLOW.make(&v).unwrap().cut), "{l} {w} {d}");
        }
        for k in KINDS {
            let net = k.make(&values(k, &[], false).unwrap()).unwrap();
            assert!(simple(&net.cut), "{}", k.id);
        }
    }
}
