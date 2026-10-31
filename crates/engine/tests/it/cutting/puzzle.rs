//! C13.5: puzzles: a jigsaw's cut lines over the selection or the artboard, one undo step.

use serde_json::json;
use vectorcraft_engine::Session;

#[test]
fn a_puzzle_is_cut_over_the_art() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 500})).unwrap();
    let pic = s.execute("shape.rectangle", &json!({"x": 50, "y": 50, "width": 500, "height": 400})).unwrap()["id"].clone();
    s.execute("cutting.op.set", &json!({"ids": [pic], "op": "none"})).unwrap();
    for bad in [json!({"rows": 1}), json!({"cols": 2.5}), json!({"tabPct": 60}), json!({"variety": 2})] {
        assert!(s.execute("cutting.puzzle", &bad).is_err(), "{bad}");
    }
    let r = s.execute("cutting.puzzle", &json!({"rows": 4, "cols": 5, "seed": 7})).unwrap();
    assert_eq!(r["pieces"], json!(20));
    let job = s.execute("cutting.job", &json!({})).unwrap();
    let paths = job["job"]["paths"].as_array().unwrap();
    assert_eq!(paths.len(), 1 + 3 + 4, "the border and 7 lines: {}", paths.len());
    assert!(paths.iter().all(|p| p["source"]["node"] == r["id"]));
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(s.execute("cutting.job", &json!({})).unwrap()["job"]["paths"].as_array().unwrap().is_empty(), "one undo step");
    // Nothing selected: over the artboard.
    s.execute("select.none", &json!({})).unwrap();
    let r = s.execute("cutting.puzzle", &json!({"rows": 2, "cols": 2})).unwrap();
    assert_eq!(r["pieces"], json!(4));
}
