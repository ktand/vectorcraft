use proptest::prelude::*;

use super::*;

fn tool(passes: u8) -> ToolSettings {
    ToolSettings { force: 0.3, speed_mm_s: 100.0, passes, overcut_mm: 0.0, ..ToolSettings::default() }
}

fn mat() -> JobMat {
    JobMat { id: "test".into(), size_mm: [300.0, 300.0], cut_area_mm: RectMm { x0: 0.0, y0: 0.0, x1: 300.0, y1: 300.0 } }
}

fn path(op: usize, closed: bool, points: &[[f64; 2]]) -> Toolpath {
    Toolpath { op, source: SourceRef { node: 7, subpath: 0 }, closed, points: points.to_vec() }
}

/// A 10 mm square at (10, 10), closed, cut twice; then a 20 mm line, scored once.
fn job() -> CutJob {
    CutJob {
        version: JOB_VERSION,
        mat: mat(),
        operations: vec![Operation { kind: OpKind::Score, tool: tool(1) }, Operation { kind: OpKind::Cut, tool: tool(2) }],
        paths: vec![path(1, true, &[[10.0, 10.0], [20.0, 10.0], [20.0, 20.0], [10.0, 20.0]]), path(0, false, &[[50.0, 10.0], [70.0, 10.0]])],
        warnings: vec![],
        track_enhancing: false,
        registration: None,
        groups: vec![],
    }
}

#[test]
fn a_good_job_validates() {
    assert_eq!(job().validate(), Ok(()));
}

#[test]
fn stats_add_up() {
    let s = job().stats();
    assert_eq!((s.path_count, s.point_count), (2, 6));
    assert!((s.path_mm - 60.0).abs() < 1e-9, "40 + 20: {s:?}");
    assert!((s.blade_mm - 100.0).abs() < 1e-9, "40 × 2 + 20 × 1: {s:?}");
    // Origin → (10, 10); the square ends where it began; (10, 10) → (50, 10).
    let want = 200f64.sqrt() + 40.0;
    assert!((s.travel_mm - want).abs() < 1e-9, "{s:?}");
}

#[test]
fn an_empty_job_is_valid_and_adds_up_to_nothing() {
    let j = CutJob { paths: vec![], ..job() };
    assert_eq!(j.validate(), Ok(()));
    assert_eq!(j.stats(), JobStats::default());
}

#[test]
fn each_problem_is_an_error() {
    let with = |f: &dyn Fn(&mut CutJob)| {
        let mut j = job();
        f(&mut j);
        j.validate()
    };
    assert_eq!(with(&|j| j.paths[1].op = 2), Err(JobError::NoSuchOperation { path: 1 }));
    assert_eq!(with(&|j| j.paths[1].points.truncate(1)), Err(JobError::TooFewPoints { path: 1 }));
    assert_eq!(with(&|j| j.paths[0].points.truncate(2)), Err(JobError::TooFewPoints { path: 0 }));
    assert_eq!(with(&|j| j.paths[0].points[2][0] = f64::NAN), Err(JobError::NotFinite { path: 0 }));
    assert_eq!(with(&|j| j.paths[1].points[1] = [301.0, 10.0]), Err(JobError::OutsideCutArea { path: 1, point: [301.0, 10.0] }));
    assert_eq!(with(&|j| j.paths[1].points[0] = [-0.5, 10.0]), Err(JobError::OutsideCutArea { path: 1, point: [-0.5, 10.0] }));
    assert_eq!(with(&|j| j.operations[1].tool.passes = 0), Err(JobError::BadSetting { op: 1, setting: "passes" }));
    assert_eq!(with(&|j| j.operations[0].tool.force = f64::INFINITY), Err(JobError::BadSetting { op: 0, setting: "force" }));
    assert_eq!(with(&|j| j.operations[0].tool.speed_mm_s = 0.0), Err(JobError::BadSetting { op: 0, setting: "speedMmS" }));
    assert_eq!(with(&|j| j.mat.cut_area_mm.x1 = 400.0), Err(JobError::BadMat));
    assert_eq!(with(&|j| j.mat.size_mm[0] = f64::NAN), Err(JobError::BadMat));
    // On the edge, give or take rounding, is inside.
    assert_eq!(with(&|j| j.paths[1].points[1] = [300.0 + 1e-9, 0.0]), Ok(()));
}

