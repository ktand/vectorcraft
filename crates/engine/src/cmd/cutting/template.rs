//! Fork: templates made to size (C15.1): a net from `vectorcraft_cutting::template` added as one
//! group on top: its outline set to Cut, its folds to Score or Perforate (in a colour of their own, so grouping
//! by colour can send them to a pen or an embosser), its glue areas marked and set to No Cut. The
//! group keeps the template and its values (`cut:template`), so with it alone selected the command
//! makes it again in place with other values. One undo step.

use std::sync::Arc;

use serde_json::{Map, Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_cutjob::Op;
use vectorcraft_cutting::ops::{NS, PREFIX, set_own_op};
use vectorcraft_cutting::placement::MM_PER_PT;
use vectorcraft_cutting::template::{self, KINDS, Kind, Values};
use vectorcraft_doc::{Appearance, Document, ForeignAttribute, Node, NodeId, NodeKind};
use vectorcraft_geom::{Affine, PathData};

use super::{CommandSpec, Result, Session, always, bad, check_numbers, cmd, has_doc};

const CMD: &str = "cutting.template";
const ATTR: &str = "template";
/// The folds' colour: one of their own.
pub const FOLD_COLOR: Color = Color::Rgb { r: 0.0, g: 0.45, b: 1.0 };
const GLUE_COLOR: Color = Color::Rgb { r: 0.85, g: 0.85, b: 0.85 };

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "cutting.templates",
            "Templates",
            [],
            None,
            "{} the templates cutting.template makes, in the panel's order → {templates: [{id, name, family, sized: whether it has inside and outside sizes, fits: whether it can be sized to art (fit), params: [{key, label, unit: mm|deg|count|toggle (true or false, or 1 or 0), default, min, max, help}]}]}",
            always,
            list
        ),
        cmd!(
            "cutting.template",
            "Add Template",
            [],
            None,
            "{kind?: a template's id (cutting.templates; required unless a template is selected), outside?: bool (default false: sizes are inside sizes, what goes in it; true: the finished net's), fit?: bool (default false; true: sized to the selected art (ids, else the selection): the face art goes on (a box's front or base, a card's face) as large as the art's bounds plus fitMarginMm each way, and placed under it), fitMarginMm?: 0–50 (default 2), folds?: score|perforate (default score: the folds scored; perforate: cut in dashes with the perforation settings, to fold or tear along), x?, y?: the net's top left, pt (default: centred on the cutting artboard), …the template's params (each in its range; mm unless it says)} a template's net added as one group on top: the outline set to Cut, the folds to Score (or Perforate) in a colour of their own, the glue areas marked and set to No Cut; with a template alone selected, made again in place (its kind, and the values not given, kept); one undo step → {id, kind, fitted, folds, widthMm, heightMm, replaced, warnings: why it won't cut as it is (the net off the mat's cut area)}",
            has_doc,
            add
        ),
    ]
}

fn list(_: &mut Session, _: &Value) -> Result<Value> {
    let templates: Vec<Value> = KINDS
        .iter()
        .map(|k| {
            let params: Vec<Value> = k
                .params
                .iter()
                .map(|p| json!({"key": p.key, "label": p.label, "unit": p.unit.id(), "default": p.default, "min": p.min, "max": p.max, "help": p.help}))
                .collect();
            json!({"id": k.id, "name": k.name, "family": k.family, "sized": k.sized, "fits": k.fit.is_some(), "params": params})
        })
        .collect();
    Ok(json!({ "templates": templates }))
}

/// What a template group keeps: its kind, values and top left (pt).
struct Made {
    kind: &'static Kind,
    values: Values,
    at: (f64, f64),
    /// How the folds are cut: scored, or perforated to tear or fold along.
    perforate: bool,
}

impl Made {
    fn to_json(&self) -> String {
        let values: Map<String, Value> = self.values.iter().map(|(k, v)| (k.to_string(), json!(v))).collect();
        json!({"kind": self.kind.id, "values": values, "outside": self.values.outside, "folds": fold_id(self.perforate), "at": [self.at.0, self.at.1]}).to_string()
    }
}

