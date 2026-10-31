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

/// C12.1: the marks' arms, line and each edge's inset are set, the job tells the machine them,
/// and insets below 10 mm are refused.
#[test]
fn the_marks_are_drawn_and_searched_as_set() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 612, "height": 792})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 200, "y": 300, "width": 100, "height": 100})).unwrap();
    s.execute("cutting.settings.set", &json!({"printAndCut": true})).unwrap();
    let r = s.execute("cutting.job", &json!({})).unwrap()["job"]["registration"].clone();
    assert_eq!(
        (r["originMm"].clone(), r["lengthMm"].clone(), r["lineMm"].clone()),
        (json!([15.88, 15.88]), json!(20.0), json!(0.5)),
        "the usual marks"
    );
    s.execute("cutting.settings.set", &json!({"markLengthMm": 12, "markLineMm": 0.8, "markInsetsMm": [12, 20, 30, 40]})).unwrap();
    let r = s.execute("cutting.job", &json!({})).unwrap()["job"]["registration"].clone();
    assert_eq!((r["originMm"].clone(), r["lengthMm"].clone(), r["lineMm"].clone()), (json!([12.0, 20.0]), json!(12.0), json!(0.8)));
    let size = r["sizeMm"].as_array().unwrap().iter().map(|v| v.as_f64().unwrap()).collect::<Vec<_>>();
    assert!((size[0] - (215.9 - 42.0)).abs() < 0.01 && (size[1] - (279.4 - 60.0)).abs() < 0.01, "{size:?}");
    // The marks layer is drawn with them: the corners' line 0.8 mm.
    let layer = s.execute("cutting.marks.add", &json!({})).unwrap()["layer"].as_u64().unwrap();
    let doc = &s.doc().unwrap().doc;
    let widths: Vec<f64> = doc
        .node(vectorcraft_doc::NodeId(layer))
        .unwrap()
        .children()
        .unwrap()
        .iter()
        .filter_map(|n| n.appearance.stroke().map(|k| k.width))
        .filter(|w| *w > 0.0)
        .collect();
    assert!(widths.len() == 2 && widths.iter().all(|w| (w * 25.4 / 72.0 - 0.8).abs() < 1e-9), "{widths:?}");
    for bad in [
        json!({"markInsetMm": 7}),
        json!({"markInsetsMm": [12, 9, 12, 12]}),
        json!({"markLengthMm": 25}),
        json!({"markLineMm": 0.2}),
        json!({"markInsetsMm": [1, 2]}),
    ] {
        assert!(s.execute("cutting.settings.set", &bad).is_err(), "{bad}");
    }
    s.execute("cutting.settings.set", &json!({"markInsetsMm": null})).unwrap();
    assert_eq!(s.execute("cutting.job", &json!({})).unwrap()["job"]["registration"]["originMm"], json!([15.88, 15.88]), "back to the one inset");
}
