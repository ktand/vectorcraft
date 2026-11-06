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
/// The least room the mat preview gets, px (it keeps the mat's proportions); it takes what the
/// window leaves it (C9.2: the window is resizable, and opens three quarters of the app's height).
const PREVIEW_MIN: egui::Vec2 = egui::vec2(240.0, 300.0);
/// The share of the app window's height the window opens at.
const OPEN_HEIGHT: f32 = 0.75;
/// The column beside the preview, px.
const SIDE_W: f32 = 290.0;
/// Between the preview and the column, px.
const GAP: f32 = 12.0;
/// The playback under the preview: its buttons, slider and line, px.
const PLAYBACK_H: f32 = 76.0;
/// The column's foot (Send and the cut's progress) until it has been drawn once, px.
const FOOTER_H: f32 = 72.0;

fn footer_key() -> egui::Id {
    egui::Id::unique("cutting-send-footer-h")
}

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
    /// What the machine last said is in its holders (C10.3): the device asked, and
    /// `cutting.tools`' answer or why there is none. Asked once a machine is found, and again on
    /// Check Tools: asking is the machine's I/O, not each frame's.
    tools: Option<(String, Result<Value, String>)>,
    /// C14.12: Send was pressed with a material cut from the back: the device to send to and the
    /// material, until Cut Mirrored or As Is is chosen.
    ask_mirror: Option<(String, String)>,
    /// C12.5: what to know before sending (Silhouette Studio running), from when the machines
    /// were last looked for.
    notes: Vec<String>,
}

/// The material the document is cut with, when it is cut from the back and the job isn't Print &
/// Cut (never cut mirrored, C14.7): Send asks whether to cut mirrored (C14.12).
fn from_the_back(app: &mut VectorcraftApp) -> Option<String> {
    let listed = app.session.execute("cutting.materials", &json!({})).ok()?;
    let current = listed["current"].as_str()?;
    let print = app.session.execute("cutting.settings.get", &json!({})).ok()?["settings"]["printAndCut"].as_bool() == Some(true);
    let marked = listed["materials"].as_array()?.iter().any(|m| m["name"] == current && m["mirror"] == json!(true));
    (marked && !print).then(|| current.to_string())
}

/// Send the job to `device`; what it said goes in `m.note`.
fn send(app: &mut VectorcraftApp, m: &mut Machines, device: &str) {
    m.note = Some(match app.run("cutting.send", json!({ "device": device })) {
        Ok(r) => (false, r["warnings"].as_array().map(|w| w.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")).unwrap_or_default()),
        Err(e) => (true, e),
    });
}

/// C14.12: asked before a send with a material cut from the back: Cut Mirrored (the default) or
/// As Is sets Cut Mirrored and sends; Cancel, Escape or a click outside sends nothing.
fn ask_mirror_dialog(app: &mut VectorcraftApp, ctx: &egui::Context, m: &mut Machines) {
    let Some((device, material)) = m.ask_mirror.clone() else { return };
    let modal = egui::Modal::new(egui::Id::unique("cutting-ask-mirror")).show(ctx, |ui| {
        ui.set_max_width(340.0);
        ui.label(egui::RichText::new(tl!("Cut mirrored?")).strong());
        ui.label(format!("{material}: {}", tl!("cut from the back, so the design is usually cut mirrored.")));
        let mut answer = None;
        ui.horizontal(|ui| {
            if widgets::primary_button_enabled(ui, tl!("Cut Mirrored"), true).clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                answer = Some(Some(true));
            }
            if ui.button(tl!("As Is")).clicked() {
                answer = Some(Some(false));
            }
            if ui.button(tl!("Cancel")).clicked() {
                answer = Some(None);
            }
        });
        answer
    });
    let answer = match modal.inner {
        Some(a) => a,
        None if modal.should_close() => None,
        None => return,
    };
    m.ask_mirror = None;
    let Some(mirror) = answer else { return };
    if let Err(e) = app.run("cutting.settings.set", json!({ "mirror": mirror })) {
        m.note = Some((true, e));
        return;
    }
    send(app, m, &device);
}

/// `cutting.tools`' holders as the driver reads them, holder 1 first.
fn fitted(answer: &Value) -> Vec<vectorcraft_cutter_silhouette::Fitted> {
    use vectorcraft_cutter_silhouette::Fitted;
    answer["holders"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|h| match h["state"].as_str() {
            Some("empty") => Fitted::Empty,
            Some("tool") => Fitted::Tool(h["tool"].as_str().unwrap_or_default().to_string()),
            Some("error") => Fitted::Error,
            _ => Fitted::Unknown(h["code"].as_str().unwrap_or_default().to_string()),
        })
        .collect()
}

/// What is in each holder, and where the job needs a tool that isn't fitted (C10.3). The job is
/// checked against the last answer each frame, so a tool chosen since is checked too.
fn tools_rows(app: &mut VectorcraftApp, ui: &mut egui::Ui, m: &mut Machines, device: &str, sending: bool) {
    let t = Tokens::get(ui.ctx());
    let stale = m.tools.as_ref().is_none_or(|(asked, _)| asked != device);
    if stale && !sending {
        m.tools = Some((device.to_string(), app.session.execute("cutting.tools", &json!({ "device": device })).map_err(|e| e.to_string())));
    }
    let Some((_, answer)) = &m.tools else { return };
    let mut ask_again = false;
    match answer {
        Ok(a) if a["canTell"] != true => {}
        Ok(a) if a["answered"] != true => {
            ui.horizontal(|ui| {
                widgets::dim_label(ui, tl!("The cutter didn't say which tools are fitted."));
                ask_again = !sending && ui.button(tl!("Check Tools")).clicked();
            });
        }
        Ok(a) => {
            let line: Vec<String> = a["holders"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|h| {
                    let what = match h["state"].as_str() {
                        Some("empty") => tl!("empty").to_string(),
                        Some("tool") => h["label"].as_str().unwrap_or_default().to_string(),
                        Some("error") => tl!("can't tell").to_string(),
                        _ => format!("{} {}", tl!("unknown tool"), h["code"].as_str().unwrap_or_default()),
                    };
                    format!("{} {}: {what}", tl!("Holder"), h["holder"])
                })
                .collect();
            ui.horizontal_wrapped(|ui| {
                widgets::dim_label(ui, &line.join(" · "));
                ask_again = !sending && ui.button(tl!("Check Tools")).clicked();
            });
            let holders = fitted(a);
            let job = crate::cutting::job(app, ui.ctx());
            if let Some(Ok(made)) = job.as_deref() {
                for w in vectorcraft_cutter_silhouette::tool_warnings(&made.job, &holders) {
                    ui.label(egui::RichText::new(w).color(t.warning));
                }
            }
        }
        Err(e) => {
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(e).color(t.text_dim));
                ask_again = !sending && ui.button(tl!("Check Tools")).clicked();
            });
        }
    }
    if ask_again {
        m.tools = None;
    }
}

