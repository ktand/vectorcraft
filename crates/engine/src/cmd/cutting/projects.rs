//! Fork: guided projects (C18.12): the settings a kind of project needs, set in one step, for
//! the Projects panel's checklists (which also call the existing commands for the other steps:
//! the sticker sheet, the stencil's bridges, the Send window). An iron-on design is cut from the
//! back of heat transfer vinyl: mirrored, with a weed box; an adhesive vinyl decal is cut as
//! drawn, with a weed box and weed lines. Either, with more than one fill colour, is cut colour by
//! colour, knocked out so the colours don't pile up, with a cross to line them up by.

use serde_json::{Value, json};
use vectorcraft_cutjob::Grouping;
use vectorcraft_cutting::setups;

use super::materials::{find, with_material};
use super::{CommandSpec, Result, Session, bad, cmd, has_doc, str_param};

const CMD: &str = "cutting.project.setup";

/// The projects `cutting.project.setup` sets up, and the material each starts from when the
/// document has none of the right kind (a built-in one).
const PROJECTS: [(&str, &str, &str); 2] =
    [("ironOn", "Set Up Iron-On Project", "Heat Transfer Vinyl"), ("vinylDecal", "Set Up Vinyl Decal Project", "Vinyl")];

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.project.setup",
        "Set Up Cutting Project",
        [],
        None,
        "{project: ironOn|vinylDecal} the document's cutting settings for a kind of project, one undo step (C18.12): ironOn (heat transfer vinyl): cut mirrored, a weed box, the material Heat Transfer Vinyl unless one cut from the back is chosen; vinylDecal (adhesive vinyl): not mirrored, a weed box and weed lines, the material Vinyl unless one not cut from the back is chosen; either, with more than one fill colour, cut by fill colour (or by layer, if so already) with knockout and align marks, else without them → {project, colours: the design's fill colours, material, settings: as cutting.settings.get's}",
        has_doc,
        setup
    )]
}

fn setup(s: &mut Session, p: &Value) -> Result<Value> {
    let project = str_param(p, "project").ok_or_else(|| bad(CMD, "project is required: ironOn or vinylDecal"))?;
    let &(id, label, starting) =
        PROJECTS.iter().find(|(id, ..)| *id == project).ok_or_else(|| bad(CMD, format!("project is ironOn or vinylDecal, not {project:?}")))?;
    let iron_on = id == "ironOn";
    let d = &s.doc()?.doc;
    let mut settings = vectorcraft_cutting::settings(d);
    let before = settings.clone();
    // How many colours of vinyl: the design's fill colours, as cutting by fill colour groups them.
    let mut probe = settings.clone();
    probe.grouping = Grouping::FillColor;
    let colours = setups::group_names(d, &probe).len();
    // The material: the one chosen if it is of the right kind (cut from the back or not), else
    // the built-in starting point.
    let chosen = settings.material.as_deref().and_then(find).filter(|m| m.mirror == iron_on);
    if let Some(m) = chosen.or_else(|| find(starting)) {
        with_material(&mut settings, &m);
    }
    settings.mirror = iron_on;
    settings.print_and_cut = false;
    settings.weed_box = true;
    settings.weed_lines = !iron_on;
    let layered = colours > 1;
    if layered && settings.grouping != Grouping::Layer {
        settings.grouping = Grouping::FillColor;
    }
    settings.knockout = layered;
    settings.align_marks = layered;
    if settings != before {
        s.edit(label, |d, _| {
            vectorcraft_cutting::set_settings(d, &settings);
            Ok(())
        })?;
    }
    let got = s.execute("cutting.settings.get", &json!({}))?;
    Ok(json!({"project": id, "colours": colours, "material": settings.material, "settings": got["settings"]}))
}
