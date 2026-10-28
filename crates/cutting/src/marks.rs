//! Print & Cut (C6.1): the registration marks as a layer of the document, so they print with the
//! art, and where they lie on the mat for the job. The layer is ordinary art, locked, set to no
//! cut, and found again by a foreign attribute in the cutting namespace (`cut:marks="1"`,
//! ARCHITECTURE.md › 4.2) whatever it is renamed to. The marks themselves are generated in code
//! from [`vectorcraft_cutjob::marks`].

use vectorcraft_color::{Color, Paint};
use vectorcraft_cutjob::{CuttingSettings, JobRegistration, JobWarning, Mark, MarkLayout, Op, RectMm};
use vectorcraft_doc::{Appearance, Document, ForeignAttribute, LayerColor, Node, NodeId};
use vectorcraft_geom::{PathData, Point, SubPath};

use crate::ops::{NS, PREFIX, set_own_op};
use crate::placement::{MM_PER_PT, Placement};
use crate::prepare::Polyline;

/// The marks layer's attribute.
pub const NAME: &str = "marks";
/// The name the layer is given.
pub const LAYER_NAME: &str = "Registration Marks";

fn is_marks_layer(n: &Node) -> bool {
    n.extras().attrs.as_deref().is_some_and(|a| a.foreign.iter().any(|f| f.ns == NS && f.name == NAME))
}

/// The document's marks layer, if it has one (a top-level layer).
pub fn marks_layer(d: &Document) -> Option<NodeId> {
    d.layers.iter().find(|l| is_marks_layer(l)).map(|l| l.id)
}

/// The marks on the settings' artboard, page mm; `None` without that artboard or when it is too
/// small for them.
pub fn layout(s: &CuttingSettings, d: &Document) -> Option<MarkLayout> {
    let a = d.artboards.get(usize::try_from(s.artboard).ok()?)?.rect;
    MarkLayout::for_page([a.width() * MM_PER_PT, a.height() * MM_PER_PT], s.mark_inset_mm)
}

/// A new marks layer for the settings' artboard (ids from `d`), or why there can't be one.
pub fn marks_layer_node(s: &CuttingSettings, d: &mut Document) -> Result<Node, String> {
    let a = d.artboards.get(usize::try_from(s.artboard).unwrap_or(usize::MAX)).map(|a| a.rect).ok_or("the artboard isn't in the document")?;
    let m = layout(s, d).ok_or("the artboard is too small for registration marks")?;
    let pt = |q: [f64; 2]| Point::new(a.x0 + q[0] / MM_PER_PT, a.y0 + q[1] / MM_PER_PT);
    let black = Paint::solid(Color::BLACK);
    let mut layer = Node::layer(d.alloc_id(), LAYER_NAME, LayerColor::Preset(0));
    layer.locked = true;
    set_own_op(&mut layer, Some(Op::NoCut));
    layer.edit_attrs(|at| {
        at.foreign.push(ForeignAttribute { ns: NS.to_string(), prefix: PREFIX.to_string(), name: NAME.to_string(), value: "1".to_string() });
    });
    let mut children = vec![];
    for mark in m.marks() {
        let node = match mark {
            Mark::Square(r) => {
                let corners = [[r.x0, r.y0], [r.x1, r.y0], [r.x1, r.y1], [r.x0, r.y1]].map(pt);
                Node::path(d.alloc_id(), PathData::single(SubPath::polyline(&corners, true)), Appearance::basic(black.clone(), Paint::None, 0.0))
            }
            Mark::Corner(points) => {
                let line = vectorcraft_cutjob::marks::LINE_MM / MM_PER_PT;
                Node::path(
                    d.alloc_id(),
                    PathData::single(SubPath::polyline(&points.map(pt), false)),
                    Appearance::basic(Paint::None, black.clone(), line),
                )
            }
        };
        children.push(std::sync::Arc::new(node));
    }
    if let Some(c) = layer.children_mut() {
        *c = children;
    }
    Ok(layer)
}

