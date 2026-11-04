//! Fork (cutting workspace): the cutting manual as a PDF, built on demand.
//!
//! ```text
//! cargo run --release --manifest-path tools/cutting-manual/Cargo.toml -- [options]
//!   --no-shots        keep the screenshots from the last run (those missing show a placeholder)
//!   --only a,b        take only these screenshots (the others are kept)
//!   --app PATH        the VectorCraft to photograph (default: target/release, else target/debug)
//!   --out DIR         where to write (default: target/cutting-manual)
//!   --pages           also write each page as a PNG (pages/page-001.png …), to look at without a PDF viewer
//! ```
//!
//! The text is `docs/cutting/user-guide.md`; a `<!-- shot: name -->` line in it puts the
//! screenshot made by the recipe `name` in `tools/cutting-manual/shots.toml` there. Screenshots
//! are taken by starting the app with throwaway preferences and driving it over its control
//! channel, so the screen must be on and unlocked while they are taken. Nothing here is
//! committed: the PDF and screenshots go under `target/`.

#![forbid(unsafe_code)]

mod shots;
mod typst_md;

use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use typst_as_lib::TypstEngine;
use typst_as_lib::typst_kit_options::TypstKitFontOptions;
use typst_layout::PagedDocument;

const TITLE: &str = "Cutting with VectorCraft";
const SUBTITLE: &str = "The cutting workspace: a user manual";
const PDF_NAME: &str = "VectorCraft-Cutting-Manual.pdf";

struct Args {
    shots: bool,
    only: Vec<String>,
    app: Option<PathBuf>,
    out: Option<PathBuf>,
    pages: bool,
}

fn args() -> Result<Args, String> {
    let mut a = Args { shots: true, only: Vec::new(), app: None, out: None, pages: false };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--no-shots" => a.shots = false,
            "--only" => a.only = it.next().ok_or("--only needs names")?.split(',').map(|s| s.trim().to_string()).collect(),
            "--app" => a.app = Some(it.next().ok_or("--app needs a path")?.into()),
            "--pages" => a.pages = true,
            "--out" => a.out = Some(it.next().ok_or("--out needs a folder")?.into()),
            "-h" | "--help" => return Err("usage: cutting-manual [--no-shots] [--only a,b] [--app PATH] [--out DIR] [--pages]".into()),
            other => return Err(format!("unknown option {other:?} (--help)")),
        }
    }
    Ok(a)
}

fn git(repo: &Path, args: &[&str]) -> Option<String> {
    let out = Command::new("git").arg("-C").arg(repo).args(args).output().ok()?;
    out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("cutting-manual: {e}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), String> {
    let args = args()?;
    let here = Path::new(env!("CARGO_MANIFEST_DIR"));
    let repo = std::path::absolute(here.join("../..")).map_err(|e| e.to_string())?;
    let out = args.out.clone().unwrap_or_else(|| repo.join("target").join("cutting-manual"));
    let shot_dir = out.join("shots");
    std::fs::create_dir_all(&shot_dir).map_err(|e| format!("{}: {e}", shot_dir.display()))?;

    let guide = repo.join("docs/cutting/user-guide.md");
    let md = std::fs::read_to_string(&guide).map_err(|e| format!("{}: {e}", guide.display()))?;
    let recipes = shots::read(&here.join("shots.toml"))?;
    let wanted = typst_md::shots(&md);
    for name in &wanted {
        if !recipes.shot.iter().any(|s| &s.name == name) {
            eprintln!("warning: the guide asks for shot {name:?}, which shots.toml has no recipe for");
        }
    }
    for s in &recipes.shot {
        if !wanted.contains(&s.name) {
            eprintln!("warning: shots.toml's {:?} isn't in the guide", s.name);
        }
    }

    let mut failed = Vec::new();
    if args.shots {
        let exe = shots::find_app(&repo, args.app.clone())
            .ok_or("no VectorCraft build found: build it (cargo build --release -p vectorcraft) or pass --app")?;
        eprintln!("taking screenshots with {}", exe.display());
        failed = shots::take(&exe, &recipes, &args.only, &shot_dir);
        let _ = std::fs::remove_dir_all(shot_dir.join("profile"));
        let _ = std::fs::remove_dir_all(shot_dir.join("app"));
    }

    // The Typst: the template next to the main file, the screenshots under shots/.
    std::fs::write(out.join("template.typ"), include_str!("../template.typ")).map_err(|e| e.to_string())?;
    let shot_typst = |name: &str| {
        let Some(r) = recipes.shot.iter().find(|s| s.name == name) else { return format!("#missing({})", typst_md::string(name)) };
        if !shot_dir.join(format!("{name}.png")).exists() {
            return format!("#missing({})", typst_md::string(name));
        }
        format!(
            "#shot({}, [{}], {})",
            typst_md::string(&format!("shots/{name}.png")),
            typst_md::escape(&r.caption),
            r.width.as_deref().unwrap_or("100%")
        )
    };
    let body = typst_md::convert(&md, &shot_typst);
    let built = match (git(&repo, &["log", "-1", "--format=%cs"]), git(&repo, &["rev-parse", "--short", "HEAD"])) {
        (Some(date), Some(commit)) => format!("Built from {commit}, {date}"),
        _ => String::new(),
    };
    let main = format!(
        "#import \"template.typ\": *\n#show: manual.with(title: {}, subtitle: {}, built: {})\n\n{body}",
        typst_md::string(TITLE),
        typst_md::string(SUBTITLE),
        typst_md::string(&built)
    );
    std::fs::write(out.join("manual.typ"), &main).map_err(|e| e.to_string())?;

    let engine = TypstEngine::builder()
        .main_file(("/manual.typ", main))
        .search_fonts_with(TypstKitFontOptions::default().include_system_fonts(true).include_embedded_fonts(true))
        .with_file_system_resolver(&out)
        .build();
    let compiled = engine.compile::<PagedDocument>();
    for w in &compiled.warnings {
        eprintln!("typst warning: {}", w.message);
    }
    let doc = compiled.output.map_err(|e| format!("typst: {e:?}"))?;
    let pdf = typst_pdf::pdf(&doc, &typst_pdf::PdfOptions::default()).map_err(|e| format!("pdf: {e:?}"))?;
    let path = out.join(PDF_NAME);
    std::fs::write(&path, pdf).map_err(|e| format!("{}: {e}", path.display()))?;
    if args.pages {
        let dir = out.join("pages");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        for (i, page) in doc.pages().iter().enumerate() {
            let png = typst_render::render(page, &typst_render::RenderOptions::default()).encode_png().map_err(|e| e.to_string())?;
            std::fs::write(dir.join(format!("page-{:03}.png", i + 1)), png).map_err(|e| e.to_string())?;
        }
    }
    println!("{} ({} pages)", path.display(), doc.pages().len());
    if !failed.is_empty() {
        return Err(format!("{} screenshot(s) failed: {}", failed.len(), failed.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(", ")));
    }
    Ok(())
}
