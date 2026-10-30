//! Fork: the Cut Settings panel (`docs/cutting/ARCHITECTURE.md`; the id is still `cutting`, so
//! saved workspaces open it): the job in a line with Send to Cutter… (the Send window, C7.1, runs
//! the cut), the selection's operation, the document's default operation and groups, the
//! material and tool settings (cut and score side by side, C9.1), the rarely changed ones behind
//! Advanced (C7.2). The mat and Print & Cut are the Cutting Mat panel's ([`mat`]). It reads the
//! engine's `cutting.*` queries and acts through `app.run`, as every panel does.

use egui::Ui;
use serde_json::{Value, json};
use vectorcraft_doc::Unit;

use vectorcraft_cutter_silhouette as silhouette;

use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub mod mat;

pub const ID: &str = "cutting";

/// Points per millimetre: the number fields take points and show millimetres.
const PT: f64 = 72.0 / 25.4;
const LABEL_W: f32 = 80.0;
/// A number field in the tool table.
const CELL_W: f32 = 60.0;

fn set(app: &mut VectorcraftApp, p: Value) {
    // A refusal (a value out of range) leaves the settings as they were; the field shows them.
    let _ = app.run("cutting.settings.set", p);
}

fn mm_field(ui: &mut Ui, id: &'static str, mm: f64, min_mm: f64) -> Option<f64> {
    widgets::spin_field(ui, ("cutting", id), Some(mm * PT), Unit::Millimeters, 64.0, PT, min_mm * PT, &[]).map(|pt| pt / PT)
}

/// What the document cuts by default and how it groups the cut.
fn job_settings_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    let s = &info["settings"];
    let ops = ["Cut", "Cut Outline", "Score", "No Cut"];
    let op_ids = ["cut", "outline", "score", "none"];
    let default = op_ids.iter().position(|o| s["defaultOp"] == *o).unwrap_or(0);
    // The operation of objects that, with their groups and layers, set none.
    widgets::label_row(ui, tl!("Default"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "default"), ops[default], &ops, 120.0) {
            set(app, json!({ "defaultOp": op_ids.get(i).copied().unwrap_or("cut") }));
        }
    });
    let groupings = ["None", "By Layer", "By Colour"];
    let grouping_ids = ["none", "layer", "color"];
    let grouping = grouping_ids.iter().position(|g| s["grouping"] == *g).unwrap_or(0);
    // Cut in groups, with a pause before each after the first to change the tool or material.
    widgets::label_row(ui, tl!("Groups"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "grouping"), groupings[grouping], &groupings, 120.0) {
            set(app, json!({ "grouping": grouping_ids.get(i).copied().unwrap_or("none") }));
        }
    });
}

/// Weeding (C8.2, C8.3): a box round the design and lines from it in to the design, cut by the
/// job (not added to the art), so vinyl's waste comes away easily.
fn weeding_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    let s = &info["settings"];
    let boxed = s["weedBox"].as_bool() == Some(true);
    if widgets::check(ui, tl!("Weed Box"), boxed, true) {
        set(app, json!({ "weedBox": !boxed }));
    }
    if boxed {
        let margin = s["weedMarginMm"].as_f64().unwrap_or(3.0);
        widgets::label_row(ui, tl!("Margin"), LABEL_W, |ui| {
            if let Some(v) = mm_field(ui, "weed-margin", margin, 0.0) {
                set(app, json!({ "weedMarginMm": v.clamp(0.0, 50.0) }));
            }
        });
    }
    let lines = s["weedLines"].as_bool() == Some(true);
    if widgets::check(ui, tl!("Weed Lines"), lines, true) {
        set(app, json!({ "weedLines": !lines }));
    }
    if lines {
        let spacing = s["weedLineSpacingMm"].as_f64().unwrap_or(0.0);
        widgets::label_row(ui, tl!("Every"), LABEL_W, |ui| {
            if let Some(v) = mm_field(ui, "weed-spacing", spacing, 0.0) {
                set(app, json!({ "weedLineSpacingMm": v.clamp(0.0, 200.0) }));
            }
        });
        widgets::dim_label(ui, tl!("0: one line in from the middle of each side."));
    }
}

