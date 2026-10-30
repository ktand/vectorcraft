//! C11.4: an object set to its stroke's edge is cut round its stroke, not down its centre line;
//! one undo step, inherited from a layer, kept through SVG.

use serde_json::{Value, json};
use vectorcraft_engine::Session;

/// A 100 mm line with a 4 mm stroke, in a millimetre document: the session and the line's id.
fn line() -> (Session, u64) {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200mm" height="100mm" viewBox="0 0 200 100">
        <path d="M 20 50 L 120 50" stroke="black" stroke-width="4" fill="none"/></svg>"#;
    let mut s = Session::new();
    s.execute("document.open", &json!({"name": "line.svg", "dataBase64": vectorcraft_format::base64_encode(svg.as_bytes())})).unwrap();
    let mut id = 0;
    s.active().unwrap().doc.walk(|n| {
        if !n.is_layer() {
            id = n.id.0;
        }
    });
    (s, id)
}

fn paths(s: &mut Session) -> Vec<Value> {
    s.execute("cutting.job", &json!({})).unwrap()["job"]["paths"].as_array().unwrap().clone()
}

/// The height of a path's points, in mm.
fn height(p: &Value) -> f64 {
    let ys: Vec<f64> = p["points"].as_array().unwrap().iter().map(|q| q[1].as_f64().unwrap()).collect();
    ys.iter().copied().fold(f64::MIN, f64::max) - ys.iter().copied().fold(f64::MAX, f64::min)
}

#[test]
fn a_stroke_edge_is_cut_round_the_stroke() {
    let (mut s, id) = line();
    let before = paths(&mut s);
    assert_eq!((before.len(), before[0]["closed"].as_bool()), (1, Some(false)), "down the centre line");
    assert!(s.execute("cutting.edge.set", &json!({"ids": [id]})).unwrap_err().to_string().contains("edge is required"));
    assert!(s.execute("cutting.edge.set", &json!({"ids": [id], "edge": "outline"})).is_err());
    let r = s.execute("cutting.edge.set", &json!({"ids": [id], "edge": "stroke"})).unwrap();
    assert_eq!(r["ids"], json!([id]));
    let after = paths(&mut s);
    assert_eq!((after.len(), after[0]["closed"].as_bool()), (1, Some(true)), "{after:?}");
    assert!((height(&after[0]) - 4.0).abs() < 0.05, "the stroke's width across: {}", height(&after[0]));
    let got = s.execute("cutting.op.get", &json!({"ids": [id]})).unwrap();
    assert_eq!((got["objects"][0]["edge"].as_str(), got["objects"][0]["edgeFrom"].as_u64()), (Some("stroke"), Some(id)));
    assert_eq!(s.execute("cutting.edge.set", &json!({"ids": [id], "edge": "stroke"})).unwrap()["ids"], json!([]), "unchanged");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(paths(&mut s)[0]["closed"].as_bool(), Some(false), "one undo step");
}

#[test]
fn a_layers_edge_is_inherited_and_kept_through_svg() {
    let (mut s, id) = line();
    let layer = s.active().unwrap().doc.layers[0].id.0;
    s.execute("cutting.edge.set", &json!({"ids": [layer], "edge": "stroke"})).unwrap();
    let got = s.execute("cutting.op.get", &json!({"ids": [id]})).unwrap();
    assert_eq!((got["objects"][0]["edge"].as_str(), got["objects"][0]["edgeFrom"].as_u64()), (Some("stroke"), Some(layer)));
    assert_eq!(paths(&mut s)[0]["closed"].as_bool(), Some(true));
    s.execute("cutting.edge.set", &json!({"ids": [layer], "edge": "inherit"})).unwrap();
    s.execute("cutting.edge.set", &json!({"ids": [id], "edge": "stroke"})).unwrap();
    let svg = s.execute("document.serialize", &json!({"format": "svg"})).unwrap()["text"].as_str().unwrap().to_string();
    assert!(svg.contains(r#"cut:edge="stroke""#), "{svg}");
    s.execute("document.open", &json!({"name": "again.svg", "dataBase64": vectorcraft_format::base64_encode(svg.as_bytes())})).unwrap();
    assert_eq!(paths(&mut s)[0]["closed"].as_bool(), Some(true), "still round the stroke");
}
