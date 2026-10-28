//! C3.5: `cutting.job` makes the document's cut job, in cutting order, without changing it.

use serde_json::{Value, json};
use vectorcraft_engine::Session;

const PT: f64 = 72.0 / 25.4;

/// A square of `side` mm at (`x`, `y`) mm.
fn square(s: &mut Session, x: f64, y: f64, side: f64) -> u64 {
    s.execute("shape.rectangle", &json!({"x": x * PT, "y": y * PT, "width": side * PT, "height": side * PT})).unwrap()["id"].as_u64().unwrap()
}

fn job(s: &mut Session) -> Value {
    s.execute("cutting.job", &json!({})).unwrap()
}

fn sources(j: &Value) -> Vec<u64> {
    j["job"]["paths"].as_array().unwrap().iter().map(|p| p["source"]["node"].as_u64().unwrap()).collect()
}

fn new_doc() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 200.0 * PT, "height": 200.0 * PT})).unwrap();
    s
}

#[test]
fn the_job_cuts_inner_shapes_first_and_scores_before_cutting() {
    let mut s = new_doc();
    let outer = square(&mut s, 10.0, 10.0, 50.0);
    let inner = square(&mut s, 25.0, 25.0, 20.0);
    let fold = square(&mut s, 100.0, 10.0, 30.0);
    s.execute("cutting.op.set", &json!({"ids": [fold], "op": "score"})).unwrap();
    let before = s.active().unwrap().doc.clone();
    let j = job(&mut s);
    assert!(std::sync::Arc::ptr_eq(&before, &s.active().unwrap().doc), "the document isn't changed");
    assert_eq!(sources(&j), [fold, inner, outer]);
    let ops = &j["job"]["operations"];
    assert_eq!((ops[0]["kind"].clone(), ops[1]["kind"].clone()), (json!("score"), json!("cut")));
    assert_eq!(j["job"]["paths"][0]["op"], 0);
    // In millimetres on the mat: the inner square's corners at 25 and 45 mm.
    let pts = j["job"]["paths"][1]["points"].as_array().unwrap();
    let xs: Vec<f64> = pts.iter().map(|p| p[0].as_f64().unwrap()).collect();
    assert!(xs.iter().all(|x| (x - 25.0).abs() < 1e-6 || (x - 45.0).abs() < 1e-6), "{xs:?}");
    assert_eq!(j["stats"]["pathCount"], 3);
    assert!((j["stats"]["pathMm"].as_f64().unwrap() - (200.0 + 80.0 + 120.0)).abs() < 1e-6);
}

#[test]
fn art_off_the_mat_is_an_error_unless_clipped_to_it() {
    let mut s = new_doc();
    square(&mut s, 10.0, 10.0, 10.0);
    s.execute("cutting.settings.set", &json!({"offsetMm": [290, 0]})).unwrap();
    let e = s.execute("cutting.job", &json!({})).unwrap_err().to_string();
    assert!(e.contains("cut area"), "{e}");
    s.execute("cutting.settings.set", &json!({"clipToMat": true})).unwrap();
    let j = job(&mut s);
    assert!(j["job"]["warnings"].as_array().unwrap().iter().any(|w| w["message"].as_str().unwrap().contains("edge")));
    assert!(j["stats"]["pathCount"].as_u64().unwrap() >= 1);
    assert!(s.execute("cutting.settings.set", &json!({"clipToMat": "yes"})).is_err());
}

/// C3.7: the blade's run through the job, from the cut area's origin.
#[test]
fn the_simulation_follows_the_job() {
    let mut s = new_doc();
    let a = square(&mut s, 10.0, 10.0, 10.0);
    square(&mut s, 50.0, 10.0, 10.0);
    let start = s.execute("cutting.simulate", &json!({"t": 0})).unwrap();
    assert_eq!((start["position"].clone(), start["down"].clone()), (json!([0.0, 0.0]), json!(false)));
    let total = start["totalMm"].as_f64().unwrap();
    let stats = job(&mut s)["stats"].clone();
    assert!((total - stats["bladeMm"].as_f64().unwrap() - stats["travelMm"].as_f64().unwrap()).abs() < 1e-6);
    let starts = start["pathStartsMm"].as_array().unwrap().clone();
    assert_eq!(starts.len(), 2);
    // 1 mm after the blade comes down on the first square: down, on it, cutting.
    let on = s.execute("cutting.simulate", &json!({"mm": starts[0].as_f64().unwrap() + 1.0})).unwrap();
    assert_eq!((on["down"].clone(), on["path"].clone(), on["op"].clone()), (json!(true), json!(0), json!("cut")));
    assert_eq!(job(&mut s)["job"]["paths"][0]["source"]["node"], json!(a));
    let end = s.execute("cutting.simulate", &json!({})).unwrap();
    assert_eq!(end["doneMm"], end["totalMm"]);
    assert!(end["totalSeconds"].as_f64().unwrap() > 0.0);
    assert!(s.execute("cutting.simulate", &json!({"t": "half"})).is_err());
}