/// A group row's material choices: the document's setup first, then every material.
fn group_choices(app: &mut VectorcraftApp) -> Vec<String> {
    let listed = app.session.execute("cutting.materials", &json!({})).unwrap_or_default();
    let names = listed["materials"].as_array().into_iter().flatten().filter_map(|m| m["name"].as_str().map(str::to_string));
    std::iter::once(tl!("Document's setup").to_string()).chain(names).collect()
}

/// Choice `i` of [`group_choices`] for group `group`: the first goes back to the document's setup,
/// the others copy that material's in.
fn choose_group_material(app: &mut VectorcraftApp, group: &str, choices: &[String], i: usize) -> Result<Value, String> {
    match (i, choices.get(i)) {
        (0, _) => app.run("cutting.group.set", json!({ "group": group, "clear": true })),
        (_, Some(name)) => app.run("cutting.group.set", json!({ "group": group, "material": name })),
        (_, None) => Err("no such material".into()),
    }
}

/// Cut in groups (C7.5): one row for each group, in the order they are cut: its name, the
/// material its setup came from (or the document's setup), and the cut's force and passes. A
/// change sets that group's own setup (`cutting.group.set`); the rest of its setup is in the
/// material. Groups without one are cut with the tool settings below.
fn group_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    let t = Tokens::get(ui.ctx());
    let Ok(listed) = app.session.execute("cutting.groups", &json!({})) else { return };
    let groups = listed["groups"].as_array().cloned().unwrap_or_default();
    if groups.is_empty() {
        widgets::dim_label(ui, tl!("Nothing to cut yet."));
        return;
    }
    let choices = group_choices(app);
    let names: Vec<&str> = choices.iter().map(String::as_str).collect();
    let doc_cut = &info["settings"]["operations"]["cut"];
    let mut error = None;
    // A grid, so the numbers line up under their headings: a group's name on a row of its own,
    // its material, force and passes on the next.
    egui::Grid::new(("cutting", "groups")).num_columns(3).spacing([6.0, 3.0]).show(ui, |ui| {
        for heading in ["Material", "Force", "Passes"] {
            widgets::dim_label(ui, tl!(heading));
        }
        ui.end_row();
        for (i, g) in groups.iter().enumerate() {
            let Some(name) = g["name"].as_str() else { continue };
            let setup = &g["setup"];
            ui.label(egui::RichText::new(format!("{}. {name}", i + 1)).strong());
            if setup["changed"] == true {
                widgets::dim_label(ui, tl!("changed"));
            }
            ui.end_row();
            let shown = if setup.is_null() {
                choices.first().map(String::as_str).unwrap_or_default()
            } else {
                setup["material"].as_str().unwrap_or(tl!("Set by hand"))
            };
            let cut = if setup.is_null() { doc_cut } else { &setup["cut"] };
            if let Some(k) = widgets::dropdown_names(ui, ("cutting", "group", i, "material"), shown, &names, 130.0) {
                error = choose_group_material(app, name, &choices, k).err();
            }
            let (f_lo, f_hi) = silhouette::FORCE;
            let step = silhouette::force_step(cut["force"].as_f64().unwrap_or(0.3));
            if let Some(v) = widgets::spin_plain(ui, ("cutting", "group", i, "force"), f64::from(step), "", 0, 44.0, 1.0, f64::from(f_lo), &[]) {
                let force = silhouette::force_from_step(v.clamp(f64::from(f_lo), f64::from(f_hi)).round() as u32);
                error = app.run("cutting.group.set", json!({ "group": name, "operations": { "cut": { "force": force } } })).err();
            }
            let passes = cut["passes"].as_f64().unwrap_or(1.0);
            if let Some(v) = widgets::spin_plain(ui, ("cutting", "group", i, "passes"), passes, "", 0, 44.0, 1.0, 1.0, &[]) {
                error =
                    app.run("cutting.group.set", json!({ "group": name, "operations": { "cut": { "passes": v.clamp(1.0, 10.0).round() } } })).err();
            }
            ui.end_row();
        }
    });
    if let Some(e) = error {
        ui.label(egui::RichText::new(e).color(t.bleed));
    }
    widgets::dim_label(ui, tl!("Force and passes of the cut; groups on the document's setup use the tool settings below."));
}

