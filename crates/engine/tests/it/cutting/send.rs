//! C4.5: sending the cut job to a machine, here a recording stand-in for a Cameo 4 that answers
//! ready. One test: the device registry is the process's.

use std::sync::Arc;

use serde_json::{Value, json};
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
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();

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
        if !status(&mut s)["sending"].as_bool().unwrap() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let st = status(&mut s);
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
        if !status(&mut s)["sending"].as_bool().unwrap() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(status(&mut s)["outcome"], json!("load the mat into the cutter first"));
    assert!(s.execute("cutting.send", &json!({"device": "usb-other"})).unwrap_err().to_string().contains("no cutter \"usb-other\""));

    // C5.3: cut by layer, the send pauses before the second layer and goes on when told.
    s.execute("layer.new", &json!({"name": "Gold"})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 100, "y": 100, "width": 50, "height": 50})).unwrap();
    s.execute("cutting.settings.set", &json!({"grouping": "layer"})).unwrap();
    let grouped = MemoryTransport::answering(b"0");
    registry::install(Some(Arc::new(FakeCameo(grouped.clone()))));
    assert_eq!(s.execute("cutting.continue", &json!({})).unwrap()["continued"], json!(false), "nothing waiting yet");
    s.execute("cutting.send", &json!({})).unwrap();
    for _ in 0..500 {
        if !status(&mut s)["waitingFor"].is_null() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let st = status(&mut s);
    assert_eq!((st["waitingFor"].clone(), st["sending"].clone()), (json!("Gold"), json!(true)));
    assert_eq!(s.execute("cutting.continue", &json!({})).unwrap()["continued"], json!(true));
    for _ in 0..500 {
        if !status(&mut s)["sending"].as_bool().unwrap() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    let st = status(&mut s);
    assert_eq!((st["outcome"].clone(), st["waitingFor"].clone()), (json!("done"), Value::Null));
    assert!(grouped.written().ends_with(b"SO0"));

    // C6.2: Print & Cut. A machine that answers the search with anything but "found" stops the
    // job there; the simulated Cameo finds its marks and the job goes on to the end.
    s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 100, "y": 100, "width": 50, "height": 50})).unwrap();
    s.execute("cutting.settings.set", &json!({"printAndCut": true})).unwrap();
    s.execute("cutting.marks.add", &json!({})).unwrap();
    let blind = MemoryTransport::answering(b"0");
    registry::install(Some(Arc::new(FakeCameo(blind.clone()))));
    s.execute("cutting.send", &json!({})).unwrap();
    let st = finished(&mut s);
    assert!(st["outcome"].as_str().unwrap().contains("didn't find the registration marks"), "{st}");
    assert!(String::from_utf8_lossy(&blind.written()).contains("TB123,"));
    assert!(!blind.written().ends_with(b"SO0"), "nothing cut");
    let mut sim = vectorcraft_cutter_silhouette::simulated_cameo4();
    sim.behaviour.busy_per_write = std::time::Duration::ZERO;
    sim.behaviour.busy_per_byte = std::time::Duration::ZERO;
    let sent = sim.sent.clone();
    registry::install(Some(Arc::new(sim)));
    s.execute("cutting.send", &json!({})).unwrap();
    assert_eq!(finished(&mut s)["outcome"], json!("done"));
    assert!(sent.bytes().ends_with(b"SO0"));

    // C8.1: the test cut, on the simulated machine: the two shapes at the corner asked for, the
    // design's own paths left alone; at the corner the design is in, a warning first.
    s.execute("file.new", &json!({"width": 300, "height": 300})).unwrap();
    s.execute("shape.rectangle", &json!({"x": 0, "y": 0, "width": 100, "height": 100})).unwrap();
    let before = sent.bytes().len();
    let r = s.execute("cutting.test", &json!({"corner": "bottomRight"})).unwrap();
    assert!(!r["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("lands on the design")), "{r}");
    assert_eq!(finished(&mut s)["outcome"], json!("done"));
    let cut = String::from_utf8_lossy(&sent.bytes()[before..]).to_string();
    assert_eq!(cut.split('\u{3}').filter(|c| c.starts_with('D')).count(), 3 + 4, "the triangle's and the square's strokes: {cut:?}");
    let r = s.execute("cutting.test", &json!({"corner": "topLeft"})).unwrap();
    assert!(r["warnings"].as_array().unwrap().iter().any(|w| w.as_str().unwrap().contains("lands on the design")), "{r}");
    finished(&mut s);
    assert!(s.execute("cutting.test", &json!({"corner": "middle"})).is_err());
    let j = s.execute("cutting.test.job", &json!({"corner": "topRight"})).unwrap();
    assert_eq!(j["job"]["paths"].as_array().unwrap().len(), 2);
    registry::install(None);
    assert!(s.execute("cutting.test", &json!({})).unwrap_err().to_string().contains("no cutter is connected"));
}
