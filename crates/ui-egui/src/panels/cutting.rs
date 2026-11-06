//! Fork: the Cut Settings panel (`docs/cutting/ARCHITECTURE.md`; the id is still `cutting`, so
//! saved workspaces open it): the job in a line with Send to Cutter… (the Send window, C7.1, runs
//! the cut), the selection's operation, the document's default operation and groups, the
//! material and tool settings (cut and score side by side, C9.1), the rarely changed ones behind
//! Advanced (C7.2). Each feature has a panel of its own (C14.9, [`PANELS`]): the mat, tiles and
//! nesting in [`mat`], contour, stencil and weeding in [`lines`], sketch fill, rhinestones and
//! puzzles in [`patterns`], the registration marks in [`print`] (C12.2). It reads the engine's
//! `cutting.*` queries and acts through `app.run`, as every panel does.

use egui::Ui;
use serde_json::{Value, json};

use vectorcraft_cutter_silhouette as silhouette;

use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub mod lines;
pub mod mat;
pub mod patterns;
mod picker;
pub mod popup;
pub mod print;
pub mod templates;

pub const ID: &str = "cutting";

/// Every cutting panel (C14.9: one for each feature), in the Cutting workspace's order: Cut
/// Settings, the mat (C14.10: first, as it is set up first), what shapes the cut, then what lays
/// it on the mat.
pub const PANELS: &[&str] = &[
    ID,
    mat::ID,
    lines::ID,
    lines::STENCIL_ID,
    lines::WEEDING_ID,
    patterns::ID,
    patterns::RHINESTONES_ID,
    patterns::PUZZLE_ID,
    popup::ID,
    templates::ID,
    mat::TILES_ID,
    mat::NESTING_ID,
    print::ID,
];

/// Whether there is a document to cut; the line saying so when there isn't.
pub(crate) fn has_document(app: &VectorcraftApp, ui: &mut Ui) -> bool {
    let open = app.session.active().is_some();
    if !open {
        widgets::dim_label(ui, tl!("Open a document to cut it."));
    }
    open
}

pub(crate) const LABEL_W: f32 = 80.0;
/// The Cut Settings panel's dropdowns: one width, so their arrows line up.
const CHOICE_W: f32 = 150.0;

/// A feature panel's first line: what it does, in a sentence (dim, wrapped).
pub(crate) fn intro(ui: &mut Ui, text: &str) {
    widgets::dim_label(ui, text);
    ui.add_space(4.0);
}

/// Under a button that needs a selection: what to select, while nothing is.
pub(crate) fn select_hint(ui: &mut Ui, selected: bool, text: &str) {
    if !selected {
        let t = Tokens::get(ui.ctx());
        ui.label(egui::RichText::new(tl!(text)).color(t.text_dim).italics());
    }
}

/// A checkbox with its text after it and its meaning on hover; whether it was toggled.
pub(crate) fn check_hint(ui: &mut Ui, label: &str, on: bool, hint: &str) -> bool {
    widgets::check_tip(ui, label, on, true, tl!(hint))
}
/// A number field in the tool table.
const CELL_W: f32 = 60.0;
/// The tool tables' label column, wide enough for "Blade offset": with two [`TABLE_COL_W`]
/// columns the tables fit 264 points, the Send window's settings column (C18.16).
const TABLE_LABEL_W: f32 = 72.0;
/// The tool tables' columns: as wide as the Tool dropdown draws "AutoBlade", so every table's
/// columns line up and a number's unit fits after its field.
const TABLE_COL_W: f32 = 92.0;
/// Between the tool tables' columns.
const TABLE_GAP: f32 = 4.0;

pub(crate) fn set(app: &mut VectorcraftApp, p: Value) {
    // A refusal (a value out of range) leaves the settings as they were; the field shows them.
    let _ = app.run("cutting.settings.set", p);
}

/// A length in millimetres, stepped by whole millimetres and kept to hundredths: held in mm, not
/// in points as the document's fields are, so a step lands on 2 mm and not on 1.9999 mm.
pub(crate) fn mm_field(ui: &mut Ui, id: &'static str, mm: f64, min_mm: f64) -> Option<f64> {
    widgets::spin_plain(ui, ("cutting", id), (mm * 100.0).round() / 100.0, " mm", 2, 80.0, 1.0, min_mm, &[]).map(|v| (v * 100.0).round() / 100.0)
}

/// What the document cuts by default and how it groups the cut.
fn job_settings_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    let s = &info["settings"];
    let ops = ["Cut", "Cut Outline", "Score", "Perforate", "No Cut"];
    let op_ids = ["cut", "outline", "score", "perforate", "none"];
    let default = op_ids.iter().position(|o| s["defaultOp"] == *o).unwrap_or(0);
    // The operation of objects that, with their groups and layers, set none.
    widgets::label_row(ui, tl!("Default"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "default"), ops[default], &ops, CHOICE_W) {
            set(app, json!({ "defaultOp": op_ids.get(i).copied().unwrap_or("cut") }));
        }
    });
    let groupings = ["None", "By Layer", "By Line Colour", "By Fill Colour"];
    let grouping_ids = ["none", "layer", "color", "fillColor"];
    let grouping = grouping_ids.iter().position(|g| s["grouping"] == *g).unwrap_or(0);
    // Cut in groups, with a pause before each after the first to change the tool or material.
    widgets::label_row(ui, tl!("Groups"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "grouping"), groupings[grouping], &groupings, CHOICE_W) {
            set(app, json!({ "grouping": grouping_ids.get(i).copied().unwrap_or("none") }));
        }
    });
    // C18.1: layered vinyl cut back to what shows of each colour, the overlap under the next.
    // Both switches are named beside their boxes (they were bare boxes under a label).
    if grouping > 0 {
        let on = s["knockout"].as_bool().unwrap_or(false);
        let hint = "Each colour is cut only where it shows, running the Overlap under the colours above it, so stacked vinyl isn't thick.";
        if check_hint(ui, "Knockout", on, hint) {
            set(app, json!({ "knockout": !on }));
        }
        if on {
            // Indented under its switch.
            ui.horizontal(|ui| {
                ui.add_space(20.0);
                widgets::dim_label(ui, tl!("Overlap"));
                let (lo, hi) = vectorcraft_cutjob::CuttingSettings::KNOCKOUT_MM;
                if let Some(v) = mm_field(ui, "knockout-overlap", s["knockoutOverlapMm"].as_f64().unwrap_or(0.5), lo) {
                    set(app, json!({ "knockoutOverlapMm": v.clamp(lo, hi) }));
                }
            });
        }
        // C18.10: a cross cut in the same place in every group, to line the layers up by.
        let marks = s["alignMarks"].as_bool().unwrap_or(false);
        let hint = "A small cross cut in the same place on every colour, outside the design: lay each cross over the last and the layers line up.";
        if check_hint(ui, "Align Marks", marks, hint) {
            set(app, json!({ "alignMarks": !marks }));
        }
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
    let entries = picker::entries(&app.session.execute("cutting.materials", &json!({})).unwrap_or_default());
    let doc_cut = &info["settings"]["operations"]["cut"];
    let mut error = None;
    // Each group a block: its name with whether it is cut and whether the machine pauses before
    // it, then its material, force and passes, each field named (C18.18: the switches no longer
    // sit under the Force and Passes headings).
    let (f_lo, f_hi) = silhouette::FORCE;
    for (i, g) in groups.iter().enumerate() {
        let Some(name) = g["name"].as_str() else { continue };
        let setup = &g["setup"];
        if i > 0 {
            ui.add_space(4.0);
        }
        // C14.3: whether the group is cut, and (after the first) whether the machine pauses
        // before it.
        let on = g["on"].as_bool() != Some(false);
        ui.horizontal(|ui| {
            let changed = if setup["changed"] == true { format!(" ({})", tl!("changed")) } else { String::new() };
            let title = format!("{}. {name}{changed}", i + 1);
            let tip = tl!("Off: this group isn't cut.");
            if widgets::check_tip(ui, &title, on, true, tip) {
                error = app.run("cutting.group.set", json!({ "group": name, "on": !on })).err();
            }
            if i > 0 {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let pause = g["pause"].as_bool() != Some(false);
                    let tip = tl!("The machine stops before this group, so the tool or material can be changed; press Continue in Send to Cutter.");
                    if widgets::check_tip(ui, "Pause before", pause, on, tip) {
                        error = app.run("cutting.group.set", json!({ "group": name, "pause": !pause })).err();
                    }
                });
            }
        });
        let shown = if setup.is_null() {
            choices.first().map(String::as_str).unwrap_or_default()
        } else {
            setup["material"].as_str().unwrap_or(tl!("Set by hand"))
        };
        let cut = if setup.is_null() { doc_cut } else { &setup["cut"] };
        // Indented under the group's name, so its rows read as its own.
        ui.add_enabled_ui(on, |ui| {
            ui.indent(("cutting", "group-rows", i), |ui| {
                widgets::label_row(ui, tl!("Material"), LABEL_W - 14.0, |ui| {
                    if let Some(p) =
                        picker::picker(ui, ("cutting", "group", i, "material"), shown, choices.first().map(String::as_str), &entries, CHOICE_W)
                    {
                        let k = match p {
                            picker::Picked::First => Some(0),
                            picker::Picked::Material(m) => choices.iter().skip(1).position(|c| *c == m).map(|k| k + 1),
                        };
                        error = k.map_or_else(|| Err("no such material".to_string()), |k| choose_group_material(app, name, &choices, k)).err();
                    }
                });
                widgets::label_row(ui, tl!("Force"), LABEL_W - 14.0, |ui| {
                    let step = silhouette::force_step(cut["force"].as_f64().unwrap_or(0.3));
                    if let Some(v) =
                        widgets::spin_plain(ui, ("cutting", "group", i, "force"), f64::from(step), "", 0, 44.0, 1.0, f64::from(f_lo), &[])
                    {
                        let force = silhouette::force_from_step(v.clamp(f64::from(f_lo), f64::from(f_hi)).round() as u32);
                        error = app.run("cutting.group.set", json!({ "group": name, "operations": { "cut": { "force": force } } })).err();
                    }
                    widgets::dim_label(ui, tl!("Passes"));
                    let passes = cut["passes"].as_f64().unwrap_or(1.0);
                    if let Some(v) = widgets::spin_plain(ui, ("cutting", "group", i, "passes"), passes, "", 0, 44.0, 1.0, 1.0, &[]) {
                        error = app
                            .run("cutting.group.set", json!({ "group": name, "operations": { "cut": { "passes": v.clamp(1.0, 10.0).round() } } }))
                            .err();
                    }
                });
            });
        });
    }
    if let Some(e) = error {
        ui.label(egui::RichText::new(e).color(t.bleed));
    }
    widgets::dim_label(ui, tl!("A group on Document's setup is cut with the tool settings in Send to Cutter…."));
}

