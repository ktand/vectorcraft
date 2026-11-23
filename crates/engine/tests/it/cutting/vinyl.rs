//! C18.30: a design made into layered vinyl through the engine: the art replaced by a layer for
//! each colour, the base at the bottom, the job cut by layer; one undo step; bad params refused.

use serde_json::{Value, json};
use vectorcraft_engine::Session;

/// A face of our own: a skin-coloured oval with a black border, a white eye with a black border
/// and a black pupil, and an open black line for a mouth.
const FACE: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" width="160mm" height="180mm" viewBox="0 0 160 180">
    <rect width="160" height="180" fill="none"/>
    <ellipse cx="80" cy="90" rx="60" ry="70" fill="#ffaf80" stroke="#000" stroke-width="5"/>
    <ellipse cx="60" cy="75" rx="12" ry="15" fill="#fff" stroke="#000" stroke-width="3"/>
    <ellipse cx="60" cy="75" rx="4" ry="5" fill="#000"/>
    <path d="M55 120 L105 120" fill="none" stroke="#000" stroke-width="4" stroke-linecap="round"/>
</svg>"##;

fn open() -> Session {
    let mut s = Session::new();
    s.execute("document.open", &json!({"name": "face.svg", "dataBase64": vectorcraft_format::base64_encode(FACE.as_bytes())})).unwrap();
    s.execute("select.all", &json!({})).unwrap();
    s
}

fn layer_names(s: &Session) -> Vec<String> {
    s.active().unwrap().doc.layers.iter().filter_map(|l| l.name.clone()).collect()
}

#[test]
fn a_face_becomes_a_layer_for_each_colour() {
    let mut s = open();
    let before = layer_names(&s);
    let r = s.execute("cutting.vinylLayers", &json!({})).unwrap();
    let names: Vec<&str> = r["layers"].as_array().unwrap().iter().map(|l| l["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["#000000", "#ffaf80", "#ffffff"], "{r}");
    assert_eq!(r["layers"][0]["base"], json!(true));
    let after = layer_names(&s);
    assert!(after.iter().any(|n| n == "1 #000000 (base)") && after.iter().any(|n| n == "3 #ffffff"), "{after:?}");
    // The art is replaced: each new layer holds one path, and the old layer is empty.
    let doc = &s.active().unwrap().doc;
    let art: usize = doc.layers.iter().map(|l| l.children().map_or(0, Vec::len)).sum();
    assert_eq!(art, 3, "one shape for each colour");
    let settings = s.execute("cutting.settings.get", &json!({})).unwrap();
    assert_eq!(settings["settings"]["grouping"], json!("layer"));
    // The job: the base one closed outline (no holes), the skin with holes for the eye and mouth.
    let job = s.execute("cutting.job", &json!({})).unwrap();
    let groups: Vec<Value> = job["job"]["paths"].as_array().unwrap().clone();
    assert!(groups.iter().all(|p| p["closed"].as_bool() == Some(true)), "every cut closed");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(layer_names(&s), before, "one undo step");
}

#[test]
fn the_base_and_overlap_are_checked() {
    let mut s = open();
    assert!(s.execute("cutting.vinylLayers", &json!({"overlapMm": 4})).unwrap_err().to_string().contains("overlapMm"));
    assert!(s.execute("cutting.vinylLayers", &json!({"base": 3})).is_err());
    let e = s.execute("cutting.vinylLayers", &json!({"base": "gold"})).unwrap_err().to_string();
    assert!(e.contains("no colour gold"), "{e}");
    let r = s.execute("cutting.vinylLayers", &json!({"base": "none", "overlapMm": 0.5})).unwrap();
    assert!(r["layers"].as_array().unwrap().iter().all(|l| l["base"] == json!(false)), "{r}");
    s.execute("select.none", &json!({})).ok();
    s.execute("edit.undo", &json!({})).unwrap();
    s.execute("select.none", &json!({})).unwrap();
    assert!(s.execute("cutting.vinylLayers", &json!({})).unwrap_err().to_string().contains("select the design"));
}
