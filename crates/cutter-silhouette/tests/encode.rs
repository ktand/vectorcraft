//! C4.4: cut jobs as Silhouette commands. The golden bytes are written from the protocol notes
//! (`docs/cutting/protocol-silhouette.md`); the round trip reads the moves back and compares them
//! with the simulation of the same job.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md › Robustness).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use serde_json::Map;
use vectorcraft_cutjob::{CutJob, JOB_VERSION, JobMat, OpKind, Operation, RectMm, Simulation, SourceRef, ToolSettings, Toolpath};
use vectorcraft_cutter::send::{Patience, Progress, send};
use vectorcraft_cutter::{DeviceInfo, Driver, MemoryTransport};
use vectorcraft_cutter_silhouette::{CHUNK_BYTES, Silhouette};

fn tool(force: f64, speed_mm_s: f64, passes: u8, overcut_mm: f64) -> ToolSettings {
    ToolSettings { force, speed_mm_s, passes, overcut_mm, extra: Map::new() }
}

fn job(ops: Vec<Operation>, paths: Vec<Toolpath>, mat: &str) -> CutJob {
    CutJob {
        version: JOB_VERSION,
        mat: JobMat { id: mat.into(), size_mm: [304.8, 304.8], cut_area_mm: RectMm { x0: 0.0, y0: 0.0, x1: 304.8, y1: 304.8 } },
        operations: ops,
        paths,
        warnings: vec![],
    }
}

fn path(op: usize, closed: bool, points: &[[f64; 2]]) -> Toolpath {
    Toolpath { op, source: SourceRef::default(), closed, points: points.to_vec() }
}

fn cameo4() -> DeviceInfo {
    DeviceInfo { id: "usb".into(), vendor_id: 0x0B4D, product_id: 0x1137, name: "Silhouette CAMEO 4".into() }
}

/// A 10 mm square at (10, 20) mm, cut once at half force and 100 mm/s.
#[test]
fn a_square_as_bytes() {
    let j = job(
        vec![Operation { kind: OpKind::Cut, tool: tool(0.5, 100.0, 1, 0.0) }],
        vec![path(0, true, &[[10.0, 20.0], [20.0, 20.0], [20.0, 30.0], [10.0, 30.0]])],
        "silhouette.12x12",
    );
    let e = Silhouette.encode(&j, &cameo4()).unwrap();
    let got = e.chunks.concat();
    let mut want = vec![0x1B, 0x04];
    for c in ["TG1", "FN0", "TB50,0", "J1", "FX17", "!10", "FC18", "M400,200", "D400,400", "D600,400", "D600,200", "D400,200", "M600,0", "SO0"] {
        want.extend(c.as_bytes());
        want.push(0x03);
    }
    assert_eq!(String::from_utf8_lossy(&got), String::from_utf8_lossy(&want));
    assert_eq!(e.chunks.len(), 1);
    assert!(e.warnings.iter().any(|w| w.contains("approximate")));
}

/// The moves read back from the bytes are the blade's moves in the simulation, in machine units.
#[test]
fn the_bytes_replay_the_simulation() {
    let j = job(
        vec![Operation { kind: OpKind::Score, tool: tool(0.1, 100.0, 1, 0.0) }, Operation { kind: OpKind::Cut, tool: tool(0.4, 200.0, 2, 1.5) }],
        vec![
            path(0, false, &[[50.0, 10.0], [70.0, 10.0], [70.0, 30.0]]),
            path(1, true, &[[10.0, 10.0], [20.0, 10.0], [20.0, 20.0], [10.0, 20.0]]),
            path(1, false, &[[100.0, 100.0], [150.5, 120.25]]),
        ],
        "silhouette.12x12",
    );
    let bytes = Silhouette.encode(&j, &cameo4()).unwrap().chunks.concat();
    // Each ETX-ended M or D command as (down, x, y) in machine units.
    let moves: Vec<(bool, i64, i64)> = bytes
        .split(|b| *b == 0x03)
        .filter_map(|c| {
            let s = std::str::from_utf8(c).ok()?;
            let down = match s.chars().next()? {
                'D' => true,
                'M' => false,
                _ => return None,
            };
            let (y, x) = s.get(1..)?.split_once(',')?;
            Some((down, x.parse().ok()?, y.parse().ok()?))
        })
        .collect();
    let u = |mm: f64| (mm * 20.0).round() as i64;
    let sim = Simulation::new(&j);
    let mut want = vec![];
    for m in &sim.moves {
        if m.down {
            want.push((true, u(m.to[0]), u(m.to[1])));
        } else {
            want.push((false, u(m.to[0]), u(m.to[1])));
        }
    }
    // The bytes end with the head moved out of the way, which the simulation doesn't.
    let (end, body) = moves.split_last().unwrap();
    assert!(!end.0);
    assert_eq!(body, want.as_slice());
}

#[test]
fn chunks_stay_small_and_split_between_commands() {
    let many: Vec<Toolpath> = (0..2_000).map(|i| path(0, false, &[[1.0, 1.0 + (i % 280) as f64], [300.0, 1.0 + (i % 280) as f64]])).collect();
    let j = job(vec![Operation { kind: OpKind::Cut, tool: tool(0.3, 100.0, 1, 0.0) }], many, "silhouette.12x12");
    let e = Silhouette.encode(&j, &cameo4()).unwrap();
    assert!(e.chunks.len() > 1);
    for c in &e.chunks {
        assert!(c.len() <= CHUNK_BYTES, "{}", c.len());
    }
    for c in e.chunks.iter().skip(1) {
        // Each chunk after the first starts a command: a letter, not the middle of numbers.
        assert!(c[0].is_ascii_alphabetic() || c[0] == b'!', "{:?}", c[0] as char);
    }
}

#[test]
fn a_mat_the_cutter_doesnt_know_is_no_mat() {
    let j = job(vec![], vec![], "custom");
    let e = Silhouette.encode(&j, &cameo4()).unwrap();
    assert!(String::from_utf8_lossy(&e.chunks.concat()).contains("TG0\u{3}"));
    assert!(e.warnings.iter().any(|w| w.contains("isn't one the cutter knows")));
}

#[test]
fn a_job_wider_than_the_cutter_is_refused() {
    let mut j = job(vec![], vec![], "custom");
    j.mat.size_mm = [350.0, 304.8];
    j.mat.cut_area_mm.x1 = 350.0;
    let e = Silhouette.encode(&j, &cameo4()).unwrap_err();
    assert!(e.contains("304.8 mm"), "{e}");
    let plus = DeviceInfo { product_id: 0x1138, ..cameo4() };
    assert!(Silhouette.encode(&j, &plus).is_ok(), "the Plus cuts 372 mm");
}

/// Sent to a machine that answers ready: each chunk after a status query.
#[test]
fn sent_to_a_ready_machine() {
    let j = job(
        vec![Operation { kind: OpKind::Cut, tool: tool(0.5, 100.0, 1, 0.0) }],
        vec![path(0, false, &[[1.0, 1.0], [2.0, 2.0]])],
        "silhouette.12x12",
    );
    let e = Silhouette.encode(&j, &cameo4()).unwrap();
    let mut t = MemoryTransport::answering(b"0\x03");
    let progress = Mutex::new(Progress::default());
    send(&mut t, &Silhouette, &e, Patience { polls: 3, poll_ms: 0 }, &AtomicBool::new(false), &progress).unwrap();
    let written = t.written();
    assert!(written.starts_with(&[0x1B, 0x05, 0x1B, 0x04]), "status, then initialise: {:?}", &written[..4]);
    assert!(written.ends_with(b"SO0\x03"));
}