/// Cut, Score, No Cut or the group's or layer's, for the selected objects, and what they are cut
/// along: rows only while something is selected (C9.1: one dropdown, at the top).
fn selection_row(app: &mut VectorcraftApp, ui: &mut Ui) {
    let objects = app.session.execute("cutting.op.get", &json!({})).ok().and_then(|r| r["objects"].as_array().cloned()).unwrap_or_default();
    widgets::section_header(ui, tl!("Selection"));
    if objects.is_empty() {
        select_hint(ui, false, "Select objects to choose how they are cut.");
        return;
    }
    let labels = ["Cut", "Cut Outline", "Score", "Perforate", "No Cut", "As Its Group or Layer"];
    let ids = ["cut", "outline", "score", "perforate", "none", "inherit"];
    let all = |f: &dyn Fn(&Value) -> bool| objects.iter().all(f);
    let current = if all(&|o| o["source"] != "own") {
        ids.iter().position(|id| *id == "inherit")
    } else if all(&|o| o["source"] == "own") {
        ids.iter().position(|id| all(&|o| o["op"] == *id))
    } else {
        None
    };
    let shown = current.and_then(|i| labels.get(i)).copied().unwrap_or("Mixed");
    widgets::label_row(ui, tl!("Cut As"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "selection-op"), shown, &labels, CHOICE_W) {
            let _ = app.run("cutting.op.set", json!({ "op": ids.get(i).copied().unwrap_or("inherit") }));
        }
    });
    // What they are cut along (C11.4): the path, or round the edge of what they paint.
    let labels = ["Path", "Stroke Edge"];
    let edges = ["path", "stroke"];
    let shown = edges.iter().position(|e| all(&|o| o["edge"] == *e)).and_then(|i| labels.get(i)).copied().unwrap_or("Mixed");
    widgets::label_row(ui, tl!("Cut Along"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "selection-edge"), shown, &labels, CHOICE_W) {
            let _ = app.run("cutting.edge.set", json!({ "edge": edges.get(i).copied().unwrap_or("path") }));
        }
    });
}

/// The settings most cuts leave alone, behind a fold that starts closed: each operation's
/// acceleration, corner overcut and blade offset, and the job's track enhancing.
fn machine_advanced_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    if !fold(ui, "cutting-tool-advanced", tl!("Advanced")) {
        return;
    }
    let ops = &info["settings"]["operations"];
    egui::Grid::new(("cutting", "advanced")).num_columns(3).spacing([TABLE_GAP, 3.0]).show(ui, |ui| {
        table_heading(ui);
        tool_row(ui, "Accel.", "The machine's acceleration steps, 1–3; Auto: none sent, the machine keeps the one it was last given.", |ui, op| {
            if let Some(v) = optional_cell(ui, op, "accel", "Accel.", ops[op]["accel"].as_f64(), 3) {
                set_optional(app, op, "accel", v, 3.0);
            }
        });
        tool_row(ui, "Corners", "How far past each corner the blade goes, 0–2 mm.", |ui, op| {
            if let Some(v) = cell(ui, op, "corners", "Corners", ops[op]["cornerOvercutMm"].as_f64().unwrap_or(0.0), "mm", 1, 0.1, 0.0) {
                set(app, json!({ "operations": { op: { "cornerOvercutMm": (v.clamp(0.0, 2.0) * 10.0).round() / 10.0 } } }));
            }
        });
        // The machine turns the blade round corners by this; the usual blade's until set.
        tool_row(ui, "Blade offset", "How far the blade's tip trails its axis, 0–2 mm. A pen has none.", |ui, op| {
            let offset = ops[op]["bladeOffsetMm"].as_f64().unwrap_or(silhouette::BLADE_OFFSET_MM);
            if silhouette::tool(ops[op]["tool"].as_str()).is_some_and(|t| t.pen) {
                widgets::dim_label(ui, "—");
            } else if let Some(v) = cell(ui, op, "blade-offset", "Blade offset", offset, "mm", 2, 0.05, 0.0) {
                set(app, json!({ "operations": { op: { "bladeOffsetMm": (v.clamp(0.0, 2.0) * 20.0).round() / 20.0 } } }));
            }
        });
    });
    let track = info["settings"]["trackEnhancing"].as_bool() == Some(true);
    if widgets::check(ui, tl!("Track Enhancing"), track, true) {
        set(app, json!({ "trackEnhancing": !track }));
    }
}

