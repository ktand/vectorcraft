//! Fork: a design made into layers for layered vinyl (C18.30). Each colour the design paints
//! becomes one filled shape, what shows of it: fills and strokes alike (a stroke as its outline,
//! as drawn), type as its glyphs, hidden parts taken out and touching parts of one colour joined
//! (Pathfinder Merge). With a base colour, that colour's shape is the whole design's silhouette
//! instead: one piece with no holes but the design's own, which every other colour sits on, so
//! the base colour's lines and dots elsewhere are holes in the colours above it and never cut
//! as pieces of their own. The base is the colour along most of the design's outer edge unless
//! one is named; without a base each colour is just what shows of it. An overlap grows each
//! colour under the colours stacked above it, so no gap shows when the layers are a little off.
//!
//! Gradients and patterns are each one colour, named by their kind; images and meshes paint
//! nothing to cut and are left out, with a warning.

use vectorcraft_color::Paint;
use vectorcraft_doc::{Document, NodeId};
use vectorcraft_geom::{FillRule, PathData};
use vectorcraft_pathops::{
    BoolOp, Cap, DEFAULT_PRECISION, Join, PathfinderOp, Shape, area, outline_stroke, pathfinder_with, try_boolean, try_unite_all,
};

use crate::geometry::{Painted, painted};
use crate::placement::MM_PER_PT;

/// Most painted areas one design may have (each fill and each stroke of each path).
pub const MAX_SHAPES: usize = 5_000;
/// The overlap's range, mm.
pub const OVERLAP_MM: (f64, f64) = (0.0, 3.0);
/// Areas smaller than this, square points, are left out (slivers the path operations leave).
const MIN_AREA_PT2: f64 = 1e-4;

/// Which colour is the base.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Base {
    /// The colour along most of the design's outer edge.
    Auto,
    /// No base: each colour what shows of it.
    None,
    /// The colour with this name ([`colour_name`]: a swatch's name or `#rrggbb`).
    Colour(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Options {
    pub base: Base,
    /// How far each colour runs under the colours above it, mm.
    pub overlap_mm: f64,
}

impl Default for Options {
    fn default() -> Self {
        Options { base: Base::Auto, overlap_mm: 0.0 }
    }
}

/// One layer of vinyl: one colour, bottom first.
#[derive(Clone, Debug, PartialEq)]
pub struct VinylLayer {
    /// The colour's name ([`colour_name`]).
    pub name: String,
    pub paint: Paint,
    /// What it covers, non-zero, document points.
    pub region: PathData,
    /// Whether it is the base: the design's silhouette.
    pub base: bool,
}

/// The layers, bottom first, and what was left out.
#[derive(Clone, Debug, PartialEq)]
pub struct Layered {
    pub layers: Vec<VinylLayer>,
    pub warnings: Vec<String>,
}

/// A paint's name: a swatch's name where it is a swatch's, else `#rrggbb`; a gradient or pattern
/// by its kind.
pub fn colour_name(p: &Paint) -> String {
    match p {
        Paint::Solid { color, swatch, .. } => swatch.clone().unwrap_or_else(|| color.to_hex()),
        Paint::Gradient(_) => "Gradient".into(),
        _ => "Pattern".into(),
    }
}

/// Most objects [`colours`] looks at.
const MAX_LISTED: usize = 10_000;

/// The colours the objects in and under `ids` paint with, by [`colour_name`], in the order first
/// met: what a base can be. Read from their appearances alone (cheap enough for a panel each
/// frame); type's and symbols' colours are their own fills and strokes.
pub fn colours(d: &Document, ids: &[NodeId]) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    let mut seen = 0usize;
    let mut stack: Vec<&vectorcraft_doc::Node> = ids.iter().rev().filter_map(|id| d.node(*id)).collect();
    while let Some(n) = stack.pop() {
        seen += 1;
        if seen > MAX_LISTED {
            break;
        }
        if !n.visible {
            continue;
        }
        for i in &n.appearance.items {
            let (shown, paint) = match i {
                vectorcraft_doc::appearance::AppearanceItem::Fill(f) => (f.visible, &f.paint),
                vectorcraft_doc::appearance::AppearanceItem::Stroke(st) => (st.visible && st.width > 0.0, &st.paint),
            };
            if shown && !paint.is_none() {
                let name = colour_name(paint);
                if !out.contains(&name) {
                    out.push(name);
                }
            }
        }
        if let Some(ch) = n.children() {
            stack.extend(ch.iter().rev().map(|c| c.as_ref()));
        }
    }
    out
}

