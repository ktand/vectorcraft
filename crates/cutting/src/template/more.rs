//! Fork: more templates (C15.5), all our own geometry: a shipping carton in the style the FEFCO
//! code numbers 0201 (the regular slotted case: four panels and a glue flap, flaps half the depth
//! so the outer ones meet in the middle; only the style's number and name come from the code), a
//! drawer box (a tray with a finger hole, and a sleeve it slides into), bunting, a paper lantern
//! and a party hat.

use std::f64::consts::{PI, TAU};

use vectorcraft_geom::{PathData, SubPath};

use super::{Edge, Kind, Net, P, Param, THICKNESS, Unit, Values, polygon, tray};

/// The space between pieces of a net of several, mm.
const GAP_MM: f64 = 10.0;

fn line(a: P, b: P) -> SubPath {
    let mut e = Edge::from(a);
    e.to(b);
    e.open()
}

fn circle(c: P, r: f64) -> SubPath {
    let mut e = Edge::from((c.0 + r, c.1));
    e.arc(c, r, 0.0, TAU);
    e.closed()
}

/// `net`'s paths moved by `d`.
fn moved(net: &Net, d: P) -> Net {
    let to = vectorcraft_geom::Affine::translate(d);
    Net {
        cut: net.cut.transformed(to),
        folds: net.folds.transformed(to),
        glue: net.glue.transformed(to),
        face: net.face.map(|f| [f[0] + d.0, f[1] + d.1, f[2] + d.0, f[3] + d.1]),
    }
}

/// Two nets as one, the paths of both.
fn join(a: Net, b: Net) -> Net {
    let cat = |x: PathData, y: PathData| PathData::new(x.subpaths.into_iter().chain(y.subpaths).collect());
    Net { cut: cat(a.cut, b.cut), folds: cat(a.folds, b.folds), glue: cat(a.glue, b.glue), face: a.face.or(b.face) }
}

pub(super) const CARTON: Kind = Kind {
    id: "carton",
    name: "Shipping Carton (0201)",
    family: "Boxes",
    params: &[
        Param { key: "lengthMm", label: "Length", unit: Unit::Mm, default: 150.0, min: 20.0, max: 1000.0, help: "inside, the long side" },
        Param { key: "widthMm", label: "Width", unit: Unit::Mm, default: 100.0, min: 20.0, max: 1000.0, help: "inside, the short side" },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 100.0, min: 20.0, max: 1000.0, help: "inside, the depth" },
        Param {
            key: "thicknessMm",
            label: "Thickness",
            unit: Unit::Mm,
            default: 1.5,
            min: 0.0,
            max: 8.0,
            help: "the board's thickness, taken up at each fold",
        },
    ],
    sized: true,
    fit: Some(("lengthMm", "heightMm")),
    make: carton,
};

