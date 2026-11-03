//! C16.1: pop-up cards: the selected design made into a card that stands up when opened, one
//! undo step: the card, the face's cut and the folds, each cut; the design no longer cut itself;
//! a pop-up alone selected made again in place.

use serde_json::json;
use vectorcraft_engine::Session;

#[test]
fn a_design_is_made_into_a_pop_up_card() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 600})).unwrap();
    assert!(s.execute("cutting.popup", &json!({})).is_err(), "nothing selected");
    let a = s.execute("shape.rectangle", &json!({"x": 100, "y": 200, "width": 120, "height": 100})).unwrap()["id"].clone();
    let b = s.execute("shape.rectangle", &json!({"x": 260, "y": 230, "width": 80, "height": 70})).unwrap()["id"].clone();
    for bad in [json!({"ids": [a], "baseWidthPct": 0}), json!({"ids": [a], "strutWidthMm": 99}), json!({"ids": [a], "dashPitchMm": "x"})] {
        assert!(s.execute("cutting.popup", &bad).is_err(), "{bad}");
    }
    let r = s.execute("cutting.popup", &json!({"ids": [a, b]})).unwrap();
    assert_eq!((r["struts"].clone(), r["replaced"].clone()), (json!(2), json!(false)));
    let job = s.execute("cutting.job", &json!({})).unwrap();
    let paths = job["job"]["paths"].as_array().unwrap();
    assert!(!paths.is_empty());
    assert!(paths.iter().all(|p| p["source"]["node"] != a && p["source"]["node"] != b), "the design isn't cut itself");
    // Undone in one step.
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(s.execute("cutting.job", &json!({})).unwrap()["job"]["paths"].as_array().unwrap().iter().any(|p| p["source"]["node"] == a));
    s.execute("edit.redo", &json!({})).unwrap();
    // Made again in place with wider struts.
    s.execute("select.set", &json!({"ids": [r["id"]]})).unwrap();
    let again = s.execute("cutting.popup", &json!({"strutWidthMm": 5})).unwrap();
    assert_eq!((again["id"].clone(), again["replaced"].clone(), again["struts"].clone()), (r["id"].clone(), json!(true), json!(2)));
}
