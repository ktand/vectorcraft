//! Fork: nesting (C8.5): the pieces moved close together on the mat by their true shapes, turned
//! where that helps (`vectorcraft_cutting::nest`), as one undo step. The objects named, else the
//! selected ones, else every top-level object that can be moved.

use serde_json::{Value, json};
use vectorcraft_cutting::nest::{Options, default_pieces, nest_document, nest_sheets};

use super::{CommandSpec, Result, Session, bad, check_numbers, cmd, has_doc};

const NEST: &str = "cutting.nest";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.nest",
        "Nest on the Mat",
        [],
        None,
        "{ids?, gapMm?: 0–50 (default 2), rotate?: bool (default true: quarter turns allowed), sheets?: bool (default false: what doesn't fit goes onto new artboards the cutting artboard's size, Sheet 2, Sheet 3…, each cut on its own)} place the objects (ids, else the selected ones, else every top-level object on visible, unlocked layers) close together on the cutting artboard where it lies on the mat by their cut outlines, each with all it holds, turned where that helps, round what else is cut there, as one undo step; ones that don't fit stay where they were → {moved: [ids], unplaced: [ids], usedLengthMm: how far down the sheet the nested pieces reach (the first sheet), outOfWork: whether it stopped before trying every piece, sheets: [the artboards used], created: new sheets made}",
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
    let flag = |key: &str, default: bool| match p.get(key) {
        None => Ok(default),
        Some(v) => v.as_bool().ok_or_else(|| bad(NEST, format!("{key} must be true or false"))),
    };
    let (rotate, sheets) = (flag("rotate", true)?, flag("sheets", false)?);
    let mut ids = super::objects(s, p, NEST)?;
    if ids.is_empty() {
        ids = default_pieces(&s.doc()?.doc);
    }
    if ids.is_empty() {
        return Err(bad(NEST, "nothing to nest"));
    }
    let opt = Options { gap_mm: gap, rotate };
    if sheets {
        let mut result = None;
        s.edit("Nest on Sheets", |d, _| {
            let n = nest_sheets(d, &ids, opt).map_err(|e| bad(NEST, e))?;
            result = Some(n);
            Ok(())
        })?;
        let n = result.ok_or_else(|| bad(NEST, "nothing was nested"))?;
        let moved: Vec<u64> = ids.iter().filter(|id| !n.unplaced.contains(id)).map(|id| id.0).collect();
        return Ok(json!({
            "moved": moved,
            "unplaced": n.unplaced.iter().map(|id| id.0).collect::<Vec<_>>(),
            "sheets": n.sheets,
            "created": n.created,
            "outOfWork": n.out_of_work,
        }));
    }
    let nested = nest_document(&s.doc()?.doc, &ids, opt).map_err(|e| bad(NEST, e))?;
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
        "sheets": if nested.moves.is_empty() { json!([]) } else { json!([vectorcraft_cutting::settings(&s.doc()?.doc).artboard]) },
        "created": 0,
    }))
}
