//! Fork: the Cutting Mat panel (C7.2): what is set once for a document, the mat, where the
//! artboard lies on it and whether it goes off it (a mirrored cut is the Send window's since
//! C14.11, with the material it is for); and, one panel each since
//! C14.9, the Tiles panel (C14.7) and the Nesting panel (C8.5). Print & Cut is its own panel since
//! C12.2 ([`super::print`]); only what asks for something is said (C9.1), and how the canvas shows
//! the mat is in the View menu. Cut Settings (the parent module) holds what is set per object and per cut.

use egui::Ui;
use serde_json::{Value, json};

use super::{LABEL_W, mm_field, set};
use crate::VectorcraftApp;
use crate::theme::Tokens;
use crate::widgets;

pub const ID: &str = "cuttingMat";
pub const TILES_ID: &str = "cuttingTiles";
pub const NESTING_ID: &str = "cuttingNesting";

/// The mat, where the artboard lies on it, and whether it fits.
fn mat_rows(app: &mut VectorcraftApp, ui: &mut Ui, info: &Value) {
    let t = Tokens::get(ui.ctx());
    let s = &info["settings"];
    let mats: Vec<&Value> = info["mats"].as_array().map(|a| a.iter().collect()).unwrap_or_default();
    let mat = s["mat"].as_str().unwrap_or_default();
    let custom = mat == "custom";
    // C17.1: material fed from the roll, with no mat: its width and the length to cut.
    let roll = mat == vectorcraft_cutjob::ROLL;
    let mut labels: Vec<&str> = mats.iter().map(|m| m["label"].as_str().unwrap_or_default()).collect();
    labels.push(tl!("Custom"));
    labels.push(tl!("Roll (no mat)"));
    let current = if custom {
        tl!("Custom")
    } else if roll {
        tl!("Roll (no mat)")
    } else {
        mats.iter().find(|m| m["id"] == mat).and_then(|m| m["label"].as_str()).unwrap_or(mat)
    };
    let approximate = !custom && !roll && info["mat"]["approximate"].as_bool() == Some(true);
    widgets::label_row(ui, tl!("Mat"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown_names(ui, ("cutting", "mat"), current, &labels, 170.0) {
            let id = match mats.get(i).and_then(|m| m["id"].as_str()) {
                Some(id) => id,
                None if i == mats.len() => "custom",
                None => vectorcraft_cutjob::ROLL,
            };
            set(app, json!({ "mat": id }));
        }
        // A note, not a warning: every built-in mat's cut area is measured, not published.
        if approximate {
            widgets::dim_name(ui, "≈").on_hover_text(tl!("The cut area isn't from the maker's specifications: check it on the mat."));
        }
    });
    if custom || roll {
        let size = |i: usize| s["customMatMm"][i].as_f64().unwrap_or(304.8);
        widgets::label_row(ui, if roll { tl!("Width, Length") } else { tl!("Size") }, LABEL_W, |ui| {
            if let Some(w) = mm_field(ui, "mat-w", size(0), 10.0) {
                set(app, json!({ "customMatMm": [w, size(1)] }));
            }
            if let Some(h) = mm_field(ui, "mat-h", size(1), 10.0) {
                set(app, json!({ "customMatMm": [size(0), h] }));
            }
        });
    }
    let orientations = ["Portrait", "Landscape"];
    let landscape = s["orientation"] == "landscape";
    widgets::label_row(ui, tl!("Orientation"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "orientation"), orientations[usize::from(landscape)], &orientations, 120.0) {
            set(app, json!({ "orientation": if i == 1 { "landscape" } else { "portrait" } }));
        }
    });
    let artboards: Vec<String> = app.session.active().map(|st| st.doc.artboards.iter().map(|a| a.name.clone()).collect()).unwrap_or_default();
    if artboards.len() > 1 {
        let names: Vec<&str> = artboards.iter().map(String::as_str).collect();
        let i = s["artboard"].as_u64().and_then(|i| usize::try_from(i).ok()).unwrap_or(0);
        widgets::label_row(ui, tl!("Artboard"), LABEL_W, |ui| {
            if let Some(i) = widgets::dropdown_names(ui, ("cutting", "artboard"), names.get(i).copied().unwrap_or_default(), &names, 140.0) {
                set(app, json!({ "artboard": i }));
            }
        });
    }
    let offset = |i: usize| s["offsetMm"][i].as_f64().unwrap_or(0.0);
    widgets::label_row(ui, tl!("Offset"), LABEL_W, |ui| {
        if let Some(x) = mm_field(ui, "offset-x", offset(0), -3_000.0) {
            set(app, json!({ "offsetMm": [x, offset(1)] }));
        }
        if let Some(y) = mm_field(ui, "offset-y", offset(1), -3_000.0) {
            set(app, json!({ "offsetMm": [offset(0), y] }));
        }
    });
    let clip = s["clipToMat"].as_bool() == Some(true);
    if widgets::check(ui, tl!("Clip to Mat"), clip, true) {
        set(app, json!({ "clipToMat": !clip }));
    }
    if info["placement"]["fits"].as_bool() == Some(false) {
        ui.label(egui::RichText::new(tl!("The artboard goes off the mat.")).color(t.bleed));
    }
}

