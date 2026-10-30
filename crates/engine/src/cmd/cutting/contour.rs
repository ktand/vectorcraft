//! Fork: the contour cut (C11.1): a cut line round the selected art, a set distance out
//! (`vectorcraft_cutting::contour`), on the document's Cut Contour layer, as one undo step. The
//! art itself is set to No Cut by default: a sticker is printed, and only the line round it is
//! cut.

use serde_json::{Value, json};
use vectorcraft_cutjob::Op;
use vectorcraft_cutting::contour::{DEFAULT_OFFSET_MM, MAX_OFFSET_MM, Options, contour, contour_layer, contour_layer_node, contour_node};
use vectorcraft_cutting::ops::set_own_op;

use super::{CommandSpec, Result, Session, bad, check_numbers, cmd, has_doc};

const CMD: &str = "cutting.contour";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.contour",
        "Add Cut Contour",
        [],
        None,
        "{ids?, offsetMm?: 0–50 (default 2), holes?: bool (default false: the outside only), noCutArt?: bool (default true: the art itself set to No Cut)} a cut line round the art of the objects (ids, else the selected ones), offsetMm out with round corners, the pieces that come within it of each other joined, as a path on the Cut Contour layer (made at the top, set to Cut, when there is none), one undo step → {id: the contour path, layer, pieces: how many closed lines}",
        has_doc,
        add
    )]
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    check_numbers(p, &["offsetMm"], CMD)?;
    let offset_mm = p.get("offsetMm").and_then(Value::as_f64).unwrap_or(DEFAULT_OFFSET_MM);
    if !(0.0..=MAX_OFFSET_MM).contains(&offset_mm) {
        return Err(bad(CMD, format!("offsetMm must be 0 to {MAX_OFFSET_MM}, not {offset_mm}")));
    }
    let flag = |key: &str, default: bool| match p.get(key) {
        None => Ok(default),
        Some(v) => v.as_bool().ok_or_else(|| bad(CMD, format!("{key} must be true or false"))),
    };
    let (holes, no_cut_art) = (flag("holes", false)?, flag("noCutArt", true)?);
    let ids = super::objects(s, p, CMD)?;
    if ids.is_empty() {
        return Err(bad(CMD, "select the art to go round, or give ids"));
    }
    let path = contour(&s.doc()?.doc, &ids, Options { offset_mm, holes }).map_err(|e| bad(CMD, e))?;
    let pieces = path.subpaths.len();
    let mut added = None;
    s.edit("Add Cut Contour", |d, _| {
        let layer = match contour_layer(d) {
            Some(l) => l,
            None => {
                let node = contour_layer_node(d);
                d.insert(None, usize::MAX, node).map_err(|e| bad(CMD, e.to_string()))?
            }
        };
        let node = contour_node(d, path.clone());
        added = Some((d.insert(Some(layer), usize::MAX, node).map_err(|e| bad(CMD, e.to_string()))?, layer));
        if no_cut_art {
            for id in &ids {
                if let Some(n) = d.node_mut(*id) {
                    set_own_op(n, Some(Op::NoCut));
                }
            }
        }
        Ok(())
    })?;
    let (id, layer) = added.ok_or_else(|| bad(CMD, "the contour wasn't added"))?;
    Ok(json!({ "id": id.0, "layer": layer.0, "pieces": pieces }))
}