fn fail(what: &str, e: impl std::fmt::Display) -> String {
    format!("{what} couldn't be worked out: {e}")
}

fn unite(parts: &[(&PathData, FillRule)]) -> Result<PathData, String> {
    if parts.is_empty() {
        return Ok(PathData::default());
    }
    try_unite_all(parts).map_err(|e| fail("the colours", e))
}

fn intersect(a: &PathData, b: &PathData) -> Result<PathData, String> {
    try_boolean(a, FillRule::NonZero, b, FillRule::NonZero, BoolOp::Intersect, DEFAULT_PRECISION).map_err(|e| fail("the colours", e))
}

/// What the art in and under `ids` paints, bottom first, each clipped to the masks around it.
fn gather(d: &Document, ids: &[NodeId], warnings: &mut Vec<String>) -> Result<Vec<Painted>, String> {
    let mut out = vec![];
    let mut graded = false;
    for item in crate::select::art_under(d, ids) {
        let (mut areas, notes) = painted(d, &item);
        warnings.extend(notes.into_iter().map(|w| w.message));
        for &clip in &item.clips {
            let Some((mask, rule)) = crate::clip::region(d, clip) else {
                // Clipped to nothing: hidden, as it is drawn.
                areas.clear();
                break;
            };
            let mask = PathData::from_bezpath(&mask);
            let mut kept = vec![];
            for a in areas {
                let shown =
                    try_boolean(&a.path, a.rule, &mask, rule, BoolOp::Intersect, DEFAULT_PRECISION).map_err(|e| fail("a clipping mask", e))?;
                kept.push(Painted { path: shown, rule: FillRule::NonZero, ..a });
            }
            areas = kept;
        }
        graded |= areas.iter().any(|a| !matches!(a.paint, Paint::Solid { .. }));
        out.extend(areas.into_iter().filter(|a| !a.path.is_empty()));
        if out.len() > MAX_SHAPES {
            return Err(format!("the design paints more than {MAX_SHAPES} areas: make layers of fewer objects at once"));
        }
    }
    if graded {
        warnings.push("a gradient or pattern is cut as one colour".into());
    }
    let finite = out.iter().all(|a| a.path.subpaths.iter().all(|sp| sp.anchors.iter().all(|p| p.p.x.is_finite() && p.p.y.is_finite())));
    if !finite {
        return Err("the design has points that aren't numbers".into());
    }
    Ok(out)
}

/// The colour whose shape runs along most of `silhouette`'s edge: the most of it in a band just
/// inside the edge.
fn edge_colour(silhouette: &PathData, regions: &[PathData]) -> Result<Option<usize>, String> {
    let Some(b) = silhouette.bounds() else { return Ok(None) };
    let width = (b.width().min(b.height()) * 0.01).clamp(0.05, 1.0);
    let ring = outline_stroke(silhouette, 2.0 * width, Cap::Butt, Join::Round, 4.0);
    let band = intersect(silhouette, &ring)?;
    let mut best: Option<(usize, f64)> = None;
    for (i, r) in regions.iter().enumerate() {
        let on_edge = area(&intersect(r, &band)?, FillRule::NonZero);
        if on_edge > MIN_AREA_PT2 && best.is_none_or(|(_, a)| on_edge > a) {
            best = Some((i, on_edge));
        }
    }
    Ok(best.map(|(i, _)| i))
}

