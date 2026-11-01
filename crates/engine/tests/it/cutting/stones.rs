//! C13.4: rhinestone templates: holes round the art, cut in its place, one undo step.

use serde_json::json;
use vectorcraft_engine::Session;

#[test]
fn a_square_becomes_a_rhinestone_template() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
    let sq = s.execute("shape.rectangle", &json!({"x": 50, "y": 50, "width": 113.385827, "height": 113.385827})).unwrap()["id"].clone();
    assert!(s.execute("cutting.rhinestones", &json!({"holeMm": 20})).is_err());
    assert!(s.execute("cutting.rhinestones", &json!({"fill": 1})).is_err());
    let r = s.execute("cutting.rhinestones", &json!({})).unwrap();
    assert_eq!(r["stones"], json!(40), "160 mm round, 3 mm holes 1 mm apart: {r}");
    let job = s.execute("cutting.job", &json!({})).unwrap();
    let paths = job["job"]["paths"].as_array().unwrap();
    assert_eq!(paths.len(), 40, "the holes, not the square");
    assert!(paths.iter().all(|p| p["source"]["node"] == r["id"] && p["closed"] == json!(true)));
    s.execute("edit.undo", &json!({})).unwrap();
    let job = s.execute("cutting.job", &json!({})).unwrap();
    assert_eq!(job["job"]["paths"][0]["source"]["node"], sq, "one undo step: the square back");
    let filled = s.execute("cutting.rhinestones", &json!({"ids": [sq], "fill": true})).unwrap();
    assert!(filled["stones"].as_u64().unwrap() > 100, "{filled}");
    // C14.5: rows following the outline inward; a fill that isn't one.
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(s.execute("cutting.rhinestones", &json!({"ids": [sq], "fill": "spots"})).is_err());
    let radial = s.execute("cutting.rhinestones", &json!({"ids": [sq], "fill": "radial"})).unwrap();
    assert!(radial["stones"].as_u64().unwrap() > 100, "{radial}");
}
