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
    MarkLayout::for_page([a.width() * MM_PER_PT, a.height() * MM_PER_PT], s.mark_style())
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
                let line = m.line_mm / MM_PER_PT;
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
    let registration = JobRegistration {
        origin_mm: [at.x0 + m.origin_mm[0], at.y0 + m.origin_mm[1]],
        size_mm: m.size_mm,
        length_mm: m.length_mm,
        line_mm: m.line_mm,
    };
    let mut warnings = vec![];
    match marks_layer(d) {
        None => warnings.push(JobWarning { source: None, message: "the registration marks aren't in the document: add them before printing".into() }),
        Some(layer) if !matches_settings(d, layer, s, &m) => warnings.push(JobWarning { source: None, message: STALE_MARKS.into() }),
        Some(_) => {}
    }
    if crate::bleed::is_stale(d, s.bleed_mm) {
        warnings.push(JobWarning { source: None, message: crate::bleed::STALE_BLEED.into() });
    }
    let clear: Vec<RectMm> = m.no_go().into_iter().map(on_mat).collect();
    let mut warned = std::collections::HashSet::new();
    for p in paths {
        let near =
            p.points.windows(2).any(|w| clear.iter().any(|c| crosses(w[0], w[1], c))) || p.points.iter().any(|q| clear.iter().any(|c| inside(*q, c)));
        if near && warned.insert(p.source.node) {
            warnings.push(JobWarning { source: Some(p.source), message: NEAR_MARK.into() });
        }
    }
    let page = m.no_go();
    for source in printed_near(d, s, &page) {
        if warned.insert(source) {
            warnings.push(JobWarning { source: Some(vectorcraft_cutjob::SourceRef { node: source, subpath: 0 }), message: PRINTED_NEAR_MARK.into() });
        }
    }
    Ok((registration, warnings))
}

/// The warning for a marks layer drawn with other settings than the job's (C12.1).
pub const STALE_MARKS: &str = "the registration marks in the document aren't the ones set: press Update Marks, then print again";

/// Whether the marks layer `layer` is drawn as the settings say: each of its marks where the
/// settings put that mark (within a hundredth of a mm), and the corners' line the settings' width.
pub(crate) fn matches_settings(d: &Document, layer: NodeId, s: &CuttingSettings, m: &MarkLayout) -> bool {
    let Some(a) = d.artboards.get(usize::try_from(s.artboard).unwrap_or(usize::MAX)).map(|a| a.rect) else { return true };
    let Some(children) = d.node(layer).and_then(|n| n.children()) else { return false };
    let want = m.marks();
    if children.len() != want.len() {
        return false;
    }
    let close = |x: f64, y: f64| (x - y).abs() < 0.01;
    children.iter().zip(&want).all(|(c, mark)| {
        let w = mark.bounds();
        let Some(b) = c.geometric_bounds() else { return false };
        let b = [(b.x0 - a.x0) * MM_PER_PT, (b.y0 - a.y0) * MM_PER_PT, (b.x1 - a.x0) * MM_PER_PT, (b.y1 - a.y0) * MM_PER_PT];
        let line = c.appearance.stroke().map(|k| k.width * MM_PER_PT).filter(|w| *w > 0.0);
        close(b[0], w.x0) && close(b[1], w.y0) && close(b[2], w.x1) && close(b[3], w.y1) && line.is_none_or(|l| close(l, m.line_mm))
    })
}

/// The warnings for art in the area kept clear round the marks.
pub const NEAR_MARK: &str = "this is cut too near a registration mark: keep the hatched areas clear, or the machine may not find the mark";
pub const PRINTED_NEAR_MARK: &str =
    "this is printed too near a registration mark: keep the hatched areas clear, or the machine may not find the mark";

/// Most outline points looked at for printed art near the marks.
const MAX_PRINTED_POINTS: usize = 2_000_000;

