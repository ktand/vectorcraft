//! Fork: View › Cutting View on the canvas and in the menus (C2.5).

use egui::{Pos2, vec2};
use serde_json::json;
use vectorcraft_engine::Session;

use crate::VectorcraftApp;

fn app() -> VectorcraftApp {
    let mut app = VectorcraftApp::new(Session::new(), Default::default());
    app.run("file.new", json!({"width": 400, "height": 300})).unwrap();
    app
}

/// One headless canvas frame on an 800 × 600 window; the texts it painted.
fn frame_texts(app: &mut VectorcraftApp, ctx: &egui::Context) -> Vec<String> {
    let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(Pos2::ZERO, vec2(800.0, 600.0))), ..Default::default() };
    let mut out = ctx.run_ui(raw, |ui| crate::canvas::show(app, ui));
    out.textures_delta.clear();
    out.shapes
        .iter()
        .filter_map(|c| match &c.shape {
            egui::Shape::Text(t) => Some(t.galley.text().to_string()),
            _ => None,
        })
        .collect()
}

/// The 12 × 12 in mat: numbered each inch along the top and left, its size in a corner, the
/// origin's unit; a custom mat numbered each 50 mm.
#[test]
fn the_cutting_view_draws_the_mat_as_printed() {
    let mut app = app();
    let ctx = egui::Context::default();
    let texts = frame_texts(&mut app, &ctx);
    assert!(!texts.iter().any(|t| t == "12 × 12 in"), "hidden by default: {texts:?}");
    assert!(crate::menus::checked(&app, "view.cutting", &json!({})) != Some(true));
    app.run("view.cutting", json!({})).unwrap();
    assert_eq!(crate::menus::checked(&app, "view.cutting", &json!({})), Some(true));
    app.run("view.setZoom", json!({"zoom": 100})).unwrap();
    let texts = frame_texts(&mut app, &ctx);
    for inch in 1..=11 {
        assert_eq!(texts.iter().filter(|t| **t == inch.to_string()).count(), 2, "{inch} on both edges: {texts:?}");
    }
    assert!(texts.iter().any(|t| t == "12 × 12 in"), "the size");
    assert!(texts.iter().any(|t| t == "in"), "the origin's unit");
    assert!(!texts.iter().any(|t| t == "Off the mat"), "the artboard fits");
    app.run("cutting.settings.set", json!({"offsetMm": [250, 0]})).unwrap();
    assert!(frame_texts(&mut app, &ctx).iter().any(|t| t == "Off the mat"));
    app.run("cutting.settings.set", json!({"mat": "custom", "customMatMm": [200, 300], "offsetMm": [0, 0]})).unwrap();
    let texts = frame_texts(&mut app, &ctx);
    for mm in [50, 100, 150] {
        assert_eq!(texts.iter().filter(|t| **t == mm.to_string()).count(), 2, "{mm} on both edges: {texts:?}");
    }
    assert!(texts.iter().any(|t| t == "200 × 300 mm") && texts.iter().any(|t| t == "mm"), "{texts:?}");
    app.run("view.cutting", json!({})).unwrap();
    assert!(!frame_texts(&mut app, &ctx).iter().any(|t| t == "200 × 300 mm"));
}

/// Zoomed out, numbers thin out to stay readable rather than overlap.
#[test]
fn labels_thin_out_when_zoomed_out() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.run("view.cutting", json!({})).unwrap();
    app.run("view.setZoom", json!({"zoom": 10})).unwrap();
    let texts = frame_texts(&mut app, &ctx);
    assert!(!texts.iter().any(|t| t == "1"), "{texts:?}");
}

/// C3.6: the job's toolpaths are numbered in cutting order over the mat; the panel's Show
/// Toolpaths hides them.
#[test]
fn the_cutting_view_numbers_the_toolpaths() {
    let mut app = app();
    let ctx = egui::Context::default();
    for x in [10, 150] {
        app.run("shape.rectangle", json!({"x": x, "y": 10, "width": 50, "height": 50})).unwrap();
    }
    // A metric mat, numbered 50, 100…: the paths' numbers stand out from the mat's.
    app.run("cutting.settings.set", json!({"mat": "custom"})).unwrap();
    app.run("view.cutting", json!({})).unwrap();
    app.run("view.setZoom", json!({"zoom": 100})).unwrap();
    let texts = frame_texts(&mut app, &ctx);
    assert!(texts.iter().any(|t| t == "1") && texts.iter().any(|t| t == "2"), "{texts:?}");
    crate::cutting::set_shows_toolpaths(&ctx, false);
    let texts = frame_texts(&mut app, &ctx);
    assert!(!texts.iter().any(|t| t == "2"), "{texts:?}");
}

/// The job is made again only when the document changes, not each frame or selection change.
#[test]
fn the_job_is_kept_until_the_document_changes() {
    let mut app = app();
    let ctx = egui::Context::default();
    let id = app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap()["id"].clone();
    let a = crate::cutting::job(&app, &ctx).unwrap();
    app.run("select.set", json!({"ids": [id]})).unwrap();
    assert!(std::sync::Arc::ptr_eq(&a, &crate::cutting::job(&app, &ctx).unwrap()), "selecting keeps it");
    app.run("cutting.op.set", json!({"op": "score"})).unwrap();
    let b = crate::cutting::job(&app, &ctx).unwrap();
    assert!(!std::sync::Arc::ptr_eq(&a, &b), "an edit makes it again");
    let job = &b.as_ref().as_ref().unwrap().job;
    assert_eq!(job.operations[0].kind, vectorcraft_cutjob::OpKind::Score);
}

/// C3.7: playing moves the blade along the run at SPEEDUP times its estimated pace, then stops
/// at the end.
#[test]
fn playback_moves_the_blade_and_stops_at_the_end() {
    use crate::cutting::{Playback, SPEEDUP, playback, set_playback, tick};
    let mut app = app();
    app.run("shape.rectangle", json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
    let ctx = egui::Context::default();
    let made = crate::cutting::job(&app, &ctx).unwrap();
    let sim = &made.as_ref().as_ref().unwrap().sim;
    let at = |time: f64| {
        let mut got = Playback::default();
        let mut out = ctx.run_ui(egui::RawInput { time: Some(time), ..Default::default() }, |ui| got = tick(ui.ctx(), sim));
        out.textures_delta.clear();
        got
    };
    at(0.0);
    set_playback(&ctx, Playback { mm: Some(0.0), playing: true, last: 0.0 });
    let p = at(0.01);
    let want = 0.01 * SPEEDUP * sim.total_mm / sim.total_seconds;
    assert!(p.playing && (p.mm.unwrap() - want).abs() < 1e-6, "{p:?}, want {want}");
    // Long after: at the end, stopped.
    let mut t = 0.01;
    while playback(&ctx).playing && t < 10_000.0 {
        t += 0.25;
        at(t);
    }
    let p = playback(&ctx);
    assert!(!p.playing && p.mm == Some(sim.total_mm), "{p:?}");
}
