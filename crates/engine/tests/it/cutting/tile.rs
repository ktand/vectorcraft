//! C14.7: tiles for a design larger than the mat, and a mirrored cut.

use serde_json::json;
use vectorcraft_engine::Session;

const PT: f64 = 72.0 / 25.4;

#[test]
fn a_large_design_is_tiled_onto_mats() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600.0 * PT, "height": 400.0 * PT})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 580.0 * PT, "height": 380.0 * PT})).unwrap();
    assert!(s.execute("cutting.job", &json!({})).is_err(), "off the mat");
    assert!(s.execute("cutting.tile", &json!({"overlapMm": 90})).is_err());
    s.execute("select.none", &json!({})).unwrap();
    let r = s.execute("cutting.tile", &json!({})).unwrap();
    let boards = r["artboards"].as_array().unwrap().clone();
    assert!(boards.len() >= 4 && r["rows"].as_u64().unwrap() >= 2 && r["cols"].as_u64().unwrap() >= 2, "{r}");
    let settings = s.execute("cutting.settings.get", &json!({})).unwrap();
    assert_eq!(settings["settings"]["artboard"], boards[0]);
    assert_eq!(settings["settings"]["clipToMat"], json!(true));
    // One undo step: the settings as they were, then the tiles again.
    s.execute("edit.undo", &json!({})).unwrap();
    let back = s.execute("cutting.settings.get", &json!({})).unwrap();
    assert_eq!((back["settings"]["artboard"].clone(), back["settings"]["clipToMat"].clone()), (json!(0), json!(false)));
    s.execute("edit.redo", &json!({})).unwrap();
    // Each tile makes a job of its own.
    for b in &boards {
        s.execute("cutting.settings.set", &json!({"artboard": b})).unwrap();
        assert!(!s.execute("cutting.job", &json!({})).unwrap()["job"]["paths"].as_array().unwrap().is_empty(), "tile {b}");
    }
}

/// A square near the artboard's left edge is cut near its right on the mat; the art isn't moved;
/// Print & Cut is never mirrored.
#[test]
fn a_mirrored_cut_flips_the_job_not_the_art() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 200.0 * PT, "height": 200.0 * PT})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 40.0 * PT, "y": 40.0 * PT, "width": 20.0 * PT, "height": 20.0 * PT})).unwrap();
    let xs = |s: &mut Session| {
        let j = s.execute("cutting.job", &json!({})).unwrap();
        let pts = j["job"]["paths"][0]["points"].as_array().unwrap().clone();
        pts.iter().map(|p| p[0].as_f64().unwrap()).fold(f64::INFINITY, f64::min)
    };
    let plain = xs(&mut s);
    s.execute("cutting.settings.set", &json!({"mirror": true})).unwrap();
    let mirrored = xs(&mut s);
    assert!(mirrored > plain + 50.0, "{plain} → {mirrored}");
    s.execute("cutting.settings.set", &json!({"mirror": false})).unwrap();
    assert!((xs(&mut s) - plain).abs() < 1e-9, "the art where it was");
    s.execute("cutting.settings.set", &json!({"mirror": true})).unwrap();
    s.execute("cutting.settings.set", &json!({"printAndCut": true})).unwrap();
    let j = s.execute("cutting.job", &json!({})).unwrap();
    assert!(j["job"]["warnings"].to_string().contains("isn't cut mirrored"), "{}", j["job"]["warnings"]);
}