/// The design's own settings most cuts leave alone (C7.2), closed at first: Perforate's cuts and
/// gaps and the cut order.
fn advanced_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    if !fold(ui, "cutting-advanced", tl!("Advanced")) {
        return;
    }
    let s = &info["settings"];
    // C14.4: Perforate's cuts and gaps, for objects without a dashed stroke of their own.
    let (lo, hi) = vectorcraft_cutjob::CuttingSettings::PERFORATE_MM;
    widgets::label_row(ui, tl!("Perforate"), LABEL_W, |ui| {
        if let Some(v) = mm_field(ui, "perforate-cut", s["perforateCutMm"].as_f64().unwrap_or(3.0), lo) {
            set(app, json!({ "perforateCutMm": v.clamp(lo, hi) }));
        }
        widgets::dim_label(ui, tl!("cut"));
        if let Some(v) = mm_field(ui, "perforate-gap", s["perforateGapMm"].as_f64().unwrap_or(1.0), lo) {
            set(app, json!({ "perforateGapMm": v.clamp(lo, hi) }));
        }
        widgets::dim_label(ui, tl!("gap"));
    });
    // C18.7: small details cut slower.
    let mut slow = s["detailSlow"].as_bool().unwrap_or(false);
    widgets::label_row(ui, tl!("Slow Details"), LABEL_W, |ui| {
        if ui
            .checkbox(&mut slow, "")
            .on_hover_text(tl!("Paths no larger than the size given are cut at the slower speed, so fine vinyl lifts less."))
            .changed()
        {
            set(app, json!({ "detailSlow": slow }));
        }
        if slow {
            let (lo, hi) = vectorcraft_cutjob::CuttingSettings::DETAIL_SIZE_MM;
            if let Some(v) = mm_field(ui, "detail-size", s["detailSizeMm"].as_f64().unwrap_or(10.0), lo) {
                set(app, json!({ "detailSizeMm": v.clamp(lo, hi) }));
            }
            let (slo, shi) = vectorcraft_cutjob::ToolSettings::SPEED_MM_S;
            if let Some(v) =
                widgets::spin_plain(ui, ("cutting", "detail-speed"), s["detailSpeedMmS"].as_f64().unwrap_or(30.0), " mm/s", 0, 80.0, 5.0, slo, &[])
            {
                set(app, json!({ "detailSpeedMmS": v.clamp(slo, shi) }));
            }
        }
    });
    // C4.14: curves sent as the machine's own curves.
    let mut curves = s["sendCurves"].as_bool().unwrap_or(true);
    widgets::label_row(ui, tl!("Send Curves"), LABEL_W, |ui| {
        if ui
            .checkbox(&mut curves, "")
            .on_hover_text(tl!("Curves go to the machine as curves, which it cuts smoother than the short straight lines they are otherwise cut as. Off: straight lines, as older versions sent."))
            .changed()
        {
            set(app, json!({ "sendCurves": curves }));
        }
    });
    // C14.6: the order paths are cut in (each inside another first, unless as in the document).
    let orders = vectorcraft_cutjob::Ordering::ALL;
    let labels = ["Shortest Travel", "Fewest Roller Moves", "As in the Document"];
    let now = s["ordering"].as_str().unwrap_or("insideFirst");
    let shown = orders.iter().position(|o| o.id() == now).unwrap_or(0);
    widgets::label_row(ui, tl!("Cut Order"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "ordering"), labels.get(shown).copied().unwrap_or_default(), &labels, CHOICE_W)
            && let Some(o) = orders.get(i)
        {
            set(app, json!({ "ordering": o.id() }));
        }
    });
}

/// A tool's name short enough for the tool table's cell (C18.16): the dropdown's list has the
/// full names.
fn table_tool_name(t: &silhouette::Tool) -> &'static str {
    match t.id {
        "ratchet" => "Ratchet",
        "deepcut" => "Deep-cut",
        "rotary" => "Rotary",
        "punch" => "Punch",
        _ => t.label,
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
    // One width for every table's labels (C18.16), so the main table's and Advanced's columns line up.
    ui.allocate_ui(egui::vec2(TABLE_LABEL_W, 20.0), |ui| {
        ui.set_min_width(TABLE_LABEL_W);
        widgets::dim_label(ui, tl!(label)).on_hover_text(tl!(hover));
    });
    for op in ["cut", "score"] {
        ui.allocate_ui(egui::vec2(TABLE_COL_W, 20.0), |ui| {
            ui.set_min_width(TABLE_COL_W);
            add(ui, op);
        });
    }
    ui.end_row();
}

/// A number cell of the tool table, named for screen readers by its row and operation; its
/// `unit`, if any, after the field, where the field's width can't clip it (C18.16).
#[allow(clippy::too_many_arguments)]
fn cell(ui: &mut Ui, op: &'static str, key: &'static str, row: &str, v: f64, unit: &str, decimals: usize, step: f64, min: f64) -> Option<f64> {
    let name = format!("{} {}", if op == "cut" { tl!("Cut") } else { tl!("Score") }, tl!(row));
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 3.0;
        let got = widgets::spin_plain_named(ui, ("cutting", op, key), Some(&name), v, "", decimals, CELL_W, step, min, &[]);
        if !unit.is_empty() {
            widgets::dim_label(ui, unit);
        }
        got
    })
    .inner
}

/// A setting the machine keeps when it isn't sent (blade depth, acceleration): Auto, or 1 to
/// `hi`. What was chosen, 0 for Auto.
fn optional_cell(ui: &mut Ui, op: &'static str, key: &'static str, row: &str, v: Option<f64>, hi: u8) -> Option<f64> {
    let name = format!("{} {}", if op == "cut" { tl!("Cut") } else { tl!("Score") }, tl!(row));
    let steps: Vec<String> = (1..=hi).map(|n| n.to_string()).collect();
    let mut names: Vec<&str> = vec![tl!("Auto")];
    names.extend(steps.iter().map(String::as_str));
    let shown = v.filter(|v| *v >= 1.0).map_or_else(|| tl!("Auto").to_string(), |v| format!("{}", v.round()));
    widgets::dropdown_names_named(ui, ("cutting", op, key), Some(&name), &shown, &names, CELL_W).map(|i| i as f64)
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
                let word = if warnings == 1 { tl!("warning") } else { tl!("warnings") };
                line = format!("{line} · {warnings} {word}");
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

/// A document measured in pixels (C11.3, R1): it is cut at 72 px/in, and most SVGs with no units
/// were drawn at 96, so the fix is a click away.
fn pixel_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    if !app.session.active().is_some_and(|st| st.doc.units == vectorcraft_doc::Unit::Pixels) {
        return;
    }
    let t = Tokens::get(ui.ctx());
    ui.label(egui::RichText::new(tl!("Measured in pixels: cut at 72 per inch.")).color(t.warning));
    let hover = tl!(
        "Shrinks the whole document to three quarters, so a pixel is 1/96 in, as in most SVGs, and measures it in mm. If 72 per inch is right, change the units in File › Document Setup instead."
    );
    if ui.button(tl!("Scale from 96 px/in")).on_hover_text(hover).clicked() {
        // Shown only for a document in pixels, the one thing the command asks for.
        let _ = app.run("cutting.scaleFromPixels", json!({}));
    }
}

