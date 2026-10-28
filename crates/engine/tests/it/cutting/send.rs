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
    registry::install(None);
}
