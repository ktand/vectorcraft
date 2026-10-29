//! C8.5: nesting through the engine: the pieces brought onto the mat close together, one undo
//! step; the selection only when there is one; refusals.

use serde_json::json;
use vectorcraft_engine::Session;

#[test]
fn the_pieces_are_nested_on_the_mat_as_one_step() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 1200, "height": 400})).unwrap();
    for k in 0..4 {
        s.execute("shape.rectangle", &json!({"x": 40 + 280 * k, "y": 40, "width": 160, "height": 120})).unwrap();
    }
    s.execute("select.none", &json!({})).unwrap();
    assert!(s.execute("cutting.job", &json!({})).is_err(), "off the mat to begin with");
    let r = s.execute("cutting.nest", &json!({})).unwrap();
    assert_eq!((r["moved"].as_array().unwrap().len(), r["unplaced"].as_array().unwrap().len()), (4, 0), "{r}");
    let job = s.execute("cutting.job", &json!({})).unwrap();
    assert_eq!(job["job"]["paths"].as_array().unwrap().len(), 4, "every piece on the cut area");
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(s.execute("cutting.job", &json!({})).is_err(), "one undo step back");
    s.execute("edit.redo", &json!({})).unwrap();

    // The selection only: the others stay where they are.
    s.execute("edit.undo", &json!({})).unwrap();
    let first = r["moved"][0].clone();
    s.execute("select.set", &json!({"ids": [first]})).unwrap();
    let r = s.execute("cutting.nest", &json!({"gapMm": 5, "rotate": false})).unwrap();
    assert_eq!(r["moved"].as_array().unwrap().len(), 1, "{r}");

    // C8.6: onto new sheets: what doesn't fit the artboard goes onto Sheet 2…, one undo step.
    s.execute("file.new", &json!({"width": 612, "height": 792})).unwrap();
    for k in 0..9 {
        s.execute("shape.rectangle", &json!({"x": 700 + 260 * k, "y": 0, "width": 250, "height": 250})).unwrap();
    }
    s.execute("select.none", &json!({})).unwrap();
    let r = s.execute("cutting.nest", &json!({"sheets": true})).unwrap();
    assert!(r["created"].as_u64().unwrap() >= 1 && r["unplaced"].as_array().unwrap().is_empty(), "{r}");
    let boards = s.active().unwrap().doc.artboards.len();
    assert_eq!(boards as u64, 1 + r["created"].as_u64().unwrap());
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.active().unwrap().doc.artboards.len(), 1, "the sheets go with the undo");

    assert!(s.execute("cutting.nest", &json!({"gapMm": 80})).is_err());
    assert!(s.execute("cutting.nest", &json!({"rotate": "yes"})).is_err());
    s.execute("file.new", &json!({"width": 200, "height": 200})).unwrap();
    assert!(s.execute("cutting.nest", &json!({})).unwrap_err().to_string().contains("nothing to nest"));
}
