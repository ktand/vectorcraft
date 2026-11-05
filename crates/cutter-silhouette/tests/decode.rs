//! C4.10: every job the encoder sends reads back, with the decoder written from the protocol
//! notes, as the blade's runs the simulator gives for the job: the same order, passes, overcut
//! and origin adjustment, the runs before each pause in the chunks before it.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md › Robustness).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use proptest::prelude::*;
use vectorcraft_cutjob::{CutJob, JOB_VERSION, JobGroup, JobMat, OpKind, Operation, RectMm, Simulation, SourceRef, ToolSettings, Toolpath};
use vectorcraft_cutter::{DeviceInfo, Driver};
use vectorcraft_cutter_silhouette::Silhouette;
use vectorcraft_cutter_silhouette::decode::{Step, blade_paths, decode};

fn cameo4() -> DeviceInfo {
    DeviceInfo { id: "usb".into(), vendor_id: 0x0B4D, product_id: 0x1137, name: "Silhouette CAMEO 4".into() }
}

/// The simulator's blade-down runs for `job`, each with the path it cuts.
fn simulated(job: &CutJob) -> Vec<(usize, Vec<[f64; 2]>)> {
    let mut out: Vec<(usize, Vec<[f64; 2]>)> = vec![];
    let mut down_before = false;
    for m in Simulation::new(job).moves {
        if !m.down {
            down_before = false;
            continue;
        }
        if down_before && let Some(last) = out.last_mut() {
            last.1.push(m.to);
        } else {
            out.push((m.path.unwrap_or(usize::MAX), vec![m.from, m.to]));
        }
        down_before = true;
    }
    out
}

/// `runs` (mat mm) from the machine's origin, which the adjustment puts at `-adjust`: as the
/// machine is sent them, before they are rounded to its units.
fn from_origin(runs: Vec<Vec<[f64; 2]>>, adjust: [f64; 2]) -> Vec<Vec<[f64; 2]>> {
    runs.into_iter().map(|r| r.into_iter().map(|q| [q[0] + adjust[0], q[1] + adjust[1]]).collect()).collect()
}

/// `runs` on the machine's grid of units, a point the same as the one before dropped, and runs
/// left with one point dropped: the simulator skips moves of no length, and the machine's units
/// round points closer than one together.
fn merged(runs: Vec<Vec<[f64; 2]>>) -> Vec<Vec<[f64; 2]>> {
    let grid = |v: f64| (v * 20.0).round() / 20.0;
    runs.into_iter()
        .map(|r| {
            let mut out: Vec<[f64; 2]> = vec![];
            for q in r.into_iter().map(|q| [grid(q[0]), grid(q[1])]) {
                if out.last() != Some(&q) {
                    out.push(q);
                }
            }
            out
        })
        .filter(|r| r.len() > 1)
        .collect()
}

/// The same within the units' rounding (half of 1/20 mm, and a little).
fn same(a: &[[f64; 2]], b: &[[f64; 2]]) -> bool {
    a.len() == b.len() && a.iter().zip(b).all(|(p, q)| (p[0] - q[0]).abs() < 0.026 && (p[1] - q[1]).abs() < 0.026)
}

#[derive(Clone, Debug)]
struct Case {
    job: CutJob,
    adjust: [f64; 2],
}