/// `[glue] [long] [short] [long] [short]` round the box; each panel has a flap at the top and the
/// bottom half the short side deep (plus a thickness), so the long sides' flaps meet in the
/// middle; a slot as wide as two thicknesses (at least 1 mm) between neighbouring flaps.
fn carton(v: &Values) -> Result<Net, String> {
    let t = v.get("thicknessMm");
    let (mut l, mut w, mut h) = (v.get("lengthMm"), v.get("widthMm"), v.get("heightMm"));
    if v.outside {
        (l, w, h) = (l - 2.0 * t, w - 2.0 * t, h - 2.0 * t);
        if l <= 0.0 || w <= 0.0 || h <= 0.0 {
            return Err("the outside sizes are too small for the thickness".to_string());
        }
    }
    let (l, w, h) = (l + t, w + t, h + t);
    let flap = w / 2.0 + t / 2.0;
    let slot = (2.0 * t).max(1.0);
    let glue = (w * 0.3).clamp(10.0, 35.0);
    let taper = glue.min(h / 4.0);
    let xs = [glue, glue + l, glue + l + w, glue + 2.0 * l + w, glue + 2.0 * l + 2.0 * w];
    let (top, bottom) = (flap, flap + h);
    // Left to right along the top: the glue flap, then each panel's flap with half a slot each
    // side where it meets a neighbour; back along the bottom.
    let mut o = Edge::from((0.0, top + taper));
    o.to((glue, top));
    for i in 0..4 {
        let (a, b) = (xs[i], xs[i + 1]);
        let a = if i == 0 { a } else { a + slot / 2.0 };
        let b = if i == 3 { b } else { b - slot / 2.0 };
        o.to((a, top)).to((a, 0.0)).to((b, 0.0)).to((b, top));
    }
    o.to((xs[4], bottom));
    for i in (0..4).rev() {
        let (a, b) = (xs[i], xs[i + 1]);
        let a = if i == 0 { a } else { a + slot / 2.0 };
        let b = if i == 3 { b } else { b - slot / 2.0 };
        o.to((b, bottom)).to((b, bottom + flap)).to((a, bottom + flap)).to((a, bottom));
    }
    o.to((glue, bottom)).to((0.0, bottom - taper));
    let mut folds = vec![];
    for &x in &xs[..4] {
        folds.push(line((x, top), (x, bottom)));
    }
    for i in 0..4 {
        folds.push(line((xs[i], top), (xs[i + 1], top)));
        folds.push(line((xs[i], bottom), (xs[i + 1], bottom)));
    }
    let glue_area = polygon(&[(0.0, top + taper), (glue, top), (glue, bottom), (0.0, bottom - taper)]);
    Ok(Net {
        cut: PathData::single(o.closed()),
        folds: PathData::new(folds),
        glue: PathData::single(glue_area),
        face: Some([xs[0], top, xs[1], bottom]),
    })
}

pub(super) const DRAWER: Kind = Kind {
    id: "drawer",
    name: "Drawer Box",
    family: "Boxes",
    params: &[
        Param { key: "lengthMm", label: "Length", unit: Unit::Mm, default: 100.0, min: 20.0, max: 600.0, help: "the drawer's inside, front to back" },
        Param { key: "widthMm", label: "Width", unit: Unit::Mm, default: 70.0, min: 20.0, max: 600.0, help: "the drawer's inside, side to side" },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 30.0, min: 8.0, max: 300.0, help: "the drawer's inside" },
        Param {
            key: "clearanceMm",
            label: "Clearance",
            unit: Unit::Mm,
            default: 1.0,
            min: 0.0,
            max: 5.0,
            help: "the play between the drawer and its sleeve",
        },
        THICKNESS,
    ],
    sized: true,
    fit: Some(("widthMm", "heightMm")),
    make: drawer,
};

/// A drawer: an open tray with a finger hole in its front wall, and under it a sleeve, a tube the
/// drawer's outside plus the clearance across and as long as the drawer, glued along one edge.
fn drawer(v: &Values) -> Result<Net, String> {
    let t = v.get("thicknessMm");
    let (mut l, mut w, mut h) = (v.get("lengthMm"), v.get("widthMm"), v.get("heightMm"));
    if v.outside {
        (l, w, h) = (l - 2.0 * t, w - 2.0 * t, h - t);
        if l <= 0.0 || w <= 0.0 || h <= 0.0 {
            return Err("the outside sizes are too small for the thickness".to_string());
        }
    }
    let tab = (h * 0.6).min(w / 2.2).min(15.0);
    let nums = vec![("lengthMm", w), ("widthMm", l), ("heightMm", h), ("thicknessMm", t), ("tabMm", tab)];
    let mut box_net = tray(&Values { nums, outside: false })?;
    // The tray's front wall is its top strip (y 0 to the wall's height), across the drawer's width.
    let wall = h + t / 2.0;
    let r = (wall * 0.22).min(8.0);
    let mid = wall + (w + t) / 2.0;
    box_net.cut.subpaths.push(circle((mid, wall / 2.0), r));
    // The sleeve: [glue] [top] [side] [bottom] [side], as long as the drawer.
    let c = v.get("clearanceMm");
    let (across, up, long) = (w + 2.0 * t + c + t, h + t + c + t, l + 2.0 * t);
    let glue = (up * 0.6).clamp(8.0, 15.0);
    let xs = [0.0, glue, glue + across, glue + across + up, glue + 2.0 * across + up, glue + 2.0 * across + 2.0 * up];
    let taper = glue.min(long / 4.0) * 0.5;
    let sleeve_cut = polygon(&[(0.0, taper), (glue, 0.0), (xs[5], 0.0), (xs[5], long), (glue, long), (0.0, long - taper)]);
    let sleeve_folds: Vec<_> = xs[1..5].iter().map(|&x| line((x, 0.0), (x, long))).collect();
    let sleeve_glue = polygon(&[(0.0, taper), (glue, 0.0), (glue, long), (0.0, long - taper)]);
    let sleeve = Net {
        cut: PathData::single(sleeve_cut),
        folds: PathData::new(sleeve_folds),
        glue: PathData::single(sleeve_glue),
        face: Some([xs[1], 0.0, xs[2], long]),
    };
    let below = box_net.size().1 + GAP_MM;
    // The art goes on the drawer's front: the tray's front wall.
    let mut out = join(box_net, moved(&sleeve, (0.0, below)));
    out.face = Some([wall, 0.0, wall + w + t, wall]);
    Ok(out)
}

