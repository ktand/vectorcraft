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

#[test]
fn the_cutting_view_draws_the_mat_in_millimetres() {
    let mut app = app();
    let ctx = egui::Context::default();
    let texts = frame_texts(&mut app, &ctx);
    assert!(!texts.iter().any(|t| t.ends_with(" mm")), "hidden by default: {texts:?}");
    assert!(crate::menus::checked(&app, "view.cutting", &json!({})) != Some(true));
    app.run("view.cutting", json!({})).unwrap();
    assert_eq!(crate::menus::checked(&app, "view.cutting", &json!({})), Some(true));
    // Zoomed in enough for a label every 50 mm, along both edges of the 304.8 mm mat.
    app.run("view.setZoom", json!({"zoom": 100})).unwrap();
    let texts = frame_texts(&mut app, &ctx);
    for mm in [50, 100, 150, 200, 250, 300] {
        assert_eq!(texts.iter().filter(|t| **t == format!("{mm} mm")).count(), 2, "{mm} mm on both edges: {texts:?}");
    }
    assert!(texts.iter().any(|t| t == "0"), "the origin");
    assert!(!texts.iter().any(|t| t == "Off the mat"), "the artboard fits");
    app.run("cutting.settings.set", json!({"offsetMm": [250, 0]})).unwrap();
    assert!(frame_texts(&mut app, &ctx).iter().any(|t| t == "Off the mat"));
    app.run("view.cutting", json!({})).unwrap();
    assert!(!frame_texts(&mut app, &ctx).iter().any(|t| t.ends_with(" mm")));
}

/// Zoomed out, labels thin out to stay readable rather than overlap.
#[test]
fn labels_thin_out_when_zoomed_out() {
    let mut app = app();
    let ctx = egui::Context::default();
    app.run("view.cutting", json!({})).unwrap();
    app.run("view.setZoom", json!({"zoom": 10})).unwrap();
    let texts = frame_texts(&mut app, &ctx);
    assert!(!texts.iter().any(|t| t == "50 mm"), "{texts:?}");
}
