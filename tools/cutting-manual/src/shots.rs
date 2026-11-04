//! Screenshots for the manual: each recipe in `shots.toml` played on a fresh copy of the app over
//! its control channel (`crates/ui-egui/src/control.rs`), then the window captured and cropped.
//!
//! Each shot starts the app anew, with a preferences folder of its own under the output folder
//! (`VECTORCRAFT_CONFIG_DIR`), so a run never reads or writes the person's own preferences,
//! materials or cutting history, and one shot's state never leaks into the next.

use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use serde::Deserialize;
use serde_json::{Value, json};

/// `shots.toml`.
#[derive(Deserialize)]
pub struct Recipes {
    /// The window's size, in points, for every shot.
    #[serde(default = "default_window")]
    pub window: [f64; 2],
    /// Steps every shot starts with.
    #[serde(default)]
    pub setup: Vec<Step>,
    #[serde(default)]
    pub shot: Vec<Shot>,
}

fn default_window() -> [f64; 2] {
    [1280.0, 800.0]
}

#[derive(Deserialize)]
pub struct Shot {
    pub name: String,
    #[serde(default)]
    pub caption: String,
    /// The width in the PDF, as Typst takes it ("60%", "8cm"). Default 100%.
    #[serde(default)]
    pub width: Option<String>,
    /// What part of the window: "window" (default), "canvas", or [x, y, width, height] in points.
    #[serde(default)]
    pub crop: Option<Crop>,
    /// Leave out the setup steps.
    #[serde(default)]
    pub no_setup: bool,
    #[serde(default)]
    pub steps: Vec<Step>,
}

#[derive(Deserialize, Clone)]
#[serde(untagged)]
pub enum Crop {
    Named(String),
    Rect([f64; 4]),
}

/// One step: a command (`run`, with `params`), a raw control request (`method`, with `params`),
/// or a pause (`wait`, ms).
#[derive(Deserialize, Clone)]
pub struct Step {
    #[serde(default)]
    pub run: Option<String>,
    #[serde(default)]
    pub method: Option<String>,
    #[serde(default)]
    pub params: Option<toml::Value>,
    #[serde(default)]
    pub wait: Option<u64>,
    /// A failure is noted and the shot goes on.
    #[serde(default)]
    pub may_fail: bool,
}

/// How long the app gets to start and open its control port.
const START_TIMEOUT: Duration = Duration::from_secs(90);
/// How long one request may take (a screenshot waits for a frame).
const REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
/// The pause before the capture, for the last frames to settle.
const SETTLE: Duration = Duration::from_millis(900);

struct App {
    child: Child,
    reader: BufReader<TcpStream>,
    writer: TcpStream,
    next_id: u64,
}

impl Drop for App {
    fn drop(&mut self) {
        // Our own copy of the app, with throwaway preferences: nothing to save.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn free_port() -> Result<u16, String> {
    let l = TcpListener::bind("127.0.0.1:0").map_err(|e| format!("no free port: {e}"))?;
    l.local_addr().map(|a| a.port()).map_err(|e| e.to_string())
}

impl App {
    fn start(exe: &Path, profile: &Path) -> Result<App, String> {
        let _ = std::fs::remove_dir_all(profile);
        std::fs::create_dir_all(profile).map_err(|e| format!("{}: {e}", profile.display()))?;
        let port = free_port()?;
        let child = Command::new(exe)
            .args(["--control", &port.to_string()])
            .env("VECTORCRAFT_CONFIG_DIR", profile)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("couldn't start {}: {e}", exe.display()))?;
        let started = Instant::now();
        let stream = loop {
            match TcpStream::connect(("127.0.0.1", port)) {
                Ok(s) => break s,
                Err(_) if started.elapsed() < START_TIMEOUT => std::thread::sleep(Duration::from_millis(250)),
                Err(e) => return Err(format!("the app didn't open its control port {port}: {e}")),
            }
        };
        stream.set_read_timeout(Some(REQUEST_TIMEOUT)).map_err(|e| e.to_string())?;
        let writer = stream.try_clone().map_err(|e| e.to_string())?;
        let mut app = App { child, reader: BufReader::new(stream), writer, next_id: 1 };
        // The first frame: the window up and drawing.
        let first = Instant::now();
        while app.request("ui.inspect", json!({})).is_err() {
            if first.elapsed() > START_TIMEOUT {
                return Err("the app never answered".into());
            }
            std::thread::sleep(Duration::from_millis(250));
        }
        Ok(app)
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        let id = self.next_id;
        self.next_id += 1;
        let line = json!({"id": id, "method": method, "params": params}).to_string();
        writeln!(self.writer, "{line}").map_err(|e| format!("{method}: {e}"))?;
        loop {
            let mut reply = String::new();
            let n = self.reader.read_line(&mut reply).map_err(|e| format!("{method}: {e}"))?;
            if n == 0 {
                return Err(format!("{method}: the app closed the connection"));
            }
            let v: Value = serde_json::from_str(&reply).map_err(|e| format!("{method}: {e}: {reply}"))?;
            // Replies carry the request's id; anything else (a notification) is skipped.
            if v.get("id").and_then(Value::as_u64).is_some_and(|i| i != id) {
                continue;
            }
            return if v.get("ok").and_then(Value::as_bool) == Some(true) {
                Ok(v.get("result").cloned().unwrap_or(Value::Null))
            } else {
                Err(format!("{method}: {}", v.get("error").and_then(Value::as_str).unwrap_or("failed")))
            };
        }
    }

