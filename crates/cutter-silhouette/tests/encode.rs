//! C4.4: cut jobs as Silhouette commands. The golden bytes are written from the protocol notes
//! (`docs/cutting/protocol-silhouette.md`); the round trip reads the moves back and compares them
//! with the simulation of the same job.
// Integration tests: unwrapping and panicking on failure is fine here, unlike in shipped code (AGENTS.md › Robustness).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::Mutex;

use vectorcraft_cutjob::{CutJob, JOB_VERSION, JobMat, OpKind, Operation, RectMm, Simulation, SourceRef, ToolSettings, Toolpath};
use vectorcraft_cutter::send::{Controls, Patience, Progress, send};
use vectorcraft_cutter::{DeviceInfo, Driver, MemoryTransport};
use vectorcraft_cutter_silhouette::{CHUNK_BYTES, Silhouette};

fn tool(force: f64, speed_mm_s: f64, passes: u8, overcut_mm: f64) -> ToolSettings {
    ToolSettings { force, speed_mm_s, passes, overcut_mm, ..ToolSettings::default() }
}

fn job(ops: Vec<Operation>, paths: Vec<Toolpath>, mat: &str) -> CutJob {
    CutJob {
        version: JOB_VERSION,
        mat: JobMat { id: mat.into(), size_mm: [304.8, 304.8], cut_area_mm: RectMm { x0: 0.0, y0: 0.0, x1: 304.8, y1: 304.8 } },
        operations: ops,
        paths,
        warnings: vec![],
        track_enhancing: false,
        registration: None,
        groups: vec![],
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
    for c in
        ["TG1", "FN0", "TB50,0", "J1", "FX17,1", "!10,1", "FC18,1,1", "M400,200", "D400,400", "D600,400", "D600,200", "D400,200", "M600,0", "SO0"]
    {
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
    send(&mut t, &Silhouette, &e, Patience { polls: 3, poll_ms: 0 }, &Controls::default(), &progress).unwrap();
    let written = t.written();
    assert!(written.starts_with(&[0x1B, 0x05, 0x1B, 0x04]), "status, then initialise: {:?}", &written[..4]);
    assert!(written.ends_with(b"SO0\x03"));
}

/// C5.2: blade depth, acceleration and track enhancing as their commands.
#[test]
fn depth_acceleration_and_track_enhancing() {
    let t = ToolSettings { depth: Some(3), accel: Some(2), ..tool(0.5, 100.0, 1, 0.0) };
    let mut j = job(vec![Operation { kind: OpKind::Cut, tool: t }], vec![path(0, false, &[[1.0, 1.0], [2.0, 2.0]])], "silhouette.12x12");
    j.track_enhancing = true;
    let text = String::from_utf8_lossy(&Silhouette.encode(&j, &cameo4()).unwrap().chunks.concat()).to_string();
    assert!(text.contains("TB50,0\u{3}FY1\u{3}J1\u{3}"), "{text:?}");
    assert!(text.contains("FC18,1,1\u{3}TF3,1\u{3}TJ2\u{3}M"), "{text:?}");
    // Left unset: not sent.
    let plain = job(
        vec![Operation { kind: OpKind::Cut, tool: tool(0.5, 100.0, 1, 0.0) }],
        vec![path(0, false, &[[1.0, 1.0], [2.0, 2.0]])],
        "silhouette.12x12",
    );
    let text = String::from_utf8_lossy(&Silhouette.encode(&plain, &cameo4()).unwrap().chunks.concat()).to_string();
    assert!(!text.contains("TF") && !text.contains("TJ") && !text.contains("FY"), "{text:?}");
}

/// The panel shows the machine's steps: each reads back as itself.
#[test]
fn steps_read_back_as_themselves() {
    use vectorcraft_cutter_silhouette::{force_from_step, force_step, speed_from_step, speed_step};
    for step in 1..=33 {
        assert_eq!(force_step(force_from_step(step)), step);
    }
    for step in 1..=30 {
        assert_eq!(speed_step(speed_from_step(step)), step);
    }
    assert_eq!((force_step(-1.0), force_step(2.0), force_step(f64::NAN)), (1, 33, 1));
    assert_eq!((speed_step(0.0), speed_step(1e9), speed_step(f64::NAN)), (1, 30, 1));
}

/// C5.3: a job cut in groups pauses before each group after the first, between chunks, and
/// gives each group the tool settings again.
#[test]
fn groups_pause_between_them() {
    let mut j = job(
        vec![Operation { kind: OpKind::Cut, tool: tool(0.5, 100.0, 1, 0.0) }],
        vec![path(0, false, &[[1.0, 1.0], [2.0, 2.0]]), path(0, false, &[[5.0, 5.0], [6.0, 6.0]])],
        "silhouette.12x12",
    );
    j.groups = vec![
        vectorcraft_cutjob::JobGroup { name: "Red".into(), start: 0, material: None },
        vectorcraft_cutjob::JobGroup { name: "Blue".into(), start: 1, material: None },
    ];
    let e = Silhouette.encode(&j, &cameo4()).unwrap();
    assert_eq!(e.pauses.len(), 1);
    let (at, name) = &e.pauses[0];
    assert_eq!(name, "Blue");
    let after = String::from_utf8_lossy(&e.chunks[*at..].concat()).to_string();
    assert!(after.starts_with("J1\u{3}FX17,1\u{3}"), "the settings again: {after:?}");
    assert!(after.contains("M100,100\u{3}"), "the second path after the pause: {after:?}");
    let before = String::from_utf8_lossy(&e.chunks[..*at].concat()).to_string();
    assert!(before.contains("M20,20\u{3}") && !before.contains("M100,100"), "{before:?}");
}

/// C7.4: each group is sent with its own operation's settings: a second group cut with a heavier
/// setup gets its force after the pause, the first group never sees it.
#[test]
fn each_group_gets_its_own_settings() {
    let mut j = job(
        vec![Operation { kind: OpKind::Cut, tool: tool(0.5, 100.0, 1, 0.0) }, Operation { kind: OpKind::Cut, tool: tool(1.0, 100.0, 2, 0.0) }],
        vec![path(0, false, &[[1.0, 1.0], [2.0, 2.0]]), path(1, false, &[[5.0, 5.0], [6.0, 6.0]])],
        "silhouette.12x12",
    );
    j.groups = vec![
        vectorcraft_cutjob::JobGroup { name: "Paper".into(), start: 0, material: None },
        vectorcraft_cutjob::JobGroup { name: "Card".into(), start: 1, material: Some("Cardstock".into()) },
    ];
    let e = Silhouette.encode(&j, &cameo4()).unwrap();
    let at = e.pauses[0].0;
    let (before, after) =
        (String::from_utf8_lossy(&e.chunks[..at].concat()).to_string(), String::from_utf8_lossy(&e.chunks[at..].concat()).to_string());
    assert!(before.contains("FX17,1\u{3}") && !before.contains("FX33,1\u{3}"), "{before:?}");
    assert!(after.starts_with("J1\u{3}FX33,1\u{3}") && !after.contains("FX17,1\u{3}"), "{after:?}");
    assert_eq!(after.matches("M100,100\u{3}").count(), 2, "two passes: {after:?}");
}

/// C5.4: the blade offset is the machine's to turn the blade by: `FC` in its units (20 a mm),
/// the usual blade's 0.9 mm until set, 0 for a pen; the paths are left where the tip cuts.
#[test]
fn the_blade_offset_goes_to_the_machine() {
    let fc = |offset: Option<f64>| {
        let t = ToolSettings { blade_offset_mm: offset, ..tool(0.5, 100.0, 1, 0.0) };
        let j = job(vec![Operation { kind: OpKind::Cut, tool: t }], vec![path(0, false, &[[1.0, 1.0], [2.0, 2.0]])], "silhouette.12x12");
        let text = String::from_utf8_lossy(&Silhouette.encode(&j, &cameo4()).unwrap().chunks.concat()).to_string();
        let fc = text.split('\u{3}').find(|c| c.starts_with("FC")).unwrap_or_default().to_string();
        assert!(text.contains("M20,20\u{3}D40,40\u{3}"), "the path as it is: {text:?}");
        fc
    };
    assert_eq!(fc(None), "FC18,1,1");
    assert_eq!(fc(Some(0.25)), "FC5,1,1");
    assert_eq!(fc(Some(0.0)), "FC0,1,1");
}

/// C4.8: the simulated Cameo 4 is one this driver drives, and answers as the machine does: ready,
/// then moving after part of a job.
#[test]
fn the_simulated_cameo_answers_like_one() {
    use vectorcraft_cutter::{DeviceProvider, DeviceStatus, Driver};
    let m = vectorcraft_cutter_silhouette::simulated_cameo4();
    let d = m.devices().remove(0);
    assert!(d.id.starts_with(vectorcraft_cutter::SIMULATED_ID_PREFIX));
    assert_eq!(Silhouette.model(&d), "Cameo 4");
    let mut t = m.open(&d.id).unwrap();
    let mut ask = || {
        t.write(Silhouette.status_query()).unwrap();
        let mut buf = [0u8; 8];
        let n = t.read(&mut buf, 0).unwrap();
        Silhouette.parse_status(&buf[..n])
    };
    assert_eq!(ask(), DeviceStatus::Ready);
    drop(ask);
    t.write(b"M20,20\x03").unwrap();
    t.write(Silhouette.status_query()).unwrap();
    let mut buf = [0u8; 8];
    let n = t.read(&mut buf, 0).unwrap();
    assert_eq!(Silhouette.parse_status(&buf[..n]), DeviceStatus::Moving);
    assert_eq!(m.sent.bytes(), b"M20,20\x03");
}

/// C6.2: Print & Cut: the portrait marks and the search for them, in machine units (the length,
/// the width, then where to start looking, 10 mm before the top-left mark), a chunk ending there
/// with the answer that means found; the paths after it measured from the top-left mark.
#[test]
fn print_and_cut_searches_for_the_marks_first() {
    let mut j = job(
        vec![Operation { kind: OpKind::Cut, tool: tool(0.5, 100.0, 1, 0.0) }],
        vec![path(0, false, &[[30.0, 40.0], [50.0, 40.0]])],
        "silhouette.12x12",
    );
    j.registration = Some(vectorcraft_cutjob::JobRegistration { origin_mm: [20.0, 25.0], size_mm: [180.0, 230.0] });
    let e = Silhouette.encode(&j, &cameo4()).unwrap();
    let text: Vec<String> = e.chunks.iter().map(|c| String::from_utf8_lossy(c).to_string()).collect();
    let first = &text[0];
    assert!(first.ends_with("TB99\u{3}TB52,2\u{3}TB51,400\u{3}TB53,10\u{3}TB55,1\u{3}TB123,4600,3600,300,200\u{3}"), "{first:?}");
    assert_eq!(e.checks.len(), 1);
    let check = &e.checks[0];
    assert_eq!((check.after_chunk, check.expect.as_slice(), check.timeout_ms), (0, b"    0\x03".as_slice(), 40_000));
    let rest = text[1..].concat();
    assert!(rest.contains("M300,200\u{3}D300,600\u{3}"), "from the mark: (30, 40) − (20, 25) mm: {rest:?}");
    assert!(e.warnings.iter().any(|w| w.contains("Print & Cut")));
    // Without it, none of that.
    j.registration = None;
    let plain = Silhouette.encode(&j, &cameo4()).unwrap();
    assert!(plain.checks.is_empty() && !String::from_utf8_lossy(&plain.chunks.concat()).contains("TB123"));
}

/// C10.2: the tool decides the holder: a holder 2 tool's settings name holder 2 and take no
/// depth (said once); a pen no blade offset; an unknown tool is cut as the AutoBlade, said; the
/// Cameo 3 refuses holder 2's specialty tools.
#[test]
fn the_tool_decides_the_holder() {
    let encode = |tool_id: Option<&str>, device: &DeviceInfo| {
        let t = ToolSettings { tool: tool_id.map(str::to_string), depth: Some(3), ..tool(0.5, 100.0, 1, 0.0) };
        let j = job(vec![Operation { kind: OpKind::Cut, tool: t }], vec![path(0, false, &[[1.0, 1.0], [2.0, 2.0]])], "silhouette.12x12");
        Silhouette.encode(&j, device).map(|e| (String::from_utf8_lossy(&e.chunks.concat()).to_string(), e.warnings))
    };
    let (text, warnings) = encode(Some("rotary"), &cameo4()).unwrap();
    assert!(text.contains("J2\u{3}FX17,2\u{3}!10,2\u{3}FC18,1,2\u{3}M"), "{text:?}");
    assert!(!text.contains("TF") && warnings.iter().any(|w| w.contains("blade depth is the AutoBlade's")), "{text:?} {warnings:?}");
    let (text, _) = encode(None, &cameo4()).unwrap();
    assert!(text.contains("J1\u{3}FX17,1\u{3}") && text.contains("TF3,1\u{3}"), "the AutoBlade: {text:?}");
    let (text, _) = encode(Some("pen"), &cameo4()).unwrap();
    assert!(text.contains("FC0,1,1\u{3}") && !text.contains("TF"), "{text:?}");
    let (text, warnings) = encode(Some("laser"), &cameo4()).unwrap();
    assert!(text.contains("J1\u{3}") && warnings.iter().any(|w| w.contains("\"laser\" isn't one this version knows")), "{warnings:?}");
    let cameo3 = DeviceInfo { product_id: 0x112F, ..cameo4() };
    assert!(encode(Some("kraft3"), &cameo3).unwrap_err().contains("Cameo 4 and later"));
    assert!(encode(Some("kraft2"), &cameo3).is_ok(), "holder 1's adapter tools are any model's");
}

/// C10.3: the answer to the tools query, read: a code per holder, the Kraft blade's by holder;
/// any bytes at all read without a panic, at most four holders.
#[test]
fn the_tools_answer_is_read() {
    use vectorcraft_cutter::tools::Fitted;
    let tool = |id: &str| Fitted::Tool(id.into());
    assert_eq!(Silhouette.parse_tools(b"2,0\x03"), vec![tool("autoblade"), Fitted::Empty]);
    assert_eq!(Silhouette.parse_tools(b"4,4\x03"), vec![tool("kraft2"), tool("kraft3")]);
    assert_eq!(Silhouette.parse_tools(b" 7 , 5 \x03"), vec![tool("pen"), tool("rotary")]);
    assert_eq!(Silhouette.parse_tools(b"6,255\x03"), vec![Fitted::Unknown("6".into()), Fitted::Error]);
    assert_eq!(Silhouette.parse_tools(b"\x03"), vec![]);
    assert!(Silhouette.tools_answer_done(b"2,0\x03") && !Silhouette.tools_answer_done(b"2,0"));
    proptest::proptest!(|(bytes in proptest::collection::vec(proptest::prelude::any::<u8>(), 0..200))| {
        proptest::prop_assert!(Silhouette.parse_tools(&bytes).len() <= 4);
    });
}
