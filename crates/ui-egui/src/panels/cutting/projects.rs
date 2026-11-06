//! Fork: the Projects panel (C18.12): guided projects. Pick what you're making (an iron-on
//! design, a vinyl decal, a sticker sheet, a stencil) and follow its steps: each step says what to
//! do, and the ones VectorCraft can do have a button that does it (`cutting.project.setup`,
//! `cutting.stickers`, `cutting.bridges`, File › Print, Send to Cutter). A step is ticked when it
//! has been done: the settings a set-up step makes are read back from the document, the others
//! are ticked by their button or by hand.

use egui::Ui;
use serde_json::{Value, json};

use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

use super::LABEL_W;

pub const ID: &str = "cuttingProjects";

/// What a step's button does.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Action {
    /// Nothing to press: the step is done by hand.
    None,
    /// `cutting.project.setup` with this project.
    Setup(&'static str),
    /// `cutting.stickers` on the selection, with a cut round the sheet.
    Stickers,
    /// `cutting.bridges` on the selection.
    Bridges,
    /// File › Print.
    Print,
    /// Send to Cutter….
    Send,
}

struct Step {
    title: &'static str,
    text: &'static str,
    action: Action,
}

struct Project {
    label: &'static str,
    summary: &'static str,
    steps: &'static [Step],
}

const fn step(title: &'static str, text: &'static str, action: Action) -> Step {
    Step { title, text, action }
}

const PROJECTS: [Project; 4] = [
    Project {
        label: "Iron-On (Heat Transfer Vinyl)",
        summary: "A design pressed onto fabric with a heat press or iron: a T-shirt, a bag, a cushion.",
        steps: &[
            step(
                "Prepare the design",
                "Give each colour of vinyl its own fill colour, stacked as they'll be pressed: the top colour highest in Layers.",
                Action::None,
            ),
            step(
                "Set up the cut",
                "Cut mirrored (the vinyl is cut from the back), with a weed box round the design. With several colours: cut colour by colour, with Knockout and Align Marks.",
                Action::Setup("ironOn"),
            ),
            step(
                "Make a test cut",
                "In Send to Cutter, check the material, then Test Cut on a corner of the vinyl: the triangle should peel off and the clear carrier stay whole. Adjust Force until it does.",
                Action::Send,
            ),
            step(
                "Cut",
                "Put the vinyl on the mat shiny side down, load it and Send. At each pause, load the next colour, then Continue.",
                Action::Send,
            ),
            step(
                "Weed",
                "Peel away the vinyl round the design, starting at the weed box, and the insides of letters. Weeding Map… in the Send window shows what goes.",
                Action::None,
            ),
            step(
                "Press",
                "Press the lowest colour first, briefly, just enough to hold it. Lay each colour above in its place (the crosses line up), press, and finish with the full press. Peel the carrier as the material says.",
                Action::None,
            ),
        ],
    },
    Project {
        label: "Vinyl Decal (Adhesive Vinyl)",
        summary: "A sticker of vinyl for a window, a wall, a mug or a car: cut, weeded and moved with transfer tape.",
        steps: &[
            step(
                "Prepare the design",
                "Give each colour of vinyl its own fill colour, stacked as they'll be applied: the top colour highest in Layers.",
                Action::None,
            ),
            step(
                "Set up the cut",
                "Cut as drawn, with a weed box and weed lines so the waste comes off in strips. With several colours: cut colour by colour, with Knockout and Align Marks.",
                Action::Setup("vinylDecal"),
            ),
            step(
                "Make a test cut",
                "In Send to Cutter, check the material, then Test Cut on a corner of the vinyl: the triangle should peel off and the backing stay whole. Adjust Force until it does.",
                Action::Send,
            ),
            step(
                "Cut",
                "Put the vinyl on the mat coloured side up, load it and Send. At each pause, load the next colour, then Continue.",
                Action::Send,
            ),
            step("Weed", "Peel away the vinyl round the design, strip by strip between the weed lines, and the insides of letters.", Action::None),
            step(
                "Apply",
                "Lay transfer tape over the decal and rub it down, peel it up with the decal, place it, rub it down again and peel the tape away. With several colours, the lowest first, lining up the crosses.",
                Action::None,
            ),
        ],
    },
    Project {
        label: "Sticker Sheet (Print & Cut)",
        summary: "Printed stickers cut out round their edges, on sticker paper, with your printer and the cutter.",
        steps: &[
            step("Prepare the designs", "Place the designs on the artboard, coloured as they'll print, and select them.", Action::None),
            step(
                "Make the sticker sheet",
                "Adds the registration marks, a cut round each design that leaves the backing whole, a print bleed, and a cut round the whole sheet.",
                Action::Stickers,
            ),
            step("Print", "Print on sticker paper at 100 % (no fit to page).", Action::Print),
            step(
                "Make a test cut",
                "In Send to Cutter, check the material, then Test Cut on a corner of a spare sheet: the square should peel off and the backing stay whole.",
                Action::Send,
            ),
            step(
                "Cut",
                "Put the printed sheet on the mat with the square mark at the top left, load it and Send. The machine finds the marks first, then cuts.",
                Action::Send,
            ),
        ],
    },
    Project {
        label: "Stencil",
        summary: "A sheet with your design cut out, to paint or etch through, the middles of letters kept by narrow bridges.",
        steps: &[
            step("Prepare the design", "Text converted to outlines, or shapes, on the artboard. Select it.", Action::None),
            step("Add bridges", "Narrow bridges keep the middles of letters (the inside of an O) joined to the sheet.", Action::Bridges),
            step("Make a test cut", "In Send to Cutter, check the material, then Test Cut on a corner of the stencil sheet.", Action::Send),
            step("Cut", "Put the stencil material on the mat, load it and Send.", Action::Send),
            step("Weed", "Lift out the cut pieces; the bridges keep the middles in place.", Action::None),
        ],
    },
];

