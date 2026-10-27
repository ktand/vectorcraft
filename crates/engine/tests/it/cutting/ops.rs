//! C2.3: the `cutting.*` settings and per-object operation commands: one undo step each, saved
//! natively, kept through SVG, copied with the object, inherited from groups and layers.

use serde_json::{Value, json};
use vectorcraft_engine::Session;

fn new_doc() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
    s
}

fn rect(s: &mut Session, x: f64) -> u64 {
    s.execute("shape.rectangle", &json!({"x": x, "y": 10, "width": 30, "height": 30})).unwrap()["id"].as_u64().unwrap()
}

fn settings(s: &mut Session) -> Value {
    s.execute("cutting.settings.get", &json!({})).unwrap()["settings"].clone()
}

/// `(op, source)` of each object, in the order given.
fn ops(s: &mut Session, ids: &[u64]) -> Vec<(String, String)> {
    let r = s.execute("cutting.op.get", &json!({"ids": ids})).unwrap();
    r["objects"].as_array().unwrap().iter().map(|o| (o["op"].as_str().unwrap().to_string(), o["source"].as_str().unwrap().to_string())).collect()
}

fn op(s: &mut Session, id: u64) -> String {
    ops(s, &[id]).remove(0).0
}

/// The document saved natively and opened again (as the active document).
fn reopen_native(s: &mut Session) {
    let data = s.execute("document.serialize", &json!({})).unwrap()["dataBase64"].as_str().unwrap().to_string();
    s.execute("document.open", &json!({"name": "again.vectorcraft", "dataBase64": data})).unwrap();
}

#[test]
fn settings_have_defaults_and_list_the_mats() {
    let mut s = new_doc();
    let r = s.execute("cutting.settings.get", &json!({})).unwrap();
    assert_eq!(r["settings"]["mat"], json!("silhouette.12x12"));
    assert_eq!(r["settings"]["defaultOp"], json!("cut"));
    assert_eq!(r["mat"]["id"], json!("silhouette.12x12"));
    assert_eq!(r["mat"]["sizeMm"], json!([304.8, 304.8]));
    assert!(r["mats"].as_array().unwrap().len() >= 2);
}

#[test]
fn setting_some_settings_is_one_undo_step() {
    let mut s = new_doc();
    let before = settings(&mut s);
    let r = s
        .execute(
            "cutting.settings.set",
            &json!({"mat": "custom", "customMatMm": [200, 300], "orientation": "landscape", "operations": {"cut": {"force": 0.5, "passes": 2}}}),
        )
        .unwrap();
    assert_eq!(r["mat"]["sizeMm"], json!([200.0, 300.0]));
    let after = settings(&mut s);
    assert_eq!((after["mat"].clone(), after["orientation"].clone()), (json!("custom"), json!("landscape")));
    assert_eq!(after["operations"]["cut"]["force"], json!(0.5));
    assert_eq!(after["operations"]["cut"]["passes"], json!(2));
    assert_eq!(after["operations"]["cut"]["speedMmS"], before["operations"]["cut"]["speedMmS"], "the rest stays");
    assert_eq!(after["operations"]["score"], before["operations"]["score"]);
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(settings(&mut s), before);
    s.execute("edit.redo", &json!({})).unwrap();
    assert_eq!(settings(&mut s), after);
}

