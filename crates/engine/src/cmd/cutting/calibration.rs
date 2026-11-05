//! Fork: the calibration sheet (C4.12, `vectorcraft_cutting::calibration`): added as one group
//! centred on the cutting artboard, its parts set to Cut and, with Print & Cut, the printed
//! crosses left out of the cut. One undo step; what to measure is hardware checks step 21.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_cutjob::Op;
use vectorcraft_cutting::calibration::{SIZE_MM, sheet};
use vectorcraft_cutting::ops::set_own_op;
use vectorcraft_cutting::placement::MM_PER_PT;
use vectorcraft_doc::{Appearance, Node, NodeId};
use vectorcraft_geom::{Affine, PathData};

use super::{CommandSpec, Result, Session, bad, cmd, has_doc};

const CMD: &str = "cutting.calibration";
/// The group's name.
pub const NAME: &str = "Calibration Sheet";
/// The printed crosses' line width, pt (about 0.25 mm).
const CROSS_PT: f64 = 0.7;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.calibration",
        "Add Calibration Sheet",
        [],
        None,
        "{printAndCut?: bool (default: the document's Print & Cut setting)} the calibration sheet (hardware checks step 21), 112 mm square, added as one group centred on the cutting artboard: a 100 mm line across and one down (scale), a 20 mm square and circle (closing, the overcut), a star, 5 and 2 mm squares and a small triangle (sharp corners, the blade offset), all set to Cut; with printAndCut, four printed crosses (not cut) with an 8 mm ring cut round each (registration); one undo step → {id, parts, printAndCut, warnings}",
        has_doc,
        add
    )]
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    let d = &s.doc()?.doc;
    let settings = vectorcraft_cutting::settings(d);
    let print_and_cut = match p.get("printAndCut") {
        None => settings.print_and_cut,
        Some(v) => v.as_bool().ok_or_else(|| bad(CMD, "printAndCut must be true or false"))?,
    };
    let r = usize::try_from(settings.artboard)
        .ok()
        .and_then(|i| d.artboards.get(i))
        .map(|a| a.rect)
        .ok_or_else(|| bad(CMD, "the cutting artboard isn't in the document"))?;
    let [w, h] = SIZE_MM.map(|v| v / MM_PER_PT);
    if r.width() < w || r.height() < h {
        return Err(bad(CMD, format!("the cutting artboard is smaller than the sheet's {} × {} mm", SIZE_MM[0], SIZE_MM[1])));
    }
    let to_doc = Affine::translate(((r.x0 + r.x1 - w) / 2.0, (r.y0 + r.y1 - h) / 2.0)) * Affine::scale(1.0 / MM_PER_PT);
    let parts = sheet(print_and_cut);
    let all = PathData::new(parts.iter().flat_map(|q| PathData::from_bezpath(&(to_doc * q.path.clone())).subpaths).collect());
    let warnings = super::popup::off_the_mat(d, &all, "calibration sheet");
    let top = d.layers.last().map(|l| l.id).ok_or_else(|| bad(CMD, "the document has no layer"))?;
    let count = parts.len();
    let mut id: Option<NodeId> = None;
    s.edit("Add Calibration Sheet", |d, _| {
        let nodes: Vec<Arc<Node>> = parts
            .iter()
            .map(|q| {
                let (fill, stroke, width) = (Paint::None, Paint::solid(Color::BLACK), if q.op == Op::NoCut { CROSS_PT } else { 0.5 });
                let mut n = Node::path(d.alloc_id(), PathData::from_bezpath(&(to_doc * q.path.clone())), Appearance::basic(fill, stroke, width));
                n.name = Some(q.name.to_string());
                set_own_op(&mut n, Some(q.op));
                Arc::new(n)
            })
            .collect();
        let mut g = Node::group(d.alloc_id(), nodes);
        g.name = Some(NAME.to_string());
        id = Some(d.insert(Some(top), usize::MAX, g).map_err(|e| bad(CMD, e.to_string()))?);
        Ok(())
    })?;
    let id = id.ok_or_else(|| bad(CMD, "the calibration sheet wasn't added"))?;
    Ok(json!({ "id": id.0, "parts": count, "printAndCut": print_and_cut, "warnings": warnings }))
}
