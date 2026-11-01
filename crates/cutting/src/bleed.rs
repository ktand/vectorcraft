//! Fork: print bleed (C12.3, C14.2). A Print & Cut cut that lands a little off the print leaves a
//! sliver of white paper at the edge; a bleed spreads each printed object's outer colour a set
//! distance past its edge, so a cut a little off still lands in colour. It is a layer of its own
//! at the bottom of the document (locked, not cut, found again by `cut:bleed`, whose value
//! fingerprints the art it was made from), made again whenever that art changes ([`crate::live`]):
//! for each printed object, its painted outline (fill and strokes, as cut round a stroke's edge,
//! [`crate::edge`]) grown by the distance with round corners, filled with its stroke's colour if
//! the stroke is at least as wide as the bleed, else its fill's (a hairline outline doesn't colour
//! it). An image bleeds its own edge pixels, stretched outward by the distance, inside its clipping
//! mask grown by it when it has one. Behind everything, only the part past the art shows. Gradient
//! meshes get none.

use std::collections::BTreeSet;

use vectorcraft_color::Paint;
use vectorcraft_cutjob::Op;
use vectorcraft_doc::appearance::FillLayer;
use vectorcraft_doc::{Appearance, AppearanceItem, Document, ForeignAttribute, ImageBlob, ImageObject, LayerColor, Node, NodeId, NodeKind};
use vectorcraft_geom::{Affine, FillRule, PathData, PathEl};
use vectorcraft_pathops::{Join, offset_path, try_unite_all};

use crate::ops::{NS, PREFIX, set_own_op};
use crate::placement::MM_PER_PT;

/// How far the bleed runs past the art, mm: the range, and the usual (as the maker's software).
pub const RADIUS_MM: (f64, f64) = (0.1, 5.0);
pub const DEFAULT_RADIUS_MM: f64 = 1.27;
/// The bleed layer's name, and the attribute that marks it.
pub const LAYER_NAME: &str = "Print Bleed";
pub const NAME: &str = "bleed";
/// Most objects one bleed is made for.
const MAX_OBJECTS: usize = 20_000;
/// Most pixels an image may have to bleed (decoded and written again).
const MAX_IMAGE_PIXELS: u64 = 25_000_000;
/// Most pixels an image's edge is stretched by.
const MAX_PAD_PX: u32 = 512;
/// The prefix of the stretched images' keys in the document's images.
const IMAGE_KEY: &str = "cut-bleed:";

fn marker(n: &Node) -> Option<&str> {
    n.extras().attrs.as_deref()?.foreign.iter().find(|f| f.ns == NS && f.name == NAME).map(|f| f.value.as_str())
}

/// The document's bleed layer, if it has one (a top-level layer).
pub fn bleed_layer(d: &Document) -> Option<NodeId> {
    d.layers.iter().find(|l| marker(l).is_some()).map(|l| l.id)
}

/// The printed objects a bleed is made for: the visible art on every layer but the bleed, the
/// registration marks and the cut contour (cut lines, not print).
pub(crate) fn sources(d: &Document) -> Vec<crate::select::Item> {
    let skip = [bleed_layer(d), crate::marks::marks_layer(d), crate::contour::contour_layer(d)];
    let layers: Vec<NodeId> = d.layers.iter().map(|l| l.id).filter(|id| !skip.contains(&Some(*id))).collect();
    crate::select::art_under(d, &layers)
}