/// What template `n` was made as, if it is one.
fn made_as(n: &Node) -> Option<Made> {
    let v = &n.extras().attrs.as_deref()?.foreign.iter().find(|f| f.ns == NS && f.name == ATTR)?.value;
    let v: Value = serde_json::from_str(v).ok()?;
    let kind = template::kind(v.get("kind")?.as_str()?)?;
    let outside = v.get("outside").and_then(Value::as_bool).unwrap_or(false);
    let old = v.get("values");
    // A value the file holds out of range falls back to the default, as a file is read leniently.
    let values = kind
        .values(&|k| old.and_then(|o| o.get(k)).and_then(Value::as_f64), None, outside)
        .or_else(|_| kind.values(&|_| None, None, outside))
        .ok()?;
    let at = v.get("at").and_then(Value::as_array).and_then(|a| Some((a.first()?.as_f64()?, a.get(1)?.as_f64()?)));
    let at = at.filter(|(x, y)| x.is_finite() && y.is_finite())?;
    let perforate = v.get("folds").and_then(Value::as_str) == Some("perforate");
    Some(Made { kind, values, at, perforate })
}

fn fold_id(perforate: bool) -> &'static str {
    if perforate { "perforate" } else { "score" }
}

fn part(d: &mut Document, name: &str, path: PathData, appearance: Appearance, op: Op) -> Arc<Node> {
    let mut n = Node::path(d.alloc_id(), path, appearance);
    n.name = Some(name.to_string());
    set_own_op(&mut n, Some(op));
    Arc::new(n)
}