/// The art in and under `ids` (in paint order) as layers of vinyl, bottom first.
pub fn layered(d: &Document, ids: &[NodeId], o: &Options) -> Result<Layered, String> {
    let (lo, hi) = OVERLAP_MM;
    if !(lo..=hi).contains(&o.overlap_mm) {
        return Err(format!("the overlap is {lo}–{hi} mm"));
    }
    let mut warnings = vec![];
    let areas = gather(d, ids, &mut warnings)?;
    // Each colour, in the order it is first painted.
    let mut colours: Vec<(String, Paint)> = vec![];
    let mut shapes = vec![];
    for a in &areas {
        let name = colour_name(&a.paint);
        let key = match colours.iter().position(|(n, _)| *n == name) {
            Some(k) => k,
            None => {
                colours.push((name, a.paint.clone()));
                colours.len() - 1
            }
        };
        shapes.push(Shape::new(a.path.clone(), a.rule, key as u64));
    }
    let all: Vec<(&PathData, FillRule)> = areas.iter().map(|a| (&a.path, a.rule)).collect();
    let silhouette = unite(&all)?;
    if area(&silhouette, FillRule::NonZero) <= MIN_AREA_PT2 {
        return Err("the selected art paints nothing to cut".into());
    }
    // What shows of each colour: hidden parts out, touching parts of one colour joined.
    let merged = pathfinder_with(PathfinderOp::Merge, &shapes, DEFAULT_PRECISION);
    let mut regions = vec![];
    for k in 0..colours.len() {
        let parts: Vec<(&PathData, FillRule)> = merged.iter().filter(|s| s.key == k as u64).map(|s| (&s.path, s.rule)).collect();
        regions.push(unite(&parts)?);
    }
    let base = match &o.base {
        Base::None => None,
        Base::Auto => edge_colour(&silhouette, &regions)?,
        Base::Colour(want) => {
            let found = colours.iter().position(|(n, _)| n.eq_ignore_ascii_case(want));
            let names: Vec<&str> = colours.iter().map(|(n, _)| n.as_str()).collect();
            Some(found.ok_or_else(|| format!("the design has no colour {want}: it has {}", names.join(", ")))?)
        }
    };
    let mut layers = vec![];
    if let Some(b) = base
        && let Some((name, paint)) = colours.get(b)
    {
        layers.push(VinylLayer { name: name.clone(), paint: paint.clone(), region: silhouette.clone(), base: true });
    }
    for (k, (name, paint)) in colours.iter().enumerate() {
        let Some(region) = regions.get(k) else { continue };
        if Some(k) == base || area(region, FillRule::NonZero) <= MIN_AREA_PT2 {
            continue;
        }
        layers.push(VinylLayer { name: name.clone(), paint: paint.clone(), region: region.clone(), base: false });
    }
    if o.overlap_mm > 0.0 {
        overlap(&mut layers, o.overlap_mm / MM_PER_PT)?;
    }
    Ok(Layered { layers, warnings })
}

/// Each colour but the base grown by `by` points under the colours stacked above it.
fn overlap(layers: &mut [VinylLayer], by: f64) -> Result<(), String> {
    let shown: Vec<PathData> = layers.iter().map(|l| l.region.clone()).collect();
    for (i, layer) in layers.iter_mut().enumerate() {
        if layer.base {
            continue;
        }
        let above: Vec<(&PathData, FillRule)> = shown.iter().skip(i + 1).map(|r| (r, FillRule::NonZero)).collect();
        if above.is_empty() {
            continue;
        }
        let above = unite(&above)?;
        let band = outline_stroke(&layer.region, 2.0 * by, Cap::Round, Join::Round, 4.0);
        let under = intersect(&band, &above)?;
        layer.region = unite(&[(&layer.region, FillRule::NonZero), (&under, FillRule::NonZero)])?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
