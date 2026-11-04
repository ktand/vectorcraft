//! Fork: a sticker sheet in one step (C18.5): the selected designs made ready to print and cut as
//! stickers, one undo step: Print & Cut on, a contour round each design (the kiss cut, through the
//! sticker and not its backing, with the document's cut settings), a print bleed (so a cut a
//! little off the print lands in colour), and optionally a cut round the whole sheet on a layer
//! of its own (the Sheet Cut layer, cut last with its own, stronger setup: through the backing).
//! The job is then cut by layer, so the sheet cut gets its own setup.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_cutjob::{GroupSetup, Grouping, Op};
use vectorcraft_cutting::contour::{Options, contour, contour_layer, contour_layer_node, contour_node};
use vectorcraft_cutting::ops::{NS, PREFIX, set_own_op};
use vectorcraft_cutting::placement::MM_PER_PT;
use vectorcraft_doc::{Appearance, ForeignAttribute, LayerColor, Node};
use vectorcraft_geom::{Rect, shapes};

use super::{CommandSpec, Result, Session, bad, check_numbers, cmd, has_doc};

const CMD: &str = "cutting.stickers";
/// The sheet cut's layer.
pub const SHEET_LAYER: &str = "Sheet Cut";
const SHEET_ATTR: &str = "sheetCut";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.stickers",
        "Make Sticker Sheet",
        [],
        None,
        "{ids?, offsetMm?: 0–20 (default 1.5: the kiss cut's distance round each design), bleed?: bool (default true: a print bleed under the art), sheetCut?: bool (default false: also cut round the whole sheet, through the backing), sheetMarginMm?: 2–50 (default 5: the sheet cut's distance round the stickers)} the designs (ids, else the selection) made into a sticker sheet, one undo step: Print & Cut on; a contour round each design on the Cut Contour layer (the kiss cut, the document's cut settings), the art itself set to No Cut; a print bleed; with sheetCut, a rectangle round the stickers on the Sheet Cut layer, cut last with its own setup (force doubled, two passes), the job then cut by layer → {contour, pieces, sheetCut: its id or null}",
        has_doc,
        add
    )]
}

fn flag(p: &Value, key: &str, default: bool) -> Result<bool> {
    match p.get(key) {
        None => Ok(default),
        Some(v) => v.as_bool().ok_or_else(|| bad(CMD, format!("{key} must be true or false"))),
    }
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    check_numbers(p, &["offsetMm", "sheetMarginMm"], CMD)?;
    let offset_mm = p.get("offsetMm").and_then(Value::as_f64).unwrap_or(1.5);
    let margin_mm = p.get("sheetMarginMm").and_then(Value::as_f64).unwrap_or(5.0);
    if !(0.0..=20.0).contains(&offset_mm) || !(2.0..=50.0).contains(&margin_mm) {
        return Err(bad(CMD, "offsetMm is 0–20, sheetMarginMm 2–50"));
    }
    let (bleed, sheet) = (flag(p, "bleed", true)?, flag(p, "sheetCut", false)?);
    let ids = super::objects(s, p, CMD)?;
    if ids.is_empty() {
        return Err(bad(CMD, "select the designs to make stickers of"));
    }
    let d = &s.doc()?.doc;
    let path = contour(d, &ids, Options { offset_mm, holes: false }).map_err(|e| bad(CMD, e))?;
    let pieces = path.subpaths.len();
    let around = path.bounds().ok_or_else(|| bad(CMD, "the designs have no size"))?;
    let m = margin_mm / MM_PER_PT;
    let sheet_rect = Rect::new(around.x0 - m, around.y0 - m, around.x1 + m, around.y1 + m);
    let mut out = (None, None);
    s.edit("Make Sticker Sheet", |d, _| {
        let mut settings = vectorcraft_cutting::settings(d);
        settings.print_and_cut = true;
        if sheet {
            settings.grouping = Grouping::Layer;
        }
        vectorcraft_cutting::set_settings(d, &settings);
        // The kiss cut.
        let layer = match contour_layer(d) {
            Some(l) => l,
            None => {
                let node = contour_layer_node(d);
                d.insert(None, usize::MAX, node).map_err(|e| bad(CMD, e.to_string()))?
            }
        };
        let node = contour_node(d, path.clone());
        let contour_id = d.insert(Some(layer), usize::MAX, node).map_err(|e| bad(CMD, e.to_string()))?;
        for id in &ids {
            if let Some(n) = d.node_mut(*id) {
                set_own_op(n, Some(Op::NoCut));
            }
        }
        if bleed {
            if let Some(old) = vectorcraft_cutting::bleed::bleed_layer(d) {
                d.remove(old).map_err(|e| bad(CMD, e.to_string()))?;
            }
            let (node, _) = vectorcraft_cutting::bleed::bleed_layer_node(d, settings.bleed_mm).map_err(|e| bad(CMD, e))?;
            d.insert(None, 0, node).map_err(|e| bad(CMD, e.to_string()))?;
            vectorcraft_cutting::bleed::prune_images(d);
        }
        // The sheet cut: its own layer on top, so it is cut last, with a stronger setup.
        let mut sheet_id = None;
        if sheet {
            let old = d
                .layers
                .iter()
                .find(|l| l.extras().attrs.as_deref().is_some_and(|a| a.foreign.iter().any(|f| f.ns == NS && f.name == SHEET_ATTR)))
                .map(|l| l.id);
            if let Some(old) = old {
                d.remove(old).map_err(|e| bad(CMD, e.to_string()))?;
            }
            let mut layer = Node::layer(d.alloc_id(), SHEET_LAYER, LayerColor::Preset(3));
            set_own_op(&mut layer, Some(Op::Cut));
            layer.edit_attrs(|a| {
                a.foreign.push(ForeignAttribute {
                    ns: NS.to_string(),
                    prefix: PREFIX.to_string(),
                    name: SHEET_ATTR.to_string(),
                    value: "1".to_string(),
                })
            });
            let through = GroupSetup {
                material: None,
                cut: vectorcraft_cutjob::ToolSettings {
                    force: (settings.operations.cut.force * 2.0).min(1.0),
                    passes: 2,
                    ..settings.operations.cut.clone()
                },
                score: settings.operations.score.clone(),
            };
            vectorcraft_cutting::setups::set_layer_setup(&mut layer, Some(&through));
            let layer = d.insert(None, usize::MAX, layer).map_err(|e| bad(CMD, e.to_string()))?;
            let rect = Node::path(
                d.alloc_id(),
                shapes::rectangle(sheet_rect),
                Appearance::basic(Paint::None, Paint::solid(Color::Rgb { r: 0.0, g: 0.6, b: 0.2 }), 0.5),
            );
            sheet_id = Some(d.insert(Some(layer), usize::MAX, rect).map_err(|e| bad(CMD, e.to_string()))?);
        }
        out = (Some(contour_id), sheet_id);
        Ok(())
    })?;
    let contour_id = out.0.ok_or_else(|| bad(CMD, "the stickers weren't made"))?;
    Ok(json!({ "contour": contour_id.0, "pieces": pieces, "sheetCut": out.1.map(|i| i.0) }))
}
