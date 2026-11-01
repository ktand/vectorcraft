//! Fork: the registration marks and the print bleed kept up to date (C14.2). After each edit the
//! engine calls [`refresh`], in the same undo step: a marks layer drawn for another artboard size
//! or other mark settings is drawn again, and so is a bleed made from art that has changed, so one
//! undo puts back the edit and what followed it together. A bleed for more than
//! [`MAX_LIVE_BLEED`] objects is left for the user to update (the job says so): making it again
//! on every edit would slow editing down.

use vectorcraft_doc::{Document, Node, NodeId};

/// Most printed objects a bleed is kept up to date for by itself.
pub const MAX_LIVE_BLEED: usize = 200;

/// Puts `layer` (made again) where the top-level layer `old` is, keeping whether it shows.
fn replace(d: &mut Document, old: NodeId, mut layer: Node) -> Result<(), String> {
    let at = d.layers.iter().position(|l| l.id == old).ok_or("the layer isn't in the document")?;
    layer.visible = d.layers.get(at).is_none_or(|l| l.visible);
    d.remove(old).map_err(|e| e.to_string())?;
    d.insert(None, at, layer).map_err(|e| e.to_string())?;
    Ok(())
}

/// Draws again what in `d` was drawn for an earlier state: the marks layer, the bleed layer.
/// Whether anything changed. Never fails the edit: what can't be drawn again (an artboard too
/// small for marks, art that can't be outlined) stays as it is, and the job warns about it.
pub fn refresh(d: &mut Document) -> bool {
    let mut changed = false;
    let s = crate::settings(d);
    if let Some(old) = crate::marks::marks_layer(d)
        && let Some(m) = crate::marks::layout(&s, d)
        && !crate::marks::matches_settings(d, old, &s, &m)
        && let Ok(layer) = crate::marks::marks_layer_node(&s, d)
    {
        changed |= replace(d, old, layer).is_ok();
    }
    if let Some(old) = crate::bleed::bleed_layer(d)
        && crate::bleed::sources(d).len() <= MAX_LIVE_BLEED
        && crate::bleed::is_stale(d, s.bleed_mm)
    {
        match crate::bleed::bleed_layer_node(d, s.bleed_mm) {
            Ok((layer, _)) => changed |= replace(d, old, layer).is_ok(),
            // Nothing printed left to bleed: the layer goes.
            Err(_) if crate::bleed::sources(d).is_empty() => changed |= d.remove(old).is_ok(),
            Err(_) => {}
        }
        crate::bleed::prune_images(d);
    }
    changed
}

#[cfg(test)]
mod tests {
    use vectorcraft_color::{Color, Paint};
    use vectorcraft_doc::Appearance;
    use vectorcraft_geom::{Affine, Rect, shapes};

    use super::*;
    use crate::{set_settings, settings};

    /// A Letter artboard, Print & Cut on, its marks and a red square's bleed added.
    fn sheet() -> (Document, NodeId) {
        let mut d = Document::new(612.0, 792.0);
        let layer = d.layers[0].id;
        let red = Appearance::basic(Paint::solid(Color::rgb(1.0, 0.0, 0.0)), Paint::None, 0.0);
        let sq = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(200.0, 300.0, 300.0, 400.0)), red);
        let sq = d.insert(Some(layer), usize::MAX, sq).unwrap();
        let s = vectorcraft_cutjob::CuttingSettings { print_and_cut: true, ..settings(&d) };
        set_settings(&mut d, &s);
        let marks = crate::marks::marks_layer_node(&s, &mut d).unwrap();
        d.insert(None, usize::MAX, marks).unwrap();
        let (bleed, _) = crate::bleed::bleed_layer_node(&mut d, s.bleed_mm).unwrap();
        d.insert(None, 0, bleed).unwrap();
        (d, sq)
    }

    fn warnings(d: &Document) -> Vec<String> {
        crate::build_job(d).unwrap().warnings.into_iter().map(|w| w.message).collect()
    }

    #[test]
    fn nothing_to_do_changes_nothing() {
        let (mut d, _) = sheet();
        assert!(!refresh(&mut d));
        assert!(warnings(&d).is_empty());
    }

    /// The artboard made larger, or the marks set longer: the marks are drawn again, where the
    /// layer was, and the job is happy.
    #[test]
    fn the_marks_follow_the_artboard_and_settings() {
        let (mut d, _) = sheet();
        let at = d.layers.iter().position(|l| Some(l.id) == crate::marks::marks_layer(&d)).unwrap();
        d.artboards[0].rect = Rect::new(0.0, 0.0, 700.0, 792.0);
        assert!(warnings(&d).iter().any(|w| w == crate::marks::STALE_MARKS));
        assert!(refresh(&mut d));
        assert!(warnings(&d).is_empty(), "{:?}", warnings(&d));
        assert_eq!(d.layers.iter().position(|l| Some(l.id) == crate::marks::marks_layer(&d)), Some(at), "in its place");
        let mut s = settings(&d);
        s.mark_length_mm = 10.0;
        set_settings(&mut d, &s);
        assert!(refresh(&mut d));
        assert!(warnings(&d).is_empty());
    }

    /// The art moved: its bleed is made again where it now is.
    #[test]
    fn the_bleed_follows_the_art() {
        let (mut d, sq) = sheet();
        d.node_mut(sq).unwrap().transform(Affine::translate((50.0, 0.0)), false);
        assert!(warnings(&d).iter().any(|w| w == crate::bleed::STALE_BLEED));
        assert!(refresh(&mut d));
        assert!(warnings(&d).is_empty());
        let bleed = d.node(crate::bleed::bleed_layer(&d).unwrap()).unwrap();
        let b = bleed.children().unwrap()[0].geometric_bounds().unwrap();
        assert!(b.x0 > 240.0 && b.x0 < 250.0, "moved with it: {b:?}");
        // All the art gone: so is its bleed.
        d.remove(sq).unwrap();
        assert!(refresh(&mut d));
        assert_eq!(crate::bleed::bleed_layer(&d), None);
    }

    /// Past the limit the bleed is left for the user to update.
    #[test]
    fn a_large_bleed_is_left_to_the_user() {
        let (mut d, _) = sheet();
        let layer = d.layers.iter().find(|l| Some(l.id) != crate::marks::marks_layer(&d) && Some(l.id) != crate::bleed::bleed_layer(&d)).unwrap().id;
        for k in 0..MAX_LIVE_BLEED {
            let x = 80.0 + f64::from(u32::try_from(k % 20).unwrap()) * 22.0;
            let y = 450.0 + f64::from(u32::try_from(k / 20).unwrap()) * 25.0;
            let n = Node::path(
                d.alloc_id(),
                shapes::rectangle(Rect::new(x, y, x + 10.0, y + 10.0)),
                Appearance::basic(Paint::solid(Color::BLACK), Paint::None, 0.0),
            );
            d.insert(Some(layer), usize::MAX, n).unwrap();
        }
        assert!(!refresh(&mut d), "too many to keep up");
        assert!(warnings(&d).iter().any(|w| w == crate::bleed::STALE_BLEED));
    }
}