/// The colour the bleed of `n` is filled with: its first painted leaf's stroke colour if that
/// stroke is at least `min_stroke` wide (points), else its fill's (type through its outlines, a
/// symbol through its art).
fn outer_paint(d: &Document, n: &Node, min_stroke: f64) -> Option<Paint> {
    let own = |n: &Node| {
        let a = &n.appearance;
        a.painted_strokes().find(|s| s.width >= min_stroke).map(|s| s.paint.stroke_as_fill()).or_else(|| {
            a.items.iter().find_map(|i| match i {
                AppearanceItem::Fill(f) if f.visible && !f.paint.is_none() => Some(f.paint.clone()),
                _ => None,
            })
        })
    };
    let mut found = None;
    let mut look = |n: &Node| {
        n.walk(&mut |c| {
            if found.is_none() {
                found = own(c);
            }
        });
    };
    match &n.kind {
        NodeKind::Text(_) => look(&vectorcraft_effects::outline_text(n)?),
        NodeKind::SymbolInstance { symbol, .. } => look(&d.symbols.iter().find(|s| &s.name == symbol)?.art),
        _ => look(n),
    }
    found
}

/// One printed object, as its bleed is made from it.
#[derive(Clone, Debug)]
enum Source {
    /// Its painted area (document points, even-odd, closed) and the colour it bleeds.
    Area(NodeId, PathData, Paint),
    /// An image, and the clipping mask round it (its region, document points), if any.
    Image(NodeId, ImageObject, Option<(PathData, FillRule)>),
}

/// The image `item` shows, and its innermost clipping mask's region, if it is an image.
fn image_source(d: &Document, item: &crate::select::Item) -> Option<Source> {
    let NodeKind::Image(img) = &d.node(item.id)?.kind else { return None };
    let clip = item.clips.last().and_then(|c| crate::clip::region(d, *c)).map(|(bp, rule)| (PathData::from_bezpath(&bp), rule));
    Some(Source::Image(item.id, (**img).clone(), clip))
}

/// Each printed object a bleed is made for, with what it is made from.
fn painted(d: &Document, radius_mm: f64) -> Result<Vec<Source>, String> {
    let items = sources(d);
    if items.len() > MAX_OBJECTS {
        return Err(format!("at most {MAX_OBJECTS} objects can have a bleed"));
    }
    let min_stroke = radius_mm / MM_PER_PT;
    let mut out = vec![];
    for item in items {
        if let Some(image) = image_source(d, &item) {
            out.push(image);
            continue;
        }
        let Some(n) = d.node(item.id) else { continue };
        let Some(paint) = outer_paint(d, n, min_stroke) else { continue };
        let edge = crate::select::Item { stroke_edge: true, ..item };
        let (outlines, _) = crate::geometry::outlines(d, std::slice::from_ref(&edge));
        let mut subpaths = vec![];
        for o in outlines {
            let mut bp = o.path;
            if !matches!(bp.elements().last(), Some(PathEl::ClosePath)) {
                bp.push(PathEl::ClosePath);
            }
            subpaths.extend(PathData::from_bezpath(&bp).subpaths);
        }
        if !subpaths.is_empty() {
            out.push(Source::Area(edge.id, PathData::new(subpaths), paint));
        }
    }
    Ok(out)
}

/// A fingerprint of what the bleed is made from (each object's area and colour, each image and
/// where it lies and its mask, and the distance): 64-bit FNV-1a, in hex. A bleed layer whose
/// fingerprint differs is out of date.
fn fingerprint(art: &[Source], radius_mm: f64) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |bytes: &[u8]| {
        for b in bytes {
            h ^= u64::from(*b);
            h = h.wrapping_mul(0x0100_0000_01b3);
        }
    };
    let path = |eat: &mut dyn FnMut(&[u8]), p: &PathData| {
        for s in &p.subpaths {
            for a in &s.anchors {
                for v in [a.p.x, a.p.y, a.h_in.x, a.h_in.y, a.h_out.x, a.h_out.y] {
                    eat(&v.to_bits().to_le_bytes());
                }
            }
        }
    };
    eat(&radius_mm.to_bits().to_le_bytes());
    for s in art {
        match s {
            Source::Area(id, p, paint) => {
                eat(&id.0.to_le_bytes());
                path(&mut eat, p);
                eat(format!("{paint:?}").as_bytes());
            }
            Source::Image(id, img, clip) => {
                eat(&id.0.to_le_bytes());
                eat(img.key.as_bytes());
                for v in img.xf.as_coeffs() {
                    eat(&v.to_bits().to_le_bytes());
                }
                if let Some((p, rule)) = clip {
                    path(&mut eat, p);
                    eat(format!("{rule:?}").as_bytes());
                }
            }
        }
    }
    format!("{h:016x}")
}

