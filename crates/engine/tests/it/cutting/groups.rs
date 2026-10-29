//! C7.4: each group's own setup, cut by layer and by colour: listed in cutting order, set from a
//! material or by hand in one undo step each, reaching the job, and cleared again.

use serde_json::{Value, json};
use vectorcraft_engine::Session;

fn groups(s: &mut Session) -> Value {
    s.execute("cutting.groups", &json!({})).unwrap()
}

fn job(s: &mut Session) -> Value {
    s.execute("cutting.job", &json!({})).unwrap()["job"].clone()
}

/// The force each path of the job is cut with.
fn forces(job: &Value) -> Vec<f64> {
    job["paths"].as_array().unwrap().iter().map(|p| job["operations"][p["op"].as_u64().unwrap() as usize]["force"].as_f64().unwrap()).collect()
}

/// A document with a square on the first layer and one on "Gold", cut by layer.
fn two_layers() -> Session {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
    s.execute("layer.new", &json!({"name": "Gold"})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 100, "y": 100, "width": 50, "height": 50})).unwrap();
    s.execute("cutting.settings.set", &json!({"grouping": "layer"})).unwrap();
    s
}

#[test]
fn a_layer_is_cut_with_its_own_material() {
    let mut s = two_layers();
    let g = groups(&mut s);
    assert_eq!(g["grouping"], "layer");
    let names: Vec<&str> = g["groups"].as_array().unwrap().iter().map(|g| g["name"].as_str().unwrap()).collect();
    assert_eq!(names.len(), 2);
    assert_eq!(names[1], "Gold");
    assert!(g["groups"].as_array().unwrap().iter().all(|g| g["setup"].is_null()), "none of their own yet");

    // A material: Gold's setup is Cardstock's, one undo step; the job cuts Gold with it.
    let r = s.execute("cutting.group.set", &json!({"group": "Gold", "material": "Cardstock"})).unwrap();
    let gold = &r["groups"][1]["setup"];
    assert_eq!((gold["material"].clone(), gold["changed"].clone(), gold["cut"]["passes"].clone()), (json!("Cardstock"), json!(false), json!(2)));
    let j = job(&mut s);
    assert_eq!(forces(&j), [0.3, 0.45], "the first layer the document's, Gold Cardstock's");
    assert_eq!(j["groups"][1]["material"], "Cardstock");
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(groups(&mut s)["groups"][1]["setup"].is_null());
    s.execute("edit.redo", &json!({})).unwrap();

    // By hand: changed since the material; out of range or unknown keys refused.
    let r = s.execute("cutting.group.set", &json!({"group": "Gold", "operations": {"cut": {"passes": 3}}})).unwrap();
    assert_eq!((r["groups"][1]["setup"]["changed"].clone(), r["groups"][1]["setup"]["cut"]["passes"].clone()), (json!(true), json!(3)));
    assert!(s.execute("cutting.group.set", &json!({"group": "Gold", "operations": {"cut": {"passes": 40}}})).is_err());
    assert!(s.execute("cutting.group.set", &json!({"group": "Gold", "operations": {"cut": {"blade": 1}}})).is_err());
    assert!(s.execute("cutting.group.set", &json!({"group": "Gold", "material": "Unobtainium"})).is_err());
    assert!(s.execute("cutting.group.set", &json!({"group": "Silver", "clear": true})).is_err());
    assert!(s.execute("cutting.group.set", &json!({"group": "Gold"})).is_err(), "nothing to set");

    // It goes with the layer when it's renamed, and it's saved with the document.
    let gold = s.active().unwrap().doc.layers.iter().find(|l| l.name.as_deref() == Some("Gold")).unwrap().id.0;
    s.execute("layer.setProps", &json!({"id": gold, "name": "Copper"})).unwrap();
    let g = groups(&mut s);
    assert_eq!((g["groups"][1]["name"].clone(), g["groups"][1]["setup"]["cut"]["passes"].clone()), (json!("Copper"), json!(3)));
    s.execute("edit.undo", &json!({})).unwrap();

    // Cleared: the document's again.
    s.execute("cutting.group.set", &json!({"group": "Gold", "clear": true})).unwrap();
    assert!(groups(&mut s)["groups"][1]["setup"].is_null());
    assert_eq!(forces(&job(&mut s)), [0.3, 0.3]);
}

#[test]
fn a_colour_keeps_its_setup_in_the_settings() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
    s.execute("cutting.settings.set", &json!({"grouping": "color"})).unwrap();
    let name = groups(&mut s)["groups"][0]["name"].as_str().unwrap().to_string();
    s.execute("cutting.group.set", &json!({"group": name, "material": "Vinyl"})).unwrap();
    let settings = s.execute("cutting.settings.get", &json!({})).unwrap()["settings"].clone();
    assert_eq!(settings["colorSetups"][&name]["material"], "Vinyl");
    assert_eq!(forces(&job(&mut s)), [0.25]);
    // Grouped otherwise, the colour's setup stays but isn't used; ungrouped, nothing can be set.
    s.execute("cutting.settings.set", &json!({"grouping": "none"})).unwrap();
    assert_eq!(forces(&job(&mut s)), [0.3]);
    assert!(s.execute("cutting.group.set", &json!({"group": name, "clear": true})).is_err());
    s.execute("cutting.settings.set", &json!({"grouping": "color"})).unwrap();
    s.execute("cutting.group.set", &json!({"group": name, "clear": true})).unwrap();
    assert!(s.execute("cutting.settings.get", &json!({})).unwrap()["settings"].get("colorSetups").is_none());
}
