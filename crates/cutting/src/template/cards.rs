//! Fork: envelopes and cards (C15.3), all our own geometry: an envelope with a square or a pointed
//! flap, a place card, a gift tag, an earring card and a gift card holder. Die Cut Studio's list
//! (`docs/cutting/IMPLEMENTATION_PLAN.md` › Phase 15) only named which kinds to offer.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use vectorcraft_geom::{PathData, SubPath};

use super::{Edge, Kind, Net, P, Param, Unit, Values, polygon};

/// The family these are listed under.
const FAMILY: &str = "Cards & Envelopes";

/// A closed circle.
fn circle(c: P, r: f64) -> SubPath {
    let mut e = Edge::from((c.0 + r, c.1));
    e.arc(c, r, 0.0, TAU);
    e.closed()
}

/// A rectangle with its corners rounded by `r` (0: square).
fn rounded_rect(x0: f64, y0: f64, x1: f64, y1: f64, r: f64) -> SubPath {
    let r = r.min((x1 - x0) / 2.0).min((y1 - y0) / 2.0).max(0.0);
    let mut e = Edge::from((x0 + r, y0));
    e.to((x1 - r, y0));
    if r > 0.0 {
        e.arc((x1 - r, y0 + r), r, -FRAC_PI_2, 0.0);
    }
    e.to((x1, y1 - r));
    if r > 0.0 {
        e.arc((x1 - r, y1 - r), r, 0.0, FRAC_PI_2);
    }
    e.to((x0 + r, y1));
    if r > 0.0 {
        e.arc((x0 + r, y1 - r), r, FRAC_PI_2, PI);
    }
    e.to((x0, y0 + r));
    if r > 0.0 {
        e.arc((x0 + r, y0 + r), r, PI, PI + FRAC_PI_2);
    }
    e.closed()
}

/// A slot with round ends, `w` long and `h` high, centred on `c` (a hang slot).
fn slot(c: P, w: f64, h: f64) -> SubPath {
    let r = h / 2.0;
    let half = (w / 2.0 - r).max(0.0);
    let mut e = Edge::from((c.0 - half, c.1 - r));
    e.to((c.0 + half, c.1 - r));
    e.arc((c.0 + half, c.1), r, -FRAC_PI_2, FRAC_PI_2);
    e.to((c.0 - half, c.1 + r));
    e.arc((c.0 - half, c.1), r, FRAC_PI_2, PI + FRAC_PI_2);
    e.closed()
}

fn line(a: P, b: P) -> SubPath {
    let mut e = Edge::from(a);
    e.to(b);
    e.open()
}

const ENVELOPE_PARAMS: &[Param] = &[
    Param { key: "cardWidthMm", label: "Card Width", unit: Unit::Mm, default: 148.0, min: 30.0, max: 400.0, help: "the card it holds, across" },
    Param {
        key: "cardHeightMm",
        label: "Card Height",
        unit: Unit::Mm,
        default: 105.0,
        min: 30.0,
        max: 400.0,
        help: "the card it holds, up and down",
    },
    Param { key: "easeMm", label: "Ease", unit: Unit::Mm, default: 3.0, min: 0.0, max: 20.0, help: "room round the card on each side" },
];

pub(super) const ENVELOPE: Kind = Kind {
    id: "envelope",
    name: "Envelope",
    family: FAMILY,
    params: ENVELOPE_PARAMS,
    sized: false,
    fit: Some(("cardWidthMm", "cardHeightMm")),
    make: envelope_square,
};

pub(super) const POINTED_ENVELOPE: Kind = Kind {
    id: "pointedEnvelope",
    name: "Pointed Envelope",
    family: FAMILY,
    params: ENVELOPE_PARAMS,
    sized: false,
    fit: Some(("cardWidthMm", "cardHeightMm")),
    make: envelope_pointed,
};

fn envelope_square(v: &Values) -> Result<Net, String> {
    envelope(v, false)
}

fn envelope_pointed(v: &Values) -> Result<Net, String> {
    envelope(v, true)
}