/// Whether the document is cut on a roll (C17.1), where a material keeps what's left of it.
fn info_settings_roll(app: &VectorcraftApp) -> bool {
    app.session.active().is_some_and(|st| vectorcraft_cutting::settings(&st.doc).mat == vectorcraft_cutjob::ROLL)
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
    let shown = current.as_deref().unwrap_or(tl!("None"));
    // C18.17: under their groups, with a filter.
    let entries = picker::entries(&info);
    widgets::label_row(ui, tl!("Material"), LABEL_W, |ui| {
        if let Some(picker::Picked::Material(name)) = picker::picker(ui, ("cutting", "material"), shown, None, &entries, 150.0) {
            let _ = app.run("cutting.material.apply", json!({ "name": name }));
        }
    });
    if info["changed"].as_bool() == Some(true) {
        widgets::dim_label(ui, tl!("Changed since the material was chosen."));
    } else if current.as_deref().is_some_and(|c| list.iter().any(|(n, b)| n == c && *b)) {
        widgets::dim_label(ui, tl!("A starting point: make a test cut first."));
    }
    // C18.15: an operation's tool the material has no settings for; C18.14: a blade set by hand,
    // how deep the material says to set it (never sent).
    for op in ["cut", "score"] {
        let chosen = &info["chosen"][op];
        if let (Some(c), Some(false)) = (current.as_deref(), chosen["forTool"].as_bool()) {
            let tool = silhouette::tool(chosen["tool"].as_str()).map_or(tl!("Unknown"), |t| t.label);
            widgets::dim_label(ui, &no_tool_note(c, tool, op == "score"));
        }
    }
    let depth = ["cut", "score"].iter().find_map(|op| info["chosen"][op]["handDepthMm"].as_f64());
    if let Some(d) = depth {
        widgets::dim_label(ui, &hand_depth_note(d));
    }
    // The name field only once Save as Material… is clicked (C9.1).
    let mut saving: Option<String> = super::pstate(&ctx, "cutting-material-saving");
    let mut error: Option<String> = super::pstate(&ctx, "cutting-material-error");
    let mut imported: Option<String> = super::pstate(&ctx, "cutting-material-imported");
    let mut asking: Option<ImportAsk> = super::pstate(&ctx, "cutting-material-asking");
    ui.horizontal_wrapped(|ui| {
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
        let own = current.as_deref().is_some_and(|c| list.iter().any(|(n, b)| n == c && !*b));
        let changed = info["changed"].as_bool() == Some(true);
        // C9.4: a changed material of the user's is updated in one click; a changed built-in one
        // is kept under a name of the user's, offered as its copy.
        if changed
            && own
            && let Some(c) = &current
            && ui.small_button(format!("{} \u{201C}{c}\u{201D}", tl!("Update"))).on_hover_text(tl!("Keep these settings in the material")).clicked()
        {
            error = app.run("cutting.material.save", json!({ "name": c })).err();
        }
        if ui.small_button(tl!("Save as Material…")).clicked() {
            let copy = match &current {
                Some(c) if changed && !own => format!("{c} ({})", tl!("copy")),
                _ => String::new(),
            };
            saving = Some(copy);
        }
        if own
            && ui.small_button(tl!("Delete")).clicked()
            && let Some(c) = &current
        {
            error = app.run("cutting.material.delete", json!({ "name": c })).err();
        }
        // C18.14: the materials of a list exported from Silhouette Studio, as the user's own.
        let pick = crate::FilePick { filters: crate::FilePick::filters([("Material list (CSV)", &["csv"][..])]), ..Default::default() };
        if crate::picks::can(app, &crate::picks::PickRequest::Open(pick.clone()))
            && ui
                .small_button(tl!("Import…"))
                .on_hover_text(tl!("Add the materials of a list exported from Silhouette Studio (their Cameo 4 settings, the AutoBlade's depth too)"))
                .clicked()
            && let Some(path) = crate::picks::open(app, &pick)
        {
            // C18.19: a look first; materials already the user's are replaced only when asked.
            asking = None;
            match app.run("cutting.material.import", json!({ "path": path, "preview": true })) {
                Ok(r) => match ImportAsk::from_preview(&path, &r) {
                    Some(ask) => (asking, error, imported) = (Some(ask), None, None),
                    None => (imported, error) = import_now(app, &path, false),
                },
                Err(e) => error = Some(e),
            }
        }
    });
    if let Some(ask) = asking.clone() {
        ui.label(ask.question());
        if ask.earlier > 0 {
            widgets::dim_label(ui, tl!("Replacing also merges the ones an earlier import named after their blade into one material each."));
        }
        ui.horizontal_wrapped(|ui| {
            if ui.small_button(tl!("Replace Mine")).on_hover_text(tl!("Use the list's settings; their price, pressing and roll stay")).clicked() {
                (imported, error) = import_now(app, &ask.path, true);
                asking = None;
            }
            if ui.small_button(tl!("Keep Mine")).on_hover_text(tl!("Add only the materials you don't have yet")).clicked() {
                (imported, error) = import_now(app, &ask.path, false);
                asking = None;
            }
            if ui.small_button(tl!("Cancel")).clicked() {
                asking = None;
            }
        });
    }
    if let Some(note) = imported.clone() {
        ui.horizontal_wrapped(|ui| {
            widgets::dim_label(ui, &note);
            if ui.small_button("\u{2715}").on_hover_text(tl!("Dismiss")).clicked() {
                imported = None;
            }
        });
    }
    // C18.16: a material of the user's keeps its price, pressing and roll in a section of their
    // own, closed at first: they are about the material, not how it is cut.
    let own = current.as_deref().and_then(|c| info["materials"].as_array()?.iter().find(|m| m["name"] == c && m["builtin"] == false)).cloned();
    let details = own.is_some() && fold(ui, "cutting-material-details", tl!("Material Details"));
    // C18.17: the group it is listed under.
    if details
        && let Some(c) = &current
        && let Some(m) = &own
    {
        group_row(app, ui, c, m, &entries, &mut error);
    }
    // C18.3: a material of the user's has a price, for the job's quote.
    if details
        && let Some(c) = &current
        && let Some(m) = &own
    {
        let price = &m["price"];
        let amount = price["amount"].as_f64();
        let per_metre = price["per"] == "metre";
        widgets::label_row(ui, tl!("Price"), LABEL_W, |ui| {
            let r = widgets::spin_plain(ui, ("cutting", "material-price"), amount.unwrap_or(0.0), "", 2, 64.0, 0.1, 0.0, &[]);
            let pers = [tl!("per sheet"), tl!("per metre")];
            let per = widgets::dropdown(ui, ("cutting", "material-price-per"), pers[usize::from(per_metre)], &pers, 84.0);
            let (amount, per_metre) = (r.unwrap_or(amount.unwrap_or(0.0)), per.map_or(per_metre, |i| i == 1));
            if r.is_some() || per.is_some() {
                let p = json!({ "name": c, "amount": amount.max(0.0), "per": if per_metre { "metre" } else { "sheet" }, "wastePct": price["wastePct"].as_f64().unwrap_or(10.0) });
                error = app.run("cutting.material.price", p).err();
            }
        });
    }
    // C18.8: how a material of the user's is pressed, for the job sheet.
    if details
        && let Some(c) = &current
        && let Some(m) = &own
    {
        let press = &m["press"];
        let on = press.is_object();
        widgets::label_row(ui, tl!("Pressing"), LABEL_W, |ui| {
            let mut on = on;
            if ui.checkbox(&mut on, tl!("Heat press")).on_hover_text(tl!("Heat-press settings for the job sheet")).changed() {
                let p = if on { json!({ "name": c, "tempC": 150, "seconds": 15 }) } else { json!({ "name": c, "tempC": null }) };
                error = app.run("cutting.material.press", p).err();
            }
        });
        // Its numbers on a row of their own, so a narrow panel doesn't clip them.
        if on {
            ui.horizontal(|ui| {
                ui.add_space(16.0);
                let (t, sec) = (press["tempC"].as_f64().unwrap_or(150.0), press["seconds"].as_f64().unwrap_or(15.0));
                let nt = widgets::spin_plain(ui, ("cutting", "press-temp"), t, " °C", 0, 68.0, 5.0, 50.0, &[]);
                let ns = widgets::spin_plain(ui, ("cutting", "press-seconds"), sec, " s", 0, 56.0, 1.0, 1.0, &[]);
                let peels = ["hot", "warm", "cold"];
                let peel = press["peel"].as_str().unwrap_or("warm");
                let shown = [tl!("peel hot"), tl!("peel warm"), tl!("peel cold")];
                let i = peels.iter().position(|p| *p == peel).unwrap_or(1);
                let np = widgets::dropdown(ui, ("cutting", "press-peel"), shown[i], &shown, 84.0);
                if nt.is_some() || ns.is_some() || np.is_some() {
                    let p = json!({
                        "name": c,
                        "tempC": nt.unwrap_or(t).clamp(50.0, 250.0),
                        "seconds": ns.unwrap_or(sec).clamp(1.0, 300.0),
                        "pressure": press["pressure"].as_str().unwrap_or("medium"),
                        "peel": peels[np.unwrap_or(i).min(2)],
                    });
                    error = app.run("cutting.material.press", p).err();
                }
            });
        }
    }
    // C18.11: what's left of the roll of a material of the user's, when the job is cut on a roll.
    if details
        && info_settings_roll(app)
        && let Some(c) = &current
        && let Some(m) = &own
    {
        let left = m["rollLeftMm"].as_f64();
        widgets::label_row(ui, tl!("Roll Left"), LABEL_W, |ui| {
            let mut keep = left.is_some();
            if ui.checkbox(&mut keep, "").on_hover_text(tl!("Keep what's left of the roll: each send takes off what it cuts.")).changed() {
                error = app.run("cutting.material.roll", json!({ "name": c, "leftMm": if keep { json!(5000.0) } else { Value::Null } })).err();
            }
            if let Some(l) = left
                && let Some(v) = widgets::spin_plain(ui, ("cutting", "roll-left"), l / 1000.0, " m", 2, 80.0, 0.1, 0.0, &[])
            {
                error =
                    app.run("cutting.material.roll", json!({ "name": c, "leftMm": (v * 1000.0).clamp(0.0, vectorcraft_cutjob::MAX_ROLL_MM) })).err();
            }
        });
    }
    if let Some(e) = &error {
        ui.label(egui::RichText::new(e).color(t.bleed));
    }
    super::set_pstate(&ctx, "cutting-material-saving", saving);
    super::set_pstate(&ctx, "cutting-material-error", error);
    super::set_pstate(&ctx, "cutting-material-imported", imported);
    super::set_pstate(&ctx, "cutting-material-asking", asking);
}