pub(super) const BUNTING: Kind = Kind {
    id: "bunting",
    name: "Bunting",
    family: "Party",
    params: &[
        Param { key: "flags", label: "Flags", unit: Unit::Count, default: 5.0, min: 1.0, max: 30.0, help: "how many pennants" },
        Param { key: "widthMm", label: "Width", unit: Unit::Mm, default: 100.0, min: 20.0, max: 400.0, help: "each pennant, across the top" },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 140.0, min: 20.0, max: 600.0, help: "top to point" },
        Param {
            key: "foldMm",
            label: "Fold Over",
            unit: Unit::Mm,
            default: 20.0,
            min: 5.0,
            max: 60.0,
            help: "the strip folded over the string and glued",
        },
        Param {
            key: "swallowtail",
            label: "Swallowtail",
            unit: Unit::Toggle,
            default: 0.0,
            min: 0.0,
            max: 1.0,
            help: "a notch in the end instead of a point",
        },
    ],
    sized: false,
    fit: Some(("widthMm", "heightMm")),
    make: bunting,
};

/// Pennants side by side: each a strip folded over the string, then a point (or a swallowtail).
fn bunting(v: &Values) -> Result<Net, String> {
    let (n, w, h, f) = (v.get("flags") as usize, v.get("widthMm"), v.get("heightMm"), v.get("foldMm"));
    let tail = v.get("swallowtail") > 0.5;
    let (mut cut, mut folds, mut glue) = (vec![], vec![], vec![]);
    for i in 0..n {
        let x = i as f64 * (w + 5.0);
        let mut shape = vec![(x, 0.0), (x + w, 0.0), (x + w, f), (x + w, f + h)];
        if tail {
            shape.push((x + w / 2.0, f + h * 0.75));
        } else {
            shape.pop();
            shape.push((x + w / 2.0, f + h));
        }
        if tail {
            shape.push((x, f + h));
        }
        shape.push((x, f));
        cut.push(polygon(&shape));
        folds.push(line((x, f), (x + w, f)));
        glue.push(polygon(&[(x, 0.0), (x + w, 0.0), (x + w, f), (x, f)]));
    }
    Ok(Net { cut: PathData::new(cut), folds: PathData::new(folds), glue: PathData::new(glue), face: Some([0.0, f, w, f + h]) })
}

