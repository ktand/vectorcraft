//! Fork: puzzles (C13.5): a jigsaw's cut lines (`vectorcraft_cutting::puzzle`) over the selected
//! objects' bounds (a printed picture, say) or, with nothing selected, the cutting artboard, added
//! as one path on top and set to Cut. The art is left as it is, to be printed. The path keeps the
//! rectangle it covers (`cut:puzzle`), so with it alone selected the command makes it again in
//! place: other settings, or another seed (Randomize). One undo step.

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_cutjob::Op;
use vectorcraft_cutting::ops::{NS, PREFIX, set_own_op};
use vectorcraft_cutting::puzzle::{Options, puzzle};
use vectorcraft_doc::{Appearance, Document, ForeignAttribute, Node, NodeId, NodeKind};
use vectorcraft_geom::Rect;

use super::{CommandSpec, Result, Session, bad, check_numbers, cmd, has_doc};

const CMD: &str = "cutting.puzzle";
/// The puzzle's name, and the attribute that keeps the rectangle it covers.
pub const NAME: &str = "Puzzle";
const ATTR: &str = "puzzle";

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.puzzle",
        "Add Puzzle",
        [],
        None,
        "{ids?, rows?: 2–50 (default 4), cols?: 2–50 (default 4; at most 2500 pieces), curvature?: bool (default true; false: a plain grid), curve?: 0–100 (default 100: the knobs' size, %), ripple?: 0–100 (default 100: how much each side waves either side of its knob, %), wobble?: 0–100 (default 100: how far the grid's corners and the knobs stray from even, %), seed?: whole number (default 1: the same settings and seed make the same puzzle)} a jigsaw's cut lines over the objects' bounds (ids, else the selected ones; with none, the cutting artboard): the border and the lines between the pieces, a knob on each piece's side; added as one path on top (set to Cut), the art left as it is; with a puzzle alone selected, that puzzle made again in place with these settings; one undo step → {id, pieces, replaced}",
        has_doc,
        add
    )]
}

fn whole(p: &Value, key: &str, default: u32) -> Result<u32> {
    match p.get(key) {
        None => Ok(default),
        Some(v) => v.as_u64().and_then(|n| u32::try_from(n).ok()).ok_or_else(|| bad(CMD, format!("{key} must be a whole number"))),
    }
}

/// `p[key]` as a share, from 0–100 %.
fn share(p: &Value, key: &str) -> f64 {
    p.get(key).and_then(Value::as_f64).map_or(1.0, |v| v / 100.0)
}

fn options(p: &Value) -> Result<Options> {
    check_numbers(p, &["rows", "cols", "curve", "ripple", "wobble", "seed"], CMD)?;
    let curvature = match p.get("curvature") {
        None => true,
        Some(v) => v.as_bool().ok_or_else(|| bad(CMD, "curvature must be true or false"))?,
    };
    Ok(Options {
        rows: whole(p, "rows", 4)?,
        cols: whole(p, "cols", 4)?,
        curvature,
        curve: share(p, "curve"),
        ripple: share(p, "ripple"),
        wobble: share(p, "wobble"),
        seed: whole(p, "seed", 1)?,
    })
}

/// The rectangle puzzle `n` was made over, if it is one.
fn made_over(n: &Node) -> Option<Rect> {
    let v = &n.extras().attrs.as_deref()?.foreign.iter().find(|f| f.ns == NS && f.name == ATTR)?.value;
    let r: Vec<f64> = serde_json::from_str::<Vec<f64>>(v).ok()?;
    let [x0, y0, x1, y1] = r.as_slice() else { return None };
    let r = Rect::new(*x0, *y0, *x1, *y1);
    (r.width() > 0.0 && r.height() > 0.0 && r.width().is_finite() && r.height().is_finite()).then_some(r)
}

/// The rectangle a new puzzle covers: the objects' bounds, else the cutting artboard.
fn area(d: &Document, ids: &[NodeId]) -> Result<Rect> {
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

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    let opt = options(p)?;
    let ids = super::objects(s, p, CMD)?;
    let d = &s.doc()?.doc;
    // A puzzle alone selected: made again where it is.
    let again = match ids.as_slice() {
        [one] => d.node(*one).and_then(made_over).map(|r| (*one, r)),
        _ => None,
    };
    let rect = match again {
        Some((_, r)) => r,
        None => area(d, &ids)?,
    };
    let lines = puzzle(rect, opt).map_err(|e| bad(CMD, e))?;
    let top = match ids.last() {
        Some(last) => d.layer_of(*last),
        None => d.layers.last().map(|l| l.id),
    }
    .ok_or_else(|| bad(CMD, "the document has no layer"))?;
    let made = json!([rect.x0, rect.y0, rect.x1, rect.y1]).to_string();
    let mut id = None;
    s.edit(if again.is_some() { "Randomize Puzzle" } else { "Add Puzzle" }, |d, _| {
        if let Some((old, _)) = again {
            if let Some(n) = d.node_mut(old)
                && let NodeKind::Path { path, .. } = &mut n.kind
            {
                *path = lines.clone();
            }
            id = Some(old);
            return Ok(());
        }
        let mut n = Node::path(d.alloc_id(), lines.clone(), Appearance::basic(Paint::None, Paint::solid(Color::BLACK), 0.5));
        n.name = Some(NAME.to_string());
        set_own_op(&mut n, Some(Op::Cut));
        n.edit_attrs(|a| {
            a.foreign.push(ForeignAttribute { ns: NS.to_string(), prefix: PREFIX.to_string(), name: ATTR.to_string(), value: made.clone() })
        });
        id = Some(d.insert(Some(top), usize::MAX, n).map_err(|e| bad(CMD, e.to_string()))?);
        Ok(())
    })?;
    let id = id.ok_or_else(|| bad(CMD, "the puzzle wasn't added"))?;
    Ok(json!({ "id": id.0, "pieces": opt.rows * opt.cols, "replaced": again.is_some() }))
}