/// The depth to set a hand-adjusted blade to, for material `d` mm.
fn hand_depth_note(d: f64) -> String {
    format!("{} {d:.1} mm.", tl!("Set the blade by hand to"))
}

/// The material `material` has no settings for `tool`, which the cut (or the score) is cut with.
fn no_tool_note(material: &str, tool: &str, score: bool) -> String {
    let used = if score { tl!("its score settings are used.") } else { tl!("its cut settings are used.") };
    format!("{material}: {} {tool}; {used}", tl!("no settings for the"))
}

/// C18.17: Material Details' Group: the one its name suggests unless the user chose one: the
/// suggested groups, the user's own in use, or a new one typed in.
fn group_row(app: &mut VectorcraftApp, ui: &mut Ui, name: &str, m: &Value, entries: &[picker::Entry], error: &mut Option<String>) {
    let ctx = ui.ctx().clone();
    let group = m["group"].as_str().unwrap_or(vectorcraft_cutjob::OTHER_GROUP);
    let set = m["groupSet"].as_bool() == Some(true);
    let mut typing: Option<String> = super::pstate(&ctx, "cutting-material-new-group");
    // The suggested groups, then the user's own in use, then a new one.
    let mut groups: Vec<&str> = vectorcraft_cutjob::GROUPS.iter().map(|(g, _)| *g).chain([vectorcraft_cutjob::OTHER_GROUP]).collect();
    for e in entries.iter().filter(|e| !e.builtin) {
        if !groups.contains(&e.group.as_str()) {
            groups.push(&e.group);
        }
    }
    let by_name = tl!("By its name");
    let new = tl!("New Group…");
    let shown: Vec<&str> = std::iter::once(by_name).chain(groups.iter().map(|g| picker::group_label(g))).chain([new]).collect();
    widgets::label_row(ui, tl!("Group"), LABEL_W, |ui| {
        let current = if set { picker::group_label(group).to_string() } else { format!("{} ({})", picker::group_label(group), tl!("by name")) };
        let picked = widgets::dropdown_names(ui, ("cutting", "material-group"), &current, &shown, CHOICE_W);
        let value = match picked {
            Some(0) => Some(Value::Null),
            Some(k) if k == shown.len() - 1 => {
                typing = Some(String::new());
                None
            }
            Some(k) => groups.get(k - 1).map(|g| json!(g)),
            None => None,
        };
        if let Some(v) = value {
            *error = app.run("cutting.material.group", json!({ "name": name, "group": v })).err();
        }
    });
    if let Some(text) = &mut typing {
        let mut done = false;
        ui.horizontal(|ui| {
            ui.add_space(LABEL_W);
            let field = ui.add(egui::TextEdit::singleline(text).hint_text(tl!("Group name")).desired_width(110.0));
            let entered = field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter));
            if (ui.add_enabled(!text.trim().is_empty(), egui::Button::new(tl!("Save"))).clicked() || entered) && !text.trim().is_empty() {
                *error = app.run("cutting.material.group", json!({ "name": name, "group": text.trim() })).err();
                done = error.is_none();
            } else if ui.button(tl!("Cancel")).clicked() {
                done = true;
            }
        });
        if done {
            typing = None;
        }
    }
    super::set_pstate(&ctx, "cutting-material-new-group", typing);
}

/// C18.19: an import waiting for the user to say what becomes of the materials already theirs.
#[derive(Clone, Debug, Default, PartialEq)]
struct ImportAsk {
    path: String,
    /// The list's materials the user already has.
    existing: usize,
    /// Of those, the ones an earlier import named after their blade ("Board (Kraft Blade)").
    earlier: usize,
}

impl ImportAsk {
    /// What a preview found already the user's; none when there is nothing to ask.
    fn from_preview(path: &str, r: &Value) -> Option<Self> {
        let existing = r["existing"].as_array().filter(|e| !e.is_empty())?;
        let earlier = existing
            .iter()
            .filter(|e| {
                let name = e["name"].as_str().unwrap_or_default();
                e["yours"].as_array().is_some_and(|y| y.iter().any(|n| n.as_str().is_some_and(|n| !n.eq_ignore_ascii_case(name))))
            })
            .count();
        Some(Self { path: path.to_string(), existing: existing.len(), earlier })
    }

    fn question(&self) -> String {
        if self.existing == 1 {
            tl!("1 material in the list is already yours. Replace it with the list's settings?").to_string()
        } else {
            format!("{} {}", self.existing, tl!("materials in the list are already yours. Replace them with the list's settings?"))
        }
    }
}

/// Imports the list at `path`, replacing the user's materials of the same names or not: the note
/// saying what it did, or the error.
fn import_now(app: &mut VectorcraftApp, path: &str, replace: bool) -> (Option<String>, Option<String>) {
    match app.run("cutting.material.import", json!({ "path": path, "replace": replace })) {
        Ok(r) => (Some(import_note(&r)), None),
        Err(e) => (None, Some(e)),
    }
}

