//! C4.5: sending the cut job to a machine, here a recording stand-in for a Cameo 4 that answers
//! ready. One test: the device registry is the process's.

use std::path::Path;
use std::sync::Arc;

use serde_json::{Value, json};
use vectorcraft_cutter::simulated::Sent;
use vectorcraft_cutter::{DeviceInfo, DeviceProvider, MemoryTransport, Transport, TransportError, registry};
use vectorcraft_engine::Session;

struct FakeCameo(MemoryTransport);

impl DeviceProvider for FakeCameo {
    fn devices(&self) -> Vec<DeviceInfo> {
        vec![DeviceInfo { id: "usb-cameo".into(), vendor_id: 0x0B4D, product_id: 0x1137, name: "Silhouette CAMEO 4".into() }]
    }
    fn open(&self, id: &str) -> Result<Box<dyn Transport>, TransportError> {
        if id == "usb-cameo" { Ok(Box::new(self.0.clone())) } else { Err(TransportError::Gone) }
    }
}

fn status(s: &mut Session) -> Value {
    s.execute("cutting.status", &json!({})).unwrap()
}

/// The status once the send has stopped.
fn finished(s: &mut Session) -> Value {
    for _ in 0..1000 {
        if !status(s)["sending"].as_bool().unwrap() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    status(s)
}

#[test]
fn the_job_goes_to_the_cutter_in_the_background() {
    // C8.4: every send below is kept in a history file of the test's own.
    let dir = std::env::temp_dir().join(format!("vectorcraft-history-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    vectorcraft_engine::cmd::cutting::history::attach(dir.join("History.json"));
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();
    // In order: each step goes on from the document and machine the one before left.
    sends_and_says_why(&mut s);
    pauses_between_groups(&mut s);
    let sent = print_and_cut(&mut s);
    test_cut(&mut s, &sent);
    let entries = history(&mut s, &dir);
    send_again(&mut s, &entries);
    tools_fitted(&mut s);
    damaged_history(&mut s, &dir);
    let _ = std::fs::remove_dir_all(&dir);
}

/// No machine, no art, no mat: each said plainly; a send to the end.
fn sends_and_says_why(s: &mut Session) {
    // No machine: nothing to send to, said plainly.
    registry::install(None);
    assert_eq!(s.execute("cutting.devices", &json!({})).unwrap()["devices"], json!([]));
    let e = s.execute("cutting.send", &json!({})).unwrap_err().to_string();
    assert!(e.contains("no cutter is connected"), "{e}");

    let cameo = MemoryTransport::answering(b"0\x03");
    registry::install(Some(Arc::new(FakeCameo(cameo.clone()))));
    let d = s.execute("cutting.devices", &json!({})).unwrap();
    assert_eq!(d["devices"], json!([{"id": "usb-cameo", "model": "Cameo 4", "name": "Silhouette CAMEO 4", "simulated": false}]));

    // Nothing drawn: nothing to cut.
    assert!(s.execute("cutting.send", &json!({})).unwrap_err().to_string().contains("nothing to cut"));

    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
    let r = s.execute("cutting.send", &json!({})).unwrap();
    assert_eq!((r["device"].clone(), r["model"].clone()), (json!("usb-cameo"), json!("Cameo 4")));
    assert!(r["chunks"].as_u64().unwrap() >= 1);
    for _ in 0..500 {
        if !status(s)["sending"].as_bool().unwrap() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let st = status(s);
    assert_eq!((st["outcome"].clone(), st["device"].clone()), (json!("done"), json!("Silhouette CAMEO 4")));
    assert_eq!(st["chunksSent"], st["chunks"]);
    let written = cameo.written();
    assert!(written.starts_with(&[0x1B, 0x05, 0x1B, 0x04]), "a status query, then initialise");
    assert!(written.ends_with(b"SO0\x03"));
    assert_eq!(s.execute("cutting.cancel", &json!({})).unwrap()["cancelled"], json!(false), "nothing running");

    // A machine that has no mat loaded: the send stops and says why.
    let unloaded = MemoryTransport::answering(b"2\x03");
    registry::install(Some(Arc::new(FakeCameo(unloaded))));
    s.execute("cutting.send", &json!({"device": "usb-cameo"})).unwrap();
    for _ in 0..500 {
        if !status(s)["sending"].as_bool().unwrap() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(status(s)["outcome"], json!("load the mat into the cutter first"));
    assert!(s.execute("cutting.send", &json!({"device": "usb-other"})).unwrap_err().to_string().contains("no cutter \"usb-other\""));
}

/// C5.3: cut by layer, the send pauses before the second layer and goes on when told.
fn pauses_between_groups(s: &mut Session) {
    s.execute("layer.new", &json!({"name": "Gold"})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 100, "y": 100, "width": 50, "height": 50})).unwrap();
    s.execute("cutting.settings.set", &json!({"grouping": "layer"})).unwrap();
    let grouped = MemoryTransport::answering(b"0");
    registry::install(Some(Arc::new(FakeCameo(grouped.clone()))));
    assert_eq!(s.execute("cutting.continue", &json!({})).unwrap()["continued"], json!(false), "nothing waiting yet");
    s.execute("cutting.send", &json!({})).unwrap();
    for _ in 0..500 {
        if !status(s)["waitingFor"].is_null() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let st = status(s);
    assert_eq!((st["waitingFor"].clone(), st["sending"].clone()), (json!("Gold"), json!(true)));
    assert_eq!(s.execute("cutting.continue", &json!({})).unwrap()["continued"], json!(true));
    for _ in 0..500 {
        if !status(s)["sending"].as_bool().unwrap() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let st = status(s);
    assert_eq!((st["outcome"].clone(), st["waitingFor"].clone()), (json!("done"), Value::Null));
    assert!(grouped.written().ends_with(b"SO0"));
}

/// C6.2: Print & Cut. A machine that answers the search with anything but "found" stops the
/// job there; the simulated Cameo finds its marks and the job goes on to the end. What it was
/// sent, for the test cut after.
fn print_and_cut(s: &mut Session) -> Sent {
    s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 100, "y": 100, "width": 50, "height": 50})).unwrap();
    s.execute("cutting.settings.set", &json!({"printAndCut": true})).unwrap();
    s.execute("cutting.marks.add", &json!({})).unwrap();
    let blind = MemoryTransport::answering(b"0");
    registry::install(Some(Arc::new(FakeCameo(blind.clone()))));
    s.execute("cutting.send", &json!({})).unwrap();
    let st = finished(s);
    assert!(st["outcome"].as_str().unwrap().contains("didn't find the registration marks"), "{st}");
    assert!(String::from_utf8_lossy(&blind.written()).contains("TB123,"));
    assert!(!blind.written().ends_with(b"SO0"), "nothing cut");
    let mut sim = vectorcraft_cutter_silhouette::simulated_cameo4();
    sim.behaviour.busy_per_write = std::time::Duration::ZERO;
    sim.behaviour.busy_per_byte = std::time::Duration::ZERO;
    let sent = sim.sent.clone();
    registry::install(Some(Arc::new(sim)));
    s.execute("cutting.send", &json!({})).unwrap();
    assert_eq!(finished(s)["outcome"], json!("done"));
    assert!(sent.bytes().ends_with(b"SO0"));
    sent
}

/// C8.1: the test cut, on the simulated machine: the two shapes at the corner asked for, the
/// design's own paths left alone; at the corner the design is in, a warning first.
fn test_cut(s: &mut Session, sent: &Sent) {
    s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
    let before = sent.bytes().len();
    let r = s.execute("cutting.test", &json!({"corner": "bottomRight"})).unwrap();
    assert!(!r["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("lands on the design")), "{r}");
    assert_eq!(finished(s)["outcome"], json!("done"));
    let cut = String::from_utf8_lossy(&sent.bytes()[before..]).to_string();
    assert_eq!(cut.split('\u{3}').filter(|c| c.starts_with('D')).count(), 3 + 4, "the triangle's and the square's strokes: {cut:?}");
    let r = s.execute("cutting.test", &json!({"corner": "topLeft"})).unwrap();
    assert!(r["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("lands on the design")), "{r}");
    finished(s);
    assert!(s.execute("cutting.test", &json!({"corner": "middle"})).is_err());
    let j = s.execute("cutting.test.job", &json!({"corner": "topRight"})).unwrap();
    assert_eq!(j["job"]["paths"].as_array().unwrap().len(), 2);
    registry::install(None);
    assert!(s.execute("cutting.test", &json!({})).unwrap_err().to_string().contains("no cutter is connected"));
}

/// C8.4: the history, newest first: the test cuts, the Print & Cut that didn't find its marks
/// with a report to share, the sends before; kept in the file, and read back from it; an entry's
/// settings used, one undo step. The entries, newest first.
fn history(s: &mut Session, dir: &Path) -> Vec<Value> {
    let h = s.execute("cutting.history", &json!({})).unwrap();
    let entries = h["entries"].as_array().unwrap().clone();
    assert!(entries.len() >= 5, "{h}");
    let newest = &entries[0];
    assert_eq!((newest["test"].clone(), newest["outcome"].clone(), newest["machine"].clone()), (json!(true), json!("done"), json!("Cameo 4")));
    assert!(newest["shownTime"].as_str().unwrap().ends_with(" UTC") && newest["seconds"].is_number(), "{newest}");
    let failed = entries.iter().find(|e| e["outcome"].as_str().unwrap().contains("didn't find the registration marks")).unwrap();
    let report = std::fs::read_to_string(failed["report"].as_str().unwrap()).unwrap();
    assert!(
        report.contains("cutting report") && report.contains("didn't find the registration marks") && report.contains("\"printAndCut\": true"),
        "{report}"
    );
    assert!(entries.iter().all(|e| e["outcome"] != "sending"), "every send has finished");
    let on_disk: serde_json::Value = serde_json::from_slice(&std::fs::read(dir.join("History.json")).unwrap()).unwrap();
    assert_eq!(on_disk.as_array().unwrap().len(), entries.len());
    vectorcraft_engine::cmd::cutting::history::attach(dir.join("History.json"));
    assert_eq!(s.execute("cutting.history", &json!({})).unwrap()["entries"].as_array().unwrap().len(), entries.len(), "read back");

    // Use the settings an entry was cut with: one undo step.
    let index = failed["index"].clone();
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {"passes": 5}}})).unwrap();
    let r = s.execute("cutting.history.apply", &json!({"index": index})).unwrap();
    assert_eq!(r["settings"]["operations"]["cut"]["passes"], json!(1));
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.execute("cutting.settings.get", &json!({})).unwrap()["settings"]["operations"]["cut"]["passes"], json!(5));
    assert!(s.execute("cutting.history.apply", &json!({"index": 999})).is_err());
    entries
}

/// C8.8: send an entry again. Only for the document it was cut from, and with a machine.
fn send_again(s: &mut Session, entries: &[Value]) {
    let newest = &entries[0];
    let title = newest["document"].clone();
    let other = entries.iter().find(|e| e["document"] != title).unwrap();
    let e = s.execute("cutting.history.send", &json!({"index": other["index"]})).unwrap_err().to_string();
    assert!(e.contains("was cut from"), "{e}");
    let e = s.execute("cutting.history.send", &json!({"index": newest["index"]})).unwrap_err().to_string();
    assert!(e.contains("no cutter is connected"), "{e}");
    registry::install(Some(Arc::new({
        let mut sim = vectorcraft_cutter_silhouette::simulated_cameo4();
        sim.behaviour.busy_per_write = std::time::Duration::ZERO;
        sim.behaviour.busy_per_byte = std::time::Duration::ZERO;
        sim
    })));
    // A test cut again: at the same corner, a test cut in the history again.
    let r = s.execute("cutting.history.send", &json!({"index": newest["index"]})).unwrap();
    assert!(r["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("lands on the design")), "top left again: {r}");
    assert_eq!(finished(s)["outcome"], json!("done"));
    let again = s.execute("cutting.history", &json!({})).unwrap()["entries"][0].clone();
    assert_eq!((again["test"].clone(), again["corner"].clone()), (json!(true), json!("topLeft")));
    // A cut again: with the passes it was cut with, one undo step.
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {"passes": 3}}})).unwrap();
    s.execute("cutting.send", &json!({})).unwrap();
    finished(s);
    let cut = s.execute("cutting.history", &json!({})).unwrap()["entries"][0].clone();
    assert_eq!(cut["test"], json!(false));
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {"passes": 1}}})).unwrap();
    let r = s.execute("cutting.history.send", &json!({"index": cut["index"]})).unwrap();
    assert_eq!(r["applied"], json!(true));
    assert_eq!(finished(s)["outcome"], json!("done"));
    assert_eq!(s.execute("cutting.settings.get", &json!({})).unwrap()["settings"]["operations"]["cut"]["passes"], json!(3));
    s.execute("edit.undo", &json!({})).unwrap();
    assert_eq!(s.execute("cutting.settings.get", &json!({})).unwrap()["settings"]["operations"]["cut"]["passes"], json!(1));
    // Nothing to cut: refused, the settings left as they are.
    s.execute("select.all", &json!({})).unwrap();
    s.execute("edit.clear", &json!({})).unwrap();
    assert!(s.execute("cutting.history.send", &json!({"index": cut["index"]})).unwrap_err().to_string().contains("nothing to cut"));
    assert_eq!(s.execute("cutting.settings.get", &json!({})).unwrap()["settings"]["operations"]["cut"]["passes"], json!(1));
}