/// What the Tiles section keeps between frames: the overlap asked for and what the last Tile said.
#[derive(Clone, Debug)]
struct TileChoice {
    overlap_mm: f64,
    note: Option<(bool, String)>,
}

impl Default for TileChoice {
    fn default() -> Self {
        TileChoice { overlap_mm: vectorcraft_cutting::tile::DEFAULT_OVERLAP_MM, note: None }
    }
}

/// Tiles (C14.7): a design larger than the mat as artboards the mat's size, overlapping, each cut
/// on its own sheet (`cutting.tile`, one undo step).
fn tile_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut c: TileChoice = crate::panels::pstate(&ctx, "cutting-tile");
    let (lo, hi) = vectorcraft_cutting::tile::OVERLAP_MM;
    widgets::label_row(ui, tl!("Overlap"), LABEL_W, |ui| {
        if let Some(v) = mm_field(ui, "tile-overlap", c.overlap_mm, lo) {
            c.overlap_mm = v.clamp(lo, hi);
        }
    });
    if ui
        .button(tl!("Tile onto Mats"))
        .on_hover_text(tl!("Artboards the size of the mat over the selection, or over the artboard, each cut on its own sheet: choose one as the Artboard above to cut it."))
        .clicked()
    {
        c.note = Some(match app.run("cutting.tile", json!({ "overlapMm": c.overlap_mm })) {
            Ok(r) => (false, format!("{} {} ({} × {})", r["artboards"].as_array().map_or(0, Vec::len), tl!("tiles"), r["cols"], r["rows"])),
            Err(e) => (true, e),
        });
    }
    if let Some((bad, note)) = &c.note {
        ui.label(egui::RichText::new(note).color(if *bad { t.warning } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-tile", c);
}

/// What the Nesting section keeps between frames: the gap and turns asked for, and what the last
/// Nest said.
#[derive(Clone, Debug)]
struct NestChoice {
    gap_mm: f64,
    rotate: bool,
    /// Every 15°, not quarter turns (C13.2).
    fine: bool,
    /// What doesn't fit goes onto new sheets (C8.6).
    sheets: bool,
    /// Replicate (C13.1): this many more of each piece, nested with them.
    copies: usize,
    note: Option<(bool, String)>,
}

impl Default for NestChoice {
    fn default() -> Self {
        NestChoice { gap_mm: 2.0, rotate: true, fine: false, sheets: false, copies: 0, note: None }
    }
}

/// Nesting (C8.5): the pieces placed close together on the mat by their true shapes, the selected
/// ones or all, one undo step (`cutting.nest`).
fn nest_rows(app: &mut VectorcraftApp, ui: &mut Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut c: NestChoice = crate::panels::pstate(&ctx, "cutting-nest");
    widgets::label_row(ui, tl!("Gap"), LABEL_W, |ui| {
        if let Some(v) = mm_field(ui, "nest-gap", c.gap_mm, 0.0) {
            c.gap_mm = v.clamp(0.0, 50.0);
        }
    });
    let choices = ["Off", "Quarter Turns", "Every 15°"];
    let shown = match (c.rotate, c.fine) {
        (false, _) => 0,
        (true, false) => 1,
        (true, true) => 2,
    };
    widgets::label_row(ui, tl!("Turn Pieces"), LABEL_W, |ui| {
        if let Some(i) = widgets::dropdown(ui, ("cutting", "nest-turns"), choices[shown], &choices, 120.0) {
            (c.rotate, c.fine) = (i > 0, i == 2);
        }
    });
    if widgets::check(ui, tl!("Onto New Sheets"), c.sheets, true) {
        c.sheets = !c.sheets;
    }
    widgets::label_row(ui, tl!("Copies"), LABEL_W, |ui| {
        let max = vectorcraft_cutting::nest::MAX_COPIES as f64;
        if let Some(v) = widgets::spin_plain(ui, ("cutting", "nest-copies"), c.copies as f64, "", 0, 60.0, 1.0, 0.0, &[]) {
            // Whole and in range after the clamp: the cast is exact.
            c.copies = v.round().clamp(0.0, max) as usize;
        }
    });
    let selected = app.session.active().is_some_and(|st| !st.selection.is_empty());
    let label = if selected { "Nest Selected" } else { "Nest All" };
    if ui.button(tl!(label)).clicked() {
        let turns = if !c.rotate {
            "none"
        } else if c.fine {
            "fine"
        } else {
            "quarter"
        };
        let p = json!({ "gapMm": c.gap_mm, "turns": turns, "sheets": c.sheets, "copies": c.copies });
        c.note = Some(match app.run("cutting.nest", p) {
            Ok(r) => nest_note(&r, c.sheets),
            Err(e) => (true, e),
        });
    }
    // C18.13: the selected pieces moved to another sheet by hand.
    let sheets: Vec<String> = app.session.active().map(|st| st.doc.artboards.iter().map(|a| a.name.clone()).collect()).unwrap_or_default();
    if selected && sheets.len() > 1 {
        let names: Vec<&str> = sheets.iter().map(String::as_str).collect();
        widgets::label_row(ui, tl!("Move to"), LABEL_W, |ui| {
            if let Some(i) = widgets::dropdown_names(ui, ("cutting", "sheet-move"), tl!("Sheet…"), &names, 120.0) {
                c.note = Some(match app.run("cutting.sheet.move", json!({ "sheet": i })) {
                    Ok(r) => (false, format!("{} {}", r["moved"].as_array().map_or(0, Vec::len), tl!("moved"))),
                    Err(e) => (true, e),
                });
            }
        });
    }
    if let Some((bad, note)) = &c.note {
        ui.label(egui::RichText::new(note).color(if *bad { t.warning } else { t.text_dim }));
    }
    crate::panels::set_pstate(&ctx, "cutting-nest", c);
}

/// What a nesting did, in a line, and whether it calls for attention.
fn nest_note(r: &Value, sheets: bool) -> (bool, String) {
    let moved = r["moved"].as_array().map_or(0, Vec::len);
    let left = r["unplaced"].as_array().map_or(0, Vec::len);
    let used = r["usedLengthMm"].as_f64().unwrap_or(0.0);
    let (mut bad, mut text) = if sheets {
        let n = r["sheets"].as_array().map_or(0, Vec::len);
        let mut text = format!("{moved} {} {n} {}", tl!("nested on"), tl!("sheets"));
        if left > 0 {
            text = format!("{text}; {left} {}", tl!("too large for a sheet stayed where they were"));
        }
        (left > 0, text)
    } else if left > 0 {
        (true, format!("{moved} {} {used:.0} mm; {left} {}", tl!("nested in"), tl!("didn't fit and stayed where they were")))
    } else if r.get("copies").is_some() {
        (false, format!("{moved} {}", tl!("nested")))
    } else {
        (false, format!("{moved} {} {used:.0} mm {}", tl!("nested in"), tl!("of the mat")))
    };
    if let Some(out) = r["copiesLeftOut"].as_u64().filter(|n| *n > 0) {
        bad = true;
        text = format!("{text}; {out} {}", tl!("copies didn't fit and were left out"));
    }
    (bad, text)
}

pub fn show(app: &mut VectorcraftApp, ui: &mut Ui) {
    let Ok(info) = app.session.execute("cutting.settings.get", &json!({})) else {
        widgets::dim_label(ui, tl!("Open a document to cut it."));
        return;
    };
    mat_rows(app, ui, &info);
}

/// The Tiles panel.
pub fn show_tiles(app: &mut VectorcraftApp, ui: &mut Ui) {
    if super::has_document(app, ui) {
        tile_rows(app, ui);
    }
}

/// The Nesting panel.
pub fn show_nesting(app: &mut VectorcraftApp, ui: &mut Ui) {
    if super::has_document(app, ui) {
        nest_rows(app, ui);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(app: &mut VectorcraftApp, ctx: &egui::Context) -> Vec<String> {
        panel_texts(app, ctx, show)
    }

    fn panel_texts(app: &mut VectorcraftApp, ctx: &egui::Context, show: fn(&mut VectorcraftApp, &mut Ui)) -> Vec<String> {
        let raw = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 1600.0))), ..Default::default() };
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

    /// The mat and the artboard on it; no cutting settings, no view switches (the View menu's),
    /// and a line only when the artboard goes off the mat (C9.1).
    #[test]
    fn the_panel_shows_the_mat() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        assert!(texts(&mut app, &ctx).iter().any(|t| t == "Open a document to cut it."));
        app.run("file.new", json!({"width": 200, "height": 200})).unwrap();
        let t = texts(&mut app, &ctx);
        for label in ["Mat", "Orientation", "Offset", "Clip to Mat"] {
            assert!(t.iter().any(|x| x == label), "{label}: {t:?}");
        }
        // C14.9: tiles and nesting are panels of their own.
        let tiles = panel_texts(&mut app, &ctx, show_tiles);
        assert!(tiles.iter().any(|x| x == "Overlap") && tiles.iter().any(|x| x == "Tile onto Mats"), "{tiles:?}");
        for gone in
            ["Cut Mirrored", "Force", "Send to Cutter…", "Cutting View", "Show Toolpaths", "The artboard fits the mat.", "Overlap", "Nest All"]
        {
            assert!(!t.iter().any(|x| x == gone), "{gone}: {t:?}");
        }
        assert!(!t.iter().any(|x| x.starts_with("The cut area isn't")), "a hover note now: {t:?}");
        app.run("cutting.settings.set", json!({"offsetMm": [300, 0]})).unwrap();
        assert!(texts(&mut app, &ctx).iter().any(|x| x == "The artboard goes off the mat."));
        // C17.1: fed from the roll, its width and the length to cut.
        app.run("cutting.settings.set", json!({"mat": "roll", "offsetMm": [0, 0]})).unwrap();
        let t = texts(&mut app, &ctx);
        assert!(t.iter().any(|x| x == "Roll (no mat)") && t.iter().any(|x| x == "Width, Length"), "{t:?}");
    }

    /// C8.5: the Nesting section nests every piece, or the selection when there is one.
    #[test]
    fn the_panel_nests_the_pieces() {
        let mut app = VectorcraftApp::new(vectorcraft_engine::Session::new(), Default::default());
        let ctx = egui::Context::default();
        app.run("file.new", json!({"width": 1200, "height": 300})).unwrap();
        app.run("shape.rectangle", json!({"x": 1000, "y": 20, "width": 100, "height": 100})).unwrap();
        assert!(panel_texts(&mut app, &ctx, show_nesting).iter().any(|t| t == "Nest Selected"), "the new rectangle is selected");
        app.run("select.none", json!({})).unwrap();
        let t = panel_texts(&mut app, &ctx, show_nesting);
        assert!(t.iter().any(|x| x == "Nest All") && t.iter().any(|x| x == "Turn Pieces") && t.iter().any(|x| x == "Gap"), "{t:?}");
        assert!(app.run("cutting.job", json!({})).is_err(), "off the mat");
        app.run("cutting.nest", json!({})).unwrap();
        assert!(app.run("cutting.job", json!({})).is_ok(), "nested onto it");
        assert!(panel_texts(&mut app, &ctx, show_nesting).iter().any(|x| x == "Copies"), "Replicate (C13.1)");
        let r = app.run("cutting.nest", json!({"copies": 3})).unwrap();
        assert_eq!(super::nest_note(&r, false), (false, "4 nested".to_string()));
        let left = json!({"moved": [1], "unplaced": [], "copies": 1, "copiesLeftOut": 7});
        assert!(super::nest_note(&left, false).0 && super::nest_note(&left, false).1.contains("7 copies didn't fit"));
    }
}
