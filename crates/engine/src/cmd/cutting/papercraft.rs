//! Fork: papercraft from a 3D model (C15.6): an OBJ or STL file unfolded by
//! `vectorcraft_cutting::template::papercraft` and added as one group on top, centred on the
//! cutting artboard: the pieces' outlines set to Cut, the mountain and the valley folds set to
//! Score in colours of their own, the glue tabs marked and the edges' numbers printed (both No
//! Cut). One undo step.

use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_color::{Color, Paint};
use vectorcraft_cutjob::Op;
use vectorcraft_cutting::ops::set_own_op;
use vectorcraft_cutting::placement::MM_PER_PT;
use vectorcraft_cutting::template::papercraft::{self, Craft};
use vectorcraft_doc::{Appearance, CharStyle, Document, Node, NodeId, NodeKind, TextObject};
use vectorcraft_geom::{Affine, PathData, Point};

use super::{CommandSpec, Result, Session, bad, check_numbers, cmd, has_doc, str_param};

const CMD: &str = "cutting.papercraft";
/// The group's name.
pub const NAME: &str = "Papercraft";
const MOUNTAIN: Color = Color::Rgb { r: 0.0, g: 0.45, b: 1.0 };
const VALLEY: Color = Color::Rgb { r: 0.0, g: 0.65, b: 0.3 };
const GLUE: Color = Color::Rgb { r: 0.85, g: 0.85, b: 0.85 };
/// The edges' numbers' size, pt.
const NUMBER_PT: f64 = 6.0;

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "cutting.papercraft",
        "Papercraft from 3D Model",
        [],
        None,
        "{path?: an .obj or .stl file (the desktop; else name and dataBase64), name?: the file's name (with dataBase64; its extension says what it is), dataBase64?: the file's bytes, sizeMm?: 20–2000 (default 150: the model's longest side), tabMm?: 2–30 (default 6), rowMm?: 50–3000 (default: the mat's cut width: how wide the rows of pieces are laid out)} the model unfolded into flat pieces, added as one group on top centred on the cutting artboard: the outlines set to Cut, the mountain folds (blue) and the valley folds (green) set to Score, the glue tabs marked and each cut edge's number printed on both its sides, so pieces go together in pairs; one undo step → {id, pieces, faces, widthMm, heightMm, warnings}",
        has_doc,
        add
    )]
}

/// The model's name and bytes, from a path or from the params.
fn source(p: &Value) -> Result<(String, Vec<u8>)> {
    if let Some(path) = str_param(p, "path") {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let max = vectorcraft_cutting::template::mesh::MAX_BYTES;
            let bytes = super::super::fileio::read_file_head(path, max + 1)?;
            if bytes.len() > max {
                return Err(bad(CMD, format!("{path}: larger than {} MB", max / (1024 * 1024))));
            }
            return Ok((path.to_string(), bytes));
        }
        #[cfg(target_arch = "wasm32")]
        return Err(bad(CMD, format!("{path}: give the file's name and dataBase64 here")));
    }
    let (Some(name), Some(data)) = (str_param(p, "name"), str_param(p, "dataBase64")) else {
        return Err(bad(CMD, "give a path, or a name and dataBase64"));
    };
    let bytes = vectorcraft_format::base64_decode(data).ok_or_else(|| bad(CMD, "dataBase64 isn't base64"))?;
    Ok((name.to_string(), bytes))
}

fn part(d: &mut Document, name: &str, path: PathData, appearance: Appearance, op: Op) -> Arc<Node> {
    let mut n = Node::path(d.alloc_id(), path, appearance);
    n.name = Some(name.to_string());
    set_own_op(&mut n, Some(op));
    Arc::new(n)
}