/// Cut, Score, No Cut or the group's or layer's, for the selected objects: a row only while
/// something is selected (C9.1: one dropdown, at the top).
fn selection_row(app: &mut VectorcraftApp, ui: &mut Ui) {
    let objects = app.session.execute("cutting.op.get", &json!({})).ok().and_then(|r| r["objects"].as_array().cloned()).unwrap_or_default();
    if objects.is_empty() {
        return;
    }
    let labels = ["Cut", "Cut Outline", "Score", "No Cut", "As Its Group or Layer"];
    let ids = ["cut", "outline", "score", "none", "inherit"];
    let all = |f: &dyn Fn(&Value) -> bool| objects.iter().all(f);
    let current = if all(&|o| o["source"] != "own") {
        Some(4)
    } else if all(&|o| o["source"] == "own") {
        ids.iter().position(|id| all(&|o| o["op"] == *id))
    } else {
        None
    };
    let shown = current.and_then(|i| labels.get(i)).copied().unwrap_or("Mixed");
    widgets::label_row(ui, tl!("Selected"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "selection-op"), shown, &labels, 150.0) {
            let _ = app.run("cutting.op.set", json!({ "op": ids.get(i).copied().unwrap_or("inherit") }));
        }
    });
}

/// The settings most cuts leave alone, behind a fold that starts closed: each operation's
/// acceleration, corner overcut and blade offset, and the job's track enhancing.
fn advanced_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    if !fold(ui, "cutting-advanced", tl!("Advanced")) {
        return;
    }
    let ops = &info["settings"]["operations"];
    egui::Grid::new(("cutting", "advanced")).num_columns(3).spacing([8.0, 3.0]).show(ui, |ui| {
        table_heading(ui);
        tool_row(ui, "Accel.", "The machine's acceleration steps, 1–3; 0: the machine's own.", |ui, op| {
            if let Some(v) = cell(ui, op, "accel", "Accel.", ops[op]["accel"].as_f64().unwrap_or(0.0), "", 0, 1.0, 0.0) {
                set_optional(app, op, "accel", v, 3.0);
            }
        });
        tool_row(ui, "Corners", "How far past each corner the blade goes, 0–2 mm.", |ui, op| {
            if let Some(v) = cell(ui, op, "corners", "Corners", ops[op]["cornerOvercutMm"].as_f64().unwrap_or(0.0), " mm", 1, 0.1, 0.0) {
                set(app, json!({ "operations": { op: { "cornerOvercutMm": (v.clamp(0.0, 2.0) * 10.0).round() / 10.0 } } }));
            }
        });
        // The machine turns the blade round corners by this; the usual blade's until set.
        tool_row(ui, "Blade offset", "How far the blade's tip trails its axis, 0–2 mm.", |ui, op| {
            let offset = ops[op]["bladeOffsetMm"].as_f64().unwrap_or(silhouette::BLADE_OFFSET_MM);
            if let Some(v) = cell(ui, op, "blade-offset", "Blade offset", offset, " mm", 2, 0.05, 0.0) {
                set(app, json!({ "operations": { op: { "bladeOffsetMm": (v.clamp(0.0, 2.0) * 20.0).round() / 20.0 } } }));
            }
        });
    });
    let s = &info["settings"];
    let track = s["trackEnhancing"].as_bool() == Some(true);
    if widgets::check(ui, tl!("Track Enhancing"), track, true) {
        set(app, json!({ "trackEnhancing": !track }));
    }
}

/// The tool table's heading row: the two operations over their columns.
fn table_heading(ui: &mut Ui) {
    ui.label("");
    for label in ["Cut", "Score"] {
        ui.label(egui::RichText::new(tl!(label)).strong());
    }
    ui.end_row();
}