/// What an import (`cutting.material.import`'s answer) did, in a line: how many were added and
/// which were skipped, and why, and the list's actions not used.
fn import_note(r: &Value) -> String {
    let replaced = r["replaced"].as_array().map_or(0, Vec::len);
    let n = r["imported"].as_array().map_or(0, Vec::len).saturating_sub(replaced);
    let kept = r["existing"].as_array().map_or(0, Vec::len);
    // C18.19: the ones already the user's are counted, not listed: there can be hundreds.
    let skipped: Vec<String> = r["skipped"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|k| !r["existing"].as_array().is_some_and(|e| e.iter().any(|e| e["name"] == k["name"])))
        .filter_map(|k| Some(format!("{} ({})", k["name"].as_str()?, k["reason"].as_str()?)))
        .collect();
    let unused: Vec<&str> = r["unused"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
    let mut note = if n == 1 { tl!("1 material added.").to_string() } else { format!("{n} {}", tl!("materials added.")) };
    if replaced > 0 {
        note = format!("{note} {replaced} {}", tl!("replaced."));
    }
    if kept > 0 {
        note = format!("{note} {kept} {}", tl!("of yours kept as they were."));
    }
    if !skipped.is_empty() {
        note = format!("{note} {} {}", tl!("Skipped:"), skipped.join("; "));
    }
    if !unused.is_empty() {
        note = format!("{note} {} {}.", tl!("Not used:"), unused.join(", "));
    }
    note
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
    let kind = |op: &str| silhouette::tool(ops[op]["tool"].as_str());
    egui::Grid::new(("cutting", "tools")).num_columns(3).spacing([TABLE_GAP, 3.0]).show(ui, |ui| {
        table_heading(ui);
        // C10.2: the tool decides the holder; the AutoBlade (the usual one) is kept as none.
        let labels: Vec<&str> = silhouette::TOOLS.iter().map(|t| t.label).collect();
        tool_row(
            ui,
            "Tool",
            "Holder 1 (left): the AutoBlade, the pen and the blades in adapters; holder 2 (right): the 3 mm Kraft, rotary and punch tools.",
            |ui, op| {
                let shown = kind(op).map_or(tl!("Unknown"), table_tool_name);
                if let Some(i) = widgets::dropdown(ui, ("cutting", op, "tool"), shown, &labels, TABLE_COL_W)
                    && let Some(t) = silhouette::TOOLS.get(i)
                {
                    let id = if t.id == silhouette::AUTOBLADE.id { Value::Null } else { json!(t.id) };
                    set(app, json!({ "operations": { op: { "tool": id } } }));
                }
            },
        );
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
            if let Some(v) = cell(ui, op, "overcut", "Overcut", get(op, "overcutMm", 0.0), "mm", 1, 0.5, 0.0) {
                set_tool(app, op, "overcutMm", (v.clamp(0.0, 5.0) * 10.0).round() / 10.0);
            }
        });
        tool_row(
            ui,
            "Blade depth",
            "The AutoBlade's depth setting, 1–10; Auto: none sent, the blade stays at the depth it was last set to (by the last job, or Silhouette Studio). A material can set one. Other tools take none.",
            |ui, op| {
            if kind(op).is_some_and(|t| !t.autoblade) {
                widgets::dim_label(ui, "—");
            } else if let Some(v) = optional_cell(ui, op, "depth", "Blade depth", ops[op]["depth"].as_f64(), 10) {
                set_optional(app, op, "depth", v, 10.0);
            }
        });
    });
    // Which holder each goes in, once one is holder 2's.
    if ["cut", "score"].iter().any(|op| kind(op).is_some_and(|t| t.holder == 2)) {
        let holder = |op: &str| kind(op).map_or(1, |t| t.holder);
        widgets::dim_label(ui, &format!("{} {} · {} {}", tl!("Cut: holder"), holder("cut"), tl!("Score: holder"), holder("score")));
    }
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let Ok(info) = app.session.execute("cutting.settings.get", &json!({})) else {
        widgets::dim_label(ui, tl!("Open a document to cut it."));
        return;
    };
    send_rows(app, ui);
    pixel_rows(app, ui);
    widgets::divider(ui);
    // C18.18: what is set for the selection and what for the whole job, each under its heading.
    selection_row(app, ui);
    widgets::divider(ui);
    widgets::section_header(ui, tl!("Whole Job"));
    job_settings_rows(app, ui, &info);
    widgets::divider(ui);
    if info["settings"]["grouping"] != "none" {
        widgets::section_header(ui, tl!("Cutting Order"));
        group_rows(app, ui, &info);
        widgets::divider(ui);
    }
    advanced_rows(app, ui, &info);
    // C14.8: tuned at the machine, beside the test cut.
    widgets::dim_label(ui, tl!("The material, tool, force and speed are set in Send to Cutter…, beside the test cut."));
}

