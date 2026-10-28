use super::*;
use crate::job::{JOB_VERSION, JobMat, Operation, SourceRef, Toolpath};
use crate::mat::RectMm;
use crate::settings::ToolSettings;

fn tool(passes: u8, overcut_mm: f64) -> ToolSettings {
    ToolSettings { force: 0.3, speed_mm_s: 50.0, passes, overcut_mm, ..ToolSettings::default() }
}

fn path(op: usize, closed: bool, points: &[[f64; 2]]) -> Toolpath {
    Toolpath { op, source: SourceRef::default(), closed, points: points.to_vec() }
}

/// A 10 mm square at (10, 10), cut once; a 20 mm line from (50, 10), scored once.
fn job(square: ToolSettings, line: ToolSettings) -> CutJob {
    CutJob {
        version: JOB_VERSION,
        mat: JobMat { id: "t".into(), size_mm: [300.0, 300.0], cut_area_mm: RectMm { x0: 0.0, y0: 0.0, x1: 300.0, y1: 300.0 } },
        operations: vec![Operation { kind: OpKind::Score, tool: line }, Operation { kind: OpKind::Cut, tool: square }],
        paths: vec![path(1, true, &[[10.0, 10.0], [20.0, 10.0], [20.0, 20.0], [10.0, 20.0]]), path(0, false, &[[50.0, 10.0], [70.0, 10.0]])],
        warnings: vec![],
        track_enhancing: false,
        groups: vec![],
    }
}

/// With one pass and no overcut, the run is the job's stats: what is cut and what is travelled.
#[test]
fn the_run_adds_up_to_the_stats() {
    let j = job(tool(1, 0.0), tool(1, 0.0));
    let sim = Simulation::new(&j);
    let s = j.stats();
    assert!((sim.down_mm() - s.blade_mm).abs() < 1e-9, "{} vs {}", sim.down_mm(), s.blade_mm);
    assert!((sim.total_mm - (s.blade_mm + s.travel_mm)).abs() < 1e-9);
    // Cuts at 50 mm/s, travel at TRAVEL_MM_S.
    let want = s.blade_mm / 50.0 + s.travel_mm / TRAVEL_MM_S;
    assert!((sim.total_seconds - want).abs() < 1e-9);
}

/// Each pass cuts the path again; a closed one carries on round, an open one is travelled back.
#[test]
fn passes_cut_again() {
    let sim = Simulation::new(&job(tool(2, 0.0), tool(3, 0.0)));
    assert!((sim.down_mm() - (40.0 * 2.0 + 20.0 * 3.0)).abs() < 1e-9);
    let back: f64 = sim.moves.iter().filter(|m| !m.down && m.path == Some(1) && m.pass > 0).map(|m| m.mm).sum();
    assert!((back - 40.0).abs() < 1e-9, "twice back along the 20 mm line: {back}");
    assert_eq!(sim.moves.iter().filter(|m| m.down && m.path == Some(0)).map(|m| m.pass).max(), Some(1));
}

/// A closed path is cut on past its start by the overcut.
#[test]
fn the_overcut_runs_on_past_the_start() {
    let sim = Simulation::new(&job(tool(1, 2.5), tool(1, 0.0)));
    let square: f64 = sim.moves.iter().filter(|m| m.down && m.path == Some(0)).map(|m| m.mm).sum();
    assert!((square - 42.5).abs() < 1e-9, "{square}");
    let last = sim.moves.iter().rfind(|m| m.down && m.path == Some(0)).unwrap();
    assert!((last.to[0] - 12.5).abs() < 1e-9 && (last.to[1] - 10.0).abs() < 1e-9, "{:?}", last.to);
}

#[test]
fn the_blade_is_where_the_run_says() {
    let sim = Simulation::new(&job(tool(1, 0.0), tool(1, 0.0)));
    let start = sim.at_mm(0.0);
    assert_eq!((start.position, start.down, start.path), ([0.0, 0.0], false, Some(0)));
    // Travel to (10, 10) is 14.14 mm; 5 mm further, the blade is down on the square's top edge.
    let b = sim.at_mm(200f64.sqrt() + 5.0);
    assert!(b.down && b.path == Some(0) && b.op == Some(OpKind::Cut));
    assert!((b.position[0] - 15.0).abs() < 1e-9 && (b.position[1] - 10.0).abs() < 1e-9, "{:?}", b.position);
    let end = sim.at_fraction(1.0);
    assert!((end.position[0] - 70.0).abs() < 1e-9 && end.done_mm == sim.total_mm);
    assert_eq!(end.op, Some(OpKind::Score));
    // Out of range and not a number are the ends.
    assert_eq!(sim.at_mm(-5.0).position, [0.0, 0.0]);
    assert_eq!(sim.at_mm(f64::NAN).position, [0.0, 0.0]);
    assert_eq!(sim.at_mm(1e9).position, end.position);
}

#[test]
fn each_path_starts_where_its_blade_comes_down() {
    let sim = Simulation::new(&job(tool(2, 0.0), tool(1, 0.0)));
    let starts = sim.path_starts_mm();
    assert_eq!(starts.len(), 2, "two passes of the square are one path");
    assert!((starts[0] - 200f64.sqrt()).abs() < 1e-9);
    assert!((sim.at_mm(starts[1]).position[0] - 50.0).abs() < 1e-9, "the line starts at (50, 10)");
}

#[test]
fn an_empty_job_runs_nowhere() {
    let mut j = job(tool(1, 0.0), tool(1, 0.0));
    j.paths.clear();
    let sim = Simulation::new(&j);
    assert!(sim.moves.is_empty());
    assert_eq!(sim.at_fraction(0.5).position, [0.0, 0.0]);
}

#[test]
fn along_a_path() {
    let sq = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0]];
    assert_eq!(along(&sq, true, 15.0), [[0.0, 0.0], [10.0, 0.0], [10.0, 5.0]]);
    assert_eq!(along(&sq, true, 0.0), [[0.0, 0.0]]);
    assert_eq!(along(&sq, false, 100.0).len(), 4, "an open path ends");
    assert!(along(&sq, true, 1e12).len() < 100, "a huge overcut is capped");
}
