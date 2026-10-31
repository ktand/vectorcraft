//! Fork: rhinestone templates (C13.4): the selected art as holes for stones
//! (`vectorcraft_cutting::stones`), added as a path above the art and set to Cut; the art itself
//! set to No Cut and hidden, as Stencil does, so what shows is what is cut. One undo step.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_cutjob::Op;
use vectorcraft_cutting::ops::set_own_op;
use vectorcraft_cutting::stones::{DEFAULT_GAP_MM, DEFAULT_HOLE_MM, GAP_MM, HOLE_MM, Options, centres, holes};
use vectorcraft_doc::{Appearance, Node};

use super::{CommandSpec, Result, Session, bad, check_numbers, cmd, has_doc};

const CMD: &str = "cutting.rhinestones";
/// The template's name.
pub const NAME: &str = "Rhinestones";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.rhinestones",
        "Add Rhinestone Template",
        [],
        None,
        "{ids?, holeMm?: 1–10 (default 3: a hole's diameter, a little more than the stone's), gapMm?: 0.3–20 (default 1: the material between holes), fill?: bool (default false: holes inside as well, a staggered grid kept a row clear of the outline)} the art of the objects (ids, else the selected ones), taken as filled as it is cut, as a rhinestone template: round holes evenly spaced along each outline; added as a path above the art (set to Cut), the art set to No Cut and hidden; one undo step → {id, stones}",
        has_doc,
        add
    )]
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    check_numbers(p, &["holeMm", "gapMm"], CMD)?;
    let hole_mm = p.get("holeMm").and_then(Value::as_f64).unwrap_or(DEFAULT_HOLE_MM);
    let gap_mm = p.get("gapMm").and_then(Value::as_f64).unwrap_or(DEFAULT_GAP_MM);
    if !(HOLE_MM.0..=HOLE_MM.1).contains(&hole_mm) || !(GAP_MM.0..=GAP_MM.1).contains(&gap_mm) {
        return Err(bad(CMD, format!("holeMm is {} to {} and gapMm {} to {}", HOLE_MM.0, HOLE_MM.1, GAP_MM.0, GAP_MM.1)));
    }
    let fill = match p.get("fill") {
        None => false,
        Some(v) => v.as_bool().ok_or_else(|| bad(CMD, "fill must be true or false"))?,
    };
    let ids = super::objects(s, p, CMD)?;
    let Some(&last) = ids.last() else {
        return Err(bad(CMD, "select the art to set stones in, or give ids"));
    };
    let at = centres(&s.doc()?.doc, &ids, Options { hole_mm, gap_mm, fill }).map_err(|e| bad(CMD, e))?;
    let path = holes(&at, hole_mm);
    let mut added = None;
    s.edit("Add Rhinestone Template", |d, _| {
        // Just above the last object, in its group or layer (in a layer given as the art: on top).
        let (parent, index) = match (d.parent_of(last), d.index_path(last)) {
            (Some(parent), Some(path)) => (parent, path.last().map_or(usize::MAX, |i| i + 1)),
            _ => (last, usize::MAX),
        };
        let mut n = Node::path(d.alloc_id(), path.clone(), Appearance::basic(Paint::None, Paint::solid(Color::BLACK), 0.5));
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
    let id = added.ok_or_else(|| bad(CMD, "the template wasn't added"))?;
    Ok(json!({ "id": id.0, "stones": at.len() }))
}
