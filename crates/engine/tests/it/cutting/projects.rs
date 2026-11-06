//! C18.12: a guided project's settings, set in one undo step: an iron-on design mirrored with a
//! weed box, a vinyl decal as drawn with weed lines too; with more than one fill colour, cut by
//! colour with knockout and align marks.

use serde_json::{Value, json};
use vectorcraft_engine::Session;

fn settings(s: &mut Session) -> Value {
    s.execute("cutting.settings.get", &json!({})).unwrap()["settings"].clone()
}

fn one_colour() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 100, "height": 100})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#cc2222"})).unwrap();
    s
}

#[test]
fn an_iron_on_design_is_cut_mirrored_with_a_weed_box() {
    let mut s = one_colour();
    let r = s.execute("cutting.project.setup", &json!({"project": "ironOn"})).unwrap();
    assert_eq!((r["colours"].clone(), r["material"].clone()), (json!(1), json!("Heat Transfer Vinyl")));
    let got = settings(&mut s);
    assert_eq!(
        (got["mirror"].clone(), got["weedBox"].clone(), got["weedLines"].clone(), got["knockout"].clone(), got["grouping"].clone()),
        (json!(true), json!(true), json!(false), json!(false), json!("none")),
        "{got}"
    );
    // One undo step takes it all back.
    s.execute("edit.undo", &json!({})).unwrap();
    let back = settings(&mut s);
    assert_eq!((back["mirror"].clone(), back["weedBox"].clone(), back["material"].clone()), (json!(false), json!(false), Value::Null), "{back}");
}

#[test]
fn several_colours_are_cut_one_after_the_other_knocked_out() {
    let mut s = one_colour();
    s.execute("shape.ellipse", &json!({"x": 40, "y": 40, "width": 40, "height": 40})).unwrap();
    s.execute("paint.setFill", &json!({"color": "#2244cc"})).unwrap();
    let r = s.execute("cutting.project.setup", &json!({"project": "vinylDecal"})).unwrap();
    assert_eq!((r["colours"].clone(), r["material"].clone()), (json!(2), json!("Vinyl")));
    let got = settings(&mut s);
    assert_eq!(
        (got["mirror"].clone(), got["weedLines"].clone(), got["grouping"].clone(), got["knockout"].clone(), got["alignMarks"].clone()),
        (json!(false), json!(true), json!("fillColor"), json!(true), json!(true)),
        "{got}"
    );
    // Cut by layer already: kept.
    s.execute("cutting.settings.set", &json!({"grouping": "layer"})).unwrap();
    s.execute("cutting.project.setup", &json!({"project": "ironOn"})).unwrap();
    assert_eq!(settings(&mut s)["grouping"], json!("layer"));
}

#[test]
fn a_chosen_material_of_the_right_kind_is_kept() {
    let mut s = one_colour();
    s.execute("cutting.material.apply", &json!({"name": "Cardstock"})).unwrap();
    // Cardstock isn't cut from the back: an iron-on starts from heat transfer vinyl…
    let r = s.execute("cutting.project.setup", &json!({"project": "ironOn"})).unwrap();
    assert_eq!(r["material"], json!("Heat Transfer Vinyl"));
    // …and a decal from vinyl, as Heat Transfer Vinyl is.
    let r = s.execute("cutting.project.setup", &json!({"project": "vinylDecal"})).unwrap();
    assert_eq!(r["material"], json!("Vinyl"));
    // A material not cut from the back stays for a decal.
    s.execute("cutting.material.apply", &json!({"name": "Cardstock"})).unwrap();
    let r = s.execute("cutting.project.setup", &json!({"project": "vinylDecal"})).unwrap();
    assert_eq!(r["material"], json!("Cardstock"));
    for bad in [json!({}), json!({"project": "shirt"}), json!({"project": 3})] {
        assert!(s.execute("cutting.project.setup", &bad).is_err(), "{bad}");
    }
}
