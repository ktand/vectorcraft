//! Fork: sketch fill (C13.3, patterns C14.5): lines across the selected art (`vectorcraft_cutting::hatch`), added
//! as a path just above it for a pen to draw. The art itself is left as it is. One undo step.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_cutting::hatch::{DEFAULT_ANGLE_DEG, DEFAULT_SPACING_MM, Options, Pattern, SPACING_MM, hatch};
use vectorcraft_doc::{Appearance, Node};

use super::{CommandSpec, Result, Session, bad, check_numbers, cmd, has_doc};

const CMD: &str = "cutting.sketchFill";
/// The fill's name.
pub const NAME: &str = "Sketch Fill";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.sketchFill",
        "Add Sketch Fill",
        [],
        None,
        "{ids?, pattern?: lines|concentric|spiral (default lines), spacingMm?: 0.3–20 (default 2), angleDeg?: degrees anticlockwise from across (default 45), cross?: bool (default false: a second set of lines at right angles; angleDeg and cross for lines only)} lines across the art (straight; rings each spacingMm in from the last; or a spiral from the middle out) of the objects (ids, else the selected ones), taken as filled as it is cut, spacingMm apart, clipped to it (holes left empty), for a pen to draw: added as one path of open lines just above the art (a thin black line), the art left as it is; one undo step → {id, lines}",
        has_doc,
        add
    )]
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    check_numbers(p, &["spacingMm", "angleDeg"], CMD)?;
    let spacing_mm = p.get("spacingMm").and_then(Value::as_f64).unwrap_or(DEFAULT_SPACING_MM);
    if !(SPACING_MM.0..=SPACING_MM.1).contains(&spacing_mm) {
        return Err(bad(CMD, format!("spacingMm must be {} to {}, not {spacing_mm}", SPACING_MM.0, SPACING_MM.1)));
    }
    let angle_deg = p.get("angleDeg").and_then(Value::as_f64).unwrap_or(DEFAULT_ANGLE_DEG);
    let cross = match p.get("cross") {
        None => false,
        Some(v) => v.as_bool().ok_or_else(|| bad(CMD, "cross must be true or false"))?,
    };
    let pattern = match p.get("pattern") {
        None | Some(Value::Null) => Pattern::Lines,
        Some(v) => v.as_str().and_then(Pattern::parse).ok_or_else(|| bad(CMD, format!("pattern must be lines, concentric or spiral, not {v}")))?,
    };
    let ids = super::objects(s, p, CMD)?;
    let Some(&last) = ids.last() else {
        return Err(bad(CMD, "select the art to fill, or give ids"));
    };
    let lines = hatch(&s.doc()?.doc, &ids, Options { pattern, spacing_mm, angle_deg, cross }).map_err(|e| bad(CMD, e))?;
    let count = lines.subpaths.len();
    let mut added = None;
    s.edit("Add Sketch Fill", |d, _| {
        // Just above the last object, in its group or layer (in a layer given as the art: on top).
        let (parent, index) = match (d.parent_of(last), d.index_path(last)) {
            (Some(parent), Some(path)) => (parent, path.last().map_or(usize::MAX, |i| i + 1)),
            _ => (last, usize::MAX),
        };
        let mut n = Node::path(d.alloc_id(), lines.clone(), Appearance::basic(Paint::None, Paint::solid(Color::BLACK), 0.5));
        n.name = Some(NAME.to_string());
        added = Some(d.insert(Some(parent), index, n).map_err(|e| bad(CMD, e.to_string()))?);
        Ok(())
    })?;
    let id = added.ok_or_else(|| bad(CMD, "the sketch fill wasn't added"))?;
    Ok(json!({ "id": id.0, "lines": count }))
}
