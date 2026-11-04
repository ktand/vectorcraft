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
    /// Cut from the back, as heat transfer vinyl is (C14.12): applying it turns Cut Mirrored on,
    /// and a send asks whether to cut mirrored or as is.
    pub mirror: bool,
    /// What it costs (C18.3), for the job's quote; none: not given.
    pub price: Option<Price>,
    /// What's left of its roll, mm (C18.11): each send cut from it on a roll takes off its length;
    /// none: not kept.
    pub roll_left_mm: Option<f64>,
}

/// The longest roll kept, mm.
pub const MAX_ROLL_MM: f64 = 1_000_000.0;

/// What a material costs (C18.3), in whatever currency the user keeps their prices in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Price {
    pub amount: f64,
    /// Per sheet (a mat's worth), or per metre of a roll.
    pub per_metre: bool,
    /// What's spoilt or trimmed, added to a roll's length, %.
    pub waste_pct: f64,
}

impl Price {
    /// The amount's range.
    pub const AMOUNT: (f64, f64) = (0.0, 1_000_000.0);
    /// The waste's range, %.
    pub const WASTE_PCT: (f64, f64) = (0.0, 100.0);

    pub fn to_value(&self) -> Value {
        json!({"amount": self.amount, "per": if self.per_metre { "metre" } else { "sheet" }, "wastePct": self.waste_pct})
    }

    /// Read leniently: none unless the amount is a number in range.
    pub fn from_value(v: &Value) -> Option<Price> {
        let amount = v.get("amount")?.as_f64().filter(|a| a.is_finite() && (Self::AMOUNT.0..=Self::AMOUNT.1).contains(a))?;
        let per_metre = v.get("per").and_then(Value::as_str) == Some("metre");
        let waste_pct =
            v.get("wastePct").and_then(Value::as_f64).filter(|w| w.is_finite()).map_or(0.0, |w| w.clamp(Self::WASTE_PCT.0, Self::WASTE_PCT.1));
        Some(Price { amount, per_metre, waste_pct })
    }

    /// The cost of `sheets` sheets, or of `metres` of a roll (the waste added).
    pub fn cost(&self, sheets: f64, metres: f64) -> f64 {
        if self.per_metre { self.amount * metres * (1.0 + self.waste_pct / 100.0) } else { self.amount * sheets }
    }
}

fn tool(force: f64, speed_mm_s: f64, passes: u8) -> ToolSettings {
    ToolSettings { force, speed_mm_s, passes, overcut_mm: 0.0, ..ToolSettings::default() }
}

/// The starting points built in: light settings to test from, not tested values.
pub fn builtin_materials() -> Vec<Material> {
    let m = |name: &str, cut, score| Material { name: name.into(), cut, score, builtin: true, mirror: false, price: None, roll_left_mm: None };
    vec![
        m("Paper (light)", tool(0.2, 100.0, 1), tool(0.08, 100.0, 1)),
        m("Cardstock", tool(0.45, 80.0, 2), tool(0.15, 100.0, 1)),
        m("Vinyl", tool(0.25, 100.0, 1), tool(0.1, 100.0, 1)),
        Material { mirror: true, ..m("Heat Transfer Vinyl", tool(0.3, 100.0, 1), tool(0.1, 100.0, 1)) },
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
        let mut v = json!({"name": self.name, "cut": self.cut.to_value(), "score": self.score.to_value()});
        if let (true, Some(m)) = (self.mirror, v.as_object_mut()) {
            m.insert("mirror".into(), json!(true));
        }
        if let (Some(p), Some(m)) = (self.price, v.as_object_mut()) {
            m.insert("price".into(), p.to_value());
        }
        if let (Some(l), Some(m)) = (self.roll_left_mm, v.as_object_mut()) {
            m.insert("rollLeftMm".into(), json!(l));
        }
        v
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
            mirror: m.get("mirror").and_then(Value::as_bool) == Some(true),
            price: m.get("price").and_then(Price::from_value),
            roll_left_mm: m.get("rollLeftMm").and_then(Value::as_f64).filter(|l| l.is_finite()).map(|l| l.clamp(0.0, MAX_ROLL_MM)),
        })
    }
}

/// The tool setup one group of a job cut in groups is cut with (C7.4): a copy of the whole setup,
/// so the document cuts the same wherever it is opened, and the name of the material it came from
/// (none when set by hand). A group without one is cut with the document's setup.
#[derive(Clone, Debug, PartialEq)]
pub struct GroupSetup {
    pub material: Option<String>,
    pub cut: ToolSettings,
    pub score: ToolSettings,
}

impl GroupSetup {
    /// As the document keeps it (the shape of a material in the user's file, its name optional).
    pub fn to_value(&self) -> Value {
        let mut v = json!({"cut": self.cut.to_value(), "score": self.score.to_value()});
        if let (Some(name), Some(m)) = (&self.material, v.as_object_mut()) {
            m.insert("material".into(), json!(name));
        }
        v
    }