    fn step(&mut self, s: &Step) -> Result<(), String> {
        if let Some(ms) = s.wait {
            std::thread::sleep(Duration::from_millis(ms));
        }
        let params = s.params.clone().map(to_json).unwrap_or(json!({}));
        let r = match (&s.run, &s.method) {
            (Some(cmd), _) => self.request("engine.execute", json!({"command": cmd, "params": params})),
            (None, Some(m)) => self.request(m, params),
            (None, None) => Ok(Value::Null),
        };
        match r {
            Err(e) if s.may_fail => {
                eprintln!("    (went on after: {e})");
                Ok(())
            }
            r => r.map(|_| ()),
        }
    }
}

fn to_json(v: toml::Value) -> Value {
    serde_json::to_value(v).unwrap_or(Value::Null)
}

/// Plays every recipe in `names` (all when empty) and writes `<out>/<name>.png`. Returns the
/// shots that failed, with why; the others are written.
pub fn take(exe: &Path, recipes: &Recipes, names: &[String], out: &Path) -> Vec<(String, String)> {
    let mut failed = Vec::new();
    // A copy is run, so a build can replace the original while the shots are taken (Windows
    // locks a running program's file).
    let copy = out.join("app").join(exe.file_name().unwrap_or_default());
    let exe = match std::fs::create_dir_all(out.join("app")).and_then(|()| std::fs::copy(exe, &copy)) {
        Ok(_) => copy.as_path(),
        Err(e) => {
            eprintln!("  (running {} itself: couldn't copy it: {e})", exe.display());
            exe
        }
    };
    for shot in recipes.shot.iter().filter(|s| names.is_empty() || names.contains(&s.name)) {
        eprintln!("  shot {}", shot.name);
        if let Err(e) = take_one(exe, recipes, shot, out) {
            eprintln!("    failed: {e}");
            failed.push((shot.name.clone(), e));
        }
    }
    failed
}

fn take_one(exe: &Path, recipes: &Recipes, shot: &Shot, out: &Path) -> Result<(), String> {
    let profile = out.join("profile");
    let mut app = App::start(exe, &profile)?;
    let [w, h] = recipes.window;
    app.request("ui.resize", json!({"width": w, "height": h}))?;
    std::thread::sleep(Duration::from_millis(600));
    let setup: &[Step] = if shot.no_setup { &[] } else { &recipes.setup };
    for s in setup.iter().chain(&shot.steps) {
        app.step(s)?;
    }
    std::thread::sleep(SETTLE);
    let inspect = app.request("ui.inspect", json!({}))?;
    let raw = out.join(format!("{}-full.png", shot.name));
    let raw_abs = std::path::absolute(&raw).map_err(|e| e.to_string())?;
    app.request("ui.screenshot", json!({"path": raw_abs.to_string_lossy()}))?;
    drop(app);
    let window_w = inspect["window"][0].as_f64().unwrap_or(w);
    let rect = match shot.crop.clone().unwrap_or(Crop::Named("window".into())) {
        Crop::Rect(r) => Some(r),
        Crop::Named(n) if n == "canvas" => {
            let c = &inspect["canvasRect"];
            Some([0, 1, 2, 3].map(|i| c[i].as_f64().unwrap_or(0.0)))
        }
        Crop::Named(n) if n == "window" => None,
        Crop::Named(n) => return Err(format!("unknown crop {n:?}: window, canvas or [x, y, w, h]")),
    };
    crop(&raw, &out.join(format!("{}.png", shot.name)), rect, window_w)?;
    let _ = std::fs::remove_file(&raw);
    Ok(())
}

/// `from` cropped to `rect` (window points; `None`: all of it) and written to `to`.
fn crop(from: &Path, to: &Path, rect: Option<[f64; 4]>, window_w: f64) -> Result<(), String> {
    let file = std::fs::File::open(from).map_err(|e| format!("{}: {e}", from.display()))?;
    let mut decoder = png::Decoder::new(BufReader::new(file));
    decoder.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("the screenshot is too big")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let channels = info.color_type.samples();
    let (iw, ih) = (info.width as usize, info.height as usize);
    let ppp = if window_w > 0.0 { iw as f64 / window_w } else { 1.0 };
    let [x, y, w, h] = rect.unwrap_or([0.0, 0.0, iw as f64 / ppp, ih as f64 / ppp]);
    let px = |v: f64, max: usize| ((v * ppp).round().max(0.0) as usize).min(max);
    let (x0, y0) = (px(x, iw), px(y, ih));
    let (x1, y1) = (px(x + w, iw), px(y + h, ih));
    if x1 <= x0 || y1 <= y0 {
        return Err(format!("the crop {rect:?} is outside the {iw} × {ih} screenshot"));
    }
    let stride = iw * channels;
    let mut out = Vec::with_capacity((x1 - x0) * (y1 - y0) * channels);
    for row in y0..y1 {
        let start = row * stride + x0 * channels;
        out.extend_from_slice(buf.get(start..start + (x1 - x0) * channels).ok_or("short screenshot")?);
    }
    let file = std::fs::File::create(to).map_err(|e| format!("{}: {e}", to.display()))?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), (x1 - x0) as u32, (y1 - y0) as u32);
    enc.set_color(info.color_type);
    enc.set_depth(png::BitDepth::Eight);
    let mut w = enc.write_header().map_err(|e| e.to_string())?;
    w.write_image_data(&out).map_err(|e| e.to_string())?;
    Ok(())
}

/// The recipes, read from `path`.
pub fn read(path: &Path) -> Result<Recipes, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The app to drive: `--app`, else the release build, else the debug build.
pub fn find_app(repo: &Path, given: Option<PathBuf>) -> Option<PathBuf> {
    let exe = if cfg!(windows) { "vectorcraft.exe" } else { "vectorcraft" };
    given.or_else(|| ["release", "debug"].iter().map(|p| repo.join("target").join(p).join(exe)).find(|p| p.exists()))
}