/// What the panel keeps between frames: the project chosen, and the steps ticked for each.
#[derive(Clone, Debug, Default)]
struct State {
    project: usize,
    /// (project, step) pairs ticked.
    ticked: Vec<(usize, usize)>,
    /// What the last button did, or why it couldn't.
    note: Option<(bool, String)>,
}

/// Whether a set-up step's settings are on the document: read back, so it stays ticked when the
/// panel is opened again and clears when the settings are changed by hand.
fn set_up(settings: &Value, project: &str) -> bool {
    let s = settings;
    let on = |k: &str| s[k].as_bool() == Some(true);
    match project {
        "ironOn" => on("mirror") && on("weedBox"),
        "vinylDecal" => !on("mirror") && on("weedBox") && on("weedLines"),
        _ => false,
    }
}

/// The Projects panel.
pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    if !super::has_document(app, ui) {
        return;
    }
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut st: State = crate::panels::pstate(&ctx, "cutting-projects");
    super::intro(ui, tl!("Pick what you're making and follow the steps; a button does what VectorCraft can do for you."));
    let labels: Vec<&str> = PROJECTS.iter().map(|p| p.label).collect();
    let current = PROJECTS.get(st.project).map_or("", |p| p.label);
    widgets::label_row(ui, tl!("Project"), LABEL_W - 20.0, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "project"), current, &labels, 210.0) {
            st.project = i;
            st.note = None;
        }
    });
    let Some(project) = PROJECTS.get(st.project) else { return };
    widgets::dim_label(ui, tl!(project.summary));
    ui.add_space(4.0);
    let settings = app.session.execute("cutting.settings.get", &json!({})).map(|r| r["settings"].clone()).unwrap_or_default();
    let selected = app.session.active().is_some_and(|s| !s.selection.is_empty());
    for (k, step) in project.steps.iter().enumerate() {
        ui.separator();
        let auto = matches!(step.action, Action::Setup(id) if set_up(&settings, id));
        let mut done = auto || st.ticked.contains(&(st.project, k));
        ui.horizontal(|ui| {
            let title = egui::RichText::new(format!("{}. {}", k + 1, tl!(step.title))).strong();
            if ui.checkbox(&mut done, title).on_hover_text(tl!("Tick a step once it's done")).changed() {
                st.ticked.retain(|x| *x != (st.project, k));
                if done {
                    st.ticked.push((st.project, k));
                }
            }
        });
        ui.indent(("cutting", "project-step", k), |ui| {
            ui.label(egui::RichText::new(tl!(step.text)).color(if done { t.text_dim } else { t.text }));
            let needs_selection = matches!(step.action, Action::Stickers | Action::Bridges);
            let label = match step.action {
                Action::None => return,
                Action::Setup(_) => tl!("Set Up"),
                Action::Stickers => tl!("Make Sticker Sheet"),
                Action::Bridges => tl!("Add Bridges"),
                Action::Print => tl!("Print…"),
                Action::Send => tl!("Send to Cutter…"),
            };
            if ui.add_enabled(!needs_selection || selected, egui::Button::new(label)).clicked() {
                let outcome = run(app, step.action);
                if outcome.as_ref().is_ok_and(|_| !matches!(step.action, Action::Send | Action::Print)) && !st.ticked.contains(&(st.project, k)) {
                    st.ticked.push((st.project, k));
                }
                st.note = Some(match outcome {
                    Ok(Some(note)) => (false, note),
                    Ok(None) => return,
                    Err(e) => (true, e),
                });
            }
            if needs_selection {
                super::select_hint(ui, selected, "Select the designs first.");
            }
        });
    }
    if let Some((bad, note)) = &st.note {
        ui.separator();
        ui.label(egui::RichText::new(note).color(if *bad { t.warning } else { t.text_dim }));
    }
    ui.separator();
    if ui.button(tl!("Start Over")).on_hover_text(tl!("Clear this project's ticks (the document stays as it is)")).clicked() {
        let at = st.project;
        st.ticked.retain(|(p, _)| *p != at);
        st.note = None;
    }
    crate::panels::set_pstate(&ctx, "cutting-projects", st);
}

