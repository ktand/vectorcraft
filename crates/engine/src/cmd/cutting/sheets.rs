//! Fork: moving a piece to another sheet (C18.13). In a job nested onto several sheets (C8.6:
//! each sheet an artboard), the selected pieces moved by hand to another one: each keeps its place
//! relative to the sheet it is on, so a piece moved from Sheet 2 to Sheet 3 lands where it was,
//! one sheet over. One undo step.

use serde_json::{Value, json};
use vectorcraft_geom::{Affine, Point};

use super::{CommandSpec, Result, Session, bad, cmd, has_doc, index};

const CMD: &str = "cutting.sheet.move";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.sheet.move",
        "Move to Sheet",
        [],
        None,
        "{ids?, sheet: an artboard index} the objects (ids, else the selection) moved to artboard `sheet`, each keeping its place relative to the artboard its middle is on (one off every artboard keeps its place relative to the first), one undo step → {moved: [ids], sheet}",
        has_doc,
        move_to
    )]
}

fn move_to(s: &mut Session, p: &Value) -> Result<Value> {
    let to = p.get("sheet").and_then(Value::as_u64).ok_or_else(|| bad(CMD, "sheet is required: an artboard index"))?;
    let ids = super::objects(s, p, CMD)?;
    if ids.is_empty() {
        return Err(bad(CMD, "select the pieces to move, or give ids"));
    }
    let d = &s.doc()?.doc;
    let target =
        d.artboards.get(index(to)).map(|a| a.rect).ok_or_else(|| bad(CMD, format!("no artboard {to}: the document has {}", d.artboards.len())))?;
    let mut moves = vec![];
    for id in &ids {
        let Some(b) = d.bounds_of(&[*id], false) else { continue };
        let from = d.artboard_at(Point::new((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0)).unwrap_or(0);
        let Some(src) = d.artboards.get(from).map(|a| a.rect) else { continue };
        let (dx, dy) = (target.x0 - src.x0, target.y0 - src.y0);
        if dx != 0.0 || dy != 0.0 {
            moves.push((*id, Affine::translate((dx, dy))));
        }
    }
    let moved: Vec<u64> = moves.iter().map(|(id, _)| id.0).collect();
    if !moves.is_empty() {
        s.edit("Move to Sheet", |d, _| {
            for (id, a) in &moves {
                if let Some(n) = d.node_mut(*id) {
                    n.transform(*a, false);
                }
            }
            Ok(())
        })?;
    }
    Ok(json!({ "moved": moved, "sheet": to }))
}