/// The objects whose visible art (printed, whether or not it is cut; the marks aside) comes into
/// the areas `no_go` (page mm on the settings' artboard), widened by their strokes.
fn printed_near(d: &Document, s: &CuttingSettings, no_go: &[RectMm]) -> Vec<u64> {
    let Some(a) = d.artboards.get(usize::try_from(s.artboard).unwrap_or(usize::MAX)).map(|a| a.rect) else { return vec![] };
    let marks = marks_layer(d);
    let layers: Vec<NodeId> = d.layers.iter().filter(|l| Some(l.id) != marks).map(|l| l.id).collect();
    let items = crate::select::art_under(d, &layers);
    let (outlines, _) = crate::geometry::outlines(d, &items);
    let mm = |p: vectorcraft_geom::Point| [(p.x - a.x0) * MM_PER_PT, (p.y - a.y0) * MM_PER_PT];
    let mut found = vec![];
    let mut budget = MAX_PRINTED_POINTS;
    for o in &outlines {
        if found.last() == Some(&o.source.node) {
            continue;
        }
        let reach = d.node(vectorcraft_doc::NodeId(o.source.node)).map_or(0.0, |n| n.appearance.outset() * MM_PER_PT);
        let zones: Vec<RectMm> = no_go.iter().map(|c| RectMm { x0: c.x0 - reach, y0: c.y0 - reach, x1: c.x1 + reach, y1: c.y1 + reach }).collect();
        let mut pts = vec![];
        vectorcraft_geom::flatten(o.path.elements().iter().copied(), 0.25, |el| match el {
            vectorcraft_geom::PathEl::MoveTo(p) | vectorcraft_geom::PathEl::LineTo(p) => pts.push(mm(p)),
            _ => {}
        });
        if o.closed
            && let Some(first) = pts.first().copied()
        {
            pts.push(first);
        }
        budget = budget.saturating_sub(pts.len());
        if budget == 0 {
            break;
        }
        let edge = pts.windows(2).any(|w| zones.iter().any(|c| crosses(w[0], w[1], c)));
        // A filled shape round a whole area, its edges outside it.
        let filled = o.closed && zones.iter().any(|c| winds(&pts, [(c.x0 + c.x1) / 2.0, (c.y0 + c.y1) / 2.0]));
        if edge || filled {
            found.push(o.source.node);
        }
    }
    found.dedup();
    found
}

fn inside(q: [f64; 2], c: &RectMm) -> bool {
    q[0] > c.x0 && q[0] < c.x1 && q[1] > c.y0 && q[1] < c.y1
}

/// Whether the segment from `a` to `b` passes through the inside of `c`.
fn crosses(a: [f64; 2], b: [f64; 2], c: &RectMm) -> bool {
    if inside(a, c) || inside(b, c) {
        return true;
    }
    // Clip the segment to the rectangle (Liang–Barsky): some of it left means it crosses.
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a[0] - c.x0), (dx, c.x1 - a[0]), (-dy, a[1] - c.y0), (dy, c.y1 - a[1])] {
        if p == 0.0 {
            if q <= 0.0 {
                return false;
            }
        } else {
            let r = q / p;
            if p < 0.0 {
                t0 = t0.max(r);
            } else {
                t1 = t1.min(r);
            }
        }
    }
    t0 < t1
}