fn find_machines(app: &mut VectorcraftApp) -> (Vec<(String, String)>, Vec<String>) {
    let r = app.session.execute("cutting.devices", &json!({})).unwrap_or_default();
    let notes = r["notes"].as_array().into_iter().flatten().filter_map(Value::as_str).map(str::to_string).collect();
    let found = r["devices"]
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
        .unwrap_or_default();
    (found, notes)
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
    // C18.27: Esc closes it (the cut carries on), unless a field or an open list takes Esc first.
    let free = !egui::Popup::is_any_open(ctx) && ctx.memory(|m| m.focused().is_none());
    if free && ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::Escape)) {
        set_open(ctx, false);
        return;
    }
    // Opens with the preview three quarters of the app's height, as wide as the mat makes it.
    let screen = ctx.content_rect();
    let preview_h = (screen.height() * OPEN_HEIGHT - 40.0).max(PREVIEW_MIN.y);
    let preview_w = crate::canvas::cutting_overlay::preview_size(app, screen.width(), preview_h).x.max(PREVIEW_MIN.x);
    let min = egui::vec2(PREVIEW_MIN.x + GAP + SIDE_W, PREVIEW_MIN.y);
    egui::Window::new(tl!("Send to Cutter"))
        .id(egui::Id::unique("cutting-send-window"))
        .open(&mut open)
        .collapsible(false)
        .resizable(true)
        .min_size(min)
        .default_size(egui::vec2(preview_w + GAP + SIDE_W, preview_h))
        .pivot(egui::Align2::CENTER_CENTER)
        .default_pos(screen.center())
        .show(ctx, |ui| {
            let room = ui.available_size().max(min);
            ui.horizontal_top(|ui| {
                // The preview fills what the column leaves, the mat's proportions kept; the
                // playback that moves the blade over it right under it (C18.18: it was in the
                // column, away from what it plays).
                ui.vertical(|ui| {
                    let size = crate::canvas::cutting_overlay::preview_size(app, room.x - GAP - SIDE_W, room.y - PLAYBACK_H);
                    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
                    ui.painter().rect_filled(rect, 4.0, Tokens::get(ui.ctx()).pasteboard);
                    crate::canvas::cutting_overlay::preview(app, ui.painter(), rect);
                    if let Some(job) = crate::cutting::job(app, ui.ctx())
                        && let Ok(made) = &*job
                        && !made.job.paths.is_empty()
                    {
                        ui.set_max_width(size.x);
                        crate::cutting::playback_controls(ui, &made.sim, false);
                    }
                });
                ui.add_space((GAP - ui.spacing().item_spacing.x).max(0.0));
                ui.vertical(|ui| {
                    ui.set_width(SIDE_W);
                    // The column scrolls when the window is shorter than it; Send and the cut's
                    // progress stay at its foot, never scrolled away (C18.18).
                    // As tall as it was last drawn: a pause, a progress bar or a note make it taller.
                    let foot = ctx.data(|d| d.get_temp::<f32>(footer_key())).unwrap_or(FOOTER_H);
                    egui::ScrollArea::vertical().id_salt("cutting-send-side").max_height((room.y - foot).max(120.0)).show(ui, |ui| {
                        widgets::section_header(ui, tl!("Job"));
                        job_rows(app, ui);
                        sheet_rows(app, ui, &status);
                        widgets::divider(ui);
                        widgets::section_header(ui, tl!("Cutter"));
                        cutter_rows(app, ui, &status);
                        widgets::divider(ui);
                        // C14.8: tuned here, the test cut under them.
                        crate::panels::cutting::tool_settings(app, ui);
                        widgets::divider(ui);
                        widgets::section_header(ui, tl!("Test Cut"));
                        test_rows(app, ui, &status);
                        widgets::divider(ui);
                        history_rows(app, ui, &status);
                    });
                    let top = ui.cursor().top();
                    widgets::divider(ui);
                    send_rows(app, ui, &status);
                    let h = ui.cursor().top() - top;
                    ctx.data_mut(|d| d.insert_temp(footer_key(), h));
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
    // C18.2: a warning naming an object selects it on a click.
    let mut pick: Option<u64> = None;
    let mut sheet: Option<&'static str> = None;
    match &*job {
        Ok(made) => {
            let (j, s) = (&made.job, &made.stats);
            // C18.3: the material it takes and, where the material has a price, what it costs.
            let settings = app.session.active().map(|st| vectorcraft_cutting::settings(&st.doc));
            let quote = settings.map(|settings| {
                let listed = app.session.execute("cutting.materials", &json!({})).unwrap_or_default();
                let prices: Vec<(String, vectorcraft_cutjob::Price)> = listed["materials"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(|m| Some((m["name"].as_str()?.to_string(), vectorcraft_cutjob::Price::from_value(&m["price"])?)))
                    .collect();
                let price_of = |name: &str| prices.iter().find(|(n, _)| n.eq_ignore_ascii_case(name)).map(|(_, p)| *p);
                vectorcraft_cutting::quote::quote(&settings, j, &price_of)
            });
            // C18.18: the numbers in two columns, each name beside its value.
            let mut rows = vec![
                (tl!("Paths"), s.path_count.to_string()),
                (tl!("Cut length"), format!("{:.0} mm", s.blade_mm)),
                (tl!("Travel"), format!("{:.0} mm", s.travel_mm)),
                (tl!("Estimated time"), duration(made.sim.total_seconds)),
            ];
            if let Some(q) = &quote {
                let metres: f64 = q.shares.iter().map(|g| g.metres).sum();
                let sheets: f64 = q.shares.iter().map(|g| g.sheets).sum();
                let used = if metres > 0.0 {
                    format!("{metres:.2} m")
                } else if (sheets - 1.0).abs() < 1e-9 {
                    tl!("1 sheet").to_string()
                } else {
                    format!("{sheets:.0} {}", tl!("sheets"))
                };
                rows.push((tl!("Material"), used));
                if let Some(c) = q.cost {
                    rows.push((tl!("Cost"), format!("{c:.2}")));
                }
            }
            let priced = quote.as_ref().is_some_and(|q| q.cost.is_some());
            egui::Grid::new("cutting-send-stats").num_columns(2).spacing([12.0, 2.0]).show(ui, |ui| {
                for (name, value) in rows {
                    let r = widgets::dim_label(ui, name);
                    if name == tl!("Material") && !priced {
                        r.on_hover_text(tl!("Give one of your own materials a price under Tool Settings › Material Details for the cost."));
                    }
                    ui.label(value);
                    ui.end_row();
                }
            });
            // C12.5: a Print & Cut job, and how far apart the marks it looks for are.
            if let Some(r) = &j.registration {
                ui.label(egui::RichText::new(format!("{}: {:.0} × {:.0} mm", tl!("Print & Cut, marks"), r.size_mm[0], r.size_mm[1])).color(t.accent))
                    .on_hover_text(tl!("The cutter finds the printed registration marks first (about 20 s), then cuts. Load the printed sheet at the mat's top-left corner, the square mark at the top left."));
            }
            if quote.is_some() {
                // C18.8: a page to print for the machine and the press.
                ui.horizontal(|ui| {
                    if ui
                        .button(tl!("Job Sheet…"))
                        .on_hover_text(tl!("Save a page listing each group, its material, settings and pressing"))
                        .clicked()
                    {
                        sheet = Some("cutting.jobSheet");
                    }
                    // C18.9: what to pull away after the cut.
                    if ui
                        .button(tl!("Weeding Map…"))
                        .on_hover_text(tl!("Save a page showing what to pull away (grey) and what stays (white)"))
                        .clicked()
                    {
                        sheet = Some("cutting.weedingMap");
                    }
                });
            }
            if !j.warnings.is_empty() {
                egui::ScrollArea::vertical().max_height(110.0).id_salt("cutting-send-warnings").show(ui, |ui| {
                    for w in j.warnings.iter().take(SHOWN_WARNINGS) {
                        let text = egui::RichText::new(&w.message).color(t.warning);
                        match w.source {
                            Some(src) => {
                                if ui.add(egui::Label::new(text).sense(egui::Sense::click())).on_hover_text(tl!("Click to select it")).clicked() {
                                    pick = Some(src.node);
                                }
                            }
                            None => {
                                ui.label(text);
                            }
                        }
                    }
                    if j.warnings.len() > SHOWN_WARNINGS {
                        widgets::dim_label(ui, &format!("{} {}", j.warnings.len() - SHOWN_WARNINGS, tl!("more warnings")));
                    }
                });
            }
        }
        Err(e) => {
            ui.label(egui::RichText::new(e).color(t.bleed));
        }
    }
    if let Some(node) = pick
        && let Err(e) = app.run("select.set", serde_json::json!({ "ids": [node] }))
    {
        app.status(e);
    }
    if let Some(cmd) = sheet {
        crate::picks::button(app, move |app| {
            let name =
                app.session.execute(cmd, &json!({})).ok().and_then(|v| v["name"].as_str().map(str::to_string)).unwrap_or_else(|| "job.svg".into());
            if let Err(e) = crate::io::save_command_output_named(app, cmd, &name, json!({}))
                && e != "cancelled"
            {
                app.status(e);
            }
        });
    }
}

/// `seconds` as a person reads a cut's length: "45 s", "3 min 20 s", "1 h 5 min".
fn duration(seconds: f64) -> String {
    if !seconds.is_finite() || seconds < 0.0 {
        return "—".into();
    }
    // Finite and at least 0: the cast is exact for any cut's length (it saturates past u64).
    let s = seconds.round() as u64;
    match (s / 3600, (s % 3600) / 60, s % 60) {
        (0, 0, sec) => format!("{sec} s"),
        (0, min, 0) => format!("{min} min"),
        (0, min, sec) => format!("{min} min {sec} s"),
        (h, min, _) => format!("{h} h {min} min"),
    }
}

/// Most history entries the window lists.
const SHOWN_HISTORY: usize = 8;

/// Job history (C8.4), folded at first: the latest sends, newest first, what they were cut with
/// and how they ended; the settings of one that went well put back on the document; a failed
/// one's report to share.
fn history_rows(app: &mut VectorcraftApp, ui: &mut egui::Ui, status: &Value) {
    if !crate::panels::cutting::fold(ui, "cutting-history", tl!("History")) {
        return;
    }
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    // Send Again (C8.8): to the machine chosen above, for an entry cut from this document.
    let mut m: Machines = crate::panels::pstate(&ctx, "cutting-machines");
    let device = m.found.as_ref().and_then(|f| f.get(m.chosen).or(f.first())).map(|(id, _)| id.clone());
    let idle = status["sending"].as_bool() != Some(true);
    let title = app.session.active().map(|d| d.title());
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
            let took = e["seconds"].as_f64().map_or(String::new(), |s| format!(" · {}", duration(s)));
            ui.label(egui::RichText::new(format!("{material} · {shown}{took}")).color(if bad { t.bleed } else { t.text_dim }));
            ui.horizontal(|ui| {
                if let Some(i) = e["index"].as_u64()
                    && ui.button(tl!("Use Settings")).clicked()
                {
                    let _ = app.run("cutting.history.apply", json!({ "index": i }));
                }
                let this_document = title.as_deref() == e["document"].as_str();
                if let Some(i) = e["index"].as_u64()
                    && this_document
                    && ui
                        .add_enabled(device.is_some() && idle, egui::Button::new(tl!("Send Again")).small())
                        .on_hover_text(tl!("Cut this document again with these settings"))
                        .clicked()
                {
                    let device = device.clone().unwrap_or_default();
                    m.note = Some(match app.run("cutting.history.send", json!({ "index": i, "device": device })) {
                        Ok(r) => (
                            false,
                            r["warnings"].as_array().map(|w| w.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" ")).unwrap_or_default(),
                        ),
                        Err(e) => (true, e),
                    });
                    crate::panels::set_pstate(&ctx, "cutting-machines", m.clone());
                }
                if let Some(report) = e["report"].as_str() {
                    widgets::dim_label(ui, &format!("{} {report}", tl!("Report:")));
                }
            });
        }
    });
    if ui.button(tl!("Clear History")).clicked() {
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
    widgets::label_row(ui, tl!("Test at"), crate::panels::cutting::LABEL_W, |ui| {
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
        widgets::label_row(ui, tl!("Setup"), crate::panels::cutting::LABEL_W, |ui| {
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
    let device = found.get(m.chosen).or(found.first()).map(|(id, _)| id.clone());
    // C18.23: a row of test cuts to find a setting by.
    super::calibrate::rows(app, ui, corner.id(), device.clone(), !found.is_empty() && !sending);
    origin_rows(app, ui, &mut c.note);
    curve_test_rows(app, ui, device, !found.is_empty() && !sending, &mut c.note);
    crate::panels::set_pstate(&ctx, "cutting-test", c);
}

/// The curve test's rows, as the choice shows them and as `cutting.testCurves` takes them.
const CURVE_ROWS: [(&str, Option<&str>); 4] =
    [("All Three Rows", None), ("Straight Moves", Some("segments")), ("Béziers (BZ)", Some("bezier")), ("Circle (W)", Some("circle"))];

/// C4.9: the curve test (hardware checks step 18), closed at first: whether the machine cuts
/// its own curve commands as well as straight moves. An error or what to look for goes to the
/// test's note.
fn curve_test_rows(app: &mut VectorcraftApp, ui: &mut egui::Ui, device: Option<String>, can: bool, note: &mut Option<(bool, String)>) {
    if !crate::panels::cutting::fold(ui, "cutting-curves", tl!("Curve Test")) {
        return;
    }
    let ctx = ui.ctx().clone();
    let mut row: usize = crate::panels::pstate(&ctx, "cutting-curves-row");
    let names: Vec<&str> = CURVE_ROWS.iter().map(|(n, _)| crate::i18n::t(n)).collect();
    let shown = names.get(row).copied().unwrap_or_default();
    widgets::label_row(ui, tl!("Rows"), crate::panels::cutting::LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown_names(ui, ("cutting", "curves", "row"), shown, &names, 140.0) {
            row = i;
        }
    });
    let button = ui
        .add_enabled(can, egui::Button::new(tl!("Cut Curve Test")))
        .on_hover_text(tl!("A 10 mm circle and a 50 mm S-curve in three rows: as straight moves, as the machine's own Bézier curves, and the circle as its own circle command. Compare how round and smooth each row is (hardware checks, step 18)."));
    if button.clicked() {
        let mut p = json!({});
        if let Some(only) = CURVE_ROWS.get(row).and_then(|(_, id)| *id) {
            p["only"] = json!(only);
        }
        if let Some(id) = &device {
            p["device"] = json!(id);
        }
        *note = Some(match app.run("cutting.testCurves", p) {
            Ok(_) => {
                (false, tl!("Cutting the curve test: compare the rows, top to bottom: straight moves, Béziers, the circle command.").to_string())
            }
            Err(e) => (true, e),
        });
    }
    // C4.12: the sheet to measure the machine by, added to the document to cut as any design.
    let sheet = ui.button(tl!("Add Calibration Sheet")).on_hover_text(tl!(
        "Adds a 112 mm sheet to the artboard's middle: 100 mm lines to measure the scale, a square and a circle that must close, and small sharp corners; with Print & Cut, rings round printed crosses. Cut it, then measure (hardware checks, step 21)."
    ));
    if sheet.clicked() {
        *note = Some(match app.run("cutting.calibration", json!({})) {
            Ok(_) => (false, tl!("The calibration sheet is on the artboard: send it, then measure it.").to_string()),
            Err(e) => (true, e),
        });
    }
    crate::panels::set_pstate(&ctx, "cutting-curves-row", row);
}

/// The machine's origin adjustment (C4.6), under the test cut that measures it: how far every
/// cut is moved on the mat, for a machine and mat that cut off the grid (`cutting.machine.set`,
/// kept next to the preferences). An error goes to the test's note.
fn origin_rows(app: &mut VectorcraftApp, ui: &mut egui::Ui, note: &mut Option<(bool, String)>) {
    let now = app.session.execute("cutting.machine.get", &json!({})).unwrap_or_default();
    let adjust = |i: usize| now["originAdjustMm"][i].as_f64().unwrap_or(0.0);
    let (mut across, mut down) = (adjust(0), adjust(1));
    let lim = vectorcraft_engine::cmd::cutting::machine::ORIGIN_ADJUST_MM;
    let mut changed = false;
    // Closed at first, set once for the machine and mat; what it is set to shows when it isn't 0.
    let title = if across == 0.0 && down == 0.0 {
        tl!("Origin Adjustment").to_string()
    } else {
        format!("{} ({across:+.1}, {down:+.1} mm)", tl!("Origin Adjustment"))
    };
    if !crate::panels::cutting::fold(ui, "cutting-origin", &title) {
        return;
    }
    let t = Tokens::get(ui.ctx());
    ui.label(
        egui::RichText::new(tl!("Moves every cut on the mat, + to the right and down: set how far a test cut lands from where it should."))
            .color(t.text_dim),
    );
    for (label, id, v) in [(tl!("Across"), "origin-across", &mut across), (tl!("Down"), "origin-down", &mut down)] {
        widgets::label_row(ui, label, crate::panels::cutting::LABEL_W, |ui| {
            if let Some(n) = widgets::spin_plain(ui, ("cutting", id), *v, " mm", 1, 80.0, 0.1, -lim, &[]) {
                *v = ((n * 10.0).round() / 10.0).clamp(-lim, lim);
                changed = true;
            }
        });
    }
    if changed && let Err(e) = app.run("cutting.machine.set", json!({ "originAdjustMm": [across, down] })) {
        *note = Some((true, e));
    }
}

/// With more than one artboard, which sheet is on the mat (C8.6), and the next one once this is
/// cut: each sheet is a send of its own, with the mat changed in between.
fn sheet_rows(app: &mut VectorcraftApp, ui: &mut egui::Ui, status: &Value) {
    let Some(st) = app.session.active() else { return };
    let names: Vec<String> = st.doc.artboards.iter().map(|a| a.name.clone()).collect();
    if names.len() < 2 {
        return;
    }
    let at = usize::try_from(vectorcraft_cutting::settings(&st.doc).artboard).unwrap_or(0);
    let name = names.get(at).cloned().unwrap_or_default();
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(format!("{} {} {} {}: {name}", tl!("Sheet"), at + 1, tl!("of"), names.len())).strong());
        let sending = status["sending"].as_bool() == Some(true);
        if at > 0 && !sending && ui.button(tl!("Previous Sheet")).clicked() {
            let _ = app.run("cutting.settings.set", json!({ "artboard": at - 1 }));
        }
        if at + 1 < names.len() && !sending && ui.button(tl!("Next Sheet")).clicked() {
            let _ = app.run("cutting.settings.set", json!({ "artboard": at + 1 }));
        }
    });
    if status["outcome"] == "done" && at + 1 < names.len() {
        widgets::dim_label(ui, tl!("Load the next sheet's material, then Next Sheet and Send."));
    }
}

/// The machines, looked for once and again on Find Cutters, not each frame (that asks the system
/// for its USB devices): the list, and the chosen one's id.
fn machines(app: &mut VectorcraftApp, m: &mut Machines) -> (Vec<(String, String)>, Option<String>) {
    let found = match m.found.clone() {
        Some(f) => f,
        None => {
            let (f, notes) = find_machines(app);
            m.found = Some(f.clone());
            m.notes = notes;
            // Machines found again: what they have fitted is asked again too.
            m.tools = None;
            f
        }
    };
    let device = found.get(m.chosen).or(found.first()).map(|(id, _)| id.clone());
    (found, device)
}

/// The machine to send to (C4.5) with Find Cutters beside it, and what it has in its holders
/// (C10.3).
fn cutter_rows(app: &mut VectorcraftApp, ui: &mut egui::Ui, status: &Value) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut m: Machines = crate::panels::pstate(&ctx, "cutting-machines");
    let (found, device) = machines(app, &mut m);
    let sending = status["sending"].as_bool() == Some(true);
    ui.horizontal(|ui| {
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
                if let Some(i) = widgets::dropdown_names(ui, ("cutting", "machine"), current, &names, SIDE_W - 110.0) {
                    m.chosen = i;
                }
            }
        }
        // C18.18: beside the machine it looks for, not beside Send.
        if !sending && ui.button(tl!("Find Cutters")).on_hover_text(tl!("Look again for cutters, after plugging one in or switching it on")).clicked()
        {
            m.found = None;
            m.chosen = 0;
        }
    });
    for note in &m.notes {
        ui.label(egui::RichText::new(note).color(t.warning));
    }
    if found.is_empty() {
        widgets::dim_label(
            ui,
            tl!("Plug the cutter in by USB and switch it on, then Find Cutters. Close Silhouette Studio, which holds the machine."),
        );
    }
    if let Some(device) = device {
        tools_rows(app, ui, &mut m, &device, sending);
    }
    crate::panels::set_pstate(&ctx, "cutting-machines", m);
}