/// C10.3: which tools are fitted. The simulated Cameo 4 has the AutoBlade in holder 1 and
/// nothing in holder 2; a job that needs the rotary blade is warned about. A machine answering
/// with codes this version doesn't know, or nothing, is read as far as it goes.
fn tools_fitted(s: &mut Session) {
    s.execute("shape.rectangle", &json!({"x": 10, "y": 10, "width": 50, "height": 50})).unwrap();
    let t = s.execute("cutting.tools", &json!({})).unwrap();
    assert_eq!((t["canTell"].clone(), t["answered"].clone()), (json!(true), json!(true)), "{t}");
    assert_eq!(t["holders"][0], json!({"holder": 1, "state": "tool", "tool": "autoblade", "label": "AutoBlade", "code": null}));
    assert_eq!(t["holders"][1]["state"], json!("empty"));
    assert_eq!(t["warnings"], json!([]), "the AutoBlade is what the job needs");
    s.execute("cutting.settings.set", &json!({"operations": {"cut": {"tool": "rotary"}}})).unwrap();
    let t = s.execute("cutting.tools", &json!({})).unwrap();
    assert_eq!(t["warnings"], json!(["The cut uses the Rotary blade in holder 2, but holder 2 is empty"]), "{t}");
    for (answer, states) in
        [(&b"4,4\x03"[..], vec!["tool", "tool"]), (b"6,255\x03", vec!["unknown", "error"]), (b"\x03", vec![]), (b"junk,,", vec!["unknown"; 4])]
    {
        registry::install(Some(Arc::new(FakeCameo(MemoryTransport::answering(answer)))));
        let t = s.execute("cutting.tools", &json!({})).unwrap();
        let got: Vec<&str> = t["holders"].as_array().unwrap().iter().map(|h| h["state"].as_str().unwrap()).collect();
        assert_eq!(got, states, "{t}");
    }
    registry::install(Some(Arc::new(FakeCameo(MemoryTransport::answering(b"4,4\x03")))));
    let t = s.execute("cutting.tools", &json!({})).unwrap();
    assert_eq!((t["holders"][0]["tool"].clone(), t["holders"][1]["tool"].clone()), (json!("kraft2"), json!("kraft3")), "by holder");
    registry::install(None);
    assert!(s.execute("cutting.tools", &json!({})).unwrap_err().to_string().contains("no cutter is connected"));
}

