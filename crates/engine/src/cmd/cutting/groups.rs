//! Fork: each group's own tool setup when the job is cut in groups (C7.4, ARCHITECTURE.md › 4.4).
//! Cut by layer it is kept on the layer, by colour in the document's settings
//! (`vectorcraft_cutting::setups`). Setting a group's material copies the material's whole setup
//! into the group, so the document cuts the same wherever it is opened; a number set by hand
//! after that counts as changed since the material. A group without a setup of its own is cut
//! with the document's.

use serde_json::{Map, Value, json};
use vectorcraft_cutjob::{GroupSetup, Grouping, MAX_GROUP_NAME, MAX_GROUP_SETUPS};
use vectorcraft_cutting::setups;

use super::{CommandSpec, Result, Session, bad, cmd, has_doc, str_param};

const SET: &str = "cutting.group.set";
const OP_KEYS: [&str; 2] = ["cut", "score"];
const TOOL_KEYS: [&str; 8] = ["force", "speedMmS", "passes", "overcutMm", "depth", "accel", "cornerOvercutMm", "bladeOffsetMm"];

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            query "cutting.groups",
            "Cutting Groups",
            [],
            None,
            "{} the groups a job cut by layer or by colour is cut in, in cutting order → {grouping, groups: [{name, setup: null (the document's) | {material: name or null, changed: whether set by hand since the material, cut, score: {force, speedMmS, passes, overcutMm, depth, accel, cornerOvercutMm, bladeOffsetMm}}}]}",
            has_doc,
            list
        ),
        cmd!(
            "cutting.group.set",
            "Set Group Cut Setup",
            [],
            None,
            "{group, material?: name, operations?: {cut?: {…tool keys}, score?: {…}}, clear?: true} one group's own setup, one undo step: material copies that material's whole setup in; operations sets numbers over the group's setup (or the document's, for a group without one); clear: cut with the document's again. Cut by layer it is kept on the layers of that name, by colour in the settings → as cutting.groups",
            has_doc,
            set
        ),
    ]
}

/// Whether `g` is no longer the setup of the material it names.
fn changed(g: &GroupSetup) -> bool {
    g.material.as_deref().and_then(super::materials::find).is_some_and(|m| g.differs_from(&m))
}

fn listing(s: &Session) -> Result<Value> {
    let d = &s.doc()?.doc;
    let settings = vectorcraft_cutting::settings(d);
    let groups: Vec<Value> = setups::group_names(d, &settings)
        .into_iter()
        .map(|name| {
            let setup = setups::setup_of(d, &settings, &name)
                .map(|g| json!({"material": g.material, "changed": changed(&g), "cut": g.cut.to_value(), "score": g.score.to_value()}));
            json!({"name": name, "setup": setup})
        })
        .collect();
    Ok(json!({"grouping": settings.grouping.id(), "groups": groups}))
}

fn list(s: &mut Session, _: &Value) -> Result<Value> {
    listing(s)
}

/// `m`'s keys are all `allowed`.
fn known_keys(m: &Map<String, Value>, allowed: &[&str], at: &str) -> Result<()> {
    match m.keys().find(|k| !allowed.contains(&k.as_str())) {
        Some(k) => Err(bad(SET, format!("unknown key {at}{k}: takes {}", allowed.join(", ")))),
        None => Ok(()),
    }
}

/// Each number `given` sets reads back as given in `got`: one clamped or read as its default
/// was out of range or not valid.
fn check_taken(given: &Map<String, Value>, got: &Value, at: &str) -> Result<()> {
    for (k, v) in given {
        let r = got.get(k).unwrap_or(&Value::Null);
        if let Some(inner) = v.as_object() {
            check_taken(inner, r, &format!("{at}{k}."))?;
        } else if !(v == r || v.as_f64().is_some_and(|a| r.as_f64() == Some(a))) {
            return Err(bad(SET, format!("{at}{k}: {v} is out of range or not valid")));
        }
    }
    Ok(())
}