pub(super) const LANTERN: Kind = Kind {
    id: "lantern",
    name: "Paper Lantern",
    family: "Party",
    params: &[
        Param { key: "widthMm", label: "Width", unit: Unit::Mm, default: 240.0, min: 60.0, max: 800.0, help: "round the lantern" },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 160.0, min: 40.0, max: 600.0, help: "top to bottom" },
        Param { key: "slitMm", label: "Slit Spacing", unit: Unit::Mm, default: 15.0, min: 5.0, max: 60.0, help: "between the slits" },
        Param { key: "marginMm", label: "Margin", unit: Unit::Mm, default: 20.0, min: 5.0, max: 100.0, help: "uncut at the top and the bottom" },
    ],
    sized: false,
    fit: Some(("widthMm", "heightMm")),
    make: lantern,
};

/// A paper lantern: a sheet with slits between the top and bottom margins, rolled into a tube and
/// glued along the strip at its end (the slits bow out when it's pushed down), and a handle strip
/// glued inside the top.
fn lantern(v: &Values) -> Result<Net, String> {
    let (w, h, gap, m) = (v.get("widthMm"), v.get("heightMm"), v.get("slitMm"), v.get("marginMm"));
    if 2.0 * m + 10.0 > h {
        return Err(format!("margins of {m} mm leave no room for slits in a lantern {h} mm high"));
    }
    let glue = 15.0;
    let mut cut = vec![polygon(&[(0.0, 0.0), (w + glue, 0.0), (w + glue, h), (0.0, h)])];
    let n = ((w - gap) / gap).floor().max(1.0) as usize;
    let first = (w - (n - 1) as f64 * gap) / 2.0;
    for i in 0..n {
        let x = first + i as f64 * gap;
        cut.push(line((x, m), (x, h - m)));
    }
    // The handle, under the lantern.
    let (hl, hw) = ((w * 0.8).max(100.0), 12.0);
    cut.push(polygon(&[(0.0, h + GAP_MM), (hl, h + GAP_MM), (hl, h + GAP_MM + hw), (0.0, h + GAP_MM + hw)]));
    let folds = vec![line((w, 0.0), (w, h))];
    let glue_area = vec![polygon(&[(w, 0.0), (w + glue, 0.0), (w + glue, h), (w, h)])];
    Ok(Net { cut: PathData::new(cut), folds: PathData::new(folds), glue: PathData::new(glue_area), face: Some([0.0, 0.0, w, h]) })
}

pub(super) const PARTY_HAT: Kind = Kind {
    id: "partyHat",
    name: "Party Hat",
    family: "Party",
    params: &[
        Param { key: "diameterMm", label: "Diameter", unit: Unit::Mm, default: 140.0, min: 30.0, max: 400.0, help: "across the brim" },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 180.0, min: 30.0, max: 600.0, help: "brim to tip" },
        Param {
            key: "glueMm",
            label: "Glue Strip",
            unit: Unit::Mm,
            default: 12.0,
            min: 5.0,
            max: 40.0,
            help: "the strip glued under the other edge",
        },
    ],
    sized: false,
    fit: None,
    make: party_hat,
};

/// A cone with no base: a circle's sector, its arc the brim's circumference, its radius the
/// slant; a glue strip along one straight edge; two small holes near the brim for the elastic.
fn party_hat(v: &Values) -> Result<Net, String> {
    let (d, h, g) = (v.get("diameterMm"), v.get("heightMm"), v.get("glueMm"));
    let r = d / 2.0;
    let slant = h.hypot(r);
    let sweep = TAU * r / slant;
    if sweep > TAU * 0.9 {
        return Err("the hat is too flat to wrap with its glue strip: make it taller".to_string());
    }
    let (a0, a1) = (PI / 2.0 - sweep / 2.0, PI / 2.0 + sweep / 2.0);
    let c = (slant + g, g);
    let at = |rad: f64, a: f64| (c.0 + rad * a.cos(), c.1 + rad * a.sin());
    let mut o = Edge::from(c);
    o.to(at(slant, a0));
    o.arc(c, slant, a0, a1);
    // The glue strip past the edge at a1, its far end along the brim.
    let (nx, ny) = (-a1.sin(), a1.cos());
    let (rx, ry) = (a1.cos(), a1.sin());
    let s0 = at(slant, a1);
    let p1 = (s0.0 + nx * g - rx * g, s0.1 + ny * g - ry * g);
    let p2 = (c.0 + nx * g + rx * g, c.1 + ny * g + ry * g);
    o.to(p1).to(p2).to(c);
    let hole = |a: f64| circle(at(slant - 10.0, a), 1.5);
    let quarter = sweep * 0.25;
    let cut = vec![o.closed(), hole(a0 + quarter), hole(a1 - quarter)];
    Ok(Net { cut: PathData::new(cut), folds: PathData::single(line(s0, c)), glue: PathData::single(polygon(&[s0, p1, p2, c])), face: None })
}

