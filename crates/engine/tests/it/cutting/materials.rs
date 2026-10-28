//! C5.1: materials: built-in starting points and the user's own, applied to the document's tool
//! settings as one undo step, saved to and read back from their file. One test: the store is the
//! process's.

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
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {"force": 0.5}}})).unwrap();
    assert_eq!(materials(&mut s)["changed"], json!(true));

    // Saved as the user's: kept in the file, and named as the document's material.
    let r = s.execute("cutting.material.save", &json!({"name": "My card"})).unwrap();
    assert_eq!((r["current"].clone(), r["changed"].clone()), (json!("My card"), json!(false)));
    assert_eq!(names(&r).last().map(String::as_str), Some("My card"));
    let kept: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(kept[0]["name"], json!("My card"));
    assert_eq!(kept[0]["cut"]["force"], json!(0.5));

    // Read back as the next run would.
    vectorcraft_engine::cmd::cutting::materials::attach(file.clone());
    assert!(names(&materials(&mut s)).contains(&"My card".to_string()));

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
