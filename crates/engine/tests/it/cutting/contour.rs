//! C11.1: a cut line round the selected art, on the Cut Contour layer, the art set to No Cut, one
//! undo step.

use serde_json::json;
use vectorcraft_engine::Session;

#[test]
fn a_contour_goes_round_the_art() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 400, "height": 400})).unwrap();
    assert!(s.execute("cutting.contour", &json!({})).unwrap_err().to_string().contains("select the art"));
    let rect = s.execute("shape.rectangle", &json!({"x": 100, "y": 100, "width": 100, "height": 60})).unwrap()["id"].clone();
    assert!(s.execute("cutting.contour", &json!({"offsetMm": 80})).is_err(), "out of range");
    assert!(s.execute("cutting.contour", &json!({"holes": "yes"})).is_err());
    let before = s.execute("cutting.job", &json!({})).unwrap()["job"]["paths"].as_array().unwrap().len();
    assert_eq!(before, 1, "the rectangle");
    let r = s.execute("cutting.contour", &json!({"offsetMm": 3})).unwrap();
    assert_eq!(r["pieces"], json!(1), "{r}");
    // The art is No Cut now: the job cuts the contour alone, a little larger than the art.
    let job = s.execute("cutting.job", &json!({})).unwrap();
    let paths = job["job"]["paths"].as_array().unwrap().clone();
    assert_eq!(paths.len(), 1, "{job}");
    assert_eq!(paths[0]["source"]["node"], r["id"], "the contour is what is cut");
    let op = s.execute("cutting.op.get", &json!({"ids": [rect]})).unwrap();
    assert_eq!(op["objects"][0]["op"], json!("none"), "{op}");
    // Again: the same layer.
    s.execute("select.none", &json!({})).unwrap();
    let again = s.execute("cutting.contour", &json!({"ids": [rect], "noCutArt": false})).unwrap();
    assert_eq!(again["layer"], r["layer"]);
    // One undo step each: back to the rectangle alone, cut.
    s.execute("edit.undo", &json!({})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    let job = s.execute("cutting.job", &json!({})).unwrap();
    assert_eq!(job["job"]["paths"].as_array().unwrap().len(), 1);
    assert_eq!(job["job"]["paths"][0]["source"]["node"], rect);
}