/// `bytes` (an image file) decoded, its edge pixels stretched `pad` pixels outward on every side,
/// and written as a PNG: (the file, its width, height); `None` when it can't be read or is too
/// large.
fn padded(bytes: &[u8], pad: u32) -> Option<(Vec<u8>, u32, u32)> {
    let img = image::load_from_memory(bytes).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    if w == 0 || h == 0 || u64::from(w) * u64::from(h) > MAX_IMAGE_PIXELS {
        return None;
    }
    let (nw, nh) = (w.checked_add(pad.checked_mul(2)?)?, h.checked_add(pad.checked_mul(2)?)?);
    if u64::from(nw) * u64::from(nh) > MAX_IMAGE_PIXELS * 2 {
        return None;
    }
    let src = |v: u32, n: u32| v.saturating_sub(pad).min(n - 1);
    let out = image::RgbaImage::from_fn(nw, nh, |x, y| *img.get_pixel(src(x, w), src(y, h)));
    let mut buf = Vec::new();
    image::DynamicImage::ImageRgba8(out).write_to(&mut std::io::Cursor::new(&mut buf), image::ImageFormat::Png).ok()?;
    Some((buf, nw, nh))
}

/// The bleed of image `img`: a copy with its edge pixels stretched `radius_mm` outward (kept in
/// `d`'s images under a key of its own, made once per image and distance), inside `clip` grown by
/// the distance when the image is clipped. `None` when the image's pixels can't be read.
fn image_bleed(d: &mut Document, img: &ImageObject, clip: Option<&(PathData, FillRule)>, radius_mm: f64) -> Option<Node> {
    let [a, b, c, e, ..] = img.xf.as_coeffs();
    let mm_per_px = (a * e - b * c).abs().sqrt() * MM_PER_PT;
    if !(mm_per_px.is_finite() && mm_per_px > 0.0) {
        return None;
    }
    // Whole and small after the clamp: the cast is exact.
    let pad = (radius_mm / mm_per_px).ceil().clamp(1.0, f64::from(MAX_PAD_PX)) as u32;
    let key = format!("{IMAGE_KEY}{}:{pad}", img.key);
    let (width, height) = match d.images.get(&key) {
        Some(_) => (img.width.checked_add(pad * 2)?, img.height.checked_add(pad * 2)?),
        None => {
            let bytes = img.pixels.as_ref().map(|p| p.bytes.clone()).or_else(|| d.images.get(&img.key).map(|b| b.bytes.clone()))?;
            let (png, w, h) = padded(&bytes, pad)?;
            d.images.insert(key.clone(), ImageBlob { mime: "image/png".into(), bytes: std::sync::Arc::new(png), proxy: None });
            (w, h)
        }
    };
    // The decoded image may be another size than the object says: its pixels map onto the same
    // frame, stretched to it.
    let (sx, sy) = (
        f64::from(img.width) / f64::from(width.saturating_sub(pad * 2).max(1)),
        f64::from(img.height) / f64::from(height.saturating_sub(pad * 2).max(1)),
    );
    let xf = img.xf * Affine::scale_non_uniform(sx, sy) * Affine::translate((-f64::from(pad), -f64::from(pad)));
    let image = Node::new(
        d.alloc_id(),
        NodeKind::Image(Box::new(ImageObject { key, width, height, xf, link: None, placement: Default::default(), pixels: None })),
    );
    let Some((region, rule)) = clip else { return Some(image) };
    let clean = try_unite_all(&[(region, *rule)]).ok()?;
    let grown = offset_path(&clean, radius_mm / MM_PER_PT, Join::Round, 4.0);
    let mut mask = Node::path(d.alloc_id(), grown, Appearance::default());
    if let NodeKind::Path { clipping, .. } = &mut mask.kind {
        *clipping = true;
    }
    Some(Node::new(d.alloc_id(), NodeKind::Group { children: vec![std::sync::Arc::new(mask), std::sync::Arc::new(image)], clip: true }))
}

