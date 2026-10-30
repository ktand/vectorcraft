//! Fork: a document measured in pixels (C11.3, ARCHITECTURE R1). An SVG with no units, or in
//! `px`, opens at 1 px = 1 pt, 72 per inch; most such files were drawn at 96 px/in, so they cut a
//! third larger than their author meant. `cutting.scaleFromPixels` shrinks the whole document to
//! the size its author's pixels give, and measures it in millimetres from then on.

use serde_json::{Value, json};
use vectorcraft_doc::{Document, Scaling, Unit};
use vectorcraft_geom::{Affine, Rect};

use super::{CommandSpec, Result, Session, bad, check_numbers, cmd, f64_or};

const CMD: &str = "cutting.scaleFromPixels";
/// Points per inch: what the document's pixels are now.
const PT_PER_IN: f64 = 72.0;
/// Most SVGs' pixels (CSS's).
pub const DEFAULT_PX_PER_IN: f64 = 96.0;
/// The pixel sizes accepted, per inch.
const PX_PER_IN: std::ops::RangeInclusive<f64> = 1.0..=2400.0;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.scaleFromPixels",
        "Scale from Pixels",
        [],
        None,
        "{pxPerInch?: 1–2400 (default 96)} a document measured in pixels (1 px = 1 pt, 72 per inch, as an SVG with no units opens) scaled about the origin by 72 / pxPerInch: its art (strokes, effects and patterns with it), artboards, guides and ruler origins, so each pixel is 1/pxPerInch in; the document then measures in millimetres; one undo step → {scale, units}",
        in_pixels,
        scale
    )]
}

fn in_pixels(s: &Session) -> std::result::Result<(), String> {
    let st = s.doc().map_err(|e| e.to_string())?;
    if st.doc.units == Unit::Pixels { Ok(()) } else { Err("the document isn't measured in pixels".into()) }
}

fn scale(s: &mut Session, p: &Value) -> Result<Value> {
    check_numbers(p, &["pxPerInch"], CMD)?;
    let px_per_in = f64_or(p, "pxPerInch", DEFAULT_PX_PER_IN);
    if !PX_PER_IN.contains(&px_per_in) {
        return Err(bad(CMD, format!("pxPerInch must be 1 to 2400, not {px_per_in}")));
    }
    in_pixels(s).map_err(|e| bad(CMD, e))?;
    let k = PT_PER_IN / px_per_in;
    s.edit("Scale from Pixels", |d, _| {
        scale_document(d, k);
        d.units = Unit::Millimeters;
        Ok(())
    })?;
    Ok(json!({ "scale": k, "units": "mm" }))
}

/// Everything in `d` that has a place on the canvas, scaled by `k` about the origin.
fn scale_document(d: &mut Document, k: f64) {
    let a = Affine::scale(k);
    let sc = Scaling { strokes: true, effects: Some(vectorcraft_render::effects::scale_effect), patterns: true, ..Scaling::default() };
    let layers: Vec<_> = d.layers.iter().map(|l| l.id).collect();
    for id in layers {
        if let Some(n) = d.node_mut(id) {
            n.transform(a, sc.clone());
        }
    }
    for b in &mut d.artboards {
        let r = b.rect;
        b.rect = Rect::new(r.x0 * k, r.y0 * k, r.x1 * k, r.y1 * k);
    }
    for g in &mut d.guides {
        g.pos *= k;
    }
    d.ruler_origin = (d.ruler_origin.to_vec2() * k).to_point();
    for o in d.artboard_ruler_origins.values_mut() {
        *o *= k;
    }
}