/// An envelope: the face the card's size plus the ease each way; side flaps folded in first, the
/// bottom flap folded up and glued over them along its slanted edges, the closing flap folded down
/// last. A small cut across each of the face's corners lets the flaps fold without binding.
fn envelope(v: &Values, pointed: bool) -> Result<Net, String> {
    let e = v.get("easeMm");
    let (w, h) = (v.get("cardWidthMm") + 2.0 * e, v.get("cardHeightMm") + 2.0 * e);
    let side = (w * 0.3).min(h * 0.45).max(10.0); // each side flap's depth
    let bottom = h * 0.6; // reaches past the middle, to overlap the closing flap
    let top = if pointed { h * 0.75 } else { h * 0.45 };
    let relief = 2.0_f64.min(w * 0.02).min(h * 0.02);
    let (x0, y0) = (side, top);
    let (x1, y1) = (x0 + w, y0 + h);
    // The side flaps narrow to their outer edge; the bottom flap's sides slant in by the same.
    let side_in = h * 0.18;
    let bottom_in = w * 0.15;
    let r = (top * 0.25).min(w * 0.08).min(12.0);
    let mut o = Edge::from((x0 + relief, y0));
    if pointed {
        // To a rounded point over the face's middle.
        let tip = ((x0 + x1) / 2.0, 0.0);
        let towards = |from: P, to: P, d: f64| {
            let (dx, dy) = (to.0 - from.0, to.1 - from.1);
            let l = dx.hypot(dy);
            (from.0 + dx / l * d, from.1 + dy / l * d)
        };
        o.to(towards(tip, (x0 + relief, y0), r)).quad(tip, towards(tip, (x1 - relief, y0), r)).to((x1 - relief, y0));
    } else {
        let inset = w * 0.06;
        o.to((x0 + inset, r)).quad((x0 + inset, 0.0), (x0 + inset + r, 0.0)).to((x1 - inset - r, 0.0));
        o.quad((x1 - inset, 0.0), (x1 - inset, r)).to((x1 - relief, y0));
    }
    o.to((x1, y0 + relief)).to((x1 + side, y0 + side_in)).to((x1 + side, y1 - side_in)).to((x1, y1 - relief));
    o.to((x1 - relief, y1)).to((x1 - bottom_in, y1 + bottom)).to((x0 + bottom_in, y1 + bottom)).to((x0 + relief, y1));
    o.to((x0, y1 - relief)).to((0.0, y1 - side_in)).to((0.0, y0 + side_in)).to((x0, y0 + relief));
    let folds = vec![
        line((x0 + relief, y0), (x1 - relief, y0)),
        line((x1, y0 + relief), (x1, y1 - relief)),
        line((x0 + relief, y1), (x1 - relief, y1)),
        line((x0, y0 + relief), (x0, y1 - relief)),
    ];
    // Glue along the bottom flap's slanted edges, a strip a fifth of the flap's depth wide.
    let strip = bottom * 0.2;
    let glue = vec![
        polygon(&[(x0 + relief, y1), (x0 + bottom_in, y1 + bottom), (x0 + bottom_in + strip, y1 + bottom), (x0 + relief + strip, y1)]),
        polygon(&[(x1 - relief, y1), (x1 - relief - strip, y1), (x1 - bottom_in - strip, y1 + bottom), (x1 - bottom_in, y1 + bottom)]),
    ];
    Ok(Net { cut: PathData::single(o.closed()), folds: PathData::new(folds), glue: PathData::new(glue), face: Some([x0, y0, x1, y1]) })
}

pub(super) const PLACE_CARD: Kind = Kind {
    id: "placeCard",
    name: "Place Card",
    family: FAMILY,
    params: &[
        Param { key: "widthMm", label: "Width", unit: Unit::Mm, default: 90.0, min: 20.0, max: 300.0, help: "across" },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 50.0, min: 15.0, max: 200.0, help: "each face, fold to edge" },
        Param { key: "cornerMm", label: "Corner Radius", unit: Unit::Mm, default: 3.0, min: 0.0, max: 30.0, help: "0: square corners" },
    ],
    sized: false,
    fit: Some(("widthMm", "heightMm")),
    make: place_card,
};

/// A tent card: two faces with a fold between them.
fn place_card(v: &Values) -> Result<Net, String> {
    let (w, h) = (v.get("widthMm"), v.get("heightMm"));
    Ok(Net {
        cut: PathData::single(rounded_rect(0.0, 0.0, w, 2.0 * h, v.get("cornerMm"))),
        folds: PathData::single(line((0.0, h), (w, h))),
        glue: PathData::default(),
        // The face toward the guest: the lower half, upright when the card stands.
        face: Some([0.0, h, w, 2.0 * h]),
    })
}

pub(super) const GIFT_TAG: Kind = Kind {
    id: "giftTag",
    name: "Gift Tag",
    family: FAMILY,
    params: &[
        Param { key: "widthMm", label: "Width", unit: Unit::Mm, default: 50.0, min: 15.0, max: 300.0, help: "across" },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 90.0, min: 20.0, max: 400.0, help: "tip to bottom" },
        Param { key: "holeMm", label: "Hole", unit: Unit::Mm, default: 5.0, min: 0.0, max: 20.0, help: "the string's hole, across (0: none)" },
    ],
    sized: false,
    fit: Some(("widthMm", "heightMm")),
    make: gift_tag,
};

