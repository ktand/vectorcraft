//! C6.1: Print & Cut: the registration marks as a layer, through the commands, and the job
//! measured from them.

use serde_json::{Value, json};
use vectorcraft_engine::Session;

fn layers(s: &mut Session) -> usize {
    s.doc().unwrap().doc.layers.len()
}

#[test]
fn marks_are_added_once_and_the_job_cuts_from_them() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 612, "height": 792})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 200, "y": 300, "width": 100, "height": 100})).unwrap();
    s.execute("cutting.settings.set", &json!({"printAndCut": true, "markInsetMm": 12})).unwrap();
    let job = s.execute("cutting.job", &json!({})).unwrap();
    assert_eq!(job["job"]["registration"]["originMm"], json!([12.0, 12.0]));
    assert!(job["job"]["warnings"].to_string().contains("aren't in the document"));

    let before = layers(&mut s);
    let first = s.execute("cutting.marks.add", &json!({})).unwrap()["layer"].clone();
    assert_eq!(layers(&mut s), before + 1);
    // Again: the marks are replaced, not added twice.
    let again = s.execute("cutting.marks.add", &json!({})).unwrap()["layer"].clone();
    assert_ne!(first, again);
    assert_eq!(layers(&mut s), before + 1);
    let job = s.execute("cutting.job", &json!({})).unwrap();
    assert_eq!(job["job"]["paths"].as_array().unwrap().len(), 1, "the marks aren't cut");
    assert_eq!(job["job"]["warnings"], json!([]));

    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(layers(&mut s), before + 1, "undo puts the first marks back");
    assert_eq!(s.execute("cutting.marks.remove", &json!({})).unwrap()["removed"], json!(true));
    assert_eq!(layers(&mut s), before);
    assert_eq!(s.execute("cutting.marks.remove", &json!({})).unwrap()["removed"], json!(false));

    assert!(s.execute("cutting.settings.set", &json!({"markInsetMm": 2})).is_err(), "out of range");
    s.execute("cutting.settings.set", &json!({"orientation": "landscape"})).unwrap();
    assert!(s.execute("cutting.job", &json!({})).unwrap_err().to_string().contains("portrait"));
    let _: Value = s.execute("cutting.settings.set", &json!({"printAndCut": false})).unwrap();
}
