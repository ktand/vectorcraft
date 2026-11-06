//! Fork: the Send window's Calibrate (C18.23), under the test cut, closed at first: choose a
//! setting, cut a row of test cuts a step apart in it (from the test's corner, the first marked by
//! a short line), then pick the best one: its value goes into the cut settings, and into the
//! material when it is one of the user's.

use egui::Ui;
use serde_json::{Value, json};

use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

/// The settings a row can vary, as `cutting.calibrate` names them, and as the panel shows them.
const SETTINGS: [(&str, &str); 5] =
    [("force", "Force"), ("speed", "Speed"), ("depth", "Blade Depth"), ("overcut", "Overcut"), ("corners", "Corners")];

/// What the section keeps between frames.
#[derive(Clone, Debug, Default)]
struct State {
    setting: usize,
    /// The best cell's number, from 1; 0: none picked yet.
    best: usize,
    note: Option<(bool, String)>,
}

/// The rows: `corner` is the test cut's, `device` the machine chosen, `can` whether one can be
/// sent to now.
pub(crate) fn rows(app: &mut VectorcraftApp, ui: &mut Ui, corner: &str, device: Option<String>, can: bool) {
    if !crate::panels::cutting::fold(ui, "cutting-calibrate", tl!("Calibrate")) {
        return;
    }
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut st: State = crate::panels::pstate(&ctx, "cutting-calibrate-state");
    widgets::dim_label(
        ui,
        tl!("Cut a row of test cuts, each a step apart in one setting, then pick the one that came out best. The row starts at the short line."),
    );
    let names: Vec<&str> = SETTINGS.iter().map(|(_, n)| *n).collect();
    let (id, name) = SETTINGS.get(st.setting).copied().unwrap_or(SETTINGS[0]);
    widgets::label_row(ui, tl!("Setting"), crate::panels::cutting::LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "calibrate", "setting"), name, &names, 140.0) {
            st.setting = i;
            st.best = 0;
            st.note = None;
        }
    });
    let listed = app.session.execute("cutting.calibrate.cells", &json!({ "setting": id }));
    let cells: Vec<Value> = match &listed {
        Ok(r) => r["cells"].as_array().cloned().unwrap_or_default(),
        Err(e) => {
            ui.label(egui::RichText::new(e.to_string()).color(t.warning));
            vec![]
        }
    };
    if cells.is_empty() {
        crate::panels::set_pstate(&ctx, "cutting-calibrate-state", st);
        return;
    }
    let row: Vec<String> = cells.iter().map(|c| format!("{}: {}", c["n"], c["label"].as_str().unwrap_or_default())).collect();
    widgets::dim_label(ui, &row.join(" · "));
    let mut p = json!({ "setting": id, "corner": corner });
    if let Some(d) = &device {
        p["device"] = json!(d);
    }
    if ui
        .add_enabled(can, egui::Button::new(tl!("Cut Calibration Row")))
        .on_hover_text(tl!("Cuts the row at the test cut's corner, the rest of each cut as the tool settings have it"))
        .clicked()
    {
        st.note = Some(match app.run("cutting.calibrate", p) {
            Ok(_) => {
                (false, tl!("Cutting the row: when it's done, pick the cell whose triangle lifted out cleanly with the backing whole.").to_string())
            }
            Err(e) => (true, e),
        });
    }
    let labels: Vec<String> = cells.iter().map(|c| format!("{} ({})", c["n"], c["label"].as_str().unwrap_or_default())).collect();
    let choices: Vec<&str> = labels.iter().map(String::as_str).collect();
    let shown = st.best.checked_sub(1).and_then(|k| choices.get(k).copied()).unwrap_or(tl!("Pick one"));
    ui.horizontal(|ui| {
        widgets::label_row(ui, tl!("Best"), crate::panels::cutting::LABEL_W, |ui| {
            if let Some(i) = widgets::dropdown_names(ui, ("cutting", "calibrate", "best"), shown, &choices, 110.0) {
                st.best = i + 1;
            }
        });
        if ui
            .add_enabled(st.best > 0, egui::Button::new(tl!("Use It")))
            .on_hover_text(tl!("Puts that cell's value in the tool settings, and in the material if it is one of yours"))
            .clicked()
        {
            st.note = Some(match app.run("cutting.calibrate.choose", json!({ "setting": id, "cell": st.best })) {
                Ok(r) => {
                    let value = r["label"].as_str().unwrap_or_default();
                    match r["savedTo"].as_str() {
                        Some(m) => (false, format!("{} {value}, {} \u{201C}{m}\u{201D}.", tl!(name), tl!("saved in"))),
                        None => (false, format!("{} {value}. {}", tl!(name), tl!("Save as Material… keeps it."))),
                    }
                }
                Err(e) => (true, e),
            });
        }
    });
    if let Some((bad, note)) = &st.note {
        ui.label(egui::RichText::new(note).color(if *bad { t.bleed } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-calibrate-state", st);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(app: &mut VectorcraftApp, ctx: &egui::Context) -> Vec<String> {
        let mut texts = vec![];
        for _ in 0..2 {
            let raw =
                egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 1200.0))), ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| {
                egui::CentralPanel::default().show(ui, |ui| rows(app, ui, "topLeft", None, false));
            });
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

    /// C18.23: closed at first; open, the setting's row of values, the button to cut it and the
    /// choice of the best cell.
    #[test]
    fn the_row_is_offered_and_its_best_cell_chosen() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        let closed = texts(&mut app, &ctx);
        assert!(closed.iter().any(|t| t == "Calibrate") && !closed.iter().any(|t| t == "Cut Calibration Row"), "{closed:?}");
        crate::panels::set_pstate(&ctx, "cutting-calibrate", true);
        crate::panels::set_pstate(&ctx, "cutting-calibrate-state", State { setting: 3, best: 2, note: None });
        let open = texts(&mut app, &ctx);
        crate::panels::set_pstate(&ctx, "cutting-calibrate", false);
        for t in ["Overcut", "Cut Calibration Row", "1: 0 mm · 2: 0.25 mm · 3: 0.5 mm · 4: 0.75 mm · 5: 1 mm", "2 (0.25 mm)", "Use It"] {
            assert!(open.iter().any(|x| x == t), "{t}: {open:?}");
        }
    }
}
