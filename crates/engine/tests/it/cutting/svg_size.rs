//! C2.1: an SVG opened with `document.open` keeps the physical size the cutting workspace cuts it
//! at. A file with absolute units opens at its printed size, whatever its `viewBox` (Inkscape's
//! 96 px/in user units included); a unitless one opens at 1 px = 1 pt, 72 per inch, so a drawing
//! made at 96 px/in opens a third larger than its author meant (ARCHITECTURE.md › R1).

use serde_json::json;
use vectorcraft_doc::{Document, Unit};
use vectorcraft_engine::Session;
use vectorcraft_geom::Rect;

/// Millimetres per point.
const MM: f64 = 25.4 / 72.0;

fn open(svg: &str) -> Document {
    let mut s = Session::new();
    s.execute("document.open", &json!({"name": "art.svg", "dataBase64": vectorcraft_format::base64_encode(svg.as_bytes())})).unwrap();
    (*s.doc().unwrap().doc).clone()
}

/// `r`, in points, as millimetres.
fn mm(r: Rect) -> [f64; 4] {
    [r.x0 * MM, r.y0 * MM, r.x1 * MM, r.y1 * MM]
}

fn assert_mm(got: Rect, want: [f64; 4]) {
    let got = mm(got);
    assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() < 0.01), "{got:?} mm, want {want:?} mm");
}

/// The bounds of the document's only object.
fn art_bounds(d: &Document) -> Rect {
    let mut bounds = vec![];
    d.walk(|n| {
        if !n.is_layer() {
            bounds.extend(n.geometric_bounds());
        }
    });
    assert_eq!(bounds.len(), 1, "one object");
    bounds[0]
}

#[test]
fn millimetre_user_units_open_at_their_size() {
    let d = open(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="210mm" height="297mm" viewBox="0 0 210 297">
        <rect x="10" y="20" width="100" height="50"/></svg>"#,
    );
    assert_mm(d.artboards[0].rect, [0.0, 0.0, 210.0, 297.0]);
    assert_mm(art_bounds(&d), [10.0, 20.0, 110.0, 70.0]);
    assert_eq!(d.units, Unit::Millimeters);
}

/// Inkscape's documents give a size in millimetres and user units of 96 per inch.
#[test]
fn a_millimetre_size_over_pixel_user_units_opens_at_its_size() {
    let px = 96.0 / 25.4;
    let d = open(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="100mm" height="50mm" viewBox="0 0 {w} {h}">
        <rect x="{x}" y="{x}" width="{rw}" height="{rh}"/></svg>"#,
        w = 100.0 * px,
        h = 50.0 * px,
        x = 5.0 * px,
        rw = 40.0 * px,
        rh = 20.0 * px,
    ));
    assert_mm(d.artboards[0].rect, [0.0, 0.0, 100.0, 50.0]);
    assert_mm(art_bounds(&d), [5.0, 5.0, 45.0, 25.0]);
}

#[test]
fn inches_and_centimetres_open_at_their_size() {
    let d = open(r#"<svg xmlns="http://www.w3.org/2000/svg" width="4in" height="10cm"><rect width="1in" height="2cm"/></svg>"#);
    assert_mm(d.artboards[0].rect, [0.0, 0.0, 101.6, 100.0]);
    assert_mm(art_bounds(&d), [0.0, 0.0, 25.4, 20.0]);
}

/// R1: no unit means points. A 100 mm square drawn at 96 px/in (378 px) opens 133 mm wide; the
/// cutting workspace has to show the size and offer a scale, not assume this is what was meant.
#[test]
fn unitless_lengths_are_points() {
    let d = open(r#"<svg xmlns="http://www.w3.org/2000/svg" width="378" height="378"><rect width="378" height="378"/></svg>"#);
    let side = 378.0 * MM;
    assert_mm(d.artboards[0].rect, [0.0, 0.0, side, side]);
    assert_mm(art_bounds(&d), [0.0, 0.0, side, side]);
    assert!((side - 133.35).abs() < 0.01, "{side}");
    assert_eq!(d.units, Unit::Pixels);
}
