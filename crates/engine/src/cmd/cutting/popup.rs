//! Fork: pop-up cards (C16.1): the selected design made into a pop-up card
//! (`vectorcraft_cutting::popup`): the card, the face's cut and the folds' dashes, added as one
//! group on top, each set to Cut; the design itself set to No Cut, left to be printed (the face's
//! cut goes round it). The group keeps the design's ids and the settings (`cut:popup`), so with it
//! alone selected the command makes it again in place with other settings. One undo step.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_cutjob::Op;
use vectorcraft_cutting::ops::{NS, PREFIX, set_own_op};
use vectorcraft_cutting::popup::{BASE_PCT, DASH_MM, MARGIN_MM, Options, STRUT_MM, popup};
use vectorcraft_doc::{Appearance, ForeignAttribute, Node, NodeId, NodeKind};
use vectorcraft_geom::PathData;

use super::{CommandSpec, Result, Session, bad, check_numbers, cmd, has_doc};

const CMD: &str = "cutting.popup";
/// The group's name, and the attribute that keeps what it was made from.
pub const NAME: &str = "Pop Up";
const ATTR: &str = "popup";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.popup",
        "Make Pop Up",
        [],
        None,
        "{ids?, baseWidthPct?: 2–50 (default 10: the base under the design, % of its height), strutWidthMm?: 0.5–50 (default 2), dashPitchMm?: 0.25–10 (default 1: each dash of a fold, and the gap after it), marginMm?: 0–200 (default 10: the card round the pop-up)} the design (ids, else the selection; each a layer of its own, a design lower on the sheet standing further forward; group objects to stand them up together) made into a pop-up card that stands up when opened at a right angle: a group on top with the card, the design's cut (open where it joins the card) and the folds as dashes, each set to Cut, and the design set to No Cut (left to be printed); with a pop-up alone selected, made again in place from the same design; one undo step → {id, layers, struts, replaced}",
        has_doc,
        add
    )]
}

fn options(p: &Value) -> Result<Options> {
    check_numbers(p, &["baseWidthPct", "strutWidthMm", "dashPitchMm", "marginMm"], CMD)?;
    let d = Options::default();
    let get = |k: &str, v: f64| p.get(k).and_then(Value::as_f64).unwrap_or(v);
    let opt = Options {
        base_pct: get("baseWidthPct", d.base_pct),
        strut_mm: get("strutWidthMm", d.strut_mm),
        dash_mm: get("dashPitchMm", d.dash_mm),
        margin_mm: get("marginMm", d.margin_mm),
    };
    let within = |v: f64, (lo, hi): (f64, f64)| (lo..=hi).contains(&v);
    if !(within(opt.base_pct, BASE_PCT) && within(opt.strut_mm, STRUT_MM) && within(opt.dash_mm, DASH_MM) && within(opt.margin_mm, MARGIN_MM)) {
        return Err(bad(
            CMD,
            format!(
                "baseWidthPct is {}–{}, strutWidthMm {}–{}, dashPitchMm {}–{}, marginMm {}–{}",
                BASE_PCT.0, BASE_PCT.1, STRUT_MM.0, STRUT_MM.1, DASH_MM.0, DASH_MM.1, MARGIN_MM.0, MARGIN_MM.1
            ),
        ));
    }
    Ok(opt)
}

/// The design pop-up `n` was made from, if it is one.
fn made_from(n: &Node) -> Option<Vec<NodeId>> {
    let v = &n.extras().attrs.as_deref()?.foreign.iter().find(|f| f.ns == NS && f.name == ATTR)?.value;
    let ids: Vec<u64> = serde_json::from_str(v).ok()?;
    Some(ids.into_iter().map(NodeId).collect())
}

fn line(d: &mut vectorcraft_doc::Document, name: &str, path: PathData, color: Color) -> Arc<Node> {
    let mut n = Node::path(d.alloc_id(), path, Appearance::basic(Paint::None, Paint::solid(color), 0.5));
    n.name = Some(name.to_string());
    set_own_op(&mut n, Some(Op::Cut));
    Arc::new(n)
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    let opt = options(p)?;
    let ids = super::objects(s, p, CMD)?;
    let d = &s.doc()?.doc;
    // A pop-up alone selected: made again from its design.
    let again = match ids.as_slice() {
        [one] => d.node(*one).and_then(made_from).map(|from| (*one, from)),
        _ => None,
    };
    let design = match &again {
        Some((_, from)) => from.clone(),
        None => ids.clone(),
    };
    if design.is_empty() {
        return Err(bad(CMD, "select the design to pop up"));
    }
    let made = popup(d, &design, opt).map_err(|e| bad(CMD, e))?;
    let top = design.last().and_then(|last| d.layer_of(*last)).ok_or_else(|| bad(CMD, "the design isn't on a layer"))?;
    let keep = json!(design.iter().map(|i| i.0).collect::<Vec<_>>()).to_string();
    let mut id = None;
    s.edit(if again.is_some() { "Remake Pop Up" } else { "Make Pop Up" }, |d, _| {
        let parts = vec![
            line(d, "Card", made.card.clone(), Color::BLACK),
            line(d, "Pop Up Cut", made.cuts.clone(), Color::BLACK),
            line(d, "Folds", made.folds.clone(), Color::Rgb { r: 1.0, g: 0.0, b: 1.0 }),
        ];
        if let Some((old, _)) = &again {
            if let Some(n) = d.node_mut(*old)
                && let NodeKind::Group { children, .. } = &mut n.kind
            {
                *children = parts;
            }
            id = Some(*old);
            return Ok(());
        }
        for i in &design {
            if let Some(n) = d.node_mut(*i) {
                set_own_op(n, Some(Op::NoCut));
            }
        }
        let mut g = Node::group(d.alloc_id(), parts);
        g.name = Some(NAME.to_string());
        g.edit_attrs(|a| {
            a.foreign.push(ForeignAttribute { ns: NS.to_string(), prefix: PREFIX.to_string(), name: ATTR.to_string(), value: keep.clone() })
        });
        id = Some(d.insert(Some(top), usize::MAX, g).map_err(|e| bad(CMD, e.to_string()))?);
        Ok(())
    })?;
    let id = id.ok_or_else(|| bad(CMD, "the pop-up wasn't added"))?;
    Ok(json!({ "id": id.0, "layers": made.layers, "struts": made.struts, "replaced": again.is_some() }))
}