/// Where the marks lie on the mat, with the job's other checks: Print & Cut needs the portrait
/// marks (the artboard as it is, not turned) and an artboard that holds them. Warnings: the marks
/// aren't in the document, and paths too near a mark for the sensor.
pub fn registration(
    s: &CuttingSettings,
    d: &Document,
    placement: &Placement,
    paths: &[Polyline],
) -> Result<(JobRegistration, Vec<JobWarning>), String> {
    if s.orientation != vectorcraft_cutjob::Orientation::Portrait {
        return Err("Print & Cut needs the artboard in portrait on the mat".into());
    }
    let m = layout(s, d).ok_or("the artboard is too small for registration marks")?;
    let at = placement.artboard_mm;
    let on_mat = |r: RectMm| RectMm { x0: at.x0 + r.x0, y0: at.y0 + r.y0, x1: at.x0 + r.x1, y1: at.y0 + r.y1 };
    let registration = JobRegistration { origin_mm: [at.x0 + m.origin_mm[0], at.y0 + m.origin_mm[1]], size_mm: m.size_mm };
    let mut warnings = vec![];
    if marks_layer(d).is_none() {
        warnings.push(JobWarning { source: None, message: "the registration marks aren't in the document: add them before printing".into() });
    }
    let clear: Vec<RectMm> = m.keep_clear().into_iter().map(on_mat).collect();
    for p in paths {
        let near = p.points.iter().any(|q| clear.iter().any(|c| q[0] > c.x0 && q[0] < c.x1 && q[1] > c.y0 && q[1] < c.y1));
        if near {
            warnings.push(JobWarning {
                source: Some(p.source),
                message: "this is too near a registration mark: the machine may not find the mark".into(),
            });
        }
    }
    Ok((registration, warnings))
}

#[cfg(test)]
mod tests {
    use vectorcraft_cutjob::{JobError, Orientation};
    use vectorcraft_geom::{Rect, shapes};

    use super::*;
    use crate::{BuildError, build_job, set_settings, settings};

    /// A Letter artboard (612 × 792 pt) with a square in its middle, Print & Cut on.
    fn letter() -> Document {
        let mut d = Document::new(612.0, 792.0);
        let layer = d.layers[0].id;
        let sq = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(200.0, 300.0, 300.0, 400.0)), Appearance::default());
        d.insert(Some(layer), usize::MAX, sq).unwrap();
        set_settings(&mut d, &CuttingSettings { mat: "silhouette.12x12".into(), print_and_cut: true, ..CuttingSettings::default() });
        d
    }

    fn add_marks(d: &mut Document) -> NodeId {
        let layer = marks_layer_node(&settings(d), d).unwrap();
        d.insert(None, usize::MAX, layer).unwrap()
    }

    #[test]
    fn the_marks_are_a_locked_layer_that_isnt_cut() {
        let mut d = letter();
        assert_eq!(marks_layer(&d), None);
        let id = add_marks(&mut d);
        assert_eq!(marks_layer(&d), Some(id));
        let layer = d.node(id).unwrap();
        assert!(layer.locked && layer.name.as_deref() == Some(LAYER_NAME));
        assert_eq!(layer.children().map(Vec::len), Some(3), "the square and two corners");
        let job = build_job(&d).unwrap();
        assert_eq!(job.paths.len(), 1, "the marks aren't cut: only the square is");
        assert!(job.warnings.is_empty(), "{:?}", job.warnings);
    }

    /// The marks 10 mm in from the artboard, which lies at the mat's top-left: the job names
    /// them there, the area between them is what can be cut.
    #[test]
    fn the_job_says_where_the_marks_are() {
        let mut d = letter();
        add_marks(&mut d);
        let r = build_job(&d).unwrap().registration.unwrap();
        assert_eq!(r.origin_mm, [10.0, 10.0]);
        assert!((r.size_mm[0] - 195.9).abs() < 1e-6 && (r.size_mm[1] - 259.4).abs() < 1e-6, "{r:?}");
        let mut s = settings(&d);
        s.offset_mm = [20.0, 5.0];
        set_settings(&mut d, &s);
        assert_eq!(build_job(&d).unwrap().registration.unwrap().origin_mm, [30.0, 15.0], "with the artboard");
    }

    #[test]
    fn what_print_and_cut_warns_about_and_refuses() {
        let mut d = letter();
        let job = build_job(&d).unwrap();
        assert!(job.warnings.iter().any(|w| w.message.contains("aren't in the document")), "{:?}", job.warnings);
        add_marks(&mut d);
        // Art at the top-left mark: a warning, as the sensor may not find the mark.
        let layer = d.layers[0].id;
        let near = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(40.0, 40.0, 60.0, 60.0)), Appearance::default());
        d.insert(Some(layer), usize::MAX, near).unwrap();
        assert!(build_job(&d).unwrap().warnings.iter().any(|w| w.message.contains("too near a registration mark")));
        // Art outside the marks: can't be cut.
        let outside = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(5.0, 300.0, 15.0, 310.0)), Appearance::default());
        d.insert(Some(layer), usize::MAX, outside).unwrap();
        assert!(matches!(build_job(&d), Err(BuildError::Invalid(JobError::OutsideMarks { .. }))));
        // Landscape: not the marks the machine looks for.
        let mut s = settings(&d);
        s.orientation = Orientation::Landscape;
        set_settings(&mut d, &s);
        assert!(matches!(build_job(&d), Err(BuildError::PrintAndCut(_))));
    }
}
