# The cutting manual

Builds `docs/cutting/user-guide.md` into a PDF, with screenshots of the app, on demand. Nothing it
makes is committed.

```sh
cargo run --release --manifest-path tools/cutting-manual/Cargo.toml
```

It writes `target/cutting-manual/VectorCraft-Cutting-Manual.pdf`. The first build takes a few
minutes (Typst is compiled in). Later runs take about a minute, most of it the screenshots.

- `--no-shots` keeps the screenshots from the last run and only rebuilds the PDF.
- `--only workspace,send` retakes only these screenshots.
- `--app PATH` photographs this VectorCraft. By default it uses `target/release`, else
  `target/debug`; build one first.
- `--pages` also writes each page as a PNG under `target/cutting-manual/pages/`.

## Screenshots

A line `<!-- shot: name -->` in the guide puts screenshot `name` there. Its recipe in
[`shots.toml`](shots.toml) lists the commands that set the app up, and the crop and caption.
Each shot starts its own copy of the app with a throwaway preferences folder
(`VECTORCRAFT_CONFIG_DIR`), so your own preferences, materials and cutting history are never read
or changed. While the screenshots are taken, windows open and close: keep the screen on and
unlocked, and don't use the mouse over them.

## How it's built

- **Text:** [`src/typst_md.rs`](src/typst_md.rs) turns the Markdown into Typst.
- **Layout:** [`template.typ`](template.typ) sets the page design.
- **PDF:** the Typst library (`typst-as-lib`, `typst-pdf`) makes it.
- **Fonts:** the system's Noto Sans or Segoe UI, falling back to Typst's own bundled fonts. No font
  is kept in this repository.

The tool is its own Cargo project, outside the workspace, so its dependencies never reach the app,
the gate or the wasm build.
