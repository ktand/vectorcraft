//! C18.23: material calibration: a row of test cuts a step apart in one setting, the best cell's
//! value put into the document's cut and into the user's own material.

use serde_json::{Value, json};
use vectorcraft_engine::Session;

fn cut(s: &mut Session) -> Value {
    s.execute("cutting.settings.get", &json!({})).unwrap()["settings"]["operations"]["cut"].clone()
}

fn session() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800, "height": 800})).unwrap();
    s
}

#[test]
fn a_row_is_centred_on_the_cut() {
    let mut s = session();
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {"force": vectorcraft_cutter_silhouette::force_from_step(10)}}})).unwrap();
    let r = s.execute("cutting.calibrate.cells", &json!({"setting": "force"})).unwrap();
    let values: Vec<f64> = r["cells"].as_array().unwrap().iter().map(|c| c["value"].as_f64().unwrap()).collect();
    assert_eq!(values, [7.0, 8.0, 9.0, 10.0, 11.0, 12.0, 13.0]);
    assert_eq!((r["current"].clone(), r["cells"][0]["n"].clone()), (json!(10), json!(1)));
    // At the range's end the row shifts, never past it.
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {"force": vectorcraft_cutter_silhouette::force_from_step(1)}}})).unwrap();
    let r = s.execute("cutting.calibrate.cells", &json!({"setting": "force"})).unwrap();
    assert_eq!(r["cells"][0]["value"], json!(1.0));
    let r = s.execute("cutting.calibrate.cells", &json!({"setting": "overcut"})).unwrap();
    assert_eq!(r["cells"][2]["label"], json!("0.5 mm"));
    // Blade depth is the AutoBlade's.
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {"tool": "pen"}}})).unwrap();
    assert!(s.execute("cutting.calibrate.cells", &json!({"setting": "depth"})).is_err());
    for bad in [json!({}), json!({"setting": "colour"}), json!({"setting": "force", "corner": "middle"})] {
        assert!(s.execute("cutting.calibrate.job", &bad).is_err(), "{bad}");
    }
}

#[test]
fn the_job_cuts_each_cell_with_its_own_setting() {
    let mut s = session();
    let r = s.execute("cutting.calibrate.job", &json!({"setting": "speed", "corner": "bottomRight"})).unwrap();
    let job = &r["job"];
    let n = r["cells"].as_array().unwrap().len();
    assert_eq!(job["operations"].as_array().unwrap().len(), n);
    assert_eq!(job["paths"].as_array().unwrap().len(), 2 * n + 1, "a square and a triangle each, and the mark");
    let speeds: Vec<f64> = job["operations"].as_array().unwrap().iter().map(|o| o["speedMmS"].as_f64().unwrap()).collect();
    assert!(speeds.windows(2).all(|w| w[1] > w[0]), "{speeds:?}");
    assert!(s.execute("cutting.calibrate", &json!({"setting": "speed"})).is_err(), "no cutter here");
}

#[test]
fn the_best_cell_goes_into_the_cut_and_the_material() {
    let mut s = session();
    let r = s.execute("cutting.calibrate.choose", &json!({"setting": "overcut", "cell": 4})).unwrap();
    assert_eq!((r["value"].clone(), r["savedTo"].clone()), (json!(0.75), Value::Null), "a built-in or no material: not saved");
    assert_eq!(cut(&mut s)["overcutMm"], json!(0.75));
    for bad in [json!({"setting": "overcut"}), json!({"setting": "overcut", "cell": 0}), json!({"setting": "overcut", "cell": 9})] {
        assert!(s.execute("cutting.calibrate.choose", &bad).is_err(), "{bad}");
    }
    // The user's own material keeps it.
    s.execute("cutting.material.save", &json!({"name": "Calibrated Card"})).unwrap();
    let r = s.execute("cutting.calibrate.choose", &json!({"setting": "corners", "cell": 2})).unwrap();
    assert_eq!(r["savedTo"], json!("Calibrated Card"));
    let mats = s.execute("cutting.materials", &json!({})).unwrap();
    let m = mats["materials"].as_array().unwrap().iter().find(|m| m["name"] == "Calibrated Card").unwrap().clone();
    assert_eq!(m["cut"]["cornerOvercutMm"], json!(0.2));
    s.execute("cutting.material.delete", &json!({"name": "Calibrated Card"})).unwrap();
}