#[cfg(test)]
mod tests {
    use super::super::tests::{simple, values};
    use super::*;

    /// The carton's flaps meet in the middle: each half the short side (plus half a thickness)
    /// deep; its panels the inside sizes plus a thickness.
    #[test]
    fn the_cartons_flaps_meet() {
        let v = values(&CARTON, &[("lengthMm", 200.0), ("widthMm", 100.0), ("heightMm", 80.0), ("thicknessMm", 2.0)], false).unwrap();
        let net = CARTON.make(&v).unwrap();
        let (w, h) = net.size();
        let glue = (102.0f64 * 0.3).clamp(10.0, 35.0);
        assert!((w - (glue + 2.0 * 202.0 + 2.0 * 102.0)).abs() < 1e-9, "{w}");
        assert!((h - (82.0 + 2.0 * 52.0)).abs() < 1e-9, "{h}");
        assert_eq!(net.folds.subpaths.len(), 4 + 8);
        assert!(simple(&net.cut));
    }

    /// The drawer's sleeve is the drawer's outside plus the clearance across.
    #[test]
    fn the_drawer_slides_into_its_sleeve() {
        let v = values(&DRAWER, &[("lengthMm", 100.0), ("widthMm", 60.0), ("heightMm", 30.0), ("clearanceMm", 1.0), ("thicknessMm", 0.5)], false)
            .unwrap();
        let net = DRAWER.make(&v).unwrap();
        assert_eq!(net.cut.subpaths.len(), 3, "the tray, its finger hole and the sleeve");
        // The sleeve's four folds: its panels 60 + 1 + 1 + 0.5 and 30 + 0.5 + 1 + 0.5 across.
        let sleeve: Vec<f64> = net.folds.subpaths.iter().rev().take(4).map(|s| s.anchors[0].p.x).collect();
        let mut xs = sleeve.clone();
        xs.sort_by(f64::total_cmp);
        assert!(((xs[1] - xs[0]) - 62.5).abs() < 1e-9 && ((xs[2] - xs[1]) - 32.0).abs() < 1e-9, "{xs:?}");
    }

    /// Bunting gives one pennant per flag, with a fold each; the lantern one slit per spacing;
    /// the hat its two elastic holes.
    #[test]
    fn party_pieces() {
        let b = BUNTING.make(&values(&BUNTING, &[("flags", 4.0)], false).unwrap()).unwrap();
        assert_eq!((b.cut.subpaths.len(), b.folds.subpaths.len()), (4, 4));
        let t = BUNTING.make(&values(&BUNTING, &[("swallowtail", 1.0)], false).unwrap()).unwrap();
        assert_eq!(t.cut.subpaths[0].anchors.len(), 7);
        let l = LANTERN.make(&values(&LANTERN, &[("widthMm", 150.0), ("slitMm", 15.0)], false).unwrap()).unwrap();
        assert_eq!(l.cut.subpaths.len(), 1 + 9 + 1, "sheet, slits, handle");
        let hat = PARTY_HAT.make(&values(&PARTY_HAT, &[], false).unwrap()).unwrap();
        assert_eq!(hat.cut.subpaths.len(), 3);
        assert!(simple(&PathData::single(hat.cut.subpaths[0].clone())));
        assert!(PARTY_HAT.make(&values(&PARTY_HAT, &[("diameterMm", 400.0), ("heightMm", 30.0)], false).unwrap()).is_err());
    }
}