/// A tag: a rectangle with its top corners cut off at 45° and a hole for the string.
fn gift_tag(v: &Values) -> Result<Net, String> {
    let (w, h, hole) = (v.get("widthMm"), v.get("heightMm"), v.get("holeMm"));
    let c = (w * 0.3).min(h * 0.25);
    let mut cut = vec![polygon(&[(c, 0.0), (w - c, 0.0), (w, c), (w, h), (0.0, h), (0.0, c)])];
    if hole > 0.0 {
        if hole > w - 2.0 * c.max(3.0) || hole / 2.0 + 3.0 > h / 2.0 {
            return Err(format!("a {hole} mm hole doesn't fit the tag's top"));
        }
        cut.push(circle((w / 2.0, c.max(hole / 2.0 + 3.0)), hole / 2.0));
    }
    Ok(Net { cut: PathData::new(cut), folds: PathData::default(), glue: PathData::default(), face: Some([0.0, 0.0, w, h]) })
}

pub(super) const EARRING_CARD: Kind = Kind {
    id: "earringCard",
    name: "Earring Card",
    family: FAMILY,
    params: &[
        Param { key: "widthMm", label: "Width", unit: Unit::Mm, default: 50.0, min: 25.0, max: 200.0, help: "across" },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 70.0, min: 30.0, max: 300.0, help: "up and down" },
        Param {
            key: "spacingMm",
            label: "Hole Spacing",
            unit: Unit::Mm,
            default: 22.0,
            min: 5.0,
            max: 150.0,
            help: "between the two earrings' holes",
        },
        Param { key: "holeMm", label: "Hole", unit: Unit::Mm, default: 1.5, min: 0.5, max: 6.0, help: "each earring's hole, across" },
        Param {
            key: "hangSlot",
            label: "Hang Slot",
            unit: Unit::Toggle,
            default: 1.0,
            min: 0.0,
            max: 1.0,
            help: "a slot at the top to hang it on a display hook",
        },
    ],
    sized: false,
    fit: Some(("widthMm", "heightMm")),
    make: earring_card,
};

/// An earring card: a card with rounded corners, a pair of holes for the earrings a third of the
/// way down (under the hang slot, when there is one), and a slot to hang it by.
fn earring_card(v: &Values) -> Result<Net, String> {
    let (w, h, gap, hole) = (v.get("widthMm"), v.get("heightMm"), v.get("spacingMm"), v.get("holeMm"));
    if gap + hole + 6.0 > w {
        return Err(format!("holes {gap} mm apart don't fit a card {w} mm wide"));
    }
    let hang = v.get("hangSlot") > 0.5;
    let mut cut = vec![rounded_rect(0.0, 0.0, w, h, 3.0)];
    let (slot_w, slot_h, slot_y) = ((w * 0.4).min(15.0), 4.0, 6.0);
    if hang {
        cut.push(slot((w / 2.0, slot_y), slot_w, slot_h));
    }
    let y = if hang { (h / 3.0).max(slot_y + slot_h + hole + 4.0) } else { h / 3.0 };
    for x in [w / 2.0 - gap / 2.0, w / 2.0 + gap / 2.0] {
        cut.push(circle((x, y), hole / 2.0));
    }
    Ok(Net { cut: PathData::new(cut), folds: PathData::default(), glue: PathData::default(), face: Some([0.0, 0.0, w, h]) })
}

pub(super) const GIFT_CARD_HOLDER: Kind = Kind {
    id: "giftCardHolder",
    name: "Gift Card Holder",
    family: FAMILY,
    params: &[
        Param { key: "widthMm", label: "Width", unit: Unit::Mm, default: 100.0, min: 60.0, max: 300.0, help: "folded, across" },
        Param { key: "heightMm", label: "Height", unit: Unit::Mm, default: 70.0, min: 40.0, max: 300.0, help: "folded, up and down" },
        Param {
            key: "cardWidthMm",
            label: "Card Width",
            unit: Unit::Mm,
            default: 85.6,
            min: 20.0,
            max: 280.0,
            help: "the card it holds (a bank card: 85.6)",
        },
        Param {
            key: "cardHeightMm",
            label: "Card Height",
            unit: Unit::Mm,
            default: 54.0,
            min: 15.0,
            max: 280.0,
            help: "the card it holds (a bank card: 54)",
        },
    ],
    sized: false,
    fit: Some(("widthMm", "heightMm")),
    make: gift_card_holder,
};