/// The column's foot (C18.18: kept in sight however far the column is scrolled): Send, Continue
/// at a pause between groups (C5.3) and Cancel, and how the cut is going or how it ended.
fn send_rows(app: &mut VectorcraftApp, ui: &mut egui::Ui, status: &Value) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut m: Machines = crate::panels::pstate(&ctx, "cutting-machines");
    let (found, device) = machines(app, &mut m);
    let sending = status["sending"].as_bool() == Some(true);
    let waiting = status["waitingFor"].as_str();
    let has_job = crate::cutting::job(app, &ctx).is_some_and(|j| j.as_ref().as_ref().is_ok_and(|made| !made.job.paths.is_empty()));
    // C12.5: a Print & Cut job's first chunk ends with the search for the marks, answered once the
    // machine has looked.
    let print_and_cut = crate::cutting::job(app, &ctx).is_some_and(|j| j.as_ref().as_ref().is_ok_and(|made| made.job.registration.is_some()));
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
    } else if sending && print_and_cut && status["chunksSent"].as_u64().unwrap_or(0) <= 1 {
        ui.horizontal(|ui| {
            ui.spinner();
            ui.label(tl!("Looking for the registration marks…"));
        });
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
        // C4.16: what the machine said after the job, e.g. that part of it lay outside the cut area.
        for w in status["warnings"].as_array().into_iter().flatten().filter_map(Value::as_str) {
            ui.label(egui::RichText::new(w).color(t.bleed));
        }
    }
    if let Some((bad, note)) = &m.note
        && !note.is_empty()
    {
        ui.label(egui::RichText::new(note).color(if *bad { t.bleed } else { t.text_dim }));
    }
    ui.horizontal(|ui| {
        let button = widgets::primary_button_enabled(ui, "Send", device.is_some() && has_job && !sending);
        // Why Send can't be pressed, on hover.
        let why = if found.is_empty() {
            tl!("No cutter is connected.")
        } else if !has_job {
            tl!("Nothing to cut.")
        } else {
            tl!("A cut is running.")
        };
        if button.on_disabled_hover_text(why).clicked() {
            let device = device.clone().unwrap_or_default();
            match from_the_back(app) {
                Some(material) => m.ask_mirror = Some((device, material)),
                None => send(app, &mut m, &device),
            }
        }
        if waiting.is_some() && ui.button(tl!("Continue")).clicked() {
            let _ = app.run("cutting.continue", json!({}));
        }
        if sending && ui.button(tl!("Cancel")).on_hover_text(tl!("Stops after the piece the machine is cutting")).clicked() {
            let _ = app.run("cutting.cancel", json!({}));
        }
    });
    ask_mirror_dialog(app, &ctx, &mut m);
    crate::panels::set_pstate(&ctx, "cutting-machines", m);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use vectorcraft_cutter::simulated::{Behaviour, SIMULATED_ID_PREFIX, Sent, SimulatedMachine};

    use super::*;

    /// Two frames of the window on its own (a window sizes itself on its first); the texts drawn.
    /// The window's texts on a screen tall enough for its whole column (C14.8: the tool settings
    /// made it taller than most screens, where it scrolls).
    /// The send in progress is the process's (the cutter registry), so a test that sends would
    /// change what the others' windows show (no Find Cutters, no Send): they take turns.
    fn serial() -> std::sync::MutexGuard<'static, ()> {
        static SERIAL: std::sync::Mutex<()> = std::sync::Mutex::new(());
        SERIAL.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    fn window_texts(app: &mut VectorcraftApp, ctx: &egui::Context) -> Vec<String> {
        window_texts_on(app, ctx, egui::vec2(1200.0, 1600.0))
    }

    fn window_texts_on(app: &mut VectorcraftApp, ctx: &egui::Context, screen: egui::Vec2) -> Vec<String> {
        let mut texts = vec![];
        for _ in 0..2 {
            let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen)), ..Default::default() };
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

    /// C9.2: the window opens three quarters of the app's height (the preview with it), not the
    /// old fixed 320 × 440 px preview, and its content stays inside it.
    #[test]
    fn the_window_opens_large() {
        let _serial = serial();
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        app.adopt_context(&ctx);
        app.run(COMMAND, json!({})).unwrap();
        window_texts_on(&mut app, &ctx, egui::vec2(1200.0, 900.0));
        let r = ctx.memory(|m| m.area_rect(egui::Id::unique("cutting-send-window"))).unwrap();
        assert!(r.height() >= 900.0 * OPEN_HEIGHT - 50.0 && r.width() > 2.0 * SIDE_W + GAP, "the title bar aside: {r:?}");
        assert!(r.bottom() <= 900.0 && r.top() >= 0.0, "on screen: {r:?}");
    }

    /// C18.18: a cut's length as a person reads it.
    #[test]
    fn durations_read_as_minutes_and_hours() {
        let _serial = serial();
        assert_eq!(duration(7.4), "7 s");
        assert_eq!(duration(200.0), "3 min 20 s");
        assert_eq!(duration(120.0), "2 min");
        assert_eq!(duration(3900.0), "1 h 5 min");
        assert_eq!(duration(f64::NAN), "—");
        assert_eq!(duration(-1.0), "—");
    }

    /// C18.18: Send stays in sight on a screen too short for the column, which scrolls above it.
    #[test]
    fn send_stays_at_the_foot_of_a_short_window() {
        let _serial = serial();
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        set_open(&ctx, true);
        let screen = egui::vec2(1000.0, 500.0);
        let mut sends = vec![];
        for _ in 0..3 {
            let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen)), ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| show(&mut app, ui.ctx()));
            out.textures_delta.clear();
            sends = out
                .shapes
                .iter()
                .filter_map(|c| match &c.shape {
                    egui::Shape::Text(t) if t.galley.text() == "Send" => Some((t.pos.y, c.clip_rect)),
                    _ => None,
                })
                .collect();
        }
        let window = ctx.memory(|m| m.area_rect(egui::Id::unique("cutting-send-window"))).unwrap();
        let &(y, clip) = sends.last().expect("Send drawn");
        assert!(
            y > window.center().y && y < window.bottom() && y < screen.y && clip.contains(egui::pos2(clip.center().x, y)),
            "in sight: {y} in {window:?}, clip {clip:?}"
        );
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
        let _serial = serial();
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
        // C18.18: the numbers in two columns, the playback under the preview.
        let at = |label: &str| texts.iter().position(|t| t == label).unwrap();
        assert_eq!(texts.get(at("Paths") + 1).map(String::as_str), Some("1"), "the job: {texts:?}");
        assert!(at("Play") < at("Job"), "under the preview, before the column: {texts:?}");
        assert!(texts.iter().any(|t| t == "Play"), "the playback: {texts:?}");
        // C18.27: Esc closes it.
        let esc = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1200.0, 1600.0))),
            events: vec![egui::Event::Key {
                key: egui::Key::Escape,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        };
        let _ = ctx.run_ui(esc, |ui| show(&mut app, ui.ctx()));
        assert!(!is_open(&ctx), "Esc closed it");
        assert!(window_texts(&mut app, &ctx).is_empty());
    }

    /// The window lists the job's warnings, and says why there is no job.
    #[test]
    fn the_window_shows_the_warnings_and_why_there_is_no_job() {
        let _serial = serial();
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

    /// C12.5: a Print & Cut job says so, with how far apart the marks it looks for are.
    #[test]
    fn a_print_and_cut_job_says_so() {
        let _serial = serial();
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        let mm = 72.0 / 25.4;
        app.run("file.new", json!({"width": 105.0 * mm, "height": 148.0 * mm})).unwrap();
        app.run("cutting.settings.set", json!({"printAndCut": true, "markInsetMm": 12})).unwrap();
        app.run("cutting.marks.add", json!({})).unwrap();
        app.run("shape.ellipse", json!({"x": 30.0 * mm, "y": 40.0 * mm, "width": 45.0 * mm, "height": 60.0 * mm})).unwrap();
        set_open(&ctx, true);
        let texts = window_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Print & Cut, marks: 81 × 124 mm"), "{texts:?}");
        app.run("cutting.settings.set", json!({"printAndCut": false})).unwrap();
        let texts = window_texts(&mut app, &ctx);
        assert!(!texts.iter().any(|t| t.starts_with("Print & Cut")), "{texts:?}");
    }

    /// C8.1: the test cut's choices show, and say ahead when the test would land on the design.
    #[test]
    fn the_window_offers_a_test_cut() {
        let _serial = serial();
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        app.run("shape.rectangle", json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
        set_open(&ctx, true);
        let texts = window_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Test Cut") && texts.iter().any(|t| t == "Top Left"), "{texts:?}");
        // C14.8: the tool settings, tuned here, just above the test cut.
        let at = |label: &str| texts.iter().position(|t| t == label).unwrap();
        assert!(at("Tool Settings") < at("Force") && at("Force") < at("Test at") && at("Material") < at("Force"), "{texts:?}");
        // C18.18: Send at the column's foot, after everything that it scrolls.
        let send = texts.iter().rposition(|t| t == "Send").unwrap();
        assert!(at("History") < send && at("Cutter") < at("Find Cutters"), "{texts:?}");
        // C14.11: the mirrored cut goes with the material, before the test cut.
        assert!(at("Force") < at("Cut Mirrored") && at("Cut Mirrored") < at("Test at"), "{texts:?}");
        assert!(texts.iter().any(|t| t.contains("lands on the design")), "{texts:?}");
        crate::panels::set_pstate(&ctx, "cutting-test", TestChoice { corner: 3, ..Default::default() });
        let texts = window_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Bottom Right") && !texts.iter().any(|t| t.contains("lands on the design")), "{texts:?}");
        assert!(!texts.iter().any(|t| t == "Setup"), "not cut in groups");
        // C4.6: the origin adjustment, closed at first.
        assert!(texts.iter().any(|t| t == "Origin Adjustment") && !texts.iter().any(|t| t == "Across"), "{texts:?}");
        crate::panels::set_pstate(&ctx, "cutting-origin", true);
        let texts = window_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Across") && texts.iter().any(|t| t == "Down"), "{texts:?}");
        // C4.9: the curve test, closed at first.
        assert!(texts.iter().any(|t| t == "Curve Test") && !texts.iter().any(|t| t == "Cut Curve Test"), "{texts:?}");
        crate::panels::set_pstate(&ctx, "cutting-curves", true);
        let texts = window_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Cut Curve Test") && texts.iter().any(|t| t == "All Three Rows"), "{texts:?}");
        crate::panels::set_pstate(&ctx, "cutting-origin", false);
        app.run("cutting.settings.set", json!({"grouping": "layer"})).unwrap();
        assert!(window_texts(&mut app, &ctx).iter().any(|t| t == "Setup"));
    }

    /// C8.6: with several sheets, the window says which is on the mat and moves to the next.
    #[test]
    fn the_window_steps_through_the_sheets() {
        let _serial = serial();
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        set_open(&ctx, true);
        assert!(!window_texts(&mut app, &ctx).iter().any(|t| t.starts_with("Sheet ")), "one artboard: no sheets");
        app.run("artboard.new", json!({"name": "Sheet 2"})).unwrap();
        // The sheet buttons hide while a send runs; the machines are the process's, so another
        // test's send may be running.
        let idle = |app: &mut VectorcraftApp| app.session.execute("cutting.status", &json!({})).unwrap()["sending"] != true;
        let texts = window_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Sheet 1 of 2: Artboard 1"), "{texts:?}");
        assert!(!idle(&mut app) || texts.iter().any(|t| t == "Next Sheet"), "{texts:?}");
        app.run("cutting.settings.set", json!({"artboard": 1})).unwrap();
        let texts = window_texts(&mut app, &ctx);
        assert!(texts.iter().any(|t| t == "Sheet 2 of 2: Sheet 2"), "{texts:?}");
        assert!(!idle(&mut app) || texts.iter().any(|t| t == "Previous Sheet"), "{texts:?}");
    }

    /// C4.8 and C5.3: a simulated cutter is named as one, and a send cut by layer that has paused
    /// before the second says so and offers Continue, then finishes. The only test here that
    /// installs machines.
    #[test]
    fn a_send_pauses_continues_and_finishes() {
        let _serial = serial();
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
                // C4.16: nothing outside the cut area.
                answers: vec![(b"@\x03".to_vec(), b"    0\x03".to_vec())],
                replies: vec![(vec![0x1B, 0x15], b"2,0".to_vec())],
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
        // C10.3: what is fitted, asked once the machine is found (the registry is shared with
        // other tests: a send of theirs running makes it say so instead); a tool the job needs
        // that isn't fitted is said without asking again.
        if !texts.iter().any(|t| t.contains("a send is running")) {
            assert!(texts.iter().any(|t| t == "Holder 1: AutoBlade · Holder 2: empty") && texts.iter().any(|t| t == "Check Tools"), "{texts:?}");
            app.run("cutting.settings.set", json!({"operations": {"cut": {"tool": "kraft3"}}})).unwrap();
            let texts = window_texts(&mut app, &ctx);
            assert!(texts.iter().any(|t| t == "The cut uses the Kraft 3 mm in holder 2, but holder 2 is empty"), "{texts:?}");
            app.run("cutting.settings.set", json!({"operations": {"cut": {"tool": null}}})).unwrap();
        }
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
        // C8.8: and can be sent again, from this document.
        assert!(texts.iter().any(|t| t == "Send Again"), "{texts:?}");
        vectorcraft_cutter::registry::install(None);
    }

    /// C14.12: Send asks whether to cut mirrored only with a material cut from the back, and never
    /// with Print & Cut; Cut Mirrored and As Is set the switch before the send, Cancel sends nothing.
    #[test]
    fn a_material_cut_from_the_back_asks_whether_to_mirror() {
        let _serial = serial();
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        app.run("file.new", json!({"width": 300, "height": 300})).unwrap();
        app.run("cutting.material.apply", json!({"name": "Vinyl"})).unwrap();
        assert_eq!(from_the_back(&mut app), None, "plain vinyl doesn't ask");
        app.run("cutting.material.apply", json!({"name": "Heat Transfer Vinyl"})).unwrap();
        assert_eq!(from_the_back(&mut app).as_deref(), Some("Heat Transfer Vinyl"));
        app.run("cutting.settings.set", json!({"printAndCut": true})).unwrap();
        assert_eq!(from_the_back(&mut app), None, "Print & Cut is never cut mirrored");
        app.run("cutting.settings.set", json!({"printAndCut": false})).unwrap();

        // The dialog shows while the question is open, with the material's name.
        let ctx = egui::Context::default();
        let mut m = Machines { ask_mirror: Some(("no-such-device".into(), "Heat Transfer Vinyl".into())), ..Default::default() };
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))), ..Default::default() };
        // The first frame sizes the dialog unseen; the second shows it.
        ctx.run_ui(raw.clone(), |ui| ask_mirror_dialog(&mut app, ui.ctx(), &mut m)).textures_delta.clear();
        let mut out = ctx.run_ui(raw, |ui| ask_mirror_dialog(&mut app, ui.ctx(), &mut m));
        out.textures_delta.clear();
        let texts: Vec<String> = out
            .shapes
            .iter()
            .filter_map(|c| match &c.shape {
                egui::Shape::Text(t) => Some(t.galley.text().to_string()),
                _ => None,
            })
            .collect();
        for label in ["Cut mirrored?", "Cut Mirrored", "As Is", "Cancel"] {
            assert!(texts.iter().any(|t| t == label), "{label}: {texts:?}");
        }
        assert!(texts.iter().any(|t| t.starts_with("Heat Transfer Vinyl:")), "{texts:?}");
        assert!(m.ask_mirror.is_some(), "still asking");

        // Enter is Cut Mirrored: the switch set, the send tried (to a device that isn't there).
        app.run("cutting.settings.set", json!({"mirror": false})).unwrap();
        let raw = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(800.0, 600.0))),
            events: vec![egui::Event::Key {
                key: egui::Key::Enter,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui::Modifiers::NONE,
            }],
            ..Default::default()
        };
        ctx.run_ui(raw, |ui| ask_mirror_dialog(&mut app, ui.ctx(), &mut m)).textures_delta.clear();
        assert!(m.ask_mirror.is_none());
        let settings = app.session.execute("cutting.settings.get", &json!({})).unwrap();
        assert_eq!(settings["settings"]["mirror"], json!(true));
        assert!(m.note.as_ref().is_some_and(|(bad, _)| *bad), "the send was tried: {:?}", m.note);
    }
}