    /// One read leniently from the document (as its settings are): anything that isn't an object
    /// is none; missing settings are `defaults`'.
    pub fn from_value(v: &Value, defaults: &crate::settings::Operations) -> Option<GroupSetup> {
        let m = v.as_object()?;
        Some(GroupSetup {
            material: m.get("material").and_then(Value::as_str).and_then(valid_name),
            cut: ToolSettings::read_lenient(object(m, "cut"), defaults.cut.clone()),
            score: ToolSettings::read_lenient(object(m, "score"), defaults.score.clone()),
        })
    }

    /// The setup `m` gives.
    pub fn of_material(m: &Material) -> GroupSetup {
        GroupSetup { material: Some(m.name.clone()), cut: m.cut.clone(), score: m.score.clone() }
    }

    /// The setup is no longer `m`'s.
    pub fn differs_from(&self, m: &Material) -> bool {
        !self.cut.same_setup(&m.cut) || !self.score.same_setup(&m.score)
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

    /// A price round-trips; one out of range or not a number is none.
    #[test]
    fn prices_round_trip() {
        let p = Price { amount: 2.5, per_metre: true, waste_pct: 10.0 };
        assert_eq!(Price::from_value(&p.to_value()), Some(p));
        assert!((p.cost(0.0, 2.0) - 5.5).abs() < 1e-9);
        assert!((Price { per_metre: false, ..p }.cost(3.0, 0.0) - 7.5).abs() < 1e-9);
        for bad in [json!({}), json!({"amount": "2"}), json!({"amount": -1}), json!({"amount": 2e9})] {
            assert_eq!(Price::from_value(&bad), None, "{bad}");
        }
        let mut m = builtin_materials()[0].clone();
        m.builtin = false;
        m.price = Some(p);
        assert_eq!(Material::from_value(&m.to_value()).and_then(|m| m.price), Some(p));
    }

    #[test]
    fn materials_round_trip_through_their_file() {
        let user = Material {
            name: "My vinyl".into(),
            cut: tool(0.3, 90.0, 1),
            score: tool(0.1, 100.0, 1),
            builtin: false,
            mirror: false,
            price: None,
            roll_left_mm: None,
        };
        let file = json!([user.to_value()]);
        assert_eq!(read_materials(&file), std::slice::from_ref(&user));
        assert!(file[0].get("mirror").is_none(), "only a mark is written");
        // C14.12: cut from the back, kept.
        let iron_on = Material { name: "My iron-on".into(), mirror: true, ..user };
        assert_eq!(read_materials(&json!([iron_on.to_value()])), [iron_on]);
        assert!(!read_materials(&json!([{"name": "x", "mirror": "yes"}]))[0].mirror, "read leniently");
    }

    /// C5.5: the blade's setup is kept with the material, as the force and speed are.
    #[test]
    fn the_blade_setup_is_kept() {
        let cut = ToolSettings { depth: Some(4), accel: Some(2), corner_overcut_mm: 0.3, blade_offset_mm: Some(0.25), ..tool(0.3, 90.0, 1) };
        let user =
            Material { name: "Thick card".into(), cut, score: tool(0.1, 100.0, 1), builtin: false, mirror: false, price: None, roll_left_mm: None };
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

    /// C7.4: a group's setup round-trips, keeps its material's name, and reads leniently.
    #[test]
    fn group_setups_round_trip() {
        let defaults = crate::settings::Operations::default();
        let cardstock = builtin_materials().into_iter().find(|m| m.name == "Cardstock").unwrap();
        let setup = GroupSetup::of_material(&cardstock);
        assert_eq!(GroupSetup::from_value(&setup.to_value(), &defaults), Some(setup.clone()));
        assert!(!setup.differs_from(&cardstock));
        let by_hand = GroupSetup { material: None, cut: ToolSettings { passes: 3, ..setup.cut.clone() }, ..setup };
        assert!(by_hand.differs_from(&cardstock));
        assert_eq!(GroupSetup::from_value(&by_hand.to_value(), &defaults), Some(by_hand));
        let damaged = GroupSetup::from_value(&json!({"material": "", "cut": {"force": 7}}), &defaults).unwrap();
        assert_eq!((damaged.material, damaged.cut.force, damaged.score), (None, 1.0, defaults.score.clone()));
        assert_eq!(GroupSetup::from_value(&json!("junk"), &defaults), None);
    }

    #[test]
    fn builtins_are_light_starting_points() {
        for m in builtin_materials() {
            assert!(m.builtin && m.cut.problem().is_none() && m.score.problem().is_none());
            assert!(m.score.force < m.cut.force, "{}: scoring lighter than cutting", m.name);
        }
        let marked: Vec<String> = builtin_materials().into_iter().filter(|m| m.mirror).map(|m| m.name).collect();
        assert_eq!(marked, ["Heat Transfer Vinyl"], "C14.12");
        assert_eq!(valid_name(&"x".repeat(MAX_NAME + 1)), None);
        assert_eq!(valid_name("  a  ").as_deref(), Some("a"));
    }
}
