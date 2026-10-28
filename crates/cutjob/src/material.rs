//! Materials (C5.1): a named pair of tool settings, for cutting and for scoring, chosen instead of
//! setting each number. The few built in are starting points of our own, conservative, for a
//! test cut before the real one; the ones that matter are the user's own, saved after a good test
//! cut on their machine, with the whole tool setup (blade depth and offset too, C5.5). No maker's
//! material table is used.

use serde_json::{Value, json};

use crate::read::object;
use crate::settings::ToolSettings;

/// Longest material name.
pub const MAX_NAME: usize = 64;
/// Most materials a user keeps.
pub const MAX_MATERIALS: usize = 200;

/// A material: how to cut it and how to score it.
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    pub name: String,
    pub cut: ToolSettings,
    pub score: ToolSettings,
    /// Built in (a starting point that can't be changed or deleted), else the user's.
    pub builtin: bool,
}

fn tool(force: f64, speed_mm_s: f64, passes: u8) -> ToolSettings {
    ToolSettings { force, speed_mm_s, passes, overcut_mm: 0.0, ..ToolSettings::default() }
}

/// The starting points built in: light settings to test from, not tested values.
pub fn builtin_materials() -> Vec<Material> {
    let m = |name: &str, cut, score| Material { name: name.into(), cut, score, builtin: true };
    vec![
        m("Paper (light)", tool(0.2, 100.0, 1), tool(0.08, 100.0, 1)),
        m("Cardstock", tool(0.45, 80.0, 2), tool(0.15, 100.0, 1)),
        m("Vinyl", tool(0.25, 100.0, 1), tool(0.1, 100.0, 1)),
    ]
}

/// A name a material may have: trimmed, not empty, at most [`MAX_NAME`] characters.
pub fn valid_name(name: &str) -> Option<String> {
    let n = name.trim();
    (!n.is_empty() && n.chars().count() <= MAX_NAME).then(|| n.to_string())
}

impl Material {
    /// As kept in the user's file.
    pub fn to_value(&self) -> Value {
        json!({"name": self.name, "cut": self.cut.to_value(), "score": self.score.to_value()})
    }

    /// One read from the user's file (leniently, as the document's settings are), `None` when it
    /// has no valid name. Always the user's.
    pub fn from_value(v: &Value) -> Option<Material> {
        let m = v.as_object()?;
        let name = valid_name(m.get("name")?.as_str()?)?;
        let defaults = crate::settings::Operations::default();
        Some(Material {
            name,
            cut: ToolSettings::read_lenient(object(m, "cut"), defaults.cut),
            score: ToolSettings::read_lenient(object(m, "score"), defaults.score),
            builtin: false,
        })
    }
}

/// The user's materials in `v` (their file's JSON): the valid ones, the first of each name, at
/// most [`MAX_MATERIALS`]. Anything else in the file is left out.
pub fn read_materials(v: &Value) -> Vec<Material> {
    let mut out: Vec<Material> = vec![];
    for m in v.as_array().into_iter().flatten().filter_map(Material::from_value) {
        if out.len() >= MAX_MATERIALS {
            break;
        }
        if !out.iter().any(|o| o.name.eq_ignore_ascii_case(&m.name)) {
            out.push(m);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materials_round_trip_through_their_file() {
        let user = Material { name: "My vinyl".into(), cut: tool(0.3, 90.0, 1), score: tool(0.1, 100.0, 1), builtin: false };
        let file = json!([user.to_value()]);
        assert_eq!(read_materials(&file), [user]);
    }

    /// C5.5: the blade's setup is kept with the material, as the force and speed are.
    #[test]
    fn the_blade_setup_is_kept() {
        let cut = ToolSettings { depth: Some(4), accel: Some(2), corner_overcut_mm: 0.3, blade_offset_mm: Some(0.25), ..tool(0.3, 90.0, 1) };
        let user = Material { name: "Thick card".into(), cut, score: tool(0.1, 100.0, 1), builtin: false };
        let back = read_materials(&json!([user.to_value()]));
        assert_eq!(back, std::slice::from_ref(&user));
        assert!(back[0].cut.same_setup(&user.cut));
        assert!(!back[0].cut.same_setup(&ToolSettings { blade_offset_mm: None, ..user.cut.clone() }));
    }

    #[test]
    fn a_damaged_file_gives_what_it_can() {
        let file = json!([
            {"name": "  Felt  ", "cut": {"force": 9, "passes": 40}},
            {"name": ""},
            {"name": "felt", "cut": {}},
            "junk",
            {"cut": {}}
        ]);
        let got = read_materials(&file);
        assert_eq!(got.len(), 1, "{got:?}");
        assert_eq!((got[0].name.as_str(), got[0].cut.force, got[0].cut.passes), ("Felt", 1.0, 10));
        assert!(read_materials(&json!({"not": "a list"})).is_empty());
        let many: Vec<Value> = (0..500).map(|i| json!({"name": format!("m{i}")})).collect();
        assert_eq!(read_materials(&Value::Array(many)).len(), MAX_MATERIALS);
    }

    #[test]
    fn builtins_are_light_starting_points() {
        for m in builtin_materials() {
            assert!(m.builtin && m.cut.problem().is_none() && m.score.problem().is_none());
            assert!(m.score.force < m.cut.force, "{}: scoring lighter than cutting", m.name);
        }
        assert_eq!(valid_name(&"x".repeat(MAX_NAME + 1)), None);
        assert_eq!(valid_name("  a  ").as_deref(), Some("a"));
    }
}
