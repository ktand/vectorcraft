//! Fork: a design made into layered vinyl (C18.30, `vectorcraft_cutting::layered`): the selected
//! art replaced by one filled shape per colour, each on a layer of its own, bottom first (the base,
//! the design's silhouette, at the bottom), and the job cut by layer, so each colour is its own
//! group with its own material. One undo step.

use serde_json::{Value, json};
use vectorcraft_color::Paint;
use vectorcraft_cutjob::Grouping;
use vectorcraft_cutting::layered::{Base, OVERLAP_MM, Options, layered};
use vectorcraft_doc::{Appearance, LAYER_COLORS, LayerColor, Node, NodeId};

use super::{CommandSpec, Result, Session, bad, check_numbers, cmd, has_doc};

const CMD: &str = "cutting.vinylLayers";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.vinylLayers",
        "Make Vinyl Layers",
        [],
        None,
        "{ids?, base?: auto|none|a colour (#rrggbb or a swatch's name; default auto: the colour along most of the design's outer edge), overlapMm?: 0–3 (default 0: how far each colour runs under the colours above it)} the art (ids, else the selection) made into layered vinyl, one undo step: each colour it paints (fills and strokes alike, strokes as drawn, type as its glyphs) one filled shape of what shows of it, on a layer of its own named after it, bottom first; the base colour's shape is the whole design's silhouette, at the bottom; the art replaced; the job then cut by layer → {layers: [{id, path, name, base}], warnings}",
        has_doc,
        make
    )]
}

fn options(p: &Value) -> Result<Options> {
    check_numbers(p, &["overlapMm"], CMD)?;
    let overlap_mm = p.get("overlapMm").and_then(Value::as_f64).unwrap_or(0.0);
    let (lo, hi) = OVERLAP_MM;
    if !(lo..=hi).contains(&overlap_mm) {
        return Err(bad(CMD, format!("overlapMm is {lo}–{hi}")));
    }
    let base = match p.get("base") {
        None => Base::Auto,
        Some(Value::String(s)) if s == "auto" => Base::Auto,
        Some(Value::String(s)) if s == "none" => Base::None,
        Some(Value::String(s)) if !s.trim().is_empty() => Base::Colour(s.trim().to_string()),
        Some(_) => return Err(bad(CMD, "base must be auto, none or a colour's name")),
    };
    Ok(Options { base, overlap_mm })
}

fn make(s: &mut Session, p: &Value) -> Result<Value> {
    let o = options(p)?;
    let ids = super::objects(s, p, CMD)?;
    if ids.is_empty() {
        return Err(bad(CMD, "select the design to make vinyl layers of"));
    }
    let d = &s.doc()?.doc;
    // The objects themselves, not ones inside another of them, in paint order.
    let chosen: std::collections::HashSet<NodeId> = ids.iter().copied().collect();
    let mut roots: Vec<(Vec<usize>, NodeId)> = ids
        .iter()
        .filter(|id| d.ancestry(**id).is_some_and(|a| !a.iter().rev().skip(1).any(|x| chosen.contains(x))))
        .filter_map(|id| Some((d.index_path(*id)?, *id)))
        .collect();
    roots.sort();
    let roots: Vec<NodeId> = roots.into_iter().map(|(_, id)| id).collect();
    let made = layered(d, &roots, &o).map_err(|e| bad(CMD, e))?;
    // Above the layer the topmost object is on.
    let above = roots.last().and_then(|id| d.layer_of(*id)).and_then(|l| d.layers.iter().position(|n| n.id == l)).map_or(d.layers.len(), |i| i + 1);
    let mut out = vec![];
    s.edit("Make Vinyl Layers", |d, sel| {
        let mut paths = vec![];
        for (i, v) in made.layers.iter().enumerate() {
            let label = if v.base { format!("{} {} (base)", i + 1, v.name) } else { format!("{} {}", i + 1, v.name) };
            let colour = LayerColor::Preset(u8::try_from(i % LAYER_COLORS.len()).unwrap_or(0));
            let layer = Node::layer(d.alloc_id(), &label, colour);
            let layer = d.insert(None, above + i, layer).map_err(|e| bad(CMD, e.to_string()))?;
            let mut n = Node::path(d.alloc_id(), v.region.clone(), Appearance::basic(v.paint.clone(), Paint::None, 0.0));
            n.name = Some(v.name.clone());
            let path = d.insert(Some(layer), usize::MAX, n).map_err(|e| bad(CMD, e.to_string()))?;
            paths.push(path);
            out.push(json!({"id": layer.0, "path": path.0, "name": v.name, "base": v.base}));
        }
        for id in &roots {
            d.remove(*id).map_err(|e| bad(CMD, e.to_string()))?;
        }
        let mut settings = vectorcraft_cutting::settings(d);
        settings.grouping = Grouping::Layer;
        vectorcraft_cutting::set_settings(d, &settings);
        sel.set(paths);
        Ok(())
    })?;
    Ok(json!({ "layers": out, "warnings": made.warnings }))
}