/// A damaged history file gives what it can (C8.4); C8.7: a file of anything reads without a
/// panic, at most the cap, and lists, and the entries' settings can be used whatever they say.
fn damaged_history(s: &mut Session, dir: &Path) {
    std::fs::write(dir.join("History.json"), b"[{\"outcome\": \"done\"}, 7, \"junk\"]").unwrap();
    vectorcraft_engine::cmd::cutting::history::attach(dir.join("History.json"));
    assert_eq!(s.execute("cutting.history", &json!({})).unwrap()["entries"].as_array().unwrap().len(), 1);
    assert_eq!(s.execute("cutting.history.clear", &json!({})).unwrap()["cleared"], json!(1));

    // C8.7: a history file of anything reads without a panic, at most the cap, and lists; the
    // entries' settings can be used whatever they say.
    let file = dir.join("History.json");
    use proptest::strategy::Strategy;
    let mut runner = proptest::test_runner::TestRunner::deterministic();
    let junk = proptest::prop_oneof![
        proptest::collection::vec(proptest::prelude::any::<u8>(), 0..300),
        proptest::collection::vec(
            proptest::prop_oneof![
                proptest::strategy::Just(json!({"settings": {"operations": {"cut": {"force": "heavy", "passes": -3}}}})),
                proptest::strategy::Just(json!({"settings": 7, "time": "now", "outcome": [1, 2]})),
                proptest::strategy::Just(json!([{"nested": "array"}])),
                proptest::strategy::Just(json!(null)),
                (-1e9f64..1e9).prop_map(|t| json!({"time": t, "shownTime": t, "report": t})),
            ],
            0..120,
        )
        .prop_map(|v| serde_json::to_vec(&v).unwrap()),
    ];
    for _ in 0..200 {
        let bytes = proptest::strategy::ValueTree::current(&junk.new_tree(&mut runner).unwrap());
        std::fs::write(&file, &bytes).unwrap();
        vectorcraft_engine::cmd::cutting::history::attach(file.clone());
        let h = s.execute("cutting.history", &json!({})).unwrap();
        let entries = h["entries"].as_array().unwrap().clone();
        assert!(entries.len() <= vectorcraft_engine::cmd::cutting::history::MAX_ENTRIES);
        for e in &entries {
            let _ = s.execute("cutting.history.apply", &json!({"index": e["index"]}));
        }
    }
}
