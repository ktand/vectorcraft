//! The test cut (C8.1): one small shape cut at a corner of the mat's cut area with the setup a
//! real cut would use, to check the blade before a whole sheet. A 10 mm square with a triangle
//! inside it, the triangle cut first so the square still holds while it is cut, the way a test cut
//! is usually checked: the triangle lifts out and the square stays on the backing.

use vectorcraft_cutjob::{CutJob, JOB_VERSION, JobMat, JobWarning, OpKind, Operation, RectMm, SourceRef, ToolSettings, Toolpath};
use vectorcraft_doc::Document;

use crate::{BuildError, setups};

/// The square's side, mm.
pub const SQUARE_MM: f64 = 10.0;
/// How far in from the cut area's corner the square sits, mm.
pub const MARGIN_MM: f64 = 3.0;
/// How far the triangle sits inside the square, mm.
const TRIANGLE_INSET_MM: f64 = 2.0;
/// The design's paths this close to the test shape (mm) count as under it.
const CLEARANCE_MM: f64 = 1.0;

/// The corner of the cut area the test is cut at.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Corner {
    #[default]
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

impl Corner {
    pub const CHOICES: [(&'static str, Corner); 4] =
        [("topLeft", Corner::TopLeft), ("topRight", Corner::TopRight), ("bottomLeft", Corner::BottomLeft), ("bottomRight", Corner::BottomRight)];

    pub fn parse(id: &str) -> Option<Corner> {
        Self::CHOICES.iter().find(|(k, _)| *k == id).map(|(_, c)| *c)
    }

    pub fn id(self) -> &'static str {
        Self::CHOICES.iter().find(|(_, c)| *c == self).map_or("topLeft", |(k, _)| k)
    }
}

/// Where the test square goes in `area` (mm), or `None` when the area is too small for it.
pub fn square_at(area: RectMm, corner: Corner) -> Option<RectMm> {
    let need = SQUARE_MM + 2.0 * MARGIN_MM;
    if !(area.width() >= need && area.height() >= need) {
        return None;
    }
    let x0 = match corner {
        Corner::TopLeft | Corner::BottomLeft => area.x0 + MARGIN_MM,
        Corner::TopRight | Corner::BottomRight => area.x1 - MARGIN_MM - SQUARE_MM,
    };
    let y0 = match corner {
        Corner::TopLeft | Corner::TopRight => area.y0 + MARGIN_MM,
        Corner::BottomLeft | Corner::BottomRight => area.y1 - MARGIN_MM - SQUARE_MM,
    };
    Some(RectMm { x0, y0, x1: x0 + SQUARE_MM, y1: y0 + SQUARE_MM })
}

/// The test cut for `d`'s mat at `corner`, with the cut setup of group `group` (its own setup
/// when it has one) or else the document's. Warns when the design's own paths lie under it.
pub fn test_job(d: &Document, corner: Corner, group: Option<&str>) -> Result<CutJob, BuildError> {
    let s = crate::settings(d);
    let mat = s.mat_geometry();
    let area = mat.cut_area_mm;
    let sq = square_at(area, corner).ok_or_else(|| BuildError::TestCut("the mat's cut area is too small for a test cut".into()))?;
    let own = group.and_then(|g| setups::setup_of(d, &s, g));
    let tool: ToolSettings = own.map_or_else(|| s.operations.cut.clone(), |g| g.cut);
    let i = TRIANGLE_INSET_MM;
    let triangle = vec![[sq.x0 + i, sq.y1 - i], [(sq.x0 + sq.x1) / 2.0, sq.y0 + i], [sq.x1 - i, sq.y1 - i]];
    let square = vec![[sq.x0, sq.y0], [sq.x1, sq.y0], [sq.x1, sq.y1], [sq.x0, sq.y1]];
    let path = |points| Toolpath { op: 0, source: SourceRef::default(), closed: true, points };
    let mut warnings = vec![];
    // The design's job, where it can be made, to see whether the test would cut into it.
    if let Ok(job) = crate::build_job(d) {
        let c = CLEARANCE_MM;
        let near = RectMm { x0: sq.x0 - c, y0: sq.y0 - c, x1: sq.x1 + c, y1: sq.y1 + c };
        let centre = [(sq.x0 + sq.x1) / 2.0, (sq.y0 + sq.y1) / 2.0];
        // A path through the test's place, or a shape it would be cut out of.
        let under = job.paths.iter().any(|p| {
            let n = p.points.len();
            let segments = (0..n.saturating_sub(usize::from(!p.closed))).filter_map(|k| Some((*p.points.get(k)?, *p.points.get((k + 1) % n)?)));
            let mut crosses = false;
            let mut inside = false;
            for (a, b) in segments {
                crosses |= segment_meets(a, b, near);
                // Even-odd: does a ray from the centre to the right cross this edge?
                if p.closed && (a[1] > centre[1]) != (b[1] > centre[1]) {
                    let x = a[0] + (centre[1] - a[1]) / (b[1] - a[1]) * (b[0] - a[0]);
                    inside ^= x > centre[0];
                }
            }
            crosses || inside
        });
        if under {
            warnings
                .push(JobWarning { source: None, message: "the test cut lands on the design: choose another corner or move the artboard".into() });
        }
    }
    let job = CutJob {
        version: JOB_VERSION,
        mat: JobMat { id: mat.id.to_string(), size_mm: mat.size_mm, cut_area_mm: area },
        operations: vec![Operation { kind: OpKind::Cut, tool }],
        paths: vec![path(triangle), path(square)],
        warnings,
        track_enhancing: s.track_enhancing,
        groups: vec![],
        registration: None,
    };
    job.validate().map_err(BuildError::Invalid)?;
    Ok(job)
}

/// Whether the segment `a`–`b` meets `r` (clipped to it, Liang–Barsky).
fn segment_meets(a: [f64; 2], b: [f64; 2], r: RectMm) -> bool {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let (mut t0, mut t1) = (0.0f64, 1.0f64);
    for (p, q) in [(-dx, a[0] - r.x0), (dx, r.x1 - a[0]), (-dy, a[1] - r.y0), (dy, r.y1 - a[1])] {
        if p == 0.0 {
            if q < 0.0 {
                return false;
            }
        } else {
            let t = q / p;
            if p < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use vectorcraft_cutjob::{CuttingSettings, GroupSetup, Grouping, builtin_materials};
    use vectorcraft_doc::{Appearance, Node};
    use vectorcraft_geom::{Rect, shapes};

    use super::*;
    use crate::set_settings;

    #[test]
    fn the_test_shape_sits_in_each_corner() {
        let area = RectMm { x0: 10.0, y0: 10.0, x1: 300.0, y1: 300.0 };
        assert_eq!(square_at(area, Corner::TopLeft), Some(RectMm { x0: 13.0, y0: 13.0, x1: 23.0, y1: 23.0 }));
        assert_eq!(square_at(area, Corner::BottomRight), Some(RectMm { x0: 287.0, y0: 287.0, x1: 297.0, y1: 297.0 }));
        assert_eq!(square_at(RectMm { x0: 0.0, y0: 0.0, x1: 15.0, y1: 300.0 }, Corner::TopLeft), None);
        for (id, c) in Corner::CHOICES {
            assert_eq!(Corner::parse(id), Some(c));
            assert_eq!(c.id(), id);
        }
    }

    /// The triangle first, then the square round it, with the document's cut setup; a group's own
    /// setup when one is named.
    #[test]
    fn the_test_cuts_the_triangle_first_with_the_setup_asked_for() {
        let mut d = Document::new(300.0, 300.0);
        let job = test_job(&d, Corner::TopLeft, None).unwrap();
        assert_eq!(job.paths.len(), 2);
        assert_eq!(job.paths[0].points.len(), 3, "the triangle first");
        let sq = square_at(job.mat.cut_area_mm, Corner::TopLeft).unwrap();
        assert!(job.paths[0].points.iter().all(|q| q[0] > sq.x0 && q[0] < sq.x1 && q[1] > sq.y0 && q[1] < sq.y1), "inside the square");
        assert!(job.operations[0].tool.same_setup(&CuttingSettings::default().operations.cut));
        assert!(job.warnings.is_empty(), "nothing under it: {:?}", job.warnings);
        let card = builtin_materials().into_iter().find(|m| m.name == "Cardstock").unwrap();
        let mut s = CuttingSettings { grouping: Grouping::Color, ..CuttingSettings::default() };
        s.color_setups.insert("#000000".into(), GroupSetup::of_material(&card));
        set_settings(&mut d, &s);
        let job = test_job(&d, Corner::TopLeft, Some("#000000")).unwrap();
        assert!(job.operations[0].tool.same_setup(&card.cut), "the group's own");
        assert!(test_job(&d, Corner::TopLeft, Some("#123456")).unwrap().operations[0].tool.same_setup(&s.operations.cut), "no setup: the document's");
    }

    /// Art at the mat's top left: the test there warns, another corner doesn't.
    #[test]
    fn a_test_on_the_design_warns() {
        let mut d = Document::new(300.0, 300.0);
        let layer = d.layers[0].id;
        let sq = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(10.0, 10.0, 80.0, 80.0)), Appearance::default());
        d.insert(Some(layer), usize::MAX, sq).unwrap();
        let job = test_job(&d, Corner::TopLeft, None).unwrap();
        assert!(job.warnings.iter().any(|w| w.message.contains("lands on the design")), "{:?}", job.warnings);
        assert!(test_job(&d, Corner::BottomRight, None).unwrap().warnings.is_empty());
        // Only an edge across the test's place, or the test inside a shape: on the design too.
        let mut d = Document::new(300.0, 300.0);
        let layer = d.layers[0].id;
        let edge = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(0.0, 0.0, 100.0, 100.0)), Appearance::default());
        d.insert(Some(layer), usize::MAX, edge).unwrap();
        assert!(!test_job(&d, Corner::TopLeft, None).unwrap().warnings.is_empty(), "an edge across it");
        assert!(segment_meets([0.0, 5.0], [50.0, 5.0], RectMm { x0: 2.0, y0: 2.0, x1: 14.0, y1: 14.0 }));
        assert!(!segment_meets([0.0, 20.0], [50.0, 20.0], RectMm { x0: 2.0, y0: 2.0, x1: 14.0, y1: 14.0 }));
        let mut d = Document::new(800.0, 800.0);
        let layer = d.layers[0].id;
        let around = Node::path(d.alloc_id(), shapes::rectangle(Rect::new(2.0, 2.0, 790.0, 790.0)), Appearance::default());
        d.insert(Some(layer), usize::MAX, around).unwrap();
        assert!(!test_job(&d, Corner::TopLeft, None).unwrap().warnings.is_empty(), "inside a shape");
    }
}
