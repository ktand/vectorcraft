//! Fork: nesting (C8.5): the pieces moved close together on the mat by their true shapes, turned
//! where that helps (`vectorcraft_cutting::nest`), as one undo step. The objects named, else the
//! selected ones, else every top-level object that can be moved.

use serde_json::{Value, json};
use vectorcraft_cutting::nest::{Options, default_pieces, nest_document};

use super::*;

const NEST: &str = "cutting.nest";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.nest",
        "Nest on the Mat",
        [],
        None,
        "{ids?, gapMm?: 0–50 (default 2), rotate?: bool (default true: quarter turns allowed)} place the objects (ids, else the selected ones, else every top-level object on visible, unlocked layers) close together on the mat's cut area by their cut outlines, each with all it holds, turned where that helps, round what else is cut, as one undo step; ones that don't fit stay where they were → {moved: [ids], unplaced: [ids], usedLengthMm: how far down the mat the nested pieces reach, outOfWork: whether it stopped before trying every piece}",
        has_doc,
        nest
    )]
}

fn nest(s: &mut Session, p: &Value) -> Result<Value> {
    check_numbers(p, &["gapMm"], NEST)?;
    let gap = p.get("gapMm").and_then(Value::as_f64).unwrap_or(2.0);
    if !(0.0..=50.0).contains(&gap) {
        return Err(bad(NEST, format!("gapMm must be 0 to 50, not {gap}")));
    }
    let rotate = match p.get("rotate") {
        None => true,
        Some(v) => v.as_bool().ok_or_else(|| bad(NEST, "rotate must be true or false"))?,
    };
    let mut ids = super::objects(s, p, NEST)?;
    if ids.is_empty() {
        ids = default_pieces(&s.doc()?.doc);
    }
    if ids.is_empty() {
        return Err(bad(NEST, "nothing to nest"));
    }
    let nested = nest_document(&s.doc()?.doc, &ids, Options { gap_mm: gap, rotate }).map_err(|e| bad(NEST, e))?;
    let moves = nested.moves.clone();
    if !moves.is_empty() {
        s.edit("Nest on the Mat", |d, _| {
            for (id, a) in &moves {
                if let Some(n) = d.node_mut(*id) {
                    n.transform(*a, false);
                }
            }
            Ok(())
        })?;
    }
    Ok(json!({
        "moved": nested.moves.iter().map(|(id, _)| id.0).collect::<Vec<_>>(),
        "unplaced": nested.unplaced.iter().map(|id| id.0).collect::<Vec<_>>(),
        "usedLengthMm": (nested.used_length_mm * 10.0).round() / 10.0,
        "outOfWork": nested.out_of_work,
    }))
}
