//! Fork: batches from a data source (C18.4). The selected design is the template; each of the
//! document's data sets (upstream's Variables, `variablecmds.rs`) makes one copy of it, grouped
//! and named after the record, with the record's values in the copy's bound objects: a text
//! variable's text, a visibility variable's showing. The copies are nested onto as few sheets as
//! hold them (C8.6: new artboards past the first, each cut on its own with Next Sheet), and the
//! template is hidden so it isn't cut with them. A record too large for the mat is refused by
//! name before anything changes. One undo step.

use std::collections::BTreeMap;
use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_cutting::nest::{Options, nest_sheets};
use vectorcraft_doc::{DataValue, Document, Node, NodeId, VariableKind};

use super::{CommandSpec, Result, Session, bad, check_numbers, cmd, has_doc};

const CMD: &str = "cutting.batch";
/// The most records one batch makes.
pub const MAX_RECORDS: usize = 500;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.batch",
        "Batch from Data Sets",
        [],
        None,
        "{ids?: the template (default: the selection), datasets?: [names] (default: every data set, in order), gapMm?: 0–50 (default 2), rotate?: bool (default true)} one copy of the template for each data set (Window › Variables), grouped and named after it, the record's text in the copy's objects bound to its text variables and its showing in those bound to its visibility variables; the copies nested onto as few sheets as hold them (new artboards Sheet 2, Sheet 3… past the cutting artboard, each cut on its own), the template hidden; a record too large for the mat refused by name; one undo step → {records: [{name, id}], sheets: [artboards used], created: new sheets}",
        has_doc,
        batch
    )]
}

/// Each node under `original` with the node in the same place under `copy` (`copy` is
/// `original` given new ids).
fn pairs(original: &Node, copy: &Node, out: &mut BTreeMap<NodeId, NodeId>) {
    out.insert(original.id, copy.id);
    if let (Some(a), Some(b)) = (original.children(), copy.children()) {
        for (x, y) in a.iter().zip(b) {
            pairs(x, y, out);
        }
    }
}

/// Every node in and under `n`.
fn under(n: &Node, out: &mut Vec<NodeId>) {
    out.push(n.id);
    for c in n.children().into_iter().flatten() {
        under(c, out);
    }
}

/// The template's bound objects: (object, variable, kind), text and visibility variables only.
fn bindings(d: &Document, ids: &[NodeId]) -> Vec<(NodeId, String, VariableKind)> {
    let mut nodes = vec![];
    for id in ids {
        if let Some(n) = d.node(*id) {
            under(n, &mut nodes);
        }
    }
    let mut out = vec![];
    for id in nodes {
        for var in d.variables.bindings.get(&id).into_iter().flatten() {
            if let Some(v) = d.variables.variable(var)
                && matches!(v.kind, VariableKind::Text | VariableKind::Visibility)
            {
                out.push((id, var.clone(), v.kind));
            }
        }
    }
    out
}

fn batch(s: &mut Session, p: &Value) -> Result<Value> {
    check_numbers(p, &["gapMm"], CMD)?;
    let gap = p.get("gapMm").and_then(Value::as_f64).unwrap_or(2.0);
    if !(0.0..=50.0).contains(&gap) {
        return Err(bad(CMD, format!("gapMm must be 0 to 50, not {gap}")));
    }
    let rotate = match p.get("rotate") {
        None => true,
        Some(v) => v.as_bool().ok_or_else(|| bad(CMD, "rotate must be true or false"))?,
    };
    let ids = super::objects(s, p, CMD)?;
    if ids.is_empty() {
        return Err(bad(CMD, "select the design to make the batch from"));
    }
    let d = &s.doc()?.doc;
    let bound = bindings(d, &ids);
    if bound.is_empty() {
        return Err(bad(
            CMD,
            "nothing in the design is bound to a text or visibility variable (Window › Variables): bind the text that changes from record to record first",
        ));
    }
    let names: Vec<String> = match p.get("datasets") {
        None => d.variables.datasets.iter().map(|ds| ds.name.clone()).collect(),
        Some(Value::Array(a)) => {
            a.iter().map(|v| v.as_str().map(str::to_string).ok_or_else(|| bad(CMD, "datasets must be names"))).collect::<Result<_>>()?
        }
        Some(_) => return Err(bad(CMD, "datasets must be a list of names")),
    };
    if names.is_empty() {
        return Err(bad(CMD, "the document has no data sets (Window › Variables › New Data Set)"));
    }
    if names.len() > MAX_RECORDS {
        return Err(bad(CMD, format!("at most {MAX_RECORDS} records in one batch")));
    }
    let records: Vec<vectorcraft_doc::DataSet> =
        names.iter().map(|n| d.variables.dataset(n).cloned().ok_or_else(|| bad(CMD, format!("no data set named {n:?}")))).collect::<Result<_>>()?;
    let layer = ids.last().and_then(|id| d.layer_of(*id)).ok_or_else(|| bad(CMD, "the design isn't on a layer"))?;
    let opt = Options { gap_mm: gap, rotate, fine: false };
    let mut made: Vec<(String, NodeId)> = vec![];
    let mut nested = None;
    s.edit("Batch from Data Sets", |d, sel| {
        let originals: Vec<Node> = ids.iter().filter_map(|id| d.node(*id).cloned()).collect();
        for record in &records {
            let mut map = BTreeMap::new();
            let copies: Vec<Arc<Node>> = originals
                .iter()
                .map(|o| {
                    let c = d.reid(o);
                    pairs(o, &c, &mut map);
                    let mut c = c;
                    c.visible = true;
                    Arc::new(c)
                })
                .collect();
            let mut g = Node::group(d.alloc_id(), copies);
            g.name = Some(record.name.clone());
            let gid = d.insert(Some(layer), usize::MAX, g).map_err(|e| bad(CMD, e.to_string()))?;
            for (id, var, kind) in &bound {
                let (Some(copy), Some(value)) = (map.get(id), record.values.get(var)) else { continue };
                match (kind, value) {
                    (VariableKind::Text, DataValue::Text(text)) => {
                        super::super::textedit::set_story_text(d, *copy, text)?;
                    }
                    (VariableKind::Visibility, DataValue::Visible(show)) => {
                        if let Some(n) = d.node_mut(*copy) {
                            n.visible = *show;
                        }
                    }
                    _ => {}
                }
            }
            made.push((record.name.clone(), gid));
        }
        // The template isn't cut with its copies.
        for id in &ids {
            if let Some(n) = d.node_mut(*id) {
                n.visible = false;
            }
        }
        sel.clear();
        let groups: Vec<NodeId> = made.iter().map(|(_, id)| *id).collect();
        let n = nest_sheets(d, &groups, opt).map_err(|e| bad(CMD, e))?;
        if let Some((name, _)) = made.iter().find(|(_, id)| n.unplaced.contains(id)) {
            return Err(bad(CMD, format!("record {name:?} doesn't fit on the mat's cut area: make the design smaller or the text shorter")));
        }
        nested = Some(n);
        Ok(())
    })?;
    let n = nested.ok_or_else(|| bad(CMD, "nothing was made"))?;
    Ok(json!({
        "records": made.iter().map(|(name, id)| json!({"name": name, "id": id.0})).collect::<Vec<_>>(),
        "sheets": n.sheets,
        "created": n.created,
    }))
}