/// Takes out of `d`'s images the stretched copies its bleed layer no longer shows.
pub fn prune_images(d: &mut Document) {
    let mut used = BTreeSet::new();
    if let Some(layer) = bleed_layer(d).and_then(|id| d.node(id)) {
        layer.walk(&mut |n| {
            if let NodeKind::Image(img) = &n.kind {
                used.insert(img.key.clone());
            }
        });
    }
    d.images.retain(|k, _| !k.starts_with(IMAGE_KEY) || used.contains(k));
}

/// A new bleed layer for `d`'s art, `radius_mm` past it (ids from `d`; an image's stretched copy
/// kept in its images), and how many objects it bleeds, or why there can't be one.
pub fn bleed_layer_node(d: &mut Document, radius_mm: f64) -> Result<(Node, usize), String> {
    if !(radius_mm.is_finite() && (RADIUS_MM.0..=RADIUS_MM.1).contains(&radius_mm)) {
        return Err(format!("the bleed is {} to {} mm", RADIUS_MM.0, RADIUS_MM.1));
    }
    let art = painted(d, radius_mm)?;
    let print = fingerprint(&art, radius_mm);
    let distance = radius_mm / MM_PER_PT;
    let mut children = vec![];
    for s in &art {
        match s {
            Source::Area(_, area, paint) => {
                let clean = try_unite_all(&[(area, FillRule::EvenOdd)]).map_err(|e| format!("an object's outline couldn't be made: {e}"))?;
                let grown = offset_path(&clean, distance, Join::Round, 4.0);
                if grown.is_empty() {
                    continue;
                }
                let fill = Appearance { items: vec![AppearanceItem::Fill(FillLayer::new(paint.clone()))], ..Default::default() };
                children.push(std::sync::Arc::new(Node::path(d.alloc_id(), grown, fill)));
            }
            Source::Image(_, img, clip) => {
                if let Some(n) = image_bleed(d, img, clip.as_ref(), radius_mm) {
                    children.push(std::sync::Arc::new(n));
                }
            }
        }
    }
    if children.is_empty() {
        return Err("there is no printed art to bleed".into());
    }
    let count = children.len();
    let mut layer = Node::layer(d.alloc_id(), LAYER_NAME, LayerColor::Preset(3));
    layer.locked = true;
    set_own_op(&mut layer, Some(Op::NoCut));
    layer.edit_attrs(|at| at.foreign.push(ForeignAttribute { ns: NS.to_string(), prefix: PREFIX.to_string(), name: NAME.to_string(), value: print }));
    if let Some(c) = layer.children_mut() {
        *c = children;
    }
    Ok((layer, count))
}

/// The warning for a bleed made from art that has changed since (one too large to keep up to
/// date by itself, [`crate::live::MAX_LIVE_BLEED`]).
pub const STALE_BLEED: &str = "the print bleed was made before the art changed: press Update Bleed, then print again";

/// Whether `d` has a bleed layer made from other art or another distance than `radius_mm`.
pub fn is_stale(d: &Document, radius_mm: f64) -> bool {
    let Some(layer) = bleed_layer(d).and_then(|id| d.node(id)) else { return false };
    let Some(made) = marker(layer) else { return false };
    painted(d, radius_mm).map_or(true, |art| fingerprint(&art, radius_mm) != made)
}

#[cfg(test)]
mod tests {
    use vectorcraft_color::Color;
    use vectorcraft_geom::{Rect, shapes};

    use super::*;