/// A fold-over card wallet: the cover on the left, the inside on the right, a fold between; the
/// inside holds the card by four slits across its corners, the card's corners slipped under them.
fn gift_card_holder(v: &Values) -> Result<Net, String> {
    let (w, h, cw, ch) = (v.get("widthMm"), v.get("heightMm"), v.get("cardWidthMm"), v.get("cardHeightMm"));
    if cw + 6.0 > w || ch + 6.0 > h {
        return Err(format!("a {cw} × {ch} mm card needs a holder at least {:.0} × {:.0} mm", cw + 6.0, ch + 6.0));
    }
    let mut cut = vec![rounded_rect(0.0, 0.0, 2.0 * w, h, 3.0)];
    // The card centred on the inside; a slit across each corner, 4 mm in along each edge.
    let (cx0, cy0) = (w + (w - cw) / 2.0, (h - ch) / 2.0);
    let (cx1, cy1) = (cx0 + cw, cy0 + ch);
    let d = 10.0_f64.min(cw / 4.0).min(ch / 4.0);
    for (x, y, sx, sy) in [(cx0, cy0, 1.0, 1.0), (cx1, cy0, -1.0, 1.0), (cx1, cy1, -1.0, -1.0), (cx0, cy1, 1.0, -1.0)] {
        // From a point on one edge to one on the other, past the corner by 1.5 mm each way.
        cut.push(line((x + sx * d, y - sy * 1.5), (x - sx * 1.5, y + sy * d)));
    }
    Ok(Net { cut: PathData::new(cut), folds: PathData::single(line((w, 0.0), (w, h))), glue: PathData::default(), face: Some([0.0, 0.0, w, h]) })
}

#[cfg(test)]
mod tests {
    use super::super::tests::{simple, values};
    use super::*;

    /// The envelope's face is the card plus the ease; the card fits between the folds.
    #[test]
    fn the_envelope_holds_its_card() {
        for k in [&ENVELOPE, &POINTED_ENVELOPE] {
            let v = values(k, &[("cardWidthMm", 120.0), ("cardHeightMm", 80.0), ("easeMm", 2.0)], false).unwrap();
            let net = k.make(&v).unwrap();
            // The top and bottom folds are the face's width less the reliefs: 124 − 2 × relief.
            let top = &net.folds.subpaths[0];
            let len = (top.anchors[1].p.x - top.anchors[0].p.x).abs();
            assert!(len > 120.0 && len < 124.0, "{}: {len}", k.id);
            let side = &net.folds.subpaths[1];
            let len = (side.anchors[1].p.y - side.anchors[0].p.y).abs();
            assert!(len > 80.0 && len < 84.0, "{}: {len}", k.id);
            assert!(simple(&net.cut), "{}", k.id);
            assert_eq!(net.glue.subpaths.len(), 2);
        }
    }

    /// The tag's hole, the earring card's holes and slot, the holder's slits are all cut.
    #[test]
    fn holes_and_slits_are_cut() {
        let tag = GIFT_TAG.make(&values(&GIFT_TAG, &[], false).unwrap()).unwrap();
        assert_eq!(tag.cut.subpaths.len(), 2);
        let none = GIFT_TAG.make(&values(&GIFT_TAG, &[("holeMm", 0.0)], false).unwrap()).unwrap();
        assert_eq!(none.cut.subpaths.len(), 1);
        let ear = EARRING_CARD.make(&values(&EARRING_CARD, &[], false).unwrap()).unwrap();
        assert_eq!(ear.cut.subpaths.len(), 1 + 1 + 2, "outline, slot, two holes");
        let bare = EARRING_CARD.make(&values(&EARRING_CARD, &[("hangSlot", 0.0)], false).unwrap()).unwrap();
        assert_eq!(bare.cut.subpaths.len(), 3);
        let holder = GIFT_CARD_HOLDER.make(&values(&GIFT_CARD_HOLDER, &[], false).unwrap()).unwrap();
        assert_eq!((holder.cut.subpaths.len(), holder.folds.subpaths.len()), (5, 1));
        assert!(GIFT_CARD_HOLDER.make(&values(&GIFT_CARD_HOLDER, &[("widthMm", 80.0)], false).unwrap()).is_err());
        assert!(EARRING_CARD.make(&values(&EARRING_CARD, &[("spacingMm", 48.0)], false).unwrap()).is_err());
        for s in tag.cut.subpaths.iter().chain(&ear.cut.subpaths) {
            assert!(simple(&PathData::single(s.clone())));
        }
    }

    /// A round hole is round: its points lie at the radius.
    #[test]
    fn circles_are_round() {
        use vectorcraft_geom::ParamCurve;
        let c = circle((10.0, 20.0), 3.0);
        assert!(c.closed);
        for i in 0..c.segment_count() {
            for k in 0..=8 {
                let p = c.segment(i).eval(f64::from(k) / 8.0);
                assert!(((p.x - 10.0).hypot(p.y - 20.0) - 3.0).abs() < 0.005, "{p:?}");
            }
        }
    }
}