#[test]
fn caps_are_checked_before_the_points() {
    let many = path(0, false, &[[1.0, 1.0], [2.0, 2.0]]);
    let j = CutJob { paths: vec![many; MAX_PATHS + 1], ..job() };
    assert_eq!(j.validate(), Err(JobError::TooManyPaths(MAX_PATHS + 1)));
    let long = path(0, false, &vec![[1.0, 1.0]; MAX_POINTS + 1]);
    let j = CutJob { paths: vec![long], ..job() };
    assert_eq!(j.validate(), Err(JobError::TooManyPoints(MAX_POINTS + 1)));
}

#[test]
fn errors_read_as_messages() {
    let e = JobError::OutsideCutArea { path: 3, point: [301.0, 12.345] };
    assert_eq!(e.to_string(), "path 3 leaves the cut area at (301.00, 12.35) mm");
}

#[test]
fn the_job_as_json() {
    let v = job().to_value();
    assert_eq!(v["version"], 1);
    assert_eq!(v["mat"]["cutAreaMm"], serde_json::json!([0.0, 0.0, 300.0, 300.0]));
    assert_eq!(v["operations"][1]["kind"], "cut");
    assert_eq!(v["operations"][1]["passes"], 2);
    assert_eq!(v["paths"][0]["closed"], true);
    assert_eq!(v["paths"][0]["source"], serde_json::json!({"node": 7, "subpath": 0}));
    assert_eq!(v["paths"][1]["points"], serde_json::json!([[50.0, 10.0], [70.0, 10.0]]));
    assert_eq!(job().stats().to_value()["pathCount"], 2);
}

fn coord() -> impl Strategy<Value = f64> {
    prop_oneof![8 => -10.0..320.0f64, 1 => Just(f64::NAN), 1 => Just(f64::INFINITY), 1 => any::<f64>()]
}

fn any_job() -> impl Strategy<Value = CutJob> {
    let tool = (coord(), coord(), any::<u8>(), coord()).prop_map(|(force, speed_mm_s, passes, overcut_mm)| ToolSettings {
        force,
        speed_mm_s,
        passes,
        overcut_mm,
        ..ToolSettings::default()
    });
    let op = (any::<bool>(), tool).prop_map(|(cut, tool)| Operation { kind: if cut { OpKind::Cut } else { OpKind::Score }, tool });
    let path = (0usize..4, any::<bool>(), prop::collection::vec((coord(), coord()).prop_map(|(x, y)| [x, y]), 0..6))
        .prop_map(|(op, closed, points)| Toolpath { op, source: SourceRef::default(), closed, points });
    (prop::collection::vec(op, 0..3), prop::collection::vec(path, 0..6), coord(), coord()).prop_map(|(operations, paths, w, h)| CutJob {
        version: JOB_VERSION,
        mat: JobMat { id: "p".into(), size_mm: [w, h], cut_area_mm: RectMm { x0: 0.0, y0: 0.0, x1: w, y1: h } },
        operations,
        paths,
        warnings: vec![],
        track_enhancing: false,
        registration: None,
        groups: vec![],
    })
}

proptest! {
    /// Any job validates or names its problem, without panicking; one that validates has finite
    /// stats, and every point of it is in the cut area.
    #[test]
    fn any_job_validates_or_says_why(j in any_job()) {
        match j.validate() {
            Ok(()) => {
                let s = j.stats();
                prop_assert!(s.path_mm.is_finite() && s.blade_mm.is_finite() && s.travel_mm.is_finite(), "{s:?}");
                let a = j.mat.cut_area_mm;
                for p in &j.paths {
                    prop_assert!(p.points.iter().all(|&[x, y]| x >= a.x0 - 1e-6 && x <= a.x1 + 1e-6 && y >= a.y0 - 1e-6 && y <= a.y1 + 1e-6));
                }
            }
            Err(e) => prop_assert!(!e.to_string().is_empty()),
        }
        let _ = j.stats();
        let _ = j.to_value();
    }
}