    fn doc() -> (Document, NodeId) {
        let mut d = Document::new(300.0, 300.0);
        let layer = d.layers[0].id;
        let red = Appearance::basic(Paint::solid(Color::rgb(1.0, 0.0, 0.0)), Paint::None, 0.0);
        let sq = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(100.0, 100.0, 200.0, 200.0)), red);
        let id = d.insert(Some(layer), usize::MAX, sq).unwrap();
        (d, id)
    }

    fn add(d: &mut Document, radius: f64) -> NodeId {
        let (layer, _) = bleed_layer_node(d, radius).unwrap();
        d.insert(None, 0, layer).unwrap()
    }

    /// A red square's bleed: red, the distance past it all round, round at the corners; the
    /// layer at the bottom, locked, not cut, found again.
    #[test]
    fn a_square_bleeds_its_colour() {
        let (mut d, _) = doc();
        let (layer, count) = bleed_layer_node(&mut d, 2.0).unwrap();
        assert_eq!(count, 1);
        assert!(layer.locked && crate::ops::own_op(&layer) == Some(Op::NoCut));
        let id = d.insert(None, 0, layer).unwrap();
        assert_eq!(bleed_layer(&d), Some(id));
        let n = &d.node(id).unwrap().children().unwrap()[0];
        let b = n.geometric_bounds().unwrap();
        let r = 2.0 / MM_PER_PT;
        assert!((b.x0 - (100.0 - r)).abs() < 1e-3 && (b.y1 - (200.0 + r)).abs() < 1e-3, "{b:?}");
        assert_eq!(n.appearance.fill().map(|f| f.paint.clone()), Some(Paint::solid(Color::rgb(1.0, 0.0, 0.0))));
        let job = crate::build_job(&d).unwrap();
        assert_eq!(job.paths.len(), 1, "the bleed isn't cut");
    }

    /// A stroked line bleeds its stroke's colour round the stroke.
    #[test]
    fn a_stroke_bleeds_round_the_stroke() {
        let mut d = Document::new(300.0, 300.0);
        let layer = d.layers[0].id;
        let blue = Appearance::basic(Paint::None, Paint::solid(Color::rgb(0.0, 0.0, 1.0)), 10.0);
        let line =
            Node::path(d.alloc_id(), shapes::line(vectorcraft_geom::Point::new(50.0, 100.0), vectorcraft_geom::Point::new(250.0, 100.0)), blue);
        d.insert(Some(layer), usize::MAX, line).unwrap();
        let id = add(&mut d, 1.0);
        let n = &d.node(id).unwrap().children().unwrap()[0];
        let b = n.geometric_bounds().unwrap();
        let r = 1.0 / MM_PER_PT;
        assert!((b.y0 - (95.0 - r)).abs() < 1e-3 && (b.y1 - (105.0 + r)).abs() < 1e-3, "the stroke's width and the bleed: {b:?}");
        assert_eq!(n.appearance.fill().map(|f| f.paint.clone()), Some(Paint::solid(Color::rgb(0.0, 0.0, 1.0))));
    }

    /// Moving the art or changing the distance makes the bleed out of date; making it again
    /// doesn't bleed the old bleed.
    #[test]
    fn a_changed_art_makes_the_bleed_stale() {
        let (mut d, sq) = doc();
        assert!(!is_stale(&d, 2.0), "no bleed, nothing stale");
        let id = add(&mut d, 2.0);
        assert!(!is_stale(&d, 2.0));
        assert!(is_stale(&d, 3.0), "another distance");
        d.node_mut(sq).unwrap().transform(vectorcraft_geom::Affine::translate((10.0, 0.0)), false);
        assert!(is_stale(&d, 2.0));
        d.remove(id).unwrap();
        let (layer, count) = bleed_layer_node(&mut d, 2.0).unwrap();
        assert_eq!(count, 1, "the art only, not the old bleed");
        d.insert(None, 0, layer).unwrap();
        assert!(!is_stale(&d, 2.0));
    }

    #[test]
    fn nothing_and_bad_distances_are_refused() {
        let (mut d, sq) = doc();
        assert!(bleed_layer_node(&mut d, 0.0).is_err());
        assert!(bleed_layer_node(&mut d, f64::NAN).is_err());
        d.node_mut(sq).unwrap().appearance = Appearance::basic(Paint::None, Paint::None, 0.0).into();
        assert!(bleed_layer_node(&mut d, 1.0).unwrap_err().contains("no printed art"));
    }

    /// C14.2: a hairline outline doesn't colour the bleed (the fill does); a stroke as wide as the
    /// bleed does.
    #[test]
    fn a_hairline_stroke_doesnt_colour_the_bleed() {
        let colour = |stroke: f64| {
            let mut d = Document::new(300.0, 300.0);
            let layer = d.layers[0].id;
            let a = Appearance::basic(Paint::solid(Color::rgb(1.0, 0.0, 0.0)), Paint::solid(Color::BLACK), stroke);
            let sq = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(100.0, 100.0, 200.0, 200.0)), a);
            d.insert(Some(layer), usize::MAX, sq).unwrap();
            let id = add(&mut d, 1.27);
            d.node(id).unwrap().children().unwrap()[0].appearance.fill().map(|f| f.paint.clone())
        };
        assert_eq!(colour(1.0), Some(Paint::solid(Color::rgb(1.0, 0.0, 0.0))), "a 1 pt outline: the fill's red");
        assert_eq!(colour(10.0), Some(Paint::solid(Color::BLACK)), "a 10 pt border: its black");
    }

    /// C14.2: an image bleeds its edge pixels: a copy larger by the distance each side, its
    /// corner pixel the image's; made once and reused; the copies no bleed shows are let go.
    #[test]
    fn an_image_bleeds_its_edge_pixels() {
        let mut d = Document::new(300.0, 300.0);
        let layer = d.layers[0].id;
        let mut img = image::RgbaImage::from_pixel(10, 10, image::Rgba([0, 0, 255, 255]));
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        let mut png = Vec::new();
        image::DynamicImage::ImageRgba8(img).write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
        d.images.insert("pic".into(), ImageBlob { mime: "image/png".into(), bytes: std::sync::Arc::new(png), proxy: None });
        // 10 px over 100 pt: 10 pt (3.53 mm) a pixel; a 5 mm bleed is 2 pixels.
        let xf = Affine::translate((100.0, 100.0)) * Affine::scale(10.0);
        let obj = ImageObject { key: "pic".into(), width: 10, height: 10, xf, link: None, placement: Default::default(), pixels: None };
        let pic = Node::new(d.alloc_id(), NodeKind::Image(Box::new(obj)));
        d.insert(Some(layer), usize::MAX, pic).unwrap();
        let id = add(&mut d, 5.0);
        let NodeKind::Image(b) = &d.node(id).unwrap().children().unwrap()[0].kind else { panic!("an image") };
        assert_eq!((b.width, b.height), (14, 14));
        let bounds = b.xf.transform_rect_bbox(Rect::new(0.0, 0.0, 14.0, 14.0));
        assert!((bounds.x0 - 80.0).abs() < 1e-9 && (bounds.x1 - 220.0).abs() < 1e-9, "{bounds:?}");
        let pixels = image::load_from_memory(&d.images[&b.key].bytes).unwrap().to_rgba8();
        assert_eq!(pixels.get_pixel(0, 0), &image::Rgba([255, 0, 0, 255]), "the corner stretched out");
        assert_eq!(pixels.get_pixel(13, 13), &image::Rgba([0, 0, 255, 255]));
        let key = b.key.clone();
        d.remove(id).unwrap();
        prune_images(&mut d);
        assert!(!d.images.contains_key(&key) && d.images.contains_key("pic"), "the copy let go, the image kept");
    }
}
