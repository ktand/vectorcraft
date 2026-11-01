//! Fork: tiling (C14.7): a design larger than the mat cut in tiles, each an artboard of its own
//! (`vectorcraft_cutting::tile`), the cutting settings set to the first. One undo step.

use serde_json::{Value, json};
use vectorcraft_cutting::tile::{DEFAULT_OVERLAP_MM, add_tiles};
use vectorcraft_doc::{Document, NodeId};
use vectorcraft_geom::Rect;

use super::{CommandSpec, Result, Session, bad, check_numbers, cmd, has_doc};

const CMD: &str = "cutting.tile";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.tile",
        "Tile onto Mats",
        [],
        None,
        "{ids?, overlapMm?: 0–50 (default 5: how far neighbouring tiles overlap, to join the pieces)} cut a design larger than the mat in tiles: artboards the size the mat can cut (Tile 1, Tile 2…, row by row from the top-left) laid over the objects (ids, else the selected ones, else the cutting artboard), the cutting settings set to the first tile and to clip to the mat, so each tile is cut on its own sheet by choosing its artboard; an error when it fits the mat already; one undo step → {artboards: [indices], rows, cols}",
        has_doc,
        tile
    )]
}

/// The rectangle tiled: the objects' bounds, else the cutting artboard.
fn region(d: &Document, ids: &[NodeId]) -> Result<Rect> {
    if ids.is_empty() {
        let s = vectorcraft_cutting::settings(d);
        return usize::try_from(s.artboard)
            .ok()
            .and_then(|i| d.artboards.get(i))
            .map(|a| a.rect)
            .ok_or_else(|| bad(CMD, "the cutting artboard isn't in the document"));
    }
    let symbols = d.symbol_boxes();
    ids.iter()
        .filter_map(|id| d.node(*id)?.visual_bounds_with(&symbols))
        .reduce(|a, b| a.union(b))
        .ok_or_else(|| bad(CMD, "the selection has no size"))
}

fn tile(s: &mut Session, p: &Value) -> Result<Value> {
    check_numbers(p, &["overlapMm"], CMD)?;
    let overlap = p.get("overlapMm").and_then(Value::as_f64).unwrap_or(DEFAULT_OVERLAP_MM);
    let ids = super::objects(s, p, CMD)?;
    let rect = region(&s.doc()?.doc, &ids)?;
    let mut tiled = None;
    s.edit("Tile onto Mats", |d, _| {
        tiled = Some(add_tiles(d, rect, overlap).map_err(|e| bad(CMD, e))?);
        Ok(())
    })?;
    let t = tiled.ok_or_else(|| bad(CMD, "the tiles weren't made"))?;
    Ok(json!({ "artboards": t.artboards, "rows": t.rows, "cols": t.cols }))
}
