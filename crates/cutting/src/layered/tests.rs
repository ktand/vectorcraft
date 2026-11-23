use vectorcraft_color::Color;
use vectorcraft_doc::{Appearance, Node};
use vectorcraft_geom::{Point, Rect, shapes};

use super::*;

fn skin() -> Paint {
    Paint::solid(Color::from_hex("#ffaf80").unwrap())
}

fn black() -> Paint {
    Paint::solid(Color::BLACK)
}

/// A face of our own, in the way of a cartoon head: a skin-coloured oval with a 5 pt black
/// border, two white eyes with 3 pt black borders and black pupils (not touching the border), a
/// mouth that is an open black 4 pt line, and an empty frame round it all (no fill, no stroke).
fn face() -> (Document, Vec<NodeId>) {
    let mut d = Document::new(200.0, 200.0);
    let layer = d.layers[0].id;
    let add = |d: &mut Document, path: PathData, a: Appearance| {
        let n = Node::path(d.alloc_id(), path, a);
        d.insert(Some(layer), usize::MAX, n).unwrap()
    };
    let white = Paint::solid(Color::WHITE);
    let ids = vec![
        add(&mut d, shapes::rectangle(Rect::new(0.0, 0.0, 160.0, 180.0)), Appearance::basic(Paint::None, Paint::None, 0.0)),
        add(&mut d, shapes::ellipse(Rect::new(20.0, 20.0, 140.0, 160.0)), Appearance::basic(skin(), black(), 5.0)),
        add(&mut d, shapes::ellipse(Rect::new(45.0, 60.0, 70.0, 90.0)), Appearance::basic(white.clone(), black(), 3.0)),
        add(&mut d, shapes::ellipse(Rect::new(90.0, 60.0, 115.0, 90.0)), Appearance::basic(white, black(), 3.0)),
        add(&mut d, shapes::ellipse(Rect::new(52.0, 70.0, 60.0, 80.0)), Appearance::basic(black(), Paint::None, 0.0)),
        add(&mut d, shapes::ellipse(Rect::new(97.0, 70.0, 105.0, 80.0)), Appearance::basic(black(), Paint::None, 0.0)),
        add(&mut d, shapes::line(Point::new(55.0, 120.0), Point::new(105.0, 120.0)), Appearance::basic(Paint::None, black(), 4.0)),
    ];
    (d, ids)
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 0.05
}

fn area_of(p: &PathData) -> f64 {
    area(p, FillRule::NonZero)
}

/// What `a` covers that `b` doesn't.
fn outside(a: &PathData, b: &PathData) -> f64 {
    area_of(&try_boolean(a, FillRule::NonZero, b, FillRule::NonZero, BoolOp::Difference, DEFAULT_PRECISION).unwrap())
}

fn names(l: &Layered) -> Vec<(&str, bool)> {
    l.layers.iter().map(|l| (l.name.as_str(), l.base)).collect()
}

/// The face as layered vinyl: a black base that is the whole silhouette in one piece, the skin
/// with holes where the eyes and the mouth line are, the eye whites with holes for the pupils.
#[test]
fn a_face_is_a_base_and_the_colours_that_show() {
    let (d, ids) = face();
    let l = layered(&d, &ids, &Options::default()).unwrap();
    assert!(l.warnings.is_empty(), "{:?}", l.warnings);
    assert_eq!(names(&l), [("#000000", true), ("#ffaf80", false), ("#ffffff", false)], "the border's colour is the base");
    let [base, skin, white] = [&l.layers[0], &l.layers[1], &l.layers[2]];
    assert_eq!(base.region.subpaths.len(), 1, "one piece, no holes");
    let b = base.region.bounds().unwrap();
    assert!(near(b.x0, 17.5) && near(b.y0, 17.5) && near(b.x1, 142.5) && near(b.y1, 162.5), "out to the border's outer edge: {b:?}");
    let s = skin.region.bounds().unwrap();
    assert!(near(s.x0, 22.5) && near(s.y1, 157.5), "in to the border's inner edge: {s:?}");
    assert_eq!(skin.region.subpaths.len(), 4, "the outside and three holes: two eyes and the mouth");
    assert_eq!(white.region.subpaths.len(), 4, "two eyes, each with a hole for its pupil");
    let w = white.region.bounds().unwrap();
    assert!(near(w.x0, 46.5) && near(w.x1, 113.5), "inside the eyes' borders: {w:?}");
    assert!(area_of(&intersect(&skin.region, &white.region).unwrap()) < 0.01, "the colours don't overlap");
    for layer in &l.layers {
        assert!(outside(&layer.region, &base.region) < 0.01, "{} inside the silhouette", layer.name);
        assert_eq!(layer.paint, if layer.base { black() } else { layer.paint.clone() });
    }
}