/// Setting what is already set adds no undo step: Undo then undoes the change before it.
#[test]
fn setting_the_same_settings_changes_nothing() {
    let mut s = new_doc();
    let before = settings(&mut s);
    s.execute("cutting.settings.set", &json!({"toleranceMm": 0.1})).unwrap();
    s.execute("cutting.settings.set", &json!({"toleranceMm": 0.1})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(settings(&mut s), before);
}

#[test]
fn bad_settings_are_errors_and_change_nothing() {
    let mut s = new_doc();
    let before = settings(&mut s);
    for p in [
        json!({"toleranceMm": 5}),
        json!({"toleranceMm": "fine"}),
        json!({"offsetMm": [0, 9000]}),
        json!({"orientation": "sideways"}),
        json!({"defaultOp": "perforate"}),
        json!({"operations": {"cut": {"passes": 2.5}}}),
        json!({"operations": {"cut": {"force": 2}}}),
        json!({"operations": {"perforate": {}}}),
        json!({"operations": {"cut": {"blade": 1}}}),
        json!({"operations": {"cut": 3}}),
        json!({"operations": 3}),
        json!({"version": 2}),
        json!({"colour": "red"}),
        json!({"mat": "brand.new"}),
        json!({"artboard": 1}),
        json!(5),
    ] {
        assert!(s.execute("cutting.settings.set", &p).is_err(), "{p}");
    }
    assert_eq!(settings(&mut s), before);
}

#[test]
fn settings_survive_saving_natively() {
    let mut s = new_doc();
    s.execute("cutting.settings.set", &json!({"mat": "silhouette.12x24", "offsetMm": [5, 7.5], "defaultOp": "score"})).unwrap();
    let saved = settings(&mut s);
    reopen_native(&mut s);
    assert_eq!(settings(&mut s), saved);
}

#[test]
fn an_objects_op_is_set_inherited_and_undone() {
    let mut s = new_doc();
    let (a, b) = (rect(&mut s, 10.0), rect(&mut s, 50.0));
    assert_eq!(ops(&mut s, &[a, b]), [("cut".into(), "default".into()), ("cut".into(), "default".into())]);
    s.execute("select.set", &json!({"ids": [a, b]})).unwrap();
    let g = s.execute("object.group", &json!({})).unwrap()["id"].as_u64().unwrap();
    let r = s.execute("cutting.op.set", &json!({"ids": [g], "op": "none"})).unwrap();
    assert_eq!(r["ids"], json!([g]));
    assert_eq!(ops(&mut s, &[a, b]), [("none".into(), "inherited".into()), ("none".into(), "inherited".into())]);
    assert_eq!(ops(&mut s, &[g]), [("none".into(), "own".into())]);
    // As with the selection, an object given with the group that holds it is the group.
    assert_eq!(ops(&mut s, &[a, g]), [("none".into(), "own".into())]);
    assert_eq!(s.execute("cutting.op.get", &json!({"ids": [a]})).unwrap()["objects"][0]["from"], json!(g));
    s.execute("cutting.op.set", &json!({"ids": [b], "op": "score"})).unwrap();
    assert_eq!(ops(&mut s, &[a, b]), [("none".into(), "inherited".into()), ("score".into(), "own".into())]);
    // Setting it again changes nothing; inherit removes the object's own.
    assert_eq!(s.execute("cutting.op.set", &json!({"ids": [b], "op": "score"})).unwrap()["ids"], json!([]));
    s.execute("cutting.op.set", &json!({"ids": [b], "op": "inherit"})).unwrap();
    assert_eq!(op(&mut s, b), "none");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(op(&mut s, b), "score");
    s.execute("edit.undo", &json!({})).unwrap();
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(ops(&mut s, &[a, b]), [("cut".into(), "default".into()), ("cut".into(), "default".into())]);
}

#[test]
fn the_op_follows_the_documents_default() {
    let mut s = new_doc();
    let a = rect(&mut s, 10.0);
    s.execute("cutting.settings.set", &json!({"defaultOp": "none"})).unwrap();
    assert_eq!(ops(&mut s, &[a]), [("none".into(), "default".into())]);
}

#[test]
fn the_op_is_set_on_the_selection_and_layers() {
    let mut s = new_doc();
    let a = rect(&mut s, 10.0);
    s.execute("select.set", &json!({"ids": [a]})).unwrap();
    s.execute("cutting.op.set", &json!({"op": "score"})).unwrap();
    assert_eq!(op(&mut s, a), "score");
    let r = s.execute("cutting.op.get", &json!({})).unwrap();
    assert_eq!(r["objects"][0]["id"], json!(a));
    let layer = s.active().unwrap().doc.layers[0].id.0;
    s.execute("cutting.op.set", &json!({"ids": [layer], "op": "none"})).unwrap();
    s.execute("cutting.op.set", &json!({"ids": [a], "op": "inherit"})).unwrap();
    assert_eq!(ops(&mut s, &[a]), [("none".into(), "inherited".into())]);
}

#[test]
fn bad_op_params_are_errors() {
    let mut s = new_doc();
    let a = rect(&mut s, 10.0);
    s.execute("select.none", &json!({})).unwrap();
    for p in [json!({"ids": [a]}), json!({"ids": [a], "op": "perforate"}), json!({"ids": [999_999], "op": "cut"}), json!({"op": "cut"})] {
        assert!(s.execute("cutting.op.set", &p).is_err(), "{p}");
    }
    assert!(s.execute("cutting.op.get", &json!({"ids": [999_999]})).is_err());
    assert_eq!(ops(&mut s, &[a]), [("cut".into(), "default".into())]);
}

#[test]
fn ops_survive_saving_natively() {
    let mut s = new_doc();
    let (a, b) = (rect(&mut s, 10.0), rect(&mut s, 50.0));
    s.execute("cutting.op.set", &json!({"ids": [a], "op": "score"})).unwrap();
    s.execute("cutting.op.set", &json!({"ids": [b], "op": "none"})).unwrap();
    reopen_native(&mut s);
    assert_eq!(ops(&mut s, &[a, b]), [("score".into(), "own".into()), ("none".into(), "own".into())]);
}

#[test]
fn ops_survive_an_svg_round_trip() {
    let mut s = new_doc();
    let (a, b) = (rect(&mut s, 10.0), rect(&mut s, 50.0));
    s.execute("cutting.op.set", &json!({"ids": [a], "op": "score"})).unwrap();
    s.execute("cutting.op.set", &json!({"ids": [b], "op": "none"})).unwrap();
    let svg = s.execute("document.serialize", &json!({"format": "svg"})).unwrap()["text"].as_str().unwrap().to_string();
    assert!(svg.contains(r#"xmlns:cut="urn:vectorcraft:cutting:1""#), "{svg}");
    assert!(svg.contains(r#"cut:op="score""#) && svg.contains(r#"cut:op="none""#), "{svg}");
    s.execute("document.open", &json!({"name": "again.svg", "dataBase64": vectorcraft_format::base64_encode(svg.as_bytes())})).unwrap();
    let mut found = vec![];
    let st = s.active().unwrap();
    st.doc.walk(|n| {
        if !n.is_layer() {
            found.push(n.id.0);
        }
    });
    let mut got: Vec<String> = ops(&mut s, &found).into_iter().map(|(op, _)| op).collect();
    got.sort();
    assert_eq!(got, ["none", "score"]);
}

#[test]
fn copies_keep_the_op() {
    let mut s = new_doc();
    let a = rect(&mut s, 10.0);
    s.execute("cutting.op.set", &json!({"ids": [a], "op": "score"})).unwrap();
    let dup = s.execute("edit.duplicate", &json!({"ids": [a]})).unwrap()["ids"][0].as_u64().unwrap();
    assert_ne!(dup, a);
    assert_eq!(ops(&mut s, &[dup]), [("score".into(), "own".into())]);
    s.execute("select.set", &json!({"ids": [a]})).unwrap();
    s.execute("edit.copy", &json!({})).unwrap();
    s.execute("edit.paste", &json!({})).unwrap();
    let pasted = s.execute("cutting.op.get", &json!({})).unwrap()["objects"].clone();
    assert_eq!(pasted.as_array().unwrap().len(), 1);
    assert_ne!(pasted[0]["id"], json!(a));
    assert_eq!((pasted[0]["op"].clone(), pasted[0]["source"].clone()), (json!("score"), json!("own")));
}