/// A row of the tool table: `label` (its range and meaning on hover), then `add` for the cut's
/// cell and the score's.
fn tool_row(ui: &mut Ui, label: &str, hover: &str, mut add: impl FnMut(&mut Ui, &'static str)) {
    widgets::dim_label(ui, tl!(label)).on_hover_text(tl!(hover));
    for op in ["cut", "score"] {
        add(ui, op);
    }
    ui.end_row();
}

/// A number cell of the tool table, named for screen readers by its row and operation.
#[allow(clippy::too_many_arguments)]
fn cell(ui: &mut Ui, op: &'static str, key: &'static str, row: &str, v: f64, suffix: &str, decimals: usize, step: f64, min: f64) -> Option<f64> {
    let name = format!("{} {}", if op == "cut" { tl!("Cut") } else { tl!("Score") }, tl!(row));
    widgets::spin_plain_named(ui, ("cutting", op, key), Some(&name), v, suffix, decimals, CELL_W, step, min, &[])
}

/// 0: the machine's own setting, sent as nothing.
fn set_optional(app: &mut VectorcraftApp, op: &str, key: &str, v: f64, hi: f64) {
    let step = v.clamp(0.0, hi).round();
    let value = if step < 1.0 { Value::Null } else { json!(step) };
    set(app, json!({ "operations": { op: { key: value } } }));
}

/// A heading that opens and closes what follows (closed at first; kept for the session).
pub(crate) fn fold(ui: &mut Ui, key: &str, label: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut open: bool = super::pstate(&ctx, key);
    let (rect, resp) = ui.allocate_exact_size(egui::vec2(ui.available_width(), 20.0), egui::Sense::click());
    let chevron = egui::Rect::from_center_size(egui::pos2(rect.left() + 7.0, rect.center().y), egui::vec2(12.0, 12.0));
    let color = if resp.hovered() { t.text_strong } else { t.text };
    crate::icons::paint(ui, if open { "chevron-down" } else { "chevron-right" }, chevron, color);
    ui.painter().text(egui::pos2(rect.left() + 18.0, rect.center().y), egui::Align2::LEFT_CENTER, label, egui::FontId::proportional(12.5), color);
    if resp.clicked() {
        open = !open;
        super::set_pstate(&ctx, key, open);
    }
    open
}

/// The job in a line and the Send window a click away (C7.1); while a cut runs, how far it has got,
/// so it shows with the window closed.
fn send_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    match crate::cutting::job(app, &ctx).as_deref() {
        Some(Ok(made)) => {
            let warnings = made.job.warnings.len();
            let n = made.stats.path_count;
            let paths = if n == 1 { tl!("path") } else { tl!("paths") };
            let mut line = format!("{n} {paths} · {:.0} mm · {:.0} s", made.stats.blade_mm, made.sim.total_seconds);
            if warnings > 0 {
                line = format!("{line} · {warnings} {}", tl!("warnings"));
            }
            ui.label(egui::RichText::new(line).color(if warnings > 0 { t.warning } else { t.text_dim }));
        }
        Some(Err(e)) => {
            ui.label(egui::RichText::new(e).color(t.bleed));
        }
        None => {}
    }
    let status = app.session.execute("cutting.status", &json!({})).unwrap_or_default();
    if let Some(group) = status["waitingFor"].as_str() {
        ui.label(egui::RichText::new(format!("{} {group}.", tl!("Paused before"))).color(t.text_strong));
    } else if status["sending"].as_bool() == Some(true) {
        let (sent, all) = (status["chunksSent"].as_u64().unwrap_or(0), status["chunks"].as_u64().unwrap_or(0));
        widgets::dim_label(ui, &format!("{} {sent} / {all}", tl!("Sending:")));
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
    if widgets::primary_button(ui, "Send to Cutter…").clicked() {
        crate::cutting::send_window::set_open(&ctx, true);
    }
}

/// The material the tool settings come from (C5.1): built-in starting points and the user's own,
/// whether they were changed by hand since, and saving them as a material of the user's.
fn material_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let Ok(info) = app.session.execute("cutting.materials", &json!({})) else { return };
    let list: Vec<(String, bool)> = info["materials"]
        .as_array()
        .map(|a| a.iter().filter_map(|m| Some((m["name"].as_str()?.to_string(), m["builtin"].as_bool()?))).collect())
        .unwrap_or_default();
    let current = info["current"].as_str().map(str::to_string);
    let names: Vec<&str> = list.iter().map(|(n, _)| n.as_str()).collect();
    let shown = current.as_deref().unwrap_or(tl!("None"));
    widgets::label_row(ui, tl!("Material"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown_names(ui, ("cutting", "material"), shown, &names, 170.0)
            && let Some((name, _)) = list.get(i)
        {
            let _ = app.run("cutting.material.apply", json!({ "name": name }));
        }
    });
    if info["changed"].as_bool() == Some(true) {
        widgets::dim_label(ui, tl!("Changed since the material was chosen."));
    } else if current.as_deref().is_some_and(|c| list.iter().any(|(n, b)| n == c && *b)) {
        widgets::dim_label(ui, tl!("A starting point: make a test cut first."));
    }
    // The name field only once Save as Material… is clicked (C9.1).
    let mut saving: Option<String> = super::pstate(&ctx, "cutting-material-saving");
    let mut error: Option<String> = super::pstate(&ctx, "cutting-material-error");
    ui.horizontal(|ui| {
        if let Some(name) = &mut saving {
            let field = ui.add(egui::TextEdit::singleline(name).hint_text(tl!("Material name")).desired_width(120.0));
            let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (ui.add_enabled(!name.trim().is_empty(), egui::Button::new(tl!("Save"))).clicked() || entered) && !name.trim().is_empty() {
                error = app.run("cutting.material.save", json!({ "name": name.trim() })).err();
                if error.is_none() {
                    saving = None;
                }
            } else if ui.button(tl!("Cancel")).clicked() {
                saving = None;
                error = None;
            }
            return;
        }
        if ui.small_button(tl!("Save as Material…")).clicked() {
            saving = Some(String::new());
        }
        let own = current.as_deref().is_some_and(|c| list.iter().any(|(n, b)| n == c && !*b));
        if own
            && ui.small_button(tl!("Delete")).clicked()
            && let Some(c) = &current
        {
            error = app.run("cutting.material.delete", json!({ "name": c })).err();
        }
    });
    if let Some(e) = &error {
        ui.label(egui::RichText::new(e).color(t.bleed));
    }
    super::set_pstate(&ctx, "cutting-material-saving", saving);
    super::set_pstate(&ctx, "cutting-material-error", error);
}

