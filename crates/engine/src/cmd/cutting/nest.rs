//! Fork: nesting (C8.5): the pieces moved close together on the mat by their true shapes, turned
//! where that helps (`vectorcraft_cutting::nest`), as one undo step. The objects named, else the
//! selected ones, else every top-level object that can be moved.

use serde_json::{Value, json};
use vectorcraft_cutting::nest::{MAX_COPIES, Options, add_copies, default_pieces, nest_document, nest_sheets};
use vectorcraft_doc::{Document, NodeId};

use super::{CommandSpec, Result, Session, bad, check_numbers, cmd, has_doc};

const NEST: &str = "cutting.nest";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.nest",
        "Nest on the Mat",
        [],
        None,
        "{ids?, gapMm?: 0–50 (default 2), rotate?: bool (default true: quarter turns allowed), turns?: none|quarter|fine (in place of rotate; fine: every 15°, slower), sheets?: bool (default false: what doesn't fit goes onto new artboards the cutting artboard's size, Sheet 2, Sheet 3…, each cut on its own), copies?: 0–500 (default 0: Replicate, that many more of each piece made and nested with them; the copies that don't fit are left out)} place the objects (ids, else the selected ones, else every top-level object on visible, unlocked layers) close together on the cutting artboard where it lies on the mat by their cut outlines, each with all it holds, turned where that helps, round what else is cut there, as one undo step; ones that don't fit stay where they were → {moved: [ids], unplaced: [ids], usedLengthMm: how far down the sheet the nested pieces reach (the first sheet), outOfWork: whether it stopped before trying every piece, sheets: [the artboards used], created: new sheets made, copies: the copies nested, copiesLeftOut: those that didn't fit}",
        has_doc,
        nest
    )]
}

fn nest(s: &mut Session, p: &Value) -> Result<Value> {
    check_numbers(p, &["gapMm", "copies"], NEST)?;
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
    let (rotate, fine) = match super::str_param(p, "turns") {
        None => (rotate, false),
        Some("none") => (false, false),
        Some("quarter") => (true, false),
        Some("fine") => (true, true),
        Some(other) => return Err(bad(NEST, format!("turns must be none, quarter or fine, not {other:?}"))),
    };
    let opt = Options { gap_mm: gap, rotate, fine };
    let copies = match p.get("copies") {
        None => 0,
        Some(v) => v
            .as_u64()
            .and_then(|n| usize::try_from(n).ok())
            .filter(|n| *n <= MAX_COPIES)
            .ok_or_else(|| bad(NEST, format!("copies must be a whole number 0 to {MAX_COPIES}")))?,
    };
    if copies > 0 {
        return replicate(s, &ids, copies, opt, sheets);
    }
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

/// What Replicate did: the pieces moved, those left where they were, the sheets used and made,
/// the copies nested and left out, and whether it ran out of work.
struct Replicated {
    moved: Vec<NodeId>,
    unplaced: Vec<NodeId>,
    sheets: Vec<usize>,
    created: usize,
    copies: usize,
    left_out: usize,
    out_of_work: bool,
}

/// `copies` more of each of `ids`, nested with them (onto more sheets with `sheets`); the copies
/// that don't fit taken out again. Edits `d`.
fn nest_copies(d: &mut Document, ids: &[NodeId], copies: usize, opt: Options, sheets: bool) -> std::result::Result<Replicated, String> {
    let made = add_copies(d, ids, copies)?;
    let all: Vec<NodeId> = ids.iter().chain(&made).copied().collect();
    let (unplaced, used, created, out_of_work) = if sheets {
        let n = nest_sheets(d, &all, opt)?;
        (n.unplaced, n.sheets, n.created, n.out_of_work)
    } else {
        let n = nest_document(d, &all, opt)?;
        for (id, a) in &n.moves {
            if let Some(node) = d.node_mut(*id) {
                node.transform(*a, false);
            }
        }
        let used = if n.moves.is_empty() { vec![] } else { usize::try_from(vectorcraft_cutting::settings(d).artboard).into_iter().collect() };
        (n.unplaced, used, 0, n.out_of_work)
    };
    let left_out: Vec<NodeId> = made.iter().filter(|c| unplaced.contains(c)).copied().collect();
    for c in &left_out {
        d.remove(*c).map_err(|e| e.to_string())?;
    }
    Ok(Replicated {
        moved: all.iter().filter(|id| !unplaced.contains(id)).copied().collect(),
        unplaced: ids.iter().filter(|id| unplaced.contains(id)).copied().collect(),
        sheets: used,
        created,
        copies: made.len() - left_out.len(),
        left_out: left_out.len(),
        out_of_work,
    })
}

fn replicate(s: &mut Session, ids: &[NodeId], copies: usize, opt: Options, sheets: bool) -> Result<Value> {
    let mut result = None;
    s.edit("Replicate and Nest", |d, _| {
        result = Some(nest_copies(d, ids, copies, opt, sheets).map_err(|e| bad(NEST, e))?);
        Ok(())
    })?;
    let r = result.ok_or_else(|| bad(NEST, "nothing was nested"))?;
    Ok(json!({
        "moved": r.moved.iter().map(|id| id.0).collect::<Vec<_>>(),
        "unplaced": r.unplaced.iter().map(|id| id.0).collect::<Vec<_>>(),
        "sheets": r.sheets,
        "created": r.created,
        "copies": r.copies,
        "copiesLeftOut": r.left_out,
        "outOfWork": r.out_of_work,
    }))
}
