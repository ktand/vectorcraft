//! Fork: the Send window (C7.1, `docs/cutting/IMPLEMENTATION_PLAN.md` › Phase 7). Running a cut
//! is a sequence of its own (look at the mat and the warnings, choose the machine, send, follow
//! it, change the material at a pause), so it has a window of its own rather than the foot of the
//! Cutting panel. The window isn't modal: the canvas still pans and zooms while the machine cuts,
//! and closing it leaves the cut running (the Cutting panel says how far it is; opening the window
//! again shows it). It reads the engine's `cutting.*` queries and acts through `app.run`.

use serde_json::{Value, json};

use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

/// The UI command that opens the window.
pub(crate) const COMMAND: &str = "cutting.sendWindow";
/// Most of a job's warnings the window lists before "n more".
const SHOWN_WARNINGS: usize = 12;
/// The most room the mat preview takes, px (it keeps the mat's proportions).
const PREVIEW: egui::Vec2 = egui::vec2(320.0, 440.0);
/// The column beside it, px.
const SIDE_W: f32 = 250.0;

fn open_key() -> egui::Id {
    egui::Id::unique("cutting-send-window-open")
}

pub(crate) fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(open_key())).unwrap_or(false)
}

pub(crate) fn set_open(ctx: &egui::Context, open: bool) {
    ctx.data_mut(|d| d.insert_temp(open_key(), open));
    ctx.request_repaint();
}

/// [`COMMAND`]: opens the window over the active document (File › Send to Cutter…, the
/// Cutting panel's button, the control channel).
pub(crate) fn open(app: &mut VectorcraftApp) -> Result<Value, String> {
    if app.session.active().is_none() {
        return Err("no document".into());
    }
    // The window lives in the egui context the app draws in; before its first frame there is none.
    let ctx = app.ui_ctx.clone().ok_or("the app hasn't drawn a frame yet")?;
    set_open(&ctx, true);
    Ok(Value::Null)
}

/// The machines last found: (id, label), and the one chosen.
#[derive(Clone, Debug, Default)]
struct Machines {
    found: Option<Vec<(String, String)>>,
    chosen: usize,
    /// What the last Send said: its warnings, or why it didn't start.
    note: Option<(bool, String)>,
}

