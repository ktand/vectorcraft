//! Fork: stencil bridges (C11.5): the selected art as a stencil, its holes' counters held to the
//! sheet by bridges (`vectorcraft_cutting::bridges`). The bridged shape is added as a path above
//! the art, set to Cut; the art itself is set to No Cut and hidden, so what shows is what is cut.
//! One undo step.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_cutjob::Op;
use vectorcraft_cutting::bridges::{DEFAULT_WIDTH_MM, Direction, Options, WIDTH_MM, bridged};
use vectorcraft_cutting::ops::set_own_op;
use vectorcraft_doc::{Appearance, Node};

use super::{CommandSpec, Result, Session, bad, check_numbers, cmd, has_doc, str_param};

const CMD: &str = "cutting.bridges";
/// The bridged path's name.
pub const NAME: &str = "Stencil";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.bridges",
        "Add Stencil Bridges",
        [],
        None,
        "{ids?, widthMm?: 0.2–20 (default 1.5), direction?: vertical|horizontal (default vertical)} the art of the objects (ids, else the selected ones) as a stencil: its filled area, each hole bridged to the art round it by a bar through the hole's middle (above and below it, or left and right), the bars' pieces beside the hole taken out; added as a path above the art (set to Cut), the art set to No Cut and hidden; one undo step → {id, bridges, unbridged: holes the bar missed the art beside}",
        has_doc,
        add
    )]
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    check_numbers(p, &["widthMm"], CMD)?;
    let width_mm = p.get("widthMm").and_then(Value::as_f64).unwrap_or(DEFAULT_WIDTH_MM);
    if !WIDTH_MM.contains(&width_mm) {
        return Err(bad(CMD, format!("widthMm must be {} to {}, not {width_mm}", WIDTH_MM.start(), WIDTH_MM.end())));
    }
    let direction = match str_param(p, "direction") {
        None => Direction::Vertical,
        Some(v) => Direction::parse(v).ok_or_else(|| bad(CMD, format!("direction must be vertical or horizontal, not {v:?}")))?,
    };
    let ids = super::objects(s, p, CMD)?;
    let Some(&last) = ids.last() else {
        return Err(bad(CMD, "select the art to bridge, or give ids"));
    };
    let b = bridged(&s.doc()?.doc, &ids, Options { width_mm, direction }).map_err(|e| bad(CMD, e))?;
    let mut added = None;
    s.edit("Add Stencil Bridges", |d, _| {
        // Just above the last object, in its group or layer (in a layer given as the art: on top).
        let (parent, index) = match (d.parent_of(last), d.index_path(last)) {
            (Some(parent), Some(path)) => (parent, path.last().map_or(usize::MAX, |i| i + 1)),
            _ => (last, usize::MAX),
        };
        let mut n = Node::path(d.alloc_id(), b.path.clone(), Appearance::basic(Paint::solid(Color::BLACK), Paint::None, 0.0));
        n.name = Some(NAME.to_string());
        set_own_op(&mut n, Some(Op::Cut));
        added = Some(d.insert(Some(parent), index, n).map_err(|e| bad(CMD, e.to_string()))?);
        for id in &ids {
            if let Some(n) = d.node_mut(*id) {
                set_own_op(n, Some(Op::NoCut));
                n.visible = false;
            }
        }
        Ok(())
    })?;
    let id = added.ok_or_else(|| bad(CMD, "the stencil wasn't added"))?;
    Ok(json!({ "id": id.0, "bridges": b.bridges, "unbridged": b.unbridged }))
}
