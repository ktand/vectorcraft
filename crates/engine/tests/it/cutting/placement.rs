//! C2.4: `cutting.settings.get` says where the artboard lies on the mat and whether it fits.

use serde_json::{Value, json};
use vectorcraft_engine::Session;

const MM: f64 = 25.4 / 72.0;

fn placement(s: &mut Session) -> Value {
    s.execute("cutting.settings.get", &json!({})).unwrap()["placement"].clone()
}

fn assert_rect(v: &Value, want: [f64; 4]) {
    let got: Vec<f64> = v.as_array().unwrap().iter().map(|n| n.as_f64().unwrap()).collect();
    assert!(got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-9), "{got:?}, want {want:?}");
}

#[test]
fn the_artboard_lies_on_the_mat_as_the_settings_say() {
    let mut s = Session::new();
    s.execute("file.new", &json!({"width": 400, "height": 300})).unwrap();
    let (w, h) = (400.0 * MM, 300.0 * MM);
    let p = placement(&mut s);
    assert_rect(&p["artboardMm"], [0.0, 0.0, w, h]);
    assert_eq!(p["fits"], json!(true));

    s.execute("cutting.settings.set", &json!({"orientation": "landscape", "offsetMm": [10, 20]})).unwrap();
    let p = placement(&mut s);
    assert_rect(&p["artboardMm"], [10.0, 20.0, 10.0 + h, 20.0 + w]);
    assert_eq!(p["fits"], json!(true));

    // 141 mm wide from 200 mm passes the 304.8 mm mat.
    let r = s.execute("cutting.settings.set", &json!({"orientation": "portrait", "offsetMm": [200, 0]})).unwrap();
    assert_eq!(r["placement"]["fits"], json!(false));
    s.execute("cutting.settings.set", &json!({"mat": "silhouette.12x24", "offsetMm": [0, 400]})).unwrap();
    assert_eq!(placement(&mut s)["fits"], json!(true), "the long mat holds it lower down");
}