/// Does what a step's button does: a line saying what happened (none: nothing to say), or why it
/// couldn't.
fn run(app: &mut VectorcraftApp, action: Action) -> Result<Option<String>, String> {
    match action {
        Action::None => Ok(None),
        Action::Setup(project) => {
            let r = app.run("cutting.project.setup", json!({ "project": project }))?;
            let colours = r["colours"].as_u64().unwrap_or(0);
            let material = r["material"].as_str().map(str::to_string).unwrap_or_else(|| tl!("none").to_string());
            Ok(Some(if colours > 1 {
                format!("{colours} {} {material}.", tl!("colours, cut one after the other. Material:"))
            } else {
                format!("{} {material}.", tl!("Set up. Material:"))
            }))
        }
        Action::Stickers => {
            let r = app.run("cutting.stickers", json!({ "sheetCut": true }))?;
            Ok(Some(format!("{} {}", r["pieces"], tl!("stickers, and a cut round the sheet"))))
        }
        Action::Bridges => {
            let r = app.run("cutting.bridges", json!({}))?;
            let missed = r["unbridged"].as_u64().unwrap_or(0);
            Ok(Some(if missed > 0 {
                format!("{} {} · {missed} {}", r["bridges"], tl!("bridges"), tl!("holes missed: try the other direction in the Stencil panel"))
            } else {
                format!("{} {}", r["bridges"], tl!("bridges"))
            }))
        }
        Action::Print => app.run("file.print", json!({})).map(|_| None),
        Action::Send => app.run(crate::cutting::send_window::COMMAND, json!({})).map(|_| None),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(app: &mut VectorcraftApp, ctx: &egui::Context) -> Vec<String> {
        let mut texts = vec![];
        for _ in 0..2 {
            let raw =
                egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(300.0, 2000.0))), ..Default::default() };
            let mut out = ctx.run_ui(raw, |ui| {
                egui::CentralPanel::default().show(ui, |ui| show(app, ui));
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

    /// C18.12: each project's steps, numbered; a set-up step ticked once its settings are on the
    /// document; a step needing a selection says so.
    #[test]
    fn a_project_lists_its_steps() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
        let iron = texts(&mut app, &ctx);
        for t in ["Iron-On (Heat Transfer Vinyl)", "1. Prepare the design", "2. Set up the cut", "Set Up", "6. Press", "Start Over"] {
            assert!(iron.iter().any(|x| x == t), "{t}: {iron:?}");
        }
        assert!(!set_up(&app.session.execute("cutting.settings.get", &json!({})).unwrap()["settings"], "ironOn"));
        app.run("cutting.project.setup", json!({"project": "ironOn"})).unwrap();
        assert!(set_up(&app.session.execute("cutting.settings.get", &json!({})).unwrap()["settings"], "ironOn"), "read back from the document");
        // The sticker sheet: its button needs the designs selected.
        crate::panels::set_pstate(&ctx, "cutting-projects", State { project: 2, ..Default::default() });
        app.run("select.none", json!({})).unwrap();
        let stickers = texts(&mut app, &ctx);
        crate::panels::set_pstate(&ctx, "cutting-projects", State::default());
        for t in ["Sticker Sheet (Print & Cut)", "2. Make the sticker sheet", "Make Sticker Sheet", "Print…", "Select the designs first."] {
            assert!(stickers.iter().any(|x| x == t), "{t}: {stickers:?}");
        }
        assert_eq!(PROJECTS.len(), 4);
        assert!(PROJECTS.iter().all(|p| p.steps.iter().any(|s| s.action == Action::Send)), "every project ends at the cutter");
    }
}