#[test]
fn the_base_can_be_named_or_left_out() {
    let (d, ids) = face();
    let named = layered(&d, &ids, &Options { base: Base::Colour("#FFAF80".into()), ..Options::default() }).unwrap();
    assert_eq!(names(&named), [("#ffaf80", true), ("#000000", false), ("#ffffff", false)]);
    let none = layered(&d, &ids, &Options { base: Base::None, ..Options::default() }).unwrap();
    assert_eq!(names(&none), [("#ffaf80", false), ("#000000", false), ("#ffffff", false)], "in the order first painted");
    let black = &none.layers[1].region;
    assert!(black.subpaths.len() > 4, "the border, the eyes' borders, the pupils and the mouth: {}", black.subpaths.len());
    let e = layered(&d, &ids, &Options { base: Base::Colour("gold".into()), ..Options::default() }).unwrap_err();
    assert!(e.contains("no colour gold") && e.contains("#ffaf80"), "{e}");
}

/// With an overlap, each colour runs that far under the ones above it, and no further than the
/// design.
#[test]
fn an_overlap_runs_under_the_colours_above() {
    let (d, ids) = face();
    let flat = layered(&d, &ids, &Options { base: Base::None, overlap_mm: 0.0 }).unwrap();
    let grown = layered(&d, &ids, &Options { base: Base::None, overlap_mm: 1.0 }).unwrap();
    let (before, after) = (&flat.layers[0].region, &grown.layers[0].region);
    assert!(area_of(after) > area_of(before) + 10.0, "the skin runs under the black and the white");
    assert!(outside(before, after) < 0.01);
    let silhouette = layered(&d, &ids, &Options::default()).unwrap().layers[0].region.clone();
    assert!(outside(after, &silhouette) < 0.01, "not past the design");
    assert_eq!(flat.layers[2].region, grown.layers[2].region, "the top colour is whole as it is");
    assert!(layered(&d, &ids, &Options { overlap_mm: 5.0, ..Options::default() }).unwrap_err().contains("overlap"));
}

#[test]
fn nothing_painted_nothing_to_cut() {
    let (d, ids) = face();
    let e = layered(&d, &ids[..1], &Options::default()).unwrap_err();
    assert!(e.contains("paints nothing"), "{e}");
}

/// A design of one colour is one layer: its silhouette.
#[test]
fn one_colour_is_one_layer() {
    let mut d = Document::new(200.0, 200.0);
    let layer = d.layers[0].id;
    let a = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(10.0, 10.0, 50.0, 50.0)), Appearance::basic(black(), black(), 2.0));
    let b = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(40.0, 40.0, 90.0, 90.0)), Appearance::basic(black(), Paint::None, 0.0));
    let ids = vec![d.insert(Some(layer), usize::MAX, a).unwrap(), d.insert(Some(layer), usize::MAX, b).unwrap()];
    let l = layered(&d, &ids, &Options::default()).unwrap();
    assert_eq!(names(&l), [("#000000", true)]);
    assert_eq!(l.layers[0].region.subpaths.len(), 1, "the two squares joined");
}

#[test]
fn the_colours_a_base_can_be() {
    let (d, ids) = face();
    assert_eq!(colours(&d, &ids), ["#ffaf80", "#000000", "#ffffff"]);
    assert!(colours(&d, &ids[..1]).is_empty(), "the empty frame paints nothing");
}
