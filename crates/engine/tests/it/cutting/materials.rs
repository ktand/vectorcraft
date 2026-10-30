//! C5.1: materials: built-in starting points and the user's own, applied to the document's tool
//! settings as one undo step, saved to and read back from their file. One test: the store is the
//! process's. C5.5: a material keeps the blade's depth and offset with the rest of the setup.

use serde_json::{Value, json};
use vectorcraft_engine::Session;

fn materials(s: &mut Session) -> Value {
    s.execute("cutting.materials", &json!({})).unwrap()
}

fn names(v: &Value) -> Vec<String> {
    v["materials"].as_array().unwrap().iter().map(|m| m["name"].as_str().unwrap().to_string()).collect()
}

#[test]
fn materials_are_applied_saved_and_kept() {
    let dir = std::env::temp_dir().join(format!("vectorcraft-materials-{}", std::process::id()));
    let file = dir.join("Materials.json");
    let _ = std::fs::remove_dir_all(&dir);
    vectorcraft_engine::cmd::cutting::materials::attach(file.clone());

    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();
    let m = materials(&mut s);
    assert_eq!(names(&m), ["Paper (light)", "Cardstock", "Vinyl"], "the starting points, none of the user's yet");
    assert_eq!((m["current"].clone(), m["changed"].clone()), (Value::Null, json!(false)));

    // Apply: the tool settings become the material's, in one undo step.
    let r = s.execute("cutting.material.apply", &json!({"name": "cardstock"})).unwrap();
    assert_eq!(r["current"], json!("Cardstock"));
    let settings = s.execute("cutting.settings.get", &json!({})).unwrap()["settings"].clone();
    assert_eq!(settings["operations"]["cut"]["passes"], json!(2));
    assert_eq!(settings["material"], json!("Cardstock"));
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(materials(&mut s)["current"], Value::Null);
    s.execute("edit.redo", &json!({})).unwrap();

    // A number set by hand: changed since the material.
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {"force": 0.5, "depth": 4, "bladeOffsetMm": 0.25}}})).unwrap();
    assert_eq!(materials(&mut s)["changed"], json!(true));

    // Saved as the user's: kept in the file, and named as the document's material.
    let r = s.execute("cutting.material.save", &json!({"name": "My card"})).unwrap();
    assert_eq!((r["current"].clone(), r["changed"].clone()), (json!("My card"), json!(false)));
    assert_eq!(names(&r).last().map(String::as_str), Some("My card"));
    let kept: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(kept[0]["name"], json!("My card"));
    assert_eq!(kept[0]["cut"]["force"], json!(0.5));
    assert_eq!((kept[0]["cut"]["depth"].clone(), kept[0]["cut"]["bladeOffsetMm"].clone()), (json!(4), json!(0.25)));

    // Read back as the next run would.
    vectorcraft_engine::cmd::cutting::materials::attach(file.clone());
    let listed = materials(&mut s);
    let mine = listed["materials"].as_array().unwrap().iter().find(|m| m["name"] == "My card").unwrap().clone();
    assert_eq!(mine["cut"]["bladeOffsetMm"], json!(0.25), "listed with the blade setup");

    // Only the blade offset changed: that's a change too, and applying the material puts it back.
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {"bladeOffsetMm": 0.9}}})).unwrap();
    assert_eq!(materials(&mut s)["changed"], json!(true));
    s.execute("cutting.material.apply", &json!({"name": "My card"})).unwrap();
    let cut = s.execute("cutting.settings.get", &json!({})).unwrap()["settings"]["operations"]["cut"].clone();
    assert_eq!((cut["depth"].clone(), cut["bladeOffsetMm"].clone()), (json!(4), json!(0.25)));

    // Built-in names and unknown ones are refused; the user's own can go.
    assert!(s.execute("cutting.material.save", &json!({"name": "Vinyl"})).is_err());
    assert!(s.execute("cutting.material.delete", &json!({"name": "Vinyl"})).is_err());
    assert!(s.execute("cutting.material.apply", &json!({"name": "Felt"})).is_err());
    assert!(s.execute("cutting.material.save", &json!({"name": "  "})).is_err());
    s.execute("cutting.material.delete", &json!({"name": "my card"})).unwrap();
    assert!(!names(&materials(&mut s)).contains(&"My card".to_string()));
    let kept: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(kept, json!([]));
    let _ = std::fs::remove_dir_all(&dir);
}

/// C10.2: the tools to choose from; a tool chosen for an operation is kept, an unknown one is
/// refused; a material applied leaves the tool as it is (it is the machine's); a group's setup
/// without a tool of its own is cut with the document's. Nothing is saved to the store.
#[test]
fn the_tool_is_chosen_per_operation() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
    let tools = s.execute("cutting.tools.list", &json!({})).unwrap();
    let first = &tools["tools"][0];
    assert_eq!((first["id"].clone(), first["holder"].clone(), first["autoblade"].clone()), (json!("autoblade"), json!(1), json!(true)));
    assert!(tools["tools"].as_array().unwrap().iter().any(|t| t["id"] == "rotary" && t["holder"] == 2));
    let e = s.execute("cutting.settings.set", &json!({"operations": {"cut": {"tool": "laser"}}})).unwrap_err().to_string();
    assert!(e.contains("unknown tool \"laser\""), "{e}");
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {"tool": "kraft3"}}})).unwrap();
    s.execute("cutting.material.apply", &json!({"name": "Cardstock"})).unwrap();
    let got = s.execute("cutting.settings.get", &json!({})).unwrap();
    assert_eq!(got["settings"]["operations"]["cut"]["tool"], json!("kraft3"), "the material left the tool");
    let job = s.execute("cutting.job", &json!({})).unwrap();
    assert_eq!(job["job"]["operations"][0]["tool"], json!("kraft3"), "{job}");
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {"tool": null}}})).unwrap();
    assert!(s.execute("cutting.settings.get", &json!({})).unwrap()["settings"]["operations"]["cut"]["tool"].is_null());
}