/// Whether the closed polygon `pts` goes round `q` (non-zero winding).
fn winds(pts: &[[f64; 2]], q: [f64; 2]) -> bool {
    let mut w = 0i64;
    for s in pts.windows(2) {
        let (a, b) = (s[0], s[1]);
        let side = (b[0] - a[0]) * (q[1] - a[1]) - (q[0] - a[0]) * (b[1] - a[1]);
        if a[1] <= q[1] && b[1] > q[1] && side > 0.0 {
            w += 1;
        } else if a[1] > q[1] && b[1] <= q[1] && side < 0.0 {
            w -= 1;
        }
    }
    w != 0
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

    /// The marks the usual 5/8 in from the artboard, which lies at the mat's top-left: the job
    /// names them there, with their arms and line, and the area between them is what can be cut.
    #[test]
    fn the_job_says_where_the_marks_are() {
        let mut d = letter();
        add_marks(&mut d);
        let r = build_job(&d).unwrap().registration.unwrap();
        assert_eq!((r.origin_mm, r.length_mm, r.line_mm), ([15.88, 15.88], 20.0, 0.5));
        assert!((r.size_mm[0] - 184.14).abs() < 1e-6 && (r.size_mm[1] - 247.64).abs() < 1e-6, "{r:?}");
        let mut s = settings(&d);
        s.offset_mm = [20.0, 5.0];
        set_settings(&mut d, &s);
        let o = build_job(&d).unwrap().registration.unwrap().origin_mm;
        assert!((o[0] - 35.88).abs() < 1e-9 && (o[1] - 20.88).abs() < 1e-9, "with the artboard: {o:?}");
    }

    #[test]
    fn what_print_and_cut_warns_about_and_refuses() {
        let mut d = letter();
        let job = build_job(&d).unwrap();
        assert!(job.warnings.iter().any(|w| w.message.contains("aren't in the document")), "{:?}", job.warnings);
        add_marks(&mut d);
        // Art at the top-left mark: a warning, as the sensor may not find the mark.
        let layer = d.layers[0].id;
        let near = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(60.0, 60.0, 80.0, 80.0)), Appearance::default());
        let near = d.insert(Some(layer), usize::MAX, near).unwrap();
        assert!(build_job(&d).unwrap().warnings.iter().any(|w| w.message == NEAR_MARK));
        // Printed, not cut: still in the sensor's way. A title across the top band too.
        crate::ops::set_own_op(d.node_mut(near).unwrap(), Some(vectorcraft_cutjob::Op::NoCut));
        let title = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(250.0, 5.0, 350.0, 30.0)), Appearance::default());
        let title = d.insert(Some(layer), usize::MAX, title).unwrap();
        crate::ops::set_own_op(d.node_mut(title).unwrap(), Some(vectorcraft_cutjob::Op::NoCut));
        let w = build_job(&d).unwrap().warnings;
        let printed: Vec<u64> = w.iter().filter(|w| w.message == PRINTED_NEAR_MARK).filter_map(|w| w.source.map(|s| s.node)).collect();
        assert_eq!(printed, [near.0, title.0], "{w:?}");
        // A background filling the page round the marks' areas, its edges outside them.
        d.node_mut(title).unwrap().visible = false;
        d.node_mut(near).unwrap().visible = false;
        let back = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(-10.0, -10.0, 700.0, 900.0)), Appearance::default());
        let back = d.insert(Some(layer), 0, back).unwrap();
        crate::ops::set_own_op(d.node_mut(back).unwrap(), Some(vectorcraft_cutjob::Op::NoCut));
        assert!(build_job(&d).unwrap().warnings.iter().any(|w| w.source.map(|s| s.node) == Some(back.0)));
        d.node_mut(back).unwrap().visible = false;
        assert!(build_job(&d).unwrap().warnings.is_empty());
        // Marks set differently from the layer: said until it is updated.
        let mut set = settings(&d);
        set.mark_length_mm = 12.0;
        set_settings(&mut d, &set);
        assert!(build_job(&d).unwrap().warnings.iter().any(|w| w.message == STALE_MARKS));
        d.remove(marks_layer(&d).unwrap()).unwrap();
        add_marks(&mut d);
        assert!(!build_job(&d).unwrap().warnings.iter().any(|w| w.message == STALE_MARKS));
        // Art outside the marks: can't be cut.
        let outside = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(5.0, 300.0, 30.0, 310.0)), Appearance::default());
        d.insert(Some(layer), usize::MAX, outside).unwrap();
        assert!(matches!(build_job(&d), Err(BuildError::Invalid(JobError::OutsideMarks { .. }))));
        // Landscape: not the marks the machine looks for.
        let mut s = settings(&d);
        s.orientation = Orientation::Landscape;
        set_settings(&mut d, &s);
        assert!(matches!(build_job(&d), Err(BuildError::PrintAndCut(_))));
    }
}