fn case() -> impl Strategy<Value = Case> {
    let tool = (1u8..=3, 0.0..3.0f64, 0.05..1.0f64).prop_map(|(passes, overcut_mm, force)| ToolSettings {
        force,
        passes,
        overcut_mm,
        ..ToolSettings::default()
    });
    let ops = prop::collection::vec((any::<bool>(), tool), 1..=3);
    let path = (prop::collection::vec((5.0..295.0f64, 5.0..295.0f64), 2..8), any::<bool>(), any::<u8>());
    let paths = prop::collection::vec(path, 0..16);
    let groups = prop::collection::vec((1usize..16, any::<bool>()), 0..4);
    (ops, paths, groups, -3.0..3.0f64, -3.0..3.0f64).prop_map(|(ops, paths, groups, ax, ay)| {
        let operations: Vec<Operation> =
            ops.into_iter().map(|(score, tool)| Operation { kind: if score { OpKind::Score } else { OpKind::Cut }, tool }).collect();
        let n = operations.len();
        let paths: Vec<Toolpath> = paths
            .into_iter()
            .enumerate()
            .map(|(i, (pts, closed, op))| Toolpath {
                op: usize::from(op) % n,
                source: SourceRef { node: i as u64, subpath: 0 },
                // A closed path needs three points.
                closed: closed && pts.len() >= 3,
                points: pts.into_iter().map(|(x, y)| [x, y]).collect(),
            })
            .collect();
        // Groups start at 0 and then at rising path indexes inside the job.
        let mut starts: Vec<(usize, bool)> = groups.into_iter().filter(|(s, _)| *s < paths.len()).collect();
        starts.sort_by_key(|g| g.0);
        starts.dedup_by_key(|g| g.0);
        let groups: Vec<JobGroup> = if paths.is_empty() || starts.is_empty() {
            vec![]
        } else {
            std::iter::once((0, false))
                .chain(starts)
                .enumerate()
                .map(|(i, (start, pause))| JobGroup { name: format!("g{i}"), start, material: None, pause })
                .collect()
        };
        let job = CutJob {
            version: JOB_VERSION,
            mat: JobMat { id: "silhouette.12x12".into(), size_mm: [304.8, 304.8], cut_area_mm: RectMm { x0: 0.0, y0: 0.0, x1: 304.8, y1: 304.8 } },
            operations,
            paths,
            warnings: vec![],
            track_enhancing: false,
            registration: None,
            curves: vec![],
            dashes: vec![],
            groups,
        };
        Case { job, adjust: [ax, ay] }
    })
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 200, ..ProptestConfig::default() })]

    #[test]
    fn every_job_reads_back_as_its_own_cuts(c in case()) {
        prop_assume!(c.job.validate().is_ok());
        let e = Silhouette.encode_adjusted(&c.job, &cameo4(), c.adjust).unwrap();
        let steps = decode(&e.chunks.concat()).unwrap();
        prop_assert_eq!(steps.first(), Some(&Step::Reset));
        prop_assert_eq!(steps.last(), Some(&Step::End));
        let runs = merged(blade_paths(&steps).unwrap());
        let want = simulated(&c.job);
        let want: Vec<Vec<[f64; 2]>> = want.into_iter().map(|(_, r)| r).collect();
        let want = merged(from_origin(want, c.adjust));
        prop_assert_eq!(runs.len(), want.len());
        for (got, w) in runs.iter().zip(&want) {
            prop_assert!(same(got, w), "{:?} sent, {:?} simulated", got, w);
        }
        // Before each pause, the chunks hold every run of the groups before it, and no more.
        for (chunk, name) in &e.pauses {
            let group = c.job.groups.iter().find(|g| &g.name == name).unwrap();
            let before = decode(&e.chunks[..*chunk].concat()).unwrap();
            let n = merged(blade_paths(&before).unwrap()).len();
            let runs_before: Vec<Vec<[f64; 2]>> =
                simulated(&c.job).into_iter().filter(|(p, _)| *p < group.start).map(|(_, r)| r).collect();
            prop_assert_eq!(n, merged(from_origin(runs_before, c.adjust)).len(), "before {}", name);
        }
    }
}

/// A job's curves read back as runs through its points: each starts where its path does, ends
/// where it ends (a closed one where it began, past it by the overcut), and passes within the
/// flattening's reach of every point.
#[test]
fn curves_read_back_through_their_points() {
    use vectorcraft_cutjob::{Native, Piece};
    let mut job = CutJob {
        version: JOB_VERSION,
        mat: JobMat { id: "silhouette.12x12".into(), size_mm: [304.8, 304.8], cut_area_mm: RectMm { x0: 0.0, y0: 0.0, x1: 304.8, y1: 304.8 } },
        operations: vec![Operation { kind: OpKind::Cut, tool: ToolSettings { passes: 1, overcut_mm: 0.0, ..ToolSettings::default() } }],
        paths: vec![],
        warnings: vec![],
        track_enhancing: false,
        registration: None,
        curves: vec![],
        dashes: vec![],
        groups: vec![],
    };
    // A quarter circle as one cubic, its points on it; a circle as `W`.
    let k = 0.552_284_75 * 20.0;
    let quarter: Vec<[f64; 2]> =
        (0..=8).map(|i| f64::from(i) * std::f64::consts::FRAC_PI_2 / 8.0).map(|t| [50.0 + 20.0 * t.cos(), 50.0 + 20.0 * t.sin()]).collect();
    job.paths.push(Toolpath { op: 0, source: SourceRef::default(), closed: false, points: quarter.clone() });
    job.curves.push(Some(Native::Path { start: [70.0, 50.0], pieces: vec![Piece::Cubic([70.0, 50.0 + k], [50.0 + k, 70.0], [50.0, 70.0])] }));
    let ring: Vec<[f64; 2]> =
        (0..32).map(|i| f64::from(i) * std::f64::consts::TAU / 32.0).map(|t| [150.0 + 10.0 * t.sin(), 150.0 + 10.0 * t.cos()]).collect();
    job.paths.push(Toolpath { op: 0, source: SourceRef::default(), closed: true, points: ring.clone() });
    job.curves.push(Some(Native::Circle { centre: [150.0, 150.0], radius_mm: 10.0 }));
    let runs = blade_paths(&decode(&Silhouette.encode(&job, &cameo4()).unwrap().chunks.concat()).unwrap()).unwrap();
    assert_eq!(runs.len(), 2);
    let near = |run: &[[f64; 2]], q: [f64; 2], reach: f64| run.iter().any(|p| (p[0] - q[0]).hypot(p[1] - q[1]) < reach);
    for (run, pts) in runs.iter().zip([&quarter, &ring]) {
        assert!((run[0][0] - pts[0][0]).abs() < 0.026 && (run[0][1] - pts[0][1]).abs() < 0.026, "{:?} {:?}", run[0], pts[0]);
        // Sixteen points a cubic, one every 2° round a circle: within 1 mm of every point.
        assert!(pts.iter().all(|q| near(run, *q, 1.0)), "{run:?}");
    }
    let end = *runs[0].last().unwrap();
    assert!((end[0] - 50.0).abs() < 0.026 && (end[1] - 70.0).abs() < 0.026, "{end:?}");
}
