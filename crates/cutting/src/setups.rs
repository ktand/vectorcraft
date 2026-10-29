//! Each group's own tool setup, when the job is cut in groups (C7.4, ARCHITECTURE.md › 4.4). Cut by
//! layer, a layer's setup is on the layer, a foreign attribute in the cutting namespace
//! (`cut:setup`, its value the setup as JSON), so it is renamed, copied and undone with the layer.
//! Cut by colour, a colour isn't an object, so its setup is in the document's settings
//! ([`CuttingSettings::color_setups`]), under the name its group has. A group without one is cut
//! with the document's setup. A value this version can't read counts as none and is kept until
//! the setup is set.

use vectorcraft_cutjob::{CuttingSettings, GroupSetup, Grouping};
use vectorcraft_doc::{Document, ForeignAttribute, Node};

use crate::ops::{NS, PREFIX};

/// The layer attribute's local name.
pub const NAME: &str = "setup";
/// Longest attribute value read, bytes: a setup is a few hundred.
const MAX_VALUE: usize = 16 * 1024;

fn ours(a: &ForeignAttribute) -> bool {
    a.ns == NS && a.name == NAME
}

/// The name a layer's group has when cut by layer ([`crate::group::group_of`]).
pub fn layer_group_name(layer: &Node) -> String {
    layer.name.clone().unwrap_or_else(|| "Layer".into())
}

/// The setup layer `n` keeps, if it keeps one this version reads (missing settings: `s`'s).
pub fn layer_setup(n: &Node, s: &CuttingSettings) -> Option<GroupSetup> {
    let attrs = n.extras().attrs.as_deref()?;
    let raw = attrs.foreign.iter().find(|a| ours(a))?;
    if raw.value.len() > MAX_VALUE {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(&raw.value).ok()?;
    GroupSetup::from_value(&v, &s.operations)
}

/// Sets layer `n`'s setup (`None`: none, so the document's). Its other foreign attributes stay.
/// Whether anything changed.
pub fn set_layer_setup(n: &mut Node, setup: Option<&GroupSetup>) -> bool {
    let after: Vec<ForeignAttribute> = setup
        .map(|g| ForeignAttribute { ns: NS.to_string(), prefix: PREFIX.to_string(), name: NAME.to_string(), value: g.to_value().to_string() })
        .into_iter()
        .collect();
    let before: Vec<ForeignAttribute> =
        n.extras().attrs.as_deref().map(|a| a.foreign.iter().filter(|a| ours(a)).cloned().collect()).unwrap_or_default();
    if before == after {
        return false;
    }
    n.edit_attrs(|a| {
        a.foreign.retain(|f| !ours(f));
        a.foreign.extend(after);
    });
    true
}

/// The groups the document is cut in, in the order the job cuts them (the order their first
/// object comes); none without grouping.
pub fn group_names(d: &Document, s: &CuttingSettings) -> Vec<String> {
    if s.grouping == Grouping::None {
        return vec![];
    }
    let mut names: Vec<String> = vec![];
    for item in crate::select::select(d, s) {
        let name = crate::group::group_of(d, item.id, s.grouping);
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

/// The setup group `name` is cut with, if it has one of its own: cut by layer, the first
/// top-level layer of that name that keeps one; cut by colour, the colour's in the settings.
pub fn setup_of(d: &Document, s: &CuttingSettings, name: &str) -> Option<GroupSetup> {
    match s.grouping {
        Grouping::None => None,
        Grouping::Layer => d.layers.iter().filter(|l| layer_group_name(l) == name).find_map(|l| layer_setup(l, s)),
        Grouping::Color => s.color_setups.get(name).cloned(),
    }
}

#[cfg(test)]
mod tests {
    use vectorcraft_cutjob::{ToolSettings, builtin_materials};
    use vectorcraft_doc::LayerColor;

    use super::*;

    #[test]
    fn a_layer_keeps_its_setup() {
        let mut d = Document::new(100.0, 100.0);
        let s = CuttingSettings { grouping: Grouping::Layer, ..CuttingSettings::default() };
        let vinyl = builtin_materials().into_iter().find(|m| m.name == "Vinyl").unwrap();
        let setup = GroupSetup::of_material(&vinyl);
        let layer = Node::layer(d.alloc_id(), "Gold", LayerColor::Preset(1));
        let id = d.insert(None, usize::MAX, layer).unwrap();
        assert_eq!(setup_of(&d, &s, "Gold"), None);
        let n = d.node_mut(id).unwrap();
        assert!(set_layer_setup(n, Some(&setup)));
        assert!(!set_layer_setup(n, Some(&setup)), "the same again changes nothing");
        assert_eq!(setup_of(&d, &s, "Gold"), Some(setup.clone()));
        assert_eq!(setup_of(&d, &CuttingSettings::default(), "Gold"), None, "not cut by layer");
        assert!(set_layer_setup(d.node_mut(id).unwrap(), None));
        assert_eq!(setup_of(&d, &s, "Gold"), None);
    }

    /// A value this version can't read is none, and too long a one isn't read.
    #[test]
    fn a_damaged_setup_counts_as_none() {
        let s = CuttingSettings::default();
        let mut d = Document::new(100.0, 100.0);
        let id = d.layers[0].id;
        let put = |d: &mut Document, value: String| {
            d.node_mut(id).unwrap().edit_attrs(|a| {
                a.foreign.retain(|f| !ours(f));
                a.foreign.push(ForeignAttribute { ns: NS.into(), prefix: PREFIX.into(), name: NAME.into(), value });
            });
        };
        put(&mut d, "{not json".into());
        assert_eq!(layer_setup(&d.layers[0], &s), None);
        put(&mut d, format!("{{\"material\": \"{}\"}}", "x".repeat(MAX_VALUE)));
        assert_eq!(layer_setup(&d.layers[0], &s), None);
        put(&mut d, "{\"cut\": {\"passes\": 2}}".into());
        let got = layer_setup(&d.layers[0], &s).unwrap();
        assert_eq!((got.cut.passes, got.score), (2, ToolSettings { ..s.operations.score.clone() }), "the rest the document's");
    }

    #[test]
    fn a_colour_keeps_its_setup_in_the_settings() {
        let d = Document::new(100.0, 100.0);
        let card = builtin_materials().into_iter().find(|m| m.name == "Cardstock").unwrap();
        let mut s = CuttingSettings { grouping: Grouping::Color, ..CuttingSettings::default() };
        s.color_setups.insert("#ff0000".into(), GroupSetup::of_material(&card));
        assert_eq!(setup_of(&d, &s, "#ff0000").and_then(|g| g.material).as_deref(), Some("Cardstock"));
        assert_eq!(setup_of(&d, &s, "#0000ff"), None);
        let back = CuttingSettings::from_value(Some(&s.to_value()));
        assert_eq!(back.color_setups, s.color_setups, "kept in the document's settings");
    }
}