fn add(s: &mut Session, p: &Value) -> Result<Value> {
    check_numbers(p, &["sizeMm", "tabMm", "rowMm"], CMD)?;
    let get = |k: &str, v: f64| p.get(k).and_then(Value::as_f64).unwrap_or(v);
    let (size, tab) = (get("sizeMm", 150.0), get("tabMm", 6.0));
    let d = &s.doc()?.doc;
    let settings = vectorcraft_cutting::settings(d);
    let cut_w = vectorcraft_cutting::placement::place(&settings, d).map_or(280.0, |pl| pl.mat.cut_area_mm.width());
    let row = get("rowMm", cut_w);
    if !(20.0..=2000.0).contains(&size) || !(2.0..=30.0).contains(&tab) || !(50.0..=3000.0).contains(&row) {
        return Err(bad(CMD, "sizeMm is 20–2000, tabMm 2–30, rowMm 50–3000"));
    }
    let (name, bytes) = source(p)?;
    let Craft { net, valleys, labels, pieces, faces } = papercraft::from_file(&name, &bytes, size, tab, row).map_err(|e| bad(CMD, e))?;
    let (w, h) = net.size();
    let r = usize::try_from(settings.artboard)
        .ok()
        .and_then(|i| d.artboards.get(i))
        .map(|a| a.rect)
        .ok_or_else(|| bad(CMD, "the cutting artboard isn't in the document"))?;
    let at = ((r.x0 + r.x1) / 2.0 - w / MM_PER_PT / 2.0, (r.y0 + r.y1) / 2.0 - h / MM_PER_PT / 2.0);
    let to_doc = Affine::translate(at) * Affine::scale(1.0 / MM_PER_PT);
    let cut = net.cut.transformed(to_doc);
    let warnings = super::popup::off_the_mat(d, &cut, "net");
    let top = d.layers.last().map(|l| l.id).ok_or_else(|| bad(CMD, "the document has no layer"))?;
    let mut id: Option<NodeId> = None;
    s.edit("Papercraft from 3D Model", |d, _| {
        let mut parts = vec![
            part(d, "Glue", net.glue.transformed(to_doc), Appearance::basic(Paint::solid(GLUE), Paint::None, 0.0), Op::NoCut),
            part(d, "Cut", cut.clone(), Appearance::basic(Paint::None, Paint::solid(Color::BLACK), 0.5), Op::Cut),
            part(d, "Mountain Folds", net.folds.transformed(to_doc), Appearance::basic(Paint::None, Paint::solid(MOUNTAIN), 0.5), Op::Score),
            part(d, "Valley Folds", valleys.transformed(to_doc), Appearance::basic(Paint::None, Paint::solid(VALLEY), 0.5), Op::Score),
        ];
        let mut numbers = vec![];
        for l in &labels {
            let style = CharStyle { size: NUMBER_PT, ..CharStyle::default() };
            let at = to_doc * Point::new(l.at.0, l.at.1);
            let mut t = TextObject::point(Point::ZERO, &l.number.to_string(), style);
            // Along the edge, centred on its point, read from inside the face.
            t.xf = Affine::translate(at.to_vec2())
                * Affine::rotate(l.angle)
                * Affine::translate((-NUMBER_PT * 0.3 * l.number.to_string().len() as f64, NUMBER_PT * 0.35));
            let mut n = Node::new(d.alloc_id(), NodeKind::Text(Box::new(t)));
            n.name = Some(l.number.to_string());
            set_own_op(&mut n, Some(Op::NoCut));
            numbers.push(Arc::new(n));
        }
        if !numbers.is_empty() {
            let mut g = Node::group(d.alloc_id(), numbers);
            g.name = Some("Edge Numbers".to_string());
            parts.push(Arc::new(g));
        }
        let mut g = Node::group(d.alloc_id(), parts);
        g.name = Some(NAME.to_string());
        id = Some(d.insert(Some(top), usize::MAX, g).map_err(|e| bad(CMD, e.to_string()))?);
        Ok(())
    })?;
    let id = id.ok_or_else(|| bad(CMD, "the papercraft wasn't added"))?;
    Ok(json!({ "id": id.0, "pieces": pieces, "faces": faces, "widthMm": w, "heightMm": h, "warnings": warnings }))
}
