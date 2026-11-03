//! C15.1: templates made to size: the net added as one group (outline Cut, folds Score, glue No
//! Cut), centred on the artboard; a template alone selected made again in place; one undo step.

use serde_json::{Value, json};
use vectorcraft_engine::Session;

/// The job's paths' kinds: "cut" or "score", in the job's order.
fn kinds(s: &mut Session) -> Vec<String> {
    let j = s.execute("cutting.job", &json!({})).unwrap();
    let ops = j["job"]["operations"].clone();
    j["job"]["paths"].as_array().unwrap().iter().map(|p| ops[p["op"].as_u64().unwrap() as usize]["kind"].as_str().unwrap().to_string()).collect()
}

/// `id`'s bounds, pt: x, y, width, height.
fn bounds(s: &mut Session, id: &Value) -> Vec<f64> {
    let b = s.active().unwrap().doc.bounds_of(&[vectorcraft_doc::NodeId(id.as_u64().unwrap())], false).unwrap();
    vec![b.x0, b.y0, b.width(), b.height()]
}

#[test]
fn a_tray_is_added_and_made_again_in_place() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800, "height": 800})).unwrap();
    let list = s.execute("cutting.templates", &json!({})).unwrap();
    let tray = list["templates"].as_array().unwrap().iter().find(|t| t["id"] == "tray").unwrap().clone();
    assert_eq!(tray["family"], "Boxes");
    assert!(tray["params"].as_array().unwrap().iter().any(|p| p["key"] == "thicknessMm" && p["unit"] == "mm"));
    for bad in [
        json!({}),
        json!({"kind": "teapot"}),
        json!({"kind": 3}),
        json!({"kind": "tray", "lengthMm": 5}),
        json!({"kind": "tray", "lengthMm": "long"}),
        json!({"kind": "tray", "outside": "yes"}),
        json!({"kind": "tray", "x": 10}),
        json!({"kind": "tray", "heightMm": 5, "tabMm": 20}),
        json!({"kind": "tray", "folds": "crease"}),
    ] {
        assert!(s.execute("cutting.template", &bad).is_err(), "{bad}");
    }
    let r = s.execute("cutting.template", &json!({"kind": "tray", "lengthMm": 100, "widthMm": 60, "heightMm": 20, "thicknessMm": 0})).unwrap();
    assert_eq!((r["kind"].clone(), r["replaced"].clone()), (json!("tray"), json!(false)));
    assert!((r["widthMm"].as_f64().unwrap() - 140.0).abs() < 1e-9 && (r["heightMm"].as_f64().unwrap() - 100.0).abs() < 1e-9, "{r}");
    assert_eq!(r["warnings"], json!([]));
    // One outline cut, eight folds scored; the glue isn't in the job.
    let k = kinds(&mut s);
    assert_eq!((k.iter().filter(|k| *k == "cut").count(), k.iter().filter(|k| *k == "score").count()), (1, 8), "{k:?}");
    // Moved, then made again taller: it stays where it was moved to.
    s.execute("select.set", &json!({"ids": [r["id"]]})).unwrap();
    s.execute("object.move", &json!({"dx": -50, "dy": 0})).unwrap();
    let moved = bounds(&mut s, &r["id"]);
    let again = s.execute("cutting.template", &json!({"heightMm": 30})).unwrap();
    assert_eq!((again["id"].clone(), again["replaced"].clone()), (r["id"].clone(), json!(true)));
    assert!((again["widthMm"].as_f64().unwrap() - 160.0).abs() < 1e-9, "the length kept, the height new: {again}");
    let now = bounds(&mut s, &r["id"]);
    assert!((now[0] - moved[0]).abs() < 1.0 && (now[1] - moved[1]).abs() < 1.0, "{moved:?} {now:?}");
    s.execute("edit.undo", &json!({})).unwrap();
    assert!((bounds(&mut s, &r["id"])[2] - moved[2]).abs() < 1e-6, "one undo step");
}

/// A net larger than the mat is still added, with a warning saying so.
#[test]
fn a_net_off_the_mat_is_warned_about() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800, "height": 800})).unwrap();
    let r = s.execute("cutting.template", &json!({"kind": "tray", "lengthMm": 600, "widthMm": 400, "heightMm": 50})).unwrap();
    let w = r["warnings"][0].as_str().unwrap_or_default();
    assert!(w.starts_with("the net (") && w.contains("off the mat"), "{r}");
}

/// The template, saved and opened again, is still one: selected alone, it is made again.
#[test]
fn a_template_survives_save_and_open() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800, "height": 800})).unwrap();
    let r = s.execute("cutting.template", &json!({"kind": "tray", "lengthMm": 120})).unwrap();
    let data = s.execute("document.serialize", &json!({})).unwrap()["dataBase64"].as_str().unwrap().to_string();
    s.execute("document.open", &json!({"name": "again.vectorcraft", "dataBase64": data})).unwrap();
    s.execute("select.set", &json!({"ids": [r["id"]]})).unwrap();
    let again = s.execute("cutting.template", &json!({"widthMm": 50})).unwrap();
    assert_eq!((again["replaced"].clone(), again["kind"].clone()), (json!(true), json!("tray")));
    assert!((again["widthMm"].as_f64().unwrap() - (120.3 + 2.0 * 30.15)).abs() < 1e-6, "the length kept: {again}");
}

/// The folds can be perforated instead of scored, and stay so when the template is made again.
#[test]
fn folds_may_be_perforated() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800, "height": 800})).unwrap();
    let r = s.execute("cutting.template", &json!({"kind": "tray", "folds": "perforate"})).unwrap();
    assert_eq!(r["folds"], "perforate");
    let k = kinds(&mut s);
    assert_eq!(k.iter().filter(|k| *k == "score").count(), 0, "{k:?}");
    assert!(k.len() > 8, "the folds cut in dashes: {k:?}");
    s.execute("select.set", &json!({"ids": [r["id"]]})).unwrap();
    let again = s.execute("cutting.template", &json!({"lengthMm": 120})).unwrap();
    assert_eq!(again["folds"], "perforate", "kept when made again");
    let back = s.execute("cutting.template", &json!({"folds": "score"})).unwrap();
    assert_eq!(back["folds"], "score");
    assert_eq!(kinds(&mut s).iter().filter(|k| *k == "score").count(), 8);
}

/// A switch takes true or false (or 1 or 0), nothing else.
#[test]
fn switches_take_true_or_false() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 800, "height": 800})).unwrap();
    for ok in [json!(false), json!(true), json!(0), json!(1)] {
        s.execute("cutting.template", &json!({"kind": "earringCard", "hangSlot": ok})).unwrap();
        s.execute("select.none", &json!({})).unwrap();
    }
    for bad in [json!("yes"), json!(0.5), json!(2)] {
        assert!(s.execute("cutting.template", &json!({"kind": "earringCard", "hangSlot": bad})).is_err(), "{bad}");
    }
}