fn find_machines(app: &mut VectorcraftApp) -> Vec<(String, String)> {
    let r = app.session.execute("cutting.devices", &json!({})).unwrap_or_default();
    r["devices"]
        .as_array()
        .map(|a| {
            a.iter()
                .filter_map(|d| {
                    let model = d["model"].as_str()?;
                    // A machine that isn't there says so wherever it is named.
                    let label = if d["simulated"] == true { format!("{model} {}", tl!("(simulated)")) } else { model.to_string() };
                    Some((d["id"].as_str()?.to_string(), label))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Draws the window while it is open.
pub(crate) fn show(app: &mut VectorcraftApp, ctx: &egui::Context) {
    if !is_open(ctx) {
        return;
    }
    let status = app.session.execute("cutting.status", &json!({})).unwrap_or_default();
    let sending = status["sending"].as_bool() == Some(true);
    // With no document there is nothing to show, unless a cut is still running.
    if app.session.active().is_none() && !sending {
        set_open(ctx, false);
        return;
    }
    let mut open = true;
    egui::Window::new(tl!("Send to Cutter"))
        .id(egui::Id::unique("cutting-send-window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(ctx.content_rect().center())
        .show(ctx, |ui| {
            ui.horizontal_top(|ui| {
                let size = crate::canvas::cutting_overlay::preview_size(app, PREVIEW.x, PREVIEW.y);
                let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                ui.painter().rect_filled(rect, 4.0, Tokens::get(ui.ctx()).pasteboard);
                crate::canvas::cutting_overlay::preview(app, ui.painter(), rect);
                ui.vertical(|ui| {
                    ui.set_width(SIDE_W);
                    job_rows(app, ui);
                    widgets::divider(ui);
                    cutter_rows(app, ui, &status);
                    widgets::divider(ui);
                    test_rows(app, ui, &status);
                    widgets::divider(ui);
                    history_rows(app, ui);
                });
            });
        });
    if !open {
        set_open(ctx, false);
    }
    if sending {
        ctx.request_repaint_after(std::time::Duration::from_millis(250));
    }
}

/// The job: its numbers and every warning, or why there is none; the blade's run played back on
/// the preview.
fn job_rows(app: &mut VectorcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let Some(job) = crate::cutting::job(app, ui.ctx()) else {
        widgets::dim_label(ui, tl!("Open a document to cut it."));
        return;
    };
    match &*job {
        Ok(made) => {
            let (j, s) = (&made.job, &made.stats);
            ui.label(format!("{}: {}", tl!("Paths"), s.path_count));
            ui.label(format!("{}: {:.0} mm", tl!("Cut length"), s.blade_mm));
            ui.label(format!("{}: {:.0} mm", tl!("Travel"), s.travel_mm));
            ui.label(format!("{}: {:.0} s", tl!("Estimated time"), made.sim.total_seconds));
            if !j.warnings.is_empty() {
                egui::ScrollArea::vertical().max_height(110.0).id_salt("cutting-send-warnings").show(ui, |ui| {
                    for w in j.warnings.iter().take(SHOWN_WARNINGS) {
                        ui.label(egui::RichText::new(&w.message).color(t.warning));
                    }
                    if j.warnings.len() > SHOWN_WARNINGS {
                        widgets::dim_label(ui, &format!("{} {}", j.warnings.len() - SHOWN_WARNINGS, tl!("more warnings")));
                    }
                });
            }
            if !j.paths.is_empty() {
                crate::cutting::playback_controls(ui, &made.sim, false);
            }
        }
        Err(e) => {
            ui.label(egui::RichText::new(e).color(t.bleed));
        }
    }
}

/// Most history entries the window lists.
const SHOWN_HISTORY: usize = 8;

/// Job history (C8.4), folded at first: the latest sends, newest first, what they were cut with
/// and how they ended; the settings of one that went well put back on the document; a failed
/// one's report to share.
fn history_rows(app: &mut VectorcraftApp, ui: &mut egui::Ui) {
    if !crate::panels::cutting::fold(ui, "cutting-history", tl!("History")) {
        return;
    }
    let t = Tokens::get(ui.ctx());
    let listed = app.session.execute("cutting.history", &json!({})).unwrap_or_default();
    let entries = listed["entries"].as_array().cloned().unwrap_or_default();
    if entries.is_empty() {
        widgets::dim_label(ui, tl!("Nothing sent yet."));
        return;
    }
    egui::ScrollArea::vertical().max_height(180.0).id_salt("cutting-history").show(ui, |ui| {
        for e in entries.iter().take(SHOWN_HISTORY) {
            let text = |k: &str| e[k].as_str().unwrap_or_default().to_string();
            let test = if e["test"] == true { format!(" · {}", tl!("test cut")) } else { String::new() };
            ui.label(egui::RichText::new(format!("{} · {}{test}", text("shownTime"), text("document"))).strong());
            let outcome = text("outcome");
            let (shown, bad) = match outcome.as_str() {
                "done" => (tl!("cut").to_string(), false),
                "cancelled" => (tl!("cancelled").to_string(), false),
                "sending" => (tl!("sending").to_string(), false),
                other => (other.to_string(), true),
            };
            let material = e["material"].as_str().map_or_else(|| tl!("no material").to_string(), str::to_string);
            let took = e["seconds"].as_f64().map_or(String::new(), |s| format!(" · {s:.0} s"));
            ui.label(egui::RichText::new(format!("{material} · {shown}{took}")).color(if bad { t.bleed } else { t.text_dim }));
            ui.horizontal(|ui| {
                if let Some(i) = e["index"].as_u64()
                    && ui.small_button(tl!("Use Settings")).clicked()
                {
                    let _ = app.run("cutting.history.apply", json!({ "index": i }));
                }
                if let Some(report) = e["report"].as_str() {
                    widgets::dim_label(ui, &format!("{} {report}", tl!("Report:")));
                }
            });
        }
    });
    if ui.small_button(tl!("Clear History")).clicked() {
        let _ = app.run("cutting.history.clear", json!({}));
    }
}

/// The test cut's warnings, and the document, corner and group they were worked out for.
type Ahead = (std::sync::Arc<vectorcraft_doc::Document>, usize, Option<usize>, Vec<String>);

/// The test cut's corner and the setup it tests (C8.1).
#[derive(Clone, Debug, Default)]
struct TestChoice {
    corner: usize,
    /// Index into the groups; none: the document's setup.
    group: Option<usize>,
    note: Option<(bool, String)>,
    /// What the test would warn about, and what it was worked out for (the document, held so a
    /// new one at a freed address can't pass for it; the corner; the group), so the design's job
    /// isn't made again each frame.
    ahead: Option<Ahead>,
}

/// The test cut (C8.1): a small shape at a corner of the mat with the setup a real cut would
/// use (the document's, or one group's), to check the blade before the sheet; it says first when
/// the test would land on the design.
fn test_rows(app: &mut VectorcraftApp, ui: &mut egui::Ui, status: &Value) {
    use vectorcraft_cutting::test_cut::Corner;
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut c: TestChoice = crate::panels::pstate(&ctx, "cutting-test");
    let m: Machines = crate::panels::pstate(&ctx, "cutting-machines");
    let found = m.found.clone().unwrap_or_default();
    let corners = ["Top Left", "Top Right", "Bottom Left", "Bottom Right"];
    widgets::label_row(ui, tl!("Test at"), 60.0, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "test", "corner"), corners.get(c.corner).copied().unwrap_or(corners[0]), &corners, 140.0) {
            c.corner = i;
        }
    });
    let groups: Vec<String> = app
        .session
        .execute("cutting.groups", &json!({}))
        .ok()
        .and_then(|g| g["groups"].as_array().map(|a| a.iter().filter_map(|g| g["name"].as_str().map(str::to_string)).collect()))
        .unwrap_or_default();
    if !groups.is_empty() {
        let mut names: Vec<&str> = vec![tl!("Document's setup")];
        names.extend(groups.iter().map(String::as_str));
        let shown = c.group.and_then(|g| groups.get(g)).map_or(names[0], String::as_str);
        widgets::label_row(ui, tl!("Setup"), 60.0, |ui| {
            if let Some(i) = widgets::dropdown_names(ui, ("cutting", "test", "group"), shown, &names, 140.0) {
                c.group = i.checked_sub(1);
            }
        });
    }
    let corner = Corner::CHOICES.get(c.corner).map_or(Corner::TopLeft, |(_, k)| *k);
    let group = c.group.and_then(|g| groups.get(g)).cloned();
    let mut p = json!({ "corner": corner.id() });
    if let Some(g) = &group {
        p["group"] = json!(g);
    }
    // Said before cutting, not after.
    let doc = app.session.active().map(|st| st.doc.clone());
    let fresh = match (&c.ahead, &doc) {
        (Some((d, corner, g, _)), Some(now)) => std::sync::Arc::ptr_eq(d, now) && *corner == c.corner && *g == c.group,
        _ => false,
    };
    if !fresh && let Some(now) = doc {
        let warnings = app
            .session
            .execute("cutting.test.job", &p)
            .map(|j| j["warnings"].as_array().map(|w| w.iter().filter_map(Value::as_str).map(str::to_string).collect()).unwrap_or_default())
            .unwrap_or_else(|e| vec![e.to_string()]);
        c.ahead = Some((now, c.corner, c.group, warnings));
    }
    let ahead = c.ahead.as_ref().map(|a| a.3.clone()).unwrap_or_default();
    for w in &ahead {
        ui.label(egui::RichText::new(w).color(t.warning));
    }
    let sending = status["sending"].as_bool() == Some(true);
    if ui.add_enabled(!found.is_empty() && !sending, egui::Button::new(tl!("Test Cut"))).clicked() {
        if let Some((id, _)) = found.get(m.chosen).or(found.first()) {
            p["device"] = json!(id);
        }
        c.note = Some(match app.run("cutting.test", p) {
            Ok(_) => (false, tl!("Cutting the test: the triangle should lift out and the square stay.").to_string()),
            Err(e) => (true, e),
        });
    }
    if let Some((bad, note)) = &c.note {
        ui.label(egui::RichText::new(note).color(if *bad { t.bleed } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-test", c);
}

/// The machine to send to, sending and its progress (C4.5), the pause between groups (C5.3).
/// The machines are looked for once and again on Find Cutters, not each frame (that asks the
/// system for its USB devices).
fn cutter_rows(app: &mut VectorcraftApp, ui: &mut egui::Ui, status: &Value) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut m: Machines = crate::panels::pstate(&ctx, "cutting-machines");
    let found = match m.found.clone() {
        Some(f) => f,
        None => {
            let f = find_machines(app);
            m.found = Some(f.clone());
            f
        }
    };
    let sending = status["sending"].as_bool() == Some(true);
    let waiting = status["waitingFor"].as_str();
    match found.len() {
        0 => {
            widgets::dim_label(ui, tl!("No cutter is connected."));
        }
        1 => {
            ui.label(found.first().map(|(_, model)| model.as_str()).unwrap_or_default());
        }
        _ => {
            let names: Vec<&str> = found.iter().map(|(_, model)| model.as_str()).collect();
            let current = names.get(m.chosen).copied().unwrap_or_default();
            if let Some(i) = widgets::dropdown_names(ui, ("cutting", "machine"), current, &names, SIDE_W - 20.0) {
                m.chosen = i;
            }
        }
    }
    let has_job = crate::cutting::job(app, &ctx).is_some_and(|j| j.as_ref().as_ref().is_ok_and(|made| !made.job.paths.is_empty()));
    ui.horizontal(|ui| {
        if widgets::primary_button_enabled(ui, "Send", !found.is_empty() && has_job && !sending).clicked() {
            let device = found.get(m.chosen).or(found.first()).map(|(id, _)| id.clone()).unwrap_or_default();
            m.note = Some(match app.run("cutting.send", json!({ "device": device })) {
                Ok(r) => {
                    (false, r["warnings"].as_array().map(|w| w.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")).unwrap_or_default())
                }
                Err(e) => (true, e),
            });
        }
        if waiting.is_some() && ui.button(tl!("Continue")).clicked() {
            let _ = app.run("cutting.continue", json!({}));
        }
        if sending && ui.button(tl!("Cancel")).clicked() {
            let _ = app.run("cutting.cancel", json!({}));
        }
        if !sending && ui.button(tl!("Find Cutters")).clicked() {
            m.found = None;
            m.chosen = 0;
        }
    });
    if let Some(group) = waiting {
        // The material its own setup came from (C7.4), when it has one.
        let material = crate::cutting::job(app, &ctx).and_then(|j| {
            j.as_ref().as_ref().ok().and_then(|made| made.job.groups.iter().find(|g| g.name == group).and_then(|g| g.material.clone()))
        });
        let then = match material {
            Some(m) => format!("{} {m}, {}", tl!("Load"), tl!("then Continue.")),
            None => tl!("Change the tool or material, then Continue.").to_string(),
        };
        let text = format!("{} {group}. {then}", tl!("Paused before"));
        ui.label(egui::RichText::new(text).color(t.text_strong));
    } else if sending {
        let (sent, all) = (status["chunksSent"].as_u64().unwrap_or(0), status["chunks"].as_u64().unwrap_or(0));
        let share = if all > 0 { sent as f32 / all as f32 } else { 0.0 };
        ui.add(egui::ProgressBar::new(share).desired_width(SIDE_W - 10.0).text(format!("{} {sent} / {all}", tl!("Sending:"))));
    } else if let Some(outcome) = status["outcome"].as_str() {
        let (text, bad) = match outcome {
            "done" => (tl!("Sent.").to_string(), false),
            "cancelled" => (tl!("Cancelled.").to_string(), false),
            other => (other.to_string(), true),
        };
        ui.label(egui::RichText::new(text).color(if bad { t.bleed } else { t.text_dim }));
    }
    if let Some((bad, note)) = &m.note
        && !note.is_empty()
    {
        ui.label(egui::RichText::new(note).color(if *bad { t.bleed } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-machines", m);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use vectorcraft_cutter::simulated::{Behaviour, SIMULATED_ID_PREFIX, Sent, SimulatedMachine};

    use super::*;

    /// Two frames of the window on its own (a window sizes itself on its first); the texts drawn.
    fn window_texts(app: &mut VectorcraftApp, ctx: &egui::Context) -> Vec<String> {
        let mut texts = vec![];
        for _ in 0..2 {
            let raw =
                egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 900.0))), ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| show(app, ui.ctx()));
            out.textures_delta.clear();
            texts = out
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                    _ => None,
                })
                .collect();
        }
        texts
    }

    fn app_with_two_layers() -> VectorcraftApp {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        app.run("layer.new", json!({"name": "Gold"})).unwrap();
        app.run("shape.rectangle", json!({"x": 100, "y": 100, "width": 50, "height": 50})).unwrap();
        app.run("cutting.settings.set", json!({"grouping": "layer"})).unwrap();
        app
    }

    /// The command opens it over a document (not before the app has a frame, not without a
    /// document); closed, it draws nothing.
    #[test]
    fn the_command_opens_the_window() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        assert!(window_texts(&mut app, &ctx).is_empty());
        assert!(app.run(COMMAND, json!({})).is_err(), "no document");
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        assert!(app.run(COMMAND, json!({})).is_err(), "no frame yet");
        app.adopt_context(&ctx);
        app.run(COMMAND, json!({})).unwrap();
        let texts = window_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Send to Cutter"), "the title: {texts:?}");
        assert!(texts.iter().any(|t| t == "Paths: 1"), "the job: {texts:?}");
        assert!(texts.iter().any(|t| t == "Play"), "the playback: {texts:?}");
        set_open(&ctx, false);
        assert!(window_texts(&mut app, &ctx).is_empty());
    }

    /// The window lists the job's warnings, and says why there is no job.
    #[test]
    fn the_window_shows_the_warnings_and_why_there_is_no_job() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 612, "height": 792})).unwrap();
        app.run("shape.rectangle", json!({"x": 200, "y": 300, "width": 100, "height": 100})).unwrap();
        app.run("cutting.settings.set", json!({"printAndCut": true})).unwrap();
        set_open(&ctx, true);
        let texts = window_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t.contains("registration marks aren't in the document")), "{texts:?}");
        app.run("cutting.settings.set", json!({"printAndCut": false, "mat": "custom", "offsetMm": [400, 0]})).unwrap();
        let texts = window_texts(&mut app, &ctx);
        assert!(!texts.iter().any(|t| t.starts_with("Paths")), "no job: {texts:?}");
        assert!(texts.iter().any(|t| t == "Send"), "{texts:?}");
    }

    /// C8.1: the test cut's choices show, and say ahead when the test would land on the design.
    #[test]
    fn the_window_offers_a_test_cut() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        app.run("shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
        set_open(&ctx, true);
        let texts = window_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Test Cut") && texts.iter().any(|t| t == "Top Left"), "{texts:?}");
        assert!(texts.iter().any(|t| t.contains("lands on the design")), "{texts:?}");
        crate::panels::set_pstate(&ctx, "cutting-test", TestChoice { corner: 3, ..Default::default() });
        let texts = window_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Bottom Right") && !texts.iter().any(|t| t.contains("lands on the design")), "{texts:?}");
        assert!(!texts.iter().any(|t| t == "Setup"), "not cut in groups");
        app.run("cutting.settings.set", json!({"grouping": "layer"})).unwrap();
        assert!(window_texts(&mut app, &ctx).iter().any(|t| t == "Setup"));
    }

    /// C4.8 and C5.3: a simulated cutter is named as one, and a send cut by layer that has paused
    /// before the second says so and offers Continue, then finishes. The only test here that
    /// installs machines.
    #[test]
    fn a_send_pauses_continues_and_finishes() {
        let quick = SimulatedMachine {
            info: vectorcraft_cutter::DeviceInfo {
                id: format!("{SIMULATED_ID_PREFIX}quick"),
                vendor_id: vectorcraft_cutter_silhouette::VENDOR_ID,
                product_id: 0x1137,
                name: "Simulated cutter".into(),
            },
            behaviour: Behaviour {
                query: vec![0x1B, 0x05],
                ready: b"0".to_vec(),
                moving: b"1".to_vec(),
                busy_per_write: Duration::ZERO,
                busy_per_byte: Duration::ZERO,
                answers: vec![],
            },
            sent: Sent::default(),
        };
        vectorcraft_cutter::registry::install(Some(Arc::new(quick)));
        let mut app = app_with_two_layers();
        app.run("cutting.group.set", json!({"group": "Gold", "material": "Cardstock"})).unwrap();
        let ctx = egui::Context::default();
        set_open(&ctx, true);
        let texts = window_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Cameo 4 (simulated)"), "{texts:?}");
        app.run("cutting.send", json!({})).unwrap();
        let wait_for = |app: &mut VectorcraftApp, key: &str| {
            for _ in 0..500 {
                if !app.session.execute("cutting.status", &json!({})).unwrap()[key].is_null() {
                    return;
                }
                std::thread::sleep(Duration::from_millis(10));
            }
        };
        wait_for(&mut app, "waitingFor");
        let texts = window_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t.starts_with("Paused before Gold. Load Cardstock, then Continue.")), "{texts:?}");
        assert!(texts.iter().any(|t| t == "Continue") && texts.iter().any(|t| t == "Cancel"), "{texts:?}");
        app.run("cutting.continue", json!({})).unwrap();
        wait_for(&mut app, "outcome");
        let texts = window_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Sent."), "{texts:?}");
        // C8.4: the send is in the history, with what it was cut with and how it ended.
        crate::panels::set_pstate(&ctx, "cutting-history", true);
        let texts = window_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t.starts_with("no material · cut")), "{texts:?}");
        assert!(texts.iter().any(|t| t == "Use Settings") && texts.iter().any(|t| t == "Clear History"), "{texts:?}");
        vectorcraft_cutter::registry::install(None);
    }
}