/// The cutting artboard's centre, pt.
fn artboard_centre(d: &Document) -> Option<(f64, f64)> {
    let s = vectorcraft_cutting::settings(d);
    let r = usize::try_from(s.artboard).ok().and_then(|i| d.artboards.get(i))?.rect;
    Some(((r.x0 + r.x1) / 2.0, (r.y0 + r.y1) / 2.0))
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    check_numbers(p, &["x", "y"], CMD)?;
    check_numbers(p, &["fitMarginMm"], CMD)?;
    let ids = super::objects(s, p, CMD)?;
    let d = &s.doc()?.doc;
    let fit = match p.get("fit") {
        None => false,
        Some(v) => v.as_bool().ok_or_else(|| bad(CMD, "fit must be true or false"))?,
    };
    // A template alone selected: made again where it is (unless it is being fitted to art).
    let again = match ids.as_slice() {
        [one] if !fit => d.node(*one).and_then(made_as).map(|m| (*one, m)),
        _ => None,
    };
    let kind = match (p.get("kind"), &again) {
        (Some(k), _) => {
            let id = k.as_str().ok_or_else(|| bad(CMD, "kind must be a template's id"))?;
            template::kind(id).ok_or_else(|| bad(CMD, format!("no template {id:?}: cutting.templates lists them")))?
        }
        (None, Some((_, m))) => m.kind,
        (None, None) => return Err(bad(CMD, "kind is required: cutting.templates lists them")),
    };
    let outside = match p.get("outside") {
        None => again.as_ref().is_some_and(|(_, m)| m.values.outside),
        Some(v) => v.as_bool().ok_or_else(|| bad(CMD, "outside must be true or false"))?,
    };
    let perforate = match p.get("folds") {
        None => again.as_ref().is_some_and(|(_, m)| m.perforate),
        Some(v) => match v.as_str() {
            Some("score") => false,
            Some("perforate") => true,
            _ => return Err(bad(CMD, "folds must be score or perforate")),
        },
    };
    // C15.7: sized to the selected art: its face the art's bounds plus the margin each way.
    let art = if fit {
        let margin = p.get("fitMarginMm").and_then(Value::as_f64).unwrap_or(2.0);
        if !(0.0..=50.0).contains(&margin) {
            return Err(bad(CMD, "fitMarginMm is 0–50"));
        }
        let keys = kind.fit.ok_or_else(|| bad(CMD, format!("the {} has no one face to size to art", kind.name.to_lowercase())))?;
        if ids.is_empty() {
            return Err(bad(CMD, "select the art to size it to"));
        }
        let b = d.bounds_of(&ids, true).filter(|b| b.width() > 0.0 && b.height() > 0.0).ok_or_else(|| bad(CMD, "the selection has no size"))?;
        let size = (b.width() * MM_PER_PT + 2.0 * margin, b.height() * MM_PER_PT + 2.0 * margin);
        Some((keys, size, ((b.x0 + b.x1) / 2.0, (b.y0 + b.y1) / 2.0)))
    } else {
        None
    };
    // A switch may come as true or false, or as 1 or 0; a fitted face's sizes are the art's.
    let given = |key: &str| {
        if let Some(((kw, kh), (w, h), _)) = art {
            if key == kw {
                return Some(w);
            }
            if key == kh {
                return Some(h);
            }
        }
        p.get(key).map(|v| v.as_bool().map_or_else(|| v.as_f64().unwrap_or(f64::NAN), |b| f64::from(u8::from(b))))
    };
    let keys: Vec<&str> = kind.params.iter().filter(|q| q.unit != template::Unit::Toggle).map(|q| q.key).collect();
    check_numbers(p, &keys, CMD)?;
    let fallback = again.as_ref().filter(|(_, m)| m.kind.id == kind.id).map(|(_, m)| &m.values);
    let values = kind.values(&given, fallback, outside).map_err(|e| bad(CMD, e))?;
    let net = kind.make(&values).map_err(|e| bad(CMD, e))?;
    let (w, h) = net.size();
    let (wpt, hpt) = (w / MM_PER_PT, h / MM_PER_PT);
    // Fitted: the face centred on the art.
    let fitted_at = match (art, net.face) {
        (Some((_, _, (cx, cy))), Some(f)) => Some((cx - (f[0] + f[2]) / 2.0 / MM_PER_PT, cy - (f[1] + f[3]) / 2.0 / MM_PER_PT)),
        _ => None,
    };
    let at = match (p.get("x").and_then(Value::as_f64), p.get("y").and_then(Value::as_f64), &again) {
        (Some(x), Some(y), _) => (x, y),
        (None, None, None) if fitted_at.is_some() => fitted_at.unwrap_or_default(),
        // Where the template is now, if it was moved since it was made.
        (None, None, Some((old, m))) => d.bounds_of(&[*old], false).map_or(m.at, |b| (b.x0, b.y0)),
        (None, None, None) => {
            let (cx, cy) = artboard_centre(d).ok_or_else(|| bad(CMD, "the cutting artboard isn't in the document"))?;
            (cx - wpt / 2.0, cy - hpt / 2.0)
        }
        _ => return Err(bad(CMD, "give both x and y, or neither")),
    };
    let to_doc = Affine::translate(at) * Affine::scale(1.0 / MM_PER_PT);
    let (cut, folds, glue) = (net.cut.transformed(to_doc), net.folds.transformed(to_doc), net.glue.transformed(to_doc));
    let warnings = super::popup::off_the_mat(d, &cut, "net");
    let top = match &again {
        Some((old, _)) => d.layer_of(*old),
        None => d.layers.last().map(|l| l.id),
    }
    .ok_or_else(|| bad(CMD, "the document has no layer"))?;
    let made = Made { kind, values, at, perforate }.to_json();
    let fold_op = if perforate { Op::Perforate } else { Op::Score };
    let mut id: Option<NodeId> = None;
    s.edit(if again.is_some() { "Remake Template" } else { "Add Template" }, |d, _| {
        let parts = vec![
            part(d, "Glue", glue.clone(), Appearance::basic(Paint::solid(GLUE_COLOR), Paint::None, 0.0), Op::NoCut),
            part(d, "Cut", cut.clone(), Appearance::basic(Paint::None, Paint::solid(Color::BLACK), 0.5), Op::Cut),
            part(d, "Folds", folds.clone(), Appearance::basic(Paint::None, Paint::solid(FOLD_COLOR), 0.5), fold_op),
        ];
        let attr = ForeignAttribute { ns: NS.to_string(), prefix: PREFIX.to_string(), name: ATTR.to_string(), value: made.clone() };
        if let Some((old, _)) = &again
            && let Some(n) = d.node_mut(*old)
            && let NodeKind::Group { children, .. } = &mut n.kind
        {
            *children = parts;
            n.name = Some(kind.name.to_string());
            n.edit_attrs(|a| {
                a.foreign.retain(|f| !(f.ns == NS && f.name == ATTR));
                a.foreign.push(attr);
            });
            id = Some(*old);
            return Ok(());
        }
        let mut g = Node::group(d.alloc_id(), parts);
        g.name = Some(kind.name.to_string());
        g.edit_attrs(|a| a.foreign.push(attr));
        id = Some(d.insert(Some(top), usize::MAX, g).map_err(|e| bad(CMD, e.to_string()))?);
        Ok(())
    })?;
    let id = id.ok_or_else(|| bad(CMD, "the template wasn't added"))?;
    Ok(
        json!({ "id": id.0, "kind": kind.id, "fitted": art.is_some(), "folds": fold_id(perforate), "widthMm": w, "heightMm": h, "replaced": again.is_some(), "warnings": warnings }),
    )
}
