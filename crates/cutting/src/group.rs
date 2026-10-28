//! Grouped cutting (C5.3): which group an object is cut in. By layer, its top-level layer (by
//! name); by colour, its stroke's colour if it has a solid one, else its fill's (by swatch name
//! where the colour is a swatch's, else as `#rrggbb`). Groups come in the order their first
//! object does, bottom first.

use vectorcraft_color::Paint;
use vectorcraft_cutjob::Grouping;
use vectorcraft_doc::{Document, NodeId};

/// The name of the group object `id` is cut in (empty without grouping).
pub fn group_of(d: &Document, id: NodeId, grouping: Grouping) -> String {
    match grouping {
        Grouping::None => String::new(),
        Grouping::Layer => d.layer_of(id).and_then(|l| d.node(l)).and_then(|n| n.name.clone()).unwrap_or_else(|| "Layer".into()),
        Grouping::Color => {
            let Some(n) = d.node(id) else { return String::new() };
            let name = |p: &Paint| match p {
                Paint::Solid { color, swatch, .. } => Some(swatch.clone().unwrap_or_else(|| color.to_hex())),
                _ => None,
            };
            name(&n.appearance.stroke_paint()).or_else(|| name(&n.appearance.fill_paint())).unwrap_or_else(|| "No colour".into())
        }
    }
}

#[cfg(test)]
mod tests {
    use vectorcraft_color::Color;
    use vectorcraft_doc::{Appearance, LayerColor, Node};
    use vectorcraft_geom::{Rect, shapes};

    use super::*;

    #[test]
    fn groups_by_layer_and_by_colour() {
        let mut d = Document::new(100.0, 100.0);
        let first = d.layers[0].id;
        let layer = Node::layer(d.alloc_id(), "Gold vinyl", LayerColor::Preset(1));
        let second = d.insert(None, usize::MAX, layer).unwrap();
        let red = Appearance::basic(Paint::None, Paint::solid(Color::rgb(1.0, 0.0, 0.0)), 1.0);
        let blue_fill = Appearance::basic(Paint::solid(Color::rgb(0.0, 0.0, 1.0)), Paint::None, 1.0);
        let a = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(0.0, 0.0, 10.0, 10.0)), red);
        let b = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(0.0, 0.0, 10.0, 10.0)), blue_fill);
        let c = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(0.0, 0.0, 10.0, 10.0)), Appearance::basic(Paint::None, Paint::None, 0.0));
        let (a, b, c) = (d.insert(Some(first), 0, a).unwrap(), d.insert(Some(second), 0, b).unwrap(), d.insert(Some(second), 1, c).unwrap());
        assert_eq!(group_of(&d, b, Grouping::Layer), "Gold vinyl");
        assert_eq!(group_of(&d, a, Grouping::Color), "#ff0000", "the stroke's colour");
        assert_eq!(group_of(&d, b, Grouping::Color), "#0000ff", "no stroke: the fill's");
        assert_eq!(group_of(&d, c, Grouping::Color), "No colour");
        assert_eq!(group_of(&d, a, Grouping::None), "");
    }
}