fn set(s: &mut Session, p: &Value) -> Result<Value> {
    let params = p.as_object().ok_or_else(|| bad(SET, "params must be an object"))?;
    known_keys(params, &["group", "material", "operations", "clear"], "")?;
    let name = str_param(p, "group").ok_or_else(|| bad(SET, "group is required: one of cutting.groups' names"))?.to_string();
    let d = &s.doc()?.doc;
    let mut settings = vectorcraft_cutting::settings(d);
    if settings.grouping == Grouping::None {
        return Err(bad(SET, "the job isn't cut in groups: set grouping to layer or color first"));
    }
    let names = setups::group_names(d, &settings);
    let kept = settings.grouping == Grouping::Color && settings.color_setups.contains_key(&name);
    if !names.contains(&name) && !kept {
        return Err(bad(SET, format!("no group {name:?}: the groups are {}", names.join(", "))));
    }
    let clear = p.get("clear").and_then(Value::as_bool) == Some(true);
    let material = str_param(p, "material");
    let ops = p.get("operations");
    if clear && (material.is_some() || ops.is_some()) {
        return Err(bad(SET, "clear takes nothing else"));
    }
    if !clear && material.is_none() && ops.is_none() {
        return Err(bad(SET, "give material, operations or clear"));
    }
    let new = if clear {
        None
    } else {
        // The group's setup, else the document's, then the material, then the numbers.
        let mut g = setups::setup_of(d, &settings, &name).unwrap_or_else(|| GroupSetup {
            material: settings.material.clone(),
            cut: settings.operations.cut.clone(),
            score: settings.operations.score.clone(),
        });
        if let Some(m) = material {
            let found = super::materials::find(m).ok_or_else(|| bad(SET, format!("no material {m:?}")))?;
            g = GroupSetup::of_material(&found);
        }
        if let Some(ops) = ops {
            let ops = ops.as_object().ok_or_else(|| bad(SET, "operations must be an object"))?;
            known_keys(ops, &OP_KEYS, "operations.")?;
            let mut merged = g.to_value();
            for (op, tool) in ops {
                let tool = tool.as_object().ok_or_else(|| bad(SET, format!("operations.{op} must be an object")))?;
                known_keys(tool, &TOOL_KEYS, &format!("operations.{op}."))?;
                if let Some(target) = merged.get_mut(op).and_then(Value::as_object_mut) {
                    for (k, v) in tool {
                        target.insert(k.clone(), v.clone());
                    }
                }
            }
            g = GroupSetup::from_value(&merged, &settings.operations).ok_or_else(|| bad(SET, "operations aren't valid"))?;
            let back = g.to_value();
            for (op, tool) in ops {
                if let (Some(tool), Some(got)) = (tool.as_object(), back.get(op)) {
                    check_taken(tool, got, &format!("operations.{op}."))?;
                }
            }
        }
        Some(g)
    };
    match settings.grouping {
        Grouping::Layer => {
            let layers: Vec<vectorcraft_doc::NodeId> = d.layers.iter().filter(|l| setups::layer_group_name(l) == name).map(|l| l.id).collect();
            let differs = layers.iter().any(|id| d.node(*id).is_some_and(|n| setups::layer_setup(n, &settings) != new));
            if differs {
                s.edit("Group Cut Setup", |d, _| {
                    for id in &layers {
                        if let Some(n) = d.node_mut(*id) {
                            setups::set_layer_setup(n, new.as_ref());
                        }
                    }
                    Ok(())
                })?;
            }
        }
        Grouping::Color => {
            let before = settings.color_setups.clone();
            match new {
                Some(g) => {
                    if name.chars().count() > MAX_GROUP_NAME {
                        return Err(bad(SET, format!("a colour's name is at most {MAX_GROUP_NAME} characters")));
                    }
                    if !settings.color_setups.contains_key(&name) && settings.color_setups.len() >= MAX_GROUP_SETUPS {
                        return Err(bad(SET, format!("a document keeps at most {MAX_GROUP_SETUPS} colours' setups: clear some first")));
                    }
                    settings.color_setups.insert(name, g);
                }
                None => {
                    settings.color_setups.remove(&name);
                }
            }
            if settings.color_setups != before {
                s.edit("Group Cut Setup", |d, _| {
                    vectorcraft_cutting::set_settings(d, &settings);
                    Ok(())
                })?;
            }
        }
        Grouping::None => {}
    }
    listing(s)
}