/// How each operation drives the blade: force and speed in the machine's own steps (C5.2),
/// passes, overcut and blade depth, set through `cutting.settings.set` (values typed out of range
/// are brought into it); cut and score side by side, their ranges on hover (C9.1); the rest is in
/// [`advanced_rows`].
fn tool_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    material_rows(app, ui);
    let ops = &info["settings"]["operations"];
    let get = |op: &str, k: &str, d: f64| ops[op][k].as_f64().unwrap_or(d);
    let set_tool = |app: &mut VectorcraftApp, op: &str, key: &str, v: f64| set(app, json!({ "operations": { op: { key: v } } }));
    // In the machine's own steps (C5.2): what its maker's software and guides use.
    let (f_lo, f_hi) = silhouette::FORCE;
    let (s_lo, s_hi) = silhouette::SPEED;
    let force_hint = format!("{f_lo}–{f_hi}: {}", tl!("the machine's own force steps"));
    let speed_hint = format!("{s_lo}–{s_hi}: {}", tl!("the machine's own speed steps"));
    egui::Grid::new(("cutting", "tools")).num_columns(3).spacing([8.0, 3.0]).show(ui, |ui| {
        table_heading(ui);
        tool_row(ui, "Force", &force_hint, |ui, op| {
            let step = silhouette::force_step(get(op, "force", 0.3));
            if let Some(v) = cell(ui, op, "force", "Force", f64::from(step), "", 0, 1.0, f64::from(f_lo)) {
                let step = v.clamp(f64::from(f_lo), f64::from(f_hi)).round() as u32;
                set_tool(app, op, "force", silhouette::force_from_step(step));
            }
        });
        tool_row(ui, "Speed", &speed_hint, |ui, op| {
            let step = silhouette::speed_step(get(op, "speedMmS", 100.0));
            if let Some(v) = cell(ui, op, "speed", "Speed", f64::from(step), "", 0, 1.0, f64::from(s_lo)) {
                let step = v.clamp(f64::from(s_lo), f64::from(s_hi)).round() as u32;
                set_tool(app, op, "speedMmS", silhouette::speed_from_step(step));
            }
        });
        tool_row(ui, "Passes", "1–10 times round each path.", |ui, op| {
            if let Some(v) = cell(ui, op, "passes", "Passes", get(op, "passes", 1.0), "", 0, 1.0, 1.0) {
                set_tool(app, op, "passes", v.clamp(1.0, 10.0).round());
            }
        });
        tool_row(ui, "Overcut", "How far past the start a closed path is cut again, 0–5 mm.", |ui, op| {
            if let Some(v) = cell(ui, op, "overcut", "Overcut", get(op, "overcutMm", 0.0), " mm", 1, 0.5, 0.0) {
                set_tool(app, op, "overcutMm", (v.clamp(0.0, 5.0) * 10.0).round() / 10.0);
            }
        });
        tool_row(ui, "Blade depth", "The blade's depth setting, 1–10; 0: the machine's own.", |ui, op| {
            if let Some(v) = cell(ui, op, "depth", "Blade depth", get(op, "depth", 0.0), "", 0, 1.0, 0.0) {
                set_optional(app, op, "depth", v, 10.0);
            }
        });
    });
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let Ok(info) = app.session.execute("cutting.settings.get", &json!({})) else {
        widgets::dim_label(ui, tl!("Open a document to cut it."));
        return;
    };
    send_rows(app, ui);
    widgets::divider(ui);
    selection_row(app, ui);
    job_settings_rows(app, ui, &info);
    widgets::divider(ui);
    if info["settings"]["grouping"] != "none" {
        widgets::section_header(ui, tl!("Groups"));
        group_rows(app, ui, &info);
        widgets::divider(ui);
    }
    widgets::section_header(ui, tl!("Weeding"));
    weeding_rows(app, ui, &info);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Tool Settings"));
    tool_rows(app, ui, &info);
    advanced_rows(app, ui, &info);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_panel_draws_headless_with_and_without_a_job() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        let frame = |app: &mut VectorcraftApp| {
            let mut out = ctx.run_ui(egui::RawInput::default(), |ui| show(app, ui));
            out.textures_delta.clear();
        };
        frame(&mut app);
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        frame(&mut app);
        app.run("cutting.settings.set", json!({"mat": "custom", "offsetMm": [300, 0]})).unwrap();
        frame(&mut app);
        assert!(crate::cutting::job(&app, &ctx).is_some_and(|j| j.is_err()), "off the mat: no job");
    }

    fn panel_texts(app: &mut VectorcraftApp, ctx: &egui::Context) -> Vec<String> {
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 2400.0))), ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| show(app, ui));
        out.textures_delta.clear();
        out.shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect()
    }

    /// C7.1: the panel sums the job up and opens the Send window; the cutter is chosen there.
    #[test]
    fn the_panel_opens_the_send_window() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t.starts_with("1 path · ")), "{texts:?}");
        assert!(texts.iter().any(|t| t == "Send to Cutter…"), "{texts:?}");
        assert!(!texts.iter().any(|t| t == "Find Cutters" || t == "Cut length: 159 mm"), "the window's now: {texts:?}");
        assert!(!crate::cutting::send_window::is_open(&ctx));
    }

    /// C5.3: the Groups choice shows what the document cuts by.
    #[test]
    fn the_panel_shows_the_grouping() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.run("cutting.settings.set", json!({"grouping": "color"})).unwrap();
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 1600.0))), ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| show(&mut app, ui));
        out.textures_delta.clear();
        let texts: Vec<String> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        assert!(texts.iter().any(|t| t == "Groups") && texts.iter().any(|t| t == "By Colour"), "{texts:?}");
    }

    /// C7.5: cut by layer, a row for each layer in cutting order; a material chosen on a row is
    /// that group's setup, the first choice the document's again.
    #[test]
    fn the_groups_table_sets_each_groups_material() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        app.run("layer.new", json!({"name": "Gold"})).unwrap();
        app.run("shape.rectangle", json!({"x": 100, "y": 100, "width": 50, "height": 50})).unwrap();
        assert!(!panel_texts(&mut app, &ctx).iter().any(|t| t == "2. Gold"), "not cut in groups");
        app.run("cutting.settings.set", json!({"grouping": "layer"})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "2. Gold"), "{texts:?}");
        assert_eq!(texts.iter().filter(|t| *t == "Document's setup").count(), 2, "{texts:?}");
        let choices = group_choices(&mut app);
        let card = choices.iter().position(|c| c == "Cardstock").unwrap();
        choose_group_material(&mut app, "Gold", &choices, card).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Cardstock") && !texts.iter().any(|t| t == "changed"), "{texts:?}");
        app.run("cutting.group.set", json!({"group": "Gold", "operations": {"cut": {"passes": 4}}})).unwrap();
        assert!(panel_texts(&mut app, &ctx).iter().any(|t| t == "changed"));
        choose_group_material(&mut app, "Gold", &choices, 0).unwrap();
        let groups = app.session.execute("cutting.groups", &json!({})).unwrap();
        assert!(groups["groups"][1]["setup"].is_null(), "back to the document's");
    }

    /// C8.2, C8.3: the weeding switches, their numbers shown once they are on.
    #[test]
    fn the_panel_switches_weeding_on() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Weed Box") && texts.iter().any(|t| t == "Weed Lines"), "{texts:?}");
        assert!(!texts.iter().any(|t| t == "Margin" || t == "Every"), "off: no numbers");
        app.run("cutting.settings.set", json!({"weedBox": true, "weedLines": true, "weedMarginMm": 5})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Margin") && texts.iter().any(|t| t == "Every"), "{texts:?}");
        assert!(app.run("cutting.settings.set", json!({"weedMarginMm": 80})).is_err(), "out of range");
    }

    /// C7.2: the settings most cuts leave alone are behind Advanced, closed at first.
    #[test]
    fn advanced_settings_start_folded() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Advanced") && texts.iter().any(|t| t == "Force"), "{texts:?}");
        assert!(
            !texts.iter().any(|t| t == "Blade offset" || t == "Track Enhancing" || t == "Mat"),
            "folded, and the mat is the other panel's: {texts:?}"
        );
        super::super::set_pstate(&ctx, "cutting-advanced", true);
        let texts = panel_texts(&mut app, &ctx);
        assert_eq!(texts.iter().filter(|t| *t == "Blade offset").count(), 1, "cut and score in one row: {texts:?}");
        assert!(texts.iter().any(|t| t == "Track Enhancing"), "{texts:?}");
    }

    /// C9.1: cut and score side by side, one row each setting, no range squeezed into a field; the
    /// selection's operation one dropdown at the top, only with a selection; the material's name
    /// field only once saving is asked for.
    #[test]
    fn the_panel_is_compact() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        for label in ["Force", "Speed", "Passes", "Overcut", "Blade depth"] {
            assert_eq!(texts.iter().filter(|t| *t == label).count(), 1, "{label}: {texts:?}");
        }
        assert!(!texts.iter().any(|t| t.contains("(1–") || t.contains("machine's)")), "{texts:?}");
        assert!(!texts.iter().any(|t| t == "Selected"), "nothing selected: {texts:?}");
        assert!(texts.iter().any(|t| t == "Save as Material…") && !texts.iter().any(|t| t == "Material name"), "{texts:?}");
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        let at = |label: &str| texts.iter().position(|t| t == label).unwrap();
        assert!(at("Selected") < at("Default"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "As Its Group or Layer"), "{texts:?}");
        app.run("cutting.op.set", json!({"op": "score"})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        let i = texts.iter().position(|t| t == "Selected").unwrap();
        assert_eq!(texts.get(i + 1).map(String::as_str), Some("Score"), "{texts:?}");
        super::super::set_pstate(&ctx, "cutting-material-saving", Some(String::new()));
        let texts = panel_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Material name") && texts.iter().any(|t| t == "Cancel"), "{texts:?}");
    }

    /// Window → Workspace → Cutting is the cutting mode: the mat shows, and Cut Settings and
    /// Cutting Mat are docked open under Layers (not in the icon column); another workspace hides
    /// the mat.
    #[test]
    fn the_cutting_workspace_is_the_cutting_mode() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        assert!(!app.ui.view.cutting);
        crate::workspaces::switch(&mut app.ui, crate::workspaces::CUTTING).unwrap();
        assert!(app.ui.view.cutting);
        let layout = app.ui.docking.clone().expect("the workspace's own layout");
        assert!(crate::panel_docking::valid(&layout));
        for panel in [ID, mat::ID] {
            assert!(layout.contains(&panel.to_string()) && !crate::panel_docking::in_iconic_column(&layout, panel), "{panel} docked open");
        }
        assert_eq!(app.ui.dock_tab, crate::state::DockTab::Layers);
        // A Cutting workspace remembered before it had a layout of its own gets it too.
        let mut old = crate::workspaces::capture(&app.ui, crate::workspaces::CUTTING);
        old.docking = None;
        old.open_panel = Some(ID.into());
        crate::workspaces::apply(&mut app.ui, &old);
        assert!(app.ui.docking.as_ref().is_some_and(|l| !crate::panel_docking::in_iconic_column(l, ID)));
        crate::workspaces::switch(&mut app.ui, crate::workspaces::ESSENTIALS).unwrap();
        assert!(!app.ui.view.cutting);
    }
}
