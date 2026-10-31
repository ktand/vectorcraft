//! C13.5: puzzles: a jigsaw's cut lines over the selection or the artboard, one undo step; a
//! puzzle alone selected made again in place (Randomize).

use serde_json::json;
use vectorcraft_engine::Session;

fn paths(s: &mut Session) -> Vec<serde_json::Value> {
    s.execute("cutting.job", &json!({})).unwrap()["job"]["paths"].as_array().unwrap().clone()
}

#[test]
fn a_puzzle_is_cut_over_the_art() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 600, "height": 500})).unwrap();
    let pic = s.execute("shape.rectangle", &json!({"x": 50, "y": 50, "width": 500, "height": 400})).unwrap()["id"].clone();
    s.execute("cutting.op.set", &json!({"ids": [pic], "op": "none"})).unwrap();
    for bad in [json!({"rows": 1}), json!({"cols": 2.5}), json!({"curve": 150}), json!({"wobble": -1}), json!({"curvature": "yes"})] {
        assert!(s.execute("cutting.puzzle", &bad).is_err(), "{bad}");
    }
    let r = s.execute("cutting.puzzle", &json!({"rows": 4, "cols": 5, "seed": 7})).unwrap();
    assert_eq!((r["pieces"].clone(), r["replaced"].clone()), (json!(20), json!(false)));
    let first = paths(&mut s);
    assert_eq!(first.len(), 1 + 3 + 4, "the border and 7 lines");
    assert!(first.iter().all(|p| p["source"]["node"] == r["id"]));
    // Randomize: the puzzle alone selected, another seed: the same path, other lines.
    s.execute("select.set", &json!({"ids": [r["id"]]})).unwrap();
    let again = s.execute("cutting.puzzle", &json!({"rows": 4, "cols": 5, "seed": 8})).unwrap();
    assert_eq!((again["id"].clone(), again["replaced"].clone()), (r["id"].clone(), json!(true)));
    let second = paths(&mut s);
    assert_eq!(second.len(), first.len());
    assert_ne!(second, first, "another layout");
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(paths(&mut s), first, "one undo step");
    s.execute("edit.undo", &json!({})).unwrap();
    assert!(paths(&mut s).is_empty());
    // Nothing selected: over the artboard; a plain grid.
    s.execute("select.none", &json!({})).unwrap();
    let r = s.execute("cutting.puzzle", &json!({"rows": 2, "cols": 2, "curvature": false})).unwrap();
    assert_eq!(r["pieces"], json!(4));
}