/// The material and the tool settings, and the machine's Advanced ones (C14.8: in the Send
/// window, beside the test cut that tunes them; C5.1, C5.2, C9.1, C10.2).
pub(crate) fn tool_settings(app: &mut VectorcraftApp, ui: &mut Ui) {
    let Ok(info) = app.session.execute("cutting.settings.get", &json!({})) else { return };
    widgets::section_header(ui, tl!("Tool Settings"));
    tool_rows(app, ui, &info);
    // C14.7: for material cut from the back, such as heat transfer vinyl (C14.11: here, with the
    // material it is for).
    let mirror = info["settings"]["mirror"].as_bool() == Some(true);
    if widgets::check(ui, tl!("Cut Mirrored"), mirror, true) {
        set(app, json!({ "mirror": !mirror }));
    }
    machine_advanced_rows(app, ui, &info);
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

    /// What [`tool_settings`] draws (C14.8: the Send window's), as the panel's texts are read.
    /// A blade depth not set shows as Auto (nothing is sent), not as a depth of 0; set, as itself.
    #[test]
    fn an_unset_depth_is_auto() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        let after = |texts: Vec<String>| texts.iter().skip_while(|t| *t != "Blade depth").skip(1).take(2).cloned().collect::<Vec<_>>();
        assert_eq!(after(tool_texts(&mut app, &ctx)), ["Auto", "Auto"]);
        app.run("cutting.settings.set", json!({"operations": {"cut": {"depth": 3}}})).unwrap();
        assert_eq!(after(tool_texts(&mut app, &ctx)), ["3", "Auto"]);
    }

    fn tool_texts(app: &mut VectorcraftApp, ctx: &egui::Context) -> Vec<String> {
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 2400.0))), ..Default::default() };
        let mut out = ctx.run_ui(raw, |ui| tool_settings(app, ui));
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
        assert!(texts.iter().any(|t| t == "Groups") && texts.iter().any(|t| t == "By Line Colour"), "{texts:?}");
    }

    /// C14.3: each group's row has On, and after the first Pause; switched off, its Pause is
    /// greyed and the job leaves it out.
    #[test]
    fn the_groups_table_switches_groups() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        app.run("layer.new", json!({"name": "Gold"})).unwrap();
        app.run("shape.rectangle", json!({"x": 100, "y": 100, "width": 50, "height": 50})).unwrap();
        app.run("cutting.settings.set", json!({"grouping": "layer"})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        // C18.18: each group's switch carries its name; Pause before, not before the first.
        assert!(texts.iter().any(|t| t == "1. Layer 1") && texts.iter().any(|t| t == "2. Gold"), "{texts:?}");
        assert_eq!(texts.iter().filter(|t| *t == "Pause before").count(), 1, "not before the first: {texts:?}");
        assert!(!texts.iter().any(|t| t.contains("tool settings below")), "they are in Send to Cutter: {texts:?}");
        let first = app.session.execute("cutting.groups", &json!({})).unwrap()["groups"][0]["name"].clone();
        app.run("cutting.group.set", json!({"group": first, "on": false})).unwrap();
        assert_eq!(app.session.execute("cutting.job", &json!({})).unwrap()["job"]["paths"].as_array().unwrap().len(), 1);
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
        assert!(texts.iter().any(|t| t == "Cardstock") && !texts.iter().any(|t| t.ends_with("(changed)")), "{texts:?}");
        app.run("cutting.group.set", json!({"group": "Gold", "operations": {"cut": {"passes": 4}}})).unwrap();
        assert!(panel_texts(&mut app, &ctx).iter().any(|t| t.ends_with("(changed)")));
        choose_group_material(&mut app, "Gold", &choices, 0).unwrap();
        let groups = app.session.execute("cutting.groups", &json!({})).unwrap();
        assert!(groups["groups"][1]["setup"].is_null(), "back to the document's");
    }

    /// C7.2: the settings most cuts leave alone are behind Advanced, closed at first.
    #[test]
    fn advanced_settings_start_folded() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Advanced"), "{texts:?}");
        assert!(!texts.iter().any(|t| t == "Cut Order" || t == "Mat"), "folded, and the mat is the other panel's: {texts:?}");
        // C14.8: the tool table and the machine's Advanced are the Send window's, folded there too.
        assert!(!texts.iter().any(|t| t == "Force" || t == "Blade offset"), "{texts:?}");
        assert!(texts.iter().any(|t| t.contains("set in Send to Cutter")), "{texts:?}");
        let tools = tool_texts(&mut app, &ctx);
        assert!(tools.iter().any(|t| t == "Advanced") && tools.iter().any(|t| t == "Force"), "{tools:?}");
        assert!(!tools.iter().any(|t| t == "Blade offset" || t == "Track Enhancing" || t == "Cut Order"), "{tools:?}");
        super::super::set_pstate(&ctx, "cutting-tool-advanced", true);
        let tools = tool_texts(&mut app, &ctx);
        assert_eq!(tools.iter().filter(|t| *t == "Blade offset").count(), 1, "cut and score in one row: {tools:?}");
        assert!(tools.iter().any(|t| t == "Track Enhancing"), "{tools:?}");
        super::super::set_pstate(&ctx, "cutting-advanced", true);
        let texts = panel_texts(&mut app, &ctx);
        // C14.6: the cut order, shortest travel at first.
        assert!(texts.iter().any(|t| t == "Cut Order") && texts.iter().any(|t| t == "Shortest Travel"), "{texts:?}");
        app.run("cutting.settings.set", json!({"ordering": "rollers"})).unwrap();
        assert!(panel_texts(&mut app, &ctx).iter().any(|t| t == "Fewest Roller Moves"));
    }

    /// C18.16: units after the narrow fields, not in them; a material's price, pressing and roll in
    /// Material Details, closed at first; the pressing's checkbox named, its numbers on a row of
    /// their own.
    #[test]
    fn the_tool_settings_are_tidy() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        super::super::set_pstate(&ctx, "cutting-tool-advanced", true);
        let tools = tool_texts(&mut app, &ctx);
        super::super::set_pstate(&ctx, "cutting-tool-advanced", false);
        for label in ["Overcut", "Corners", "Blade offset"] {
            assert!(tools.iter().any(|t| t == label), "{label}: {tools:?}");
        }
        assert!(!tools.iter().any(|t| t.ends_with(" mm") && t.chars().next().is_some_and(|c| c.is_ascii_digit())), "no unit in a cell: {tools:?}");
        assert_eq!(tools.iter().filter(|t| *t == "mm").count(), 6, "after each field of the three rows: {tools:?}");
        assert!(!tools.iter().any(|t| t == "Material Details"), "a built-in material has none: {tools:?}");
        app.run("cutting.material.save", json!({"name": "UI Details"})).unwrap();
        let closed = tool_texts(&mut app, &ctx);
        super::super::set_pstate(&ctx, "cutting-material-details", true);
        let open = tool_texts(&mut app, &ctx);
        app.run("cutting.material.press", json!({"name": "UI Details", "tempC": 150})).unwrap();
        let pressed = tool_texts(&mut app, &ctx);
        // C18.17: its group, by its name until one is chosen; a new one typed in.
        app.run("cutting.material.group", json!({"name": "UI Details", "group": "UI Shop"})).unwrap();
        let grouped = tool_texts(&mut app, &ctx);
        super::super::set_pstate(&ctx, "cutting-material-new-group", Some("Bags".to_string()));
        let typing = tool_texts(&mut app, &ctx);
        super::super::set_pstate(&ctx, "cutting-material-new-group", None::<String>);
        super::super::set_pstate(&ctx, "cutting-material-details", false);
        app.run("cutting.material.delete", json!({"name": "UI Details"})).unwrap();
        assert!(open.iter().any(|t| t == "Group") && open.iter().any(|t| t == "Other (by name)"), "{open:?}");
        assert!(grouped.iter().any(|t| t == "UI Shop"), "{grouped:?}");
        assert!(typing.iter().any(|t| t == "Bags") && typing.iter().any(|t| t == "Cancel"), "{typing:?}");
        assert!(closed.iter().any(|t| t == "Material Details") && !closed.iter().any(|t| t == "Price" || t == "Pressing"), "{closed:?}");
        assert!(open.iter().any(|t| t == "Price") && open.iter().any(|t| t == "Heat press"), "{open:?}");
        assert!(pressed.iter().any(|t| t == "peel warm"), "{pressed:?}");
    }

    /// C18.16 (owner, 2026-11-06: "Units in blade offset and overcut are clipped"): the tool
    /// settings fit a narrow dock, Advanced and Material Details open, nothing drawn past its edge.
    #[test]
    fn the_tool_settings_fit_a_narrow_panel() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.run("cutting.material.save", json!({"name": "UI Narrow"})).unwrap();
        app.run("cutting.material.press", json!({"name": "UI Narrow", "tempC": 150})).unwrap();
        app.run("cutting.settings.set", json!({"operations": {"cut": {"tool": "deepcut", "bladeOffsetMm": 0.25}}})).unwrap();
        super::super::set_pstate(&ctx, "cutting-tool-advanced", true);
        super::super::set_pstate(&ctx, "cutting-material-details", true);
        // The Send window's settings column (C18.16: the owner's screenshot), its margins in.
        const W: f32 = 280.0;
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(W, 2400.0))), ..Default::default() };
        let mut right = 0.0f32;
        for _ in 0..3 {
            let mut out = ctx.run_ui(raw.clone(), |ui| {
                egui::CentralPanel::default().show(ui, |ui| tool_settings(&mut app, ui));
            });
            out.textures_delta.clear();
            // What the panel holds (its own background, from 0, is anti-aliased past its edge).
            right = out
                .shapes
                .iter()
                .map(|c| c.shape.visual_bounding_rect())
                .filter(|r| r.is_finite() && r.min.x > 0.5)
                .map(|r| r.max.x)
                .fold(0.0, f32::max);
        }
        super::super::set_pstate(&ctx, "cutting-tool-advanced", false);
        super::super::set_pstate(&ctx, "cutting-material-details", false);
        app.run("cutting.material.delete", json!({"name": "UI Narrow"})).unwrap();
        assert!(right <= W, "drawn to {right}, past the {W} panel");
    }

    /// C18.14: Import… shows only where a file can be picked; the note says what was added and
    /// what was skipped, and why.
    #[test]
    fn materials_are_imported_from_a_list() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        assert!(!tool_texts(&mut app, &ctx).iter().any(|t| t == "Import…"), "no open dialog here");
        app.services.pick_open = Some(Box::new(|_: &crate::FilePick| None));
        assert!(tool_texts(&mut app, &ctx).iter().any(|t| t == "Import…"));
        let r = json!({"imported": ["A", "B"], "skipped": [{"name": "Felt", "reason": "only emboss, which VectorCraft doesn't do"}], "unused": ["emboss", "stipple"]});
        assert_eq!(import_note(&r), "2 materials added. Skipped: Felt (only emboss, which VectorCraft doesn't do) Not used: emboss, stipple.");
        assert_eq!(import_note(&json!({"imported": ["A"], "skipped": []})), "1 material added.");
        // C18.19: the ones already the user's are counted, not listed.
        let r = json!({"imported": ["A", "B", "C"], "replaced": [{"name": "B"}, {"name": "C"}], "existing": [], "skipped": []});
        assert_eq!(import_note(&r), "1 material added. 2 replaced.");
        let r = json!({"imported": [], "existing": [{"name": "B"}], "skipped": [{"name": "B", "reason": "already one of yours"}]});
        assert_eq!(import_note(&r), "0 materials added. 1 of yours kept as they were.");
        // C18.19: a preview with materials already the user's asks; one without doesn't.
        assert_eq!(ImportAsk::from_preview("l.csv", &json!({"existing": []})), None);
        let preview = json!({"existing": [{"name": "Board", "yours": ["Board (Kraft Blade)"]}, {"name": "Card", "yours": ["Card"]}]});
        let ask = ImportAsk::from_preview("l.csv", &preview).unwrap();
        assert_eq!((ask.existing, ask.earlier), (2, 1));
        super::super::set_pstate(&ctx, "cutting-material-asking", Some(ask));
        let asked = tool_texts(&mut app, &ctx);
        super::super::set_pstate(&ctx, "cutting-material-asking", None::<ImportAsk>);
        for t in ["2 materials in the list are already yours. Replace them with the list's settings?", "Replace Mine", "Keep Mine"] {
            assert!(asked.iter().any(|x| x == t), "{t}: {asked:?}");
        }
        super::super::set_pstate(&ctx, "cutting-material-imported", Some("1 material added.".to_string()));
        assert!(tool_texts(&mut app, &ctx).iter().any(|t| t == "1 material added."));
        // C18.15: a material only cut with a Kraft blade: with the AutoBlade it says it has no
        // settings for it; with the Kraft blade, how deep to set it by hand.
        let list = "full_name,cutter_class_id,(force.pressure),(speed),(tool.type),(blade_depth),category,uuid,parent_uuid,action\n\
                    UI Board,desktop,1,1,x,0,group,g,r,\nUI Board for M,cameo4,12,3,kraft2,0.9,leaf,l,g,kraft_cut\n";
        app.run("cutting.material.import", json!({"text": list})).unwrap();
        app.run("cutting.material.apply", json!({"name": "UI Board"})).unwrap();
        let auto = tool_texts(&mut app, &ctx);
        app.run("cutting.settings.set", json!({"operations": {"cut": {"tool": "kraft3"}}})).unwrap();
        let kraft = tool_texts(&mut app, &ctx);
        app.run("cutting.material.delete", json!({"name": "UI Board"})).unwrap();
        assert!(auto.iter().any(|t| t == "UI Board: no settings for the AutoBlade; its cut settings are used."), "{auto:?}");
        assert!(!auto.iter().any(|t| t.starts_with("Set the blade by hand")), "{auto:?}");
        assert!(kraft.iter().any(|t| t == "Set the blade by hand to 0.9 mm."), "{kraft:?}");
        assert!(!kraft.iter().any(|t| t.contains("no settings for")), "{kraft:?}");
    }

    /// C9.1: cut and score side by side, one row each setting, no range squeezed into a field; the
    /// selection's operation one dropdown at the top, only with a selection; the material's name
    /// field only once saving is asked for.
    #[test]
    fn the_panel_is_compact() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        let tools = tool_texts(&mut app, &ctx);
        for label in ["Force", "Speed", "Passes", "Overcut", "Blade depth"] {
            assert_eq!(tools.iter().filter(|t| *t == label).count(), 1, "{label}: {tools:?}");
        }
        assert!(!tools.iter().any(|t| t.contains("(1–") || t.contains("machine's)")), "{tools:?}");
        assert!(tools.iter().any(|t| t == "Save as Material…") && !tools.iter().any(|t| t == "Material name"), "{tools:?}");
        let texts = panel_texts(&mut app, &ctx);
        assert!(!texts.iter().any(|t| t == "Cut As"), "nothing selected: {texts:?}");
        assert!(texts.iter().any(|t| t == "Select objects to choose how they are cut."), "{texts:?}");
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        let at = |label: &str| texts.iter().position(|t| t == label).unwrap();
        assert!(at("Selection") < at("Cut As") && at("Cut As") < at("Whole Job") && at("Whole Job") < at("Default"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "As Its Group or Layer"), "{texts:?}");
        app.run("cutting.op.set", json!({"op": "score"})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        let i = texts.iter().position(|t| t == "Cut As").unwrap();
        assert_eq!(texts.get(i + 1).map(String::as_str), Some("Score"), "{texts:?}");
        super::super::set_pstate(&ctx, "cutting-material-saving", Some(String::new()));
        let tools = tool_texts(&mut app, &ctx);
        assert!(tools.iter().any(|t| t == "Material name") && tools.iter().any(|t| t == "Cancel"), "{tools:?}");
    }

    /// C9.4: a material of the user's changed by hand offers Update "name", which keeps the change
    /// in it; a built-in one doesn't (it can't be changed), nor does one not changed.
    #[test]
    fn a_changed_material_can_be_updated() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        let update = |texts: &[String]| texts.iter().any(|t| t.starts_with("Update \u{201C}"));
        app.run("cutting.material.apply", json!({"name": "Cardstock"})).unwrap();
        app.run("cutting.settings.set", json!({"operations": {"cut": {"passes": 3}}})).unwrap();
        let texts = tool_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Changed since the material was chosen.") && !update(&texts), "built-in: {texts:?}");
        let name = "C9.4 panel test material";
        app.run("cutting.material.save", json!({"name": name})).unwrap();
        assert!(!update(&tool_texts(&mut app, &ctx)), "just saved: nothing changed");
        app.run("cutting.settings.set", json!({"operations": {"cut": {"passes": 5}}})).unwrap();
        let texts = tool_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| *t == format!("Update \u{201C}{name}\u{201D}")), "{texts:?}");
        // What the button runs: the material keeps the change, the document is on it again.
        app.run("cutting.material.save", json!({"name": name})).unwrap();
        let m = app.session.execute("cutting.materials", &json!({})).unwrap();
        let mine = m["materials"].as_array().unwrap().iter().find(|x| x["name"] == name).unwrap().clone();
        assert_eq!((mine["cut"]["passes"].clone(), m["changed"].clone()), (json!(5), json!(false)));
        app.run("cutting.material.delete", json!({"name": name})).unwrap();
    }

    /// C10.2: a Tool row at the head of the table, the AutoBlade at first; a holder 2 tool says
    /// which holder each goes in, and takes no depth.
    #[test]
    fn the_table_chooses_the_tool() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        let texts = tool_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Tool") && texts.iter().filter(|t| *t == "AutoBlade").count() == 2, "{texts:?}");
        assert!(!texts.iter().any(|t| t.starts_with("Cut: holder")), "{texts:?}");
        app.run("cutting.settings.set", json!({"operations": {"score": {"tool": "rotary"}}})).unwrap();
        let texts = tool_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Rotary") && texts.iter().any(|t| t == "Cut: holder 1 · Score: holder 2"), "{texts:?}");
        assert!(texts.iter().any(|t| t == "—"), "no depth for the rotary blade: {texts:?}");
    }

    /// C11.3: a document in pixels offers the scale, and the button's command scales it.
    #[test]
    fn a_pixel_document_offers_the_scale() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        assert!(!panel_texts(&mut app, &ctx).iter().any(|t| t == "Scale from 96 px/in"));
        let svg = r#"<svg xmlns="http://www.w3.org/2000/svg" width="96" height="96"><rect width="96" height="96"/></svg>"#;
        app.run("document.open", json!({"name": "a.svg", "dataBase64": vectorcraft_format::base64_encode(svg.as_bytes())})).unwrap();
        assert!(panel_texts(&mut app, &ctx).iter().any(|t| t == "Scale from 96 px/in"));
        app.run("cutting.scaleFromPixels", json!({})).unwrap();
        assert!(!panel_texts(&mut app, &ctx).iter().any(|t| t == "Scale from 96 px/in"));
    }

    /// C11.4: with a selection, Cut Along says what it is cut along.
    #[test]
    fn the_selection_row_shows_the_edge() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        assert!(!panel_texts(&mut app, &ctx).iter().any(|t| t == "Cut Along"), "nothing selected");
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        let texts = panel_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Cut Along") && texts.iter().any(|t| t == "Path"), "{texts:?}");
        app.run("cutting.edge.set", json!({"edge": "stroke"})).unwrap();
        assert!(panel_texts(&mut app, &ctx).iter().any(|t| t == "Stroke Edge"));
    }

    /// Every cutting panel is on the Window menu and in the icon column's group.
    #[test]
    fn every_cutting_panel_is_on_the_window_menu() {
        let app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let entries = crate::menus::menu_entries(&app);
        for &panel in PANELS {
            assert!(
                entries.iter().any(|e| e.command.as_deref() == Some("window.panel")
                    && e.params["panel"] == panel
                    && e.path.iter().map(String::as_str).take(2).eq(["Window", "Cutting"])),
                "{panel} on the Window menu"
            );
            assert!(crate::state::ICON_PANEL_GROUPS.iter().any(|g| g.contains(&panel)), "{panel} in a group");
        }
    }

    /// Window → Workspace → Cutting is the cutting mode: the mat shows, Cut Settings is docked
    /// open under Layers and each feature's panel is an icon beside it; another workspace hides
    /// the mat.
    #[test]
    fn the_cutting_workspace_is_the_cutting_mode() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        assert!(!app.ui.view.cutting);
        crate::workspaces::switch(&mut app.ui, crate::workspaces::CUTTING).unwrap();
        assert!(app.ui.view.cutting);
        let layout = app.ui.docking.clone().expect("the workspace's own layout");
        assert!(crate::panel_docking::valid(&layout));
        assert!(layout.contains(&ID.to_string()) && !crate::panel_docking::in_iconic_column(&layout, ID), "Cut Settings docked open");
        // C14.9: each feature an icon of its own, opened with a click.
        for &panel in &PANELS[1..] {
            assert!(crate::panel_docking::in_iconic_column(&layout, panel), "{panel} an icon");
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