#[test]
fn an_empty_document_makes_an_empty_job() {
    let mut s = new_doc();
    let j = job(&mut s);
    assert_eq!(j["job"]["paths"], json!([]));
    assert_eq!(j["stats"]["pathCount"], 0);
}

/// C3.8: the preview as an SVG opens at the mat's size; as JSON it is cutting.job.
#[test]
fn the_preview_exports_as_svg_and_json() {
    let mut s = new_doc();
    square(&mut s, 10.0, 10.0, 10.0);
    let svg = s.execute("cutting.preview.export", &json!({})).unwrap();
    assert_eq!(svg["name"], json!("Untitled-1-cut.svg"));
    let text = svg["text"].as_str().unwrap().to_string();
    assert!(text.contains(r#"data-order="1" data-op="cut""#));
    s.execute("document.open", &json!({"name": "preview.svg", "dataBase64": vectorcraft_format::base64_encode(text.as_bytes())})).unwrap();
    let ab = s.active().unwrap().doc.artboards[0].rect;
    assert!((ab.width() / PT - 304.8).abs() < 1e-3 && (ab.height() / PT - 304.8).abs() < 1e-3, "{ab:?}");
    s.execute("document.activate", &json!({"index": 0})).unwrap();
    let as_json = s.execute("cutting.preview.export", &json!({"format": "json"})).unwrap();
    let v: Value = serde_json::from_str(as_json["text"].as_str().unwrap()).unwrap();
    assert_eq!(v, job(&mut s));
    assert!(s.execute("cutting.preview.export", &json!({"format": "pdf"})).is_err());
}

/// C3.10: two overlapping squares set to Cut Outline are cut as one outline.
#[test]
fn cut_outline_unites_overlapping_shapes() {
    let mut s = new_doc();
    let a = square(&mut s, 10.0, 10.0, 40.0);
    let b = square(&mut s, 30.0, 30.0, 40.0);
    assert_eq!(job(&mut s)["stats"]["pathCount"], 2);
    s.execute("cutting.op.set", &json!({"ids": [a, b], "op": "outline"})).unwrap();
    assert_eq!(s.execute("cutting.op.get", &json!({"ids": [a]})).unwrap()["objects"][0]["op"], json!("outline"));
    let j = job(&mut s);
    assert_eq!(j["stats"]["pathCount"], 1);
    // Round both: 40 + 20 + 20 + 40 on each of two sides, less nothing inside.
    assert!((j["stats"]["pathMm"].as_f64().unwrap() - 240.0).abs() < 0.1, "{}", j["stats"]["pathMm"]);
    assert_eq!(j["job"]["operations"][0]["kind"], json!("cut"));
}

/// C5.2: blade depth, acceleration, corner overcut and track enhancing through the settings.
#[test]
fn machine_settings_reach_the_job() {
    let mut s = new_doc();
    square(&mut s, 10.0, 10.0, 20.0);
    s.execute("cutting.settings.set", &json!({"trackEnhancing": true, "operations": {"cut": {"depth": 3, "accel": 2, "cornerOvercutMm": 0.5}}}))
        .unwrap();
    let j = job(&mut s);
    assert_eq!(j["job"]["trackEnhancing"], json!(true));
    let op = &j["job"]["operations"][0];
    assert_eq!((op["depth"].clone(), op["accel"].clone(), op["cornerOvercutMm"].clone()), (json!(3), json!(2), json!(0.5)));
    // The square's three corners after its start each run past and back: 4 + 3 × 2 points.
    assert_eq!(j["job"]["paths"][0]["points"].as_array().unwrap().len(), 10);
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {"depth": null}}})).unwrap();
    assert_eq!(job(&mut s)["job"]["operations"][0]["depth"], Value::Null);
    for bad in [json!({"operations": {"cut": {"depth": 12}}}), json!({"operations": {"cut": {"accel": 0}}}), json!({"trackEnhancing": "yes"})] {
        assert!(s.execute("cutting.settings.set", &bad).is_err(), "{bad}");
    }
}
