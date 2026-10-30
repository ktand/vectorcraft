//! C11.5: stencil bridges: the art's holes held to it by bridges, the stencil cut in place of the
//! art, one undo step.

use serde_json::json;
use vectorcraft_engine::Session;

/// A square ring in a millimetre document, 60 mm across with a 30 mm hole: the session and the id
/// of the object the job cuts it from.
fn ring() -> (Session, u64) {
    let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="200mm" height="100mm" viewBox="0 0 200 100">
        <path d="M 10 10 H 70 V 70 H 10 Z M 25 25 V 55 H 55 V 25 Z" fill="black" fill-rule="evenodd"/></svg>"#;
    let mut s = Session::new();
    s.execute("document.open", &json!({"name": "ring.svg", "dataBase64": vectorcraft_format::base64_encode(svg.as_bytes())})).unwrap();
    let id = closed_paths(&mut s)[0];
    (s, id)
}

fn closed_paths(s: &mut Session) -> Vec<u64> {
    let job = s.execute("cutting.job", &json!({})).unwrap();
    job["job"]["paths"].as_array().unwrap().iter().filter(|p| p["closed"] == json!(true)).map(|p| p["source"]["node"].as_u64().unwrap()).collect()
}

#[test]
fn a_ring_becomes_a_bridged_stencil() {
    let (mut s, id) = ring();
    assert_eq!(closed_paths(&mut s), [id, id], "the hole and the outside");
    s.execute("select.none", &json!({})).unwrap();
    assert!(s.execute("cutting.bridges", &json!({})).unwrap_err().to_string().contains("select the art"));
    assert!(s.execute("cutting.bridges", &json!({"ids": [id], "widthMm": 50})).is_err());
    assert!(s.execute("cutting.bridges", &json!({"ids": [id], "direction": "diagonal"})).is_err());
    let r = s.execute("cutting.bridges", &json!({"ids": [id], "widthMm": 3})).unwrap();
    assert_eq!((r["bridges"].as_u64(), r["unbridged"].as_u64()), (Some(2), Some(0)), "{r}");
    let stencil = r["id"].as_u64().unwrap();
    assert_eq!(closed_paths(&mut s), [stencil, stencil], "two halves, the art not cut");
    let st = s.active().unwrap();
    let art = st.doc.node(vectorcraft_doc::NodeId(id)).unwrap();
    assert!(!art.visible, "the art is hidden under it");
    assert_eq!(st.doc.node(vectorcraft_doc::NodeId(stencil)).unwrap().name.as_deref(), Some("Stencil"));
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(closed_paths(&mut s), [id, id], "one undo step");
    assert!(s.active().unwrap().doc.node(vectorcraft_doc::NodeId(id)).unwrap().visible);
}
