//! Fork: puzzles (C13.5): a jigsaw's cut lines (`vectorcraft_cutting::puzzle`) over the selected
//! objects' bounds (a printed picture, say) or, with nothing selected, the cutting artboard, added
//! as one path on top and set to Cut. The art is left as it is, to be printed. One undo step.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_cutjob::Op;
use vectorcraft_cutting::ops::set_own_op;
use vectorcraft_cutting::puzzle::{DEFAULT_TAB_PCT, Options, puzzle};
use vectorcraft_doc::{Appearance, Node};

use super::{CommandSpec, Result, Session, bad, check_numbers, cmd, has_doc};

const CMD: &str = "cutting.puzzle";
/// The puzzle's name.
pub const NAME: &str = "Puzzle";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.puzzle",
        "Add Puzzle",
        [],
        None,
        "{ids?, rows?: 2–50 (default 4), cols?: 2–50 (default 5; at most 2500 pieces), tabPct?: 10–35 (default 22: a tab's height, % of a piece's side), variety?: 0–1 (default 0.5: how much the tabs vary in place and size), seed?: whole number (default 1: the same settings and seed make the same puzzle)} a jigsaw's cut lines over the objects' bounds (ids, else the selected ones; with none, the cutting artboard): the border and the lines between rows and columns, a tab on each piece's side; added as one path on top (set to Cut), the art left as it is; one undo step → {id, pieces}",
        has_doc,
        add
    )]
}

fn whole(p: &Value, key: &str, default: u32) -> Result<u32> {
    match p.get(key) {
        None => Ok(default),
        Some(v) => v.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| bad(CMD, format!("{key} must be a whole number"))),
    }
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    check_numbers(p, &["rows", "cols", "tabPct", "variety", "seed"], CMD)?;
    let opt = Options {
        rows: whole(p, "rows", 4)?,
        cols: whole(p, "cols", 5)?,
        tab_pct: p.get("tabPct").and_then(Value::as_f64).unwrap_or(DEFAULT_TAB_PCT),
        variety: p.get("variety").and_then(Value::as_f64).unwrap_or(0.5),
        seed: whole(p, "seed", 1)?,
    };
    let ids = super::objects(s, p, CMD)?;
    let d = &s.doc()?.doc;
    let rect = if ids.is_empty() {
        let s = vectorcraft_cutting::settings(d);
        usize::try_from(s.artboard)
            .ok()
            .and_then(|i| d.artboards.get(i))
            .map(|a| a.rect)
            .ok_or_else(|| bad(CMD, "the cutting artboard isn't in the document"))?
    } else {
        let symbols = d.symbol_boxes();
        ids.iter()
            .filter_map(|id| d.node(*id)?.visual_bounds_with(&symbols))
            .reduce(|a, b| a.union(b))
            .ok_or_else(|| bad(CMD, "the selection has no size"))?
    };
    let lines = puzzle(rect, opt).map_err(|e| bad(CMD, e))?;
    let top = match ids.last() {
        Some(last) => d.layer_of(*last),
        None => d.layers.last().map(|l| l.id),
    }
    .ok_or_else(|| bad(CMD, "the document has no layer"))?;
    let mut added = None;
    s.edit("Add Puzzle", |d, _| {
        let mut n = Node::path(d.alloc_id(), lines.clone(), Appearance::basic(Paint::None, Paint::solid(Color::BLACK), 0.5));
        n.name = Some(NAME.to_string());
        set_own_op(&mut n, Some(Op::Cut));
        added = Some(d.insert(Some(top), usize::MAX, n).map_err(|e| bad(CMD, e.to_string()))?);
        Ok(())
    })?;
    let id = added.ok_or_else(|| bad(CMD, "the puzzle wasn't added"))?;
    Ok(json!({ "id": id.0, "pieces": opt.rows * opt.cols }))
}
