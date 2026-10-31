//! C13.3: sketch fill: lines across the art for a pen, added above it, one undo step.

use serde_json::json;
use vectorcraft_engine::Session;

#[test]
fn a_square_gets_a_sketch_fill() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
    let sq = s.execute("shape.rectangle", &json!({"x": 50, "y": 50, "width": 142, "height": 142})).unwrap()["id"].clone();
    assert!(s.execute("cutting.sketchFill", &json!({"spacingMm": 0.1})).is_err());
    assert!(s.execute("cutting.sketchFill", &json!({"cross": "yes"})).is_err());
    let r = s.execute("cutting.sketchFill", &json!({"spacingMm": 5, "angleDeg": 0, "cross": true})).unwrap();
    let lines = r["lines"].as_u64().unwrap();
    assert!((18..=22).contains(&lines), "{r}");
    let job = s.execute("cutting.job", &json!({})).unwrap();
    let paths = job["job"]["paths"].as_array().unwrap();
    assert_eq!(paths.len() as u64, lines + 1, "the lines and the square");
    assert!(paths.iter().any(|p| p["source"]["node"] == sq));
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.execute("cutting.job", &json!({})).unwrap()["job"]["paths"].as_array().unwrap().len(), 1);
    s.execute("select.none", &json!({})).unwrap();
    assert!(s.execute("cutting.sketchFill", &json!({})).unwrap_err().to_string().contains("select the art"));
}
