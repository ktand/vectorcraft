# Cutting workspace — implementation plan

> **Status:** proposal for review, nothing implemented · **Written:** 2026-10-27 · **Design:**
> [ARCHITECTURE.md](ARCHITECTURE.md)

This page lists the tasks for the cutting workspace in order. Each task is small enough for one agent session and
one PR, has acceptance criteria that a test can check, and keeps `cargo xtask ci` green. Task ids are `C<phase>.<n>`;
commits are `C2.3: per-object cut operation`, following `AGENTS.md`'s one-id-per-commit rule.

Every task also meets the standing criteria: no panics in shipped code (the clippy lints), wasm still builds,
`cargo xtask layers` passes, any new `cutting.*` command passes `engine/tests/command_sweep.rs`, and every new
command has a params doc and tests.

## Milestone: Cutting Workspace Prototype 0.1

These requirements come from the plan's §4 initial acceptance criteria. No hardware is needed.

| Criterion | Tasks |
|---|---|
| Builds and runs without breaking existing editor features | all (CI gate) |
| Opens an SVG with known physical dimensions | C2.1 (existing importer; test pins it) |
| Shows a cutting mat with accurate millimetre coordinates | C2.4, C2.5 |
| Assigns Cut / No Cut to individual objects | C2.3, C2.6 |
| Generates a cutting job from the document | C3.1–C3.5 |
| Shows the toolpaths and their execution order in a preview | C3.6, C3.7 |
| Saves and reopens with cutting settings intact | C2.2, C2.3 |
| Cutting operations go through the command architecture | C2.2, C2.3, C3.5 (all are `cutting.*` commands) |

## Phase 1 — Baseline (done in this review)

| Id | Task | Acceptance |
|---|---|---|
| C1.1 | Fork, `upstream` remote, Windows build, baseline CI | `origin` = `ktand/vectorcraft`, `upstream` = `storytold/vectorcraft`; baseline `cargo xtask ci` result recorded in the review summary |
| C1.2 | Architecture assessment | [ARCHITECTURE.md](ARCHITECTURE.md) reviewed and accepted by the owner |
| C1.3 | Agent rules supplement | `AGENTS.md` gets a short "Fork: cutting workspace" section: points to `docs/cutting/`, says where the hooks are (ARCHITECTURE §7), and states the protocol-source rule (R2): inkscape-silhouette may be read for facts, never copied |

## Phase 2 — Cutting workspace: settings, operations, mat

| Id | Task | Acceptance |
|---|---|---|
| C2.1 | **Pin physical size of SVG import** (test only) — **done** | `crates/engine/tests/it/cutting/svg_size.rs`, through `document.open`: files in mm (with mm or Inkscape's 96 px/in user units), inches and cm open at their size (±0.01 mm); a unitless one opens at 1 px = 1 pt, which documents risk R1. |
| C2.2 | **`vectorcraft-cutjob` skeleton + settings model** — **done**: standalone crate (`serde_json` only) with `CuttingSettings` (lenient `from_value`, `to_value`, defaults, `version`), `Op`, `MatGeometry`, `MAT_PRESETS` (Silhouette 12 × 12 and 12 × 24 in, sized from their names and marked `approximate` until C2.4 cites the cut areas); layers table row | Round-trip tests; damaged values read as defaults, numbers clamped, unknown keys, ops, mat ids and newer versions kept; proptest: arbitrary JSON never panics, reads in range and round-trips |
| C2.3 | **Engine: settings and per-object ops** — **done**; creates `vectorcraft-cutting` (L3) with `ops` (the `cut:op` attribute, inheritance) and `settings`/`set_settings`, so C2.4 adds to it — `cmd/cutting.rs` with `cutting.settings.get` (query), `cutting.settings.set {…partial}`, `cutting.op.set {ids?, op: cut\|score\|none\|inherit}`, `cutting.op.get {ids?}` (query; resolved op and where it came from) | Each set is one undo step, undo restores; save as `.vectorcraft`, reopen, values equal; SVG export → import keeps `cut:op`; duplicate/copy-paste keeps it; group op inherited by children, overridden by a child's own |
| C2.4 | **Mat presets** — **done**: `vectorcraft_cutting::placement::place` (document pt → mat mm as an `Affine`, `mat_to_doc` its inverse, the artboard on the mat, `fits`); landscape turns the artboard clockwise; `cutting.settings.get` reports `placement`; presets cite their sources (12 × 24 in from Silhouette's Cameo 4 specifications; 12 × 12 in from retailers and a fab lab, still marked `approximate`) — mats, orientation, artboard → mat placement, `offsetMm` | Unit tests: corners of a 100 × 50 mm artboard map to the expected mat mm (both orientations, with offset); preset sizes carry a citation in a comment (R9) |
| C2.5 | **Cutting view overlay** (UI) — **done**: View › Cutting View (UI command `view.cutting`, kept with the preferences) draws the mat, the cut area, a 10 mm grid in the mat's colour with every 50 mm stronger, mm labels that thin out when zoomed out and follow a turned mat or view, the origin, and the artboard in red ("Off the mat") when it overhangs | Headless egui tests; screenshots over the control channel: a 100 mm square at (10, 10) mm lands on the 10 mm grid lines within 1 px at 400 %, the mat's edge on the ruler's 304.8 mm, landscape turned as `placement` says |
| C2.7 | **Mat look** (UI) — **done**: `mat_plate` draws behind the artboards and the art (a grey rounded plate past the grid by `MatGeometry::plate_margin_mm`, the grid in inches on the imperial presets (`inch_grid`) else 10 mm, numbered inside along the top and left, ticks down the right edge, the feed arrow, the size in plain text); the cut area, origin (with its unit), off-the-mat warning, toolpaths and playback stay over the art; checked in the app — the cutting view draws the mat as the physical object: a grey plate past the grid with rounded corners (its margin approximate: no published figure), a 1-inch grid with inch numbers on the imperial presets (a per-mat flag; metric and custom keep 10 mm), the feed arrow at the top edge, ruler ticks down the right edge, and the mat's name in plain text in a corner. **No maker's logo or wordmark** (owner decision, 2026-10-28; ASSETS rules). Drawn in code, colours from `theme::Tokens` | Headless test of the inch labels and the arrow; screenshot compared with the plan (not with the maker's software) |
| C2.6 | **Cutting panel** (UI) — **done**: `panels/cutting.rs` (Window › Cutting; an icon in the dock's column) with mat, custom size, the approximate-cut-area note, orientation, artboard, offset, default op, Clip to Mat, whether the artboard fits; the selection's Cut / Score / No Cut / As Its Group or Layer (none checked when mixed); the job's counts and lengths, warnings or error; Cutting View and Show Toolpaths. Window › Workspace › Cutting opens it with the mat shown; other workspaces hide the mat. Op tinting is the C3.6 preview. Done after C3.1–C3.5, so the panel shows a real job; checked in the app | Panel acts only through `app.run("cutting.*")`; colours from `theme::Tokens`; strings go through i18n; headless frame test |

## Phase 3 — Toolpath engine and simulator

| Id | Task | Acceptance |
|---|---|---|
| C3.1 | **Job model** — **done** — in `cutjob`: `CutJob`, `JobMat`, `Operation` (`OpKind` score/cut), `ToolSettings` (ranges shared with the settings reader, `problem()`), `Toolpath` with `SourceRef`, `JobWarning`, `JobStats` (path, blade and travel mm), `CutJob::validate` with caps (`MAX_PATHS`, `MAX_POINTS`), `to_value` | Validation rejects non-finite, out-of-area, out-of-range, over-cap jobs with an error, never a panic (proptest); stats tests |
| C3.2 | **Select and resolve** (pipeline 1) in `cutting` — **done**: `select::select` → items (id, op, enclosing clip groups) in paint order | Tests: hidden, guide, template, clipping-path and off-artboard objects excluded, locked ones cut; op resolution matches `cutting.op.get`; a compound path is one item. **Changed from the plan:** layers that don't print *are* cut (print-and-cut keeps its cut lines on one); No Cut is how to leave art out |
| C3.3 | **Geometry** (pipeline 2–3) — **done**: `geometry::outlines` → one closed or open subpath per outline, document pt, with its source; paths and compound paths after geometry effects (`preview::drawn_art`), on the centre line; type through its glyph outlines; symbols through their placed art; blends, envelopes, repeats and placed documents through their evaluated art; outlines drawn twice in one object cut once; images and meshes skipped with a warning. **Decided:** clip groups aren't applied yet; clipped art is cut whole, with a warning (C3.3b) | One test per object kind; a warning names the source id |
| C3.3b | **Clipping** — **done**: `clip` (a closed outline cut to the visible part, `pathops` Intersect; an open path cut along its runs inside, either fill rule; nested masks each clip; a mask clipping to nothing hides; a refused boolean cuts whole with a warning). Clip groups inside a symbol's or live object's own art are still cut whole with a warning. The fuzz test found an unguarded sweep panic in `pathops::normalize_bez` (reached through Unite in `clip_outline`), fixed the way `Arrangement::new` already guards its sweep — clip each item's closed outlines to its clip groups' region (`effects::clip_outline`, `pathops` booleans) and split open ones where they cross it | A square half under a clip rectangle cuts the visible half; an open line crossing it is cut inside only; the C3.3 warning goes |
| C3.4 | **Transform, flatten, bounds** (pipeline 4–6) — **done**: `prepare::prepare` → mat-mm polylines (repeats and straight-run points merged, closing point not repeated, zero length dropped, a two-point "closed" shape cut as a line); off the cut area → `PrepError` naming the object; new setting `clipToMat` (off) cuts up to the edge in open pieces with a warning; non-finite outlines left out with a warning; the job's point cap is an error. `cutting.settings.set` learns `clipToMat` in C3.5 | A circle of r = 10 mm flattens within `toleranceMm` of the true circle; out-of-mat path → error naming it; with "clip to mat" → clipped + warning |
| C3.5 | **Order + `cutting.job` command** (pipeline 7–8) — **done**: `order::order` (score before cut; inside first; nearest next, closed paths turned and open ones reversed to start near the blade; inner-first by size past 4,000 paths), `build_job` (the whole pipeline, validated), `cutting.job {}` (query) → `{job, stats}` with the job's warnings in it; `cutting.settings.set` takes `clipToMat`; golden test `crates/cutting/tests/golden/` | Insiders before outsiders (letter "O": inner counter first); nearest-neighbour reduces travel vs document order on a fixture; **determinism**: same input twice → identical JSON; golden job files under `crates/cutting/tests/golden/` (hand-written SVG inputs, small) |
| C3.6 | **Preview overlay** — **done**: over the mat, cuts solid and scores dashed in their colours, direction arrows, order numbers (thinned; none past 500 paths), travel dashed; at most 400k points drawn a frame. The job (`ui-egui/src/cutting.rs`) is made again only when the document value changes, not on redraws or selection changes; still on the UI thread (follow-up if a real file shows a pause); checked in the app | Headless test; 5k-path document keeps the frame under the existing canvas budget (`vectorcraft-cli perf` unchanged) |
| C3.7 | **Simulator** — **done**: `cutjob::Simulation` (travel up, cut down once per pass, overcut on closed paths, open paths travelled back between passes; time from each operation's speed, travel at 200 mm/s; `at_mm`, `path_starts_mm`), `vectorcraft-cutter::VirtualCutter` (refuses a job that doesn't validate, records each stroke), `cutting.simulate {t?|mm?}`, panel playback (Play/Pause ×10, Previous/Next path, scrub, Done; estimated time) and the cut-so-far and blade marker on the canvas — `Simulation` in `cutjob`; `VirtualCutter` in new crate `vectorcraft-cutter` (layers row); panel playback (play, step, scrub); `cutting.simulate {t?}` (query) → knife position and state | Simulation length = stats; replay of the virtual cutter equals the job; playback shows the same order numbers as C3.6 |
| C3.8 | **Preview export** — **done**: `preview::to_svg` (mat-sized, mm user units, travel dashed, paths with `data-order`/`data-op`, numbered) and `cutting.preview.export {path?, format?: svg|json}` (written through the engine's safe write, or returned with a `<document>-cut` name) — `cutting.preview.export {path?, format: svg\|json}` writes the toolpaths (mm, order, op) for agents and bug reports | SVG output opens in VectorCraft at the mat's size; JSON equals `cutting.job` |
| C3.9 | **Fuzz** — **done**: `crates/cutting/tests/fuzz.rs` (proptest: random documents — NaN, infinite and huge coordinates, empty paths, nested groups and clip groups, compound paths, symbol instances, hidden objects, junk `cut:op` — and random settings; a job validates and simulates, else an error; 20,000 cases checked); `command_sweep` already covers the `cutting.*` commands' params — extend `engine/tests/command_sweep.rs` and `import_fuzz.rs` style tests: random documents → `cutting.job` never panics | Runs in CI time budget |
| C3.10 | **Cut Outline** — a fourth operation (`cut:op="outline"`, inherited like the others): the closed shapes set to it are united (`pathops`) and only the boundary is cut, holes that stay holes included, the lines inside not; open paths are cut as they are, with a warning; the Selection section offers it; plain Cut (every line) stays the default. Asked for by the owner (2026-10-28), from a workflow cutting tools commonly offer | Two overlapping squares set to Cut Outline make one closed path along their joint boundary; an O keeps its counter; a group set to it unites its members; ops resolve as before; the fuzz test covers it |

**Prototype 0.1 = C2.1–C3.8 done** — reached 2026-10-28 with C3.9 too; tagged `cutting-0.1` (local). Open follow-ups: C3.3b (clipping masks), C2.7 (mat look).

## Phase 4 — Silhouette hardware

| Id | Task | Acceptance |
|---|---|---|
| C4.1 | **Device service design** — **done**: a registry per process in `vectorcraft-cutter` (the app installs a `DeviceProvider`; `Driver`s per family), `Transport`, `DeviceInfo`, `DeviceStatus`, `Encoded`, the sender (`send`, `spawn`, `Running`: ready before each chunk, progress, cancel, outcomes as messages) and `MemoryTransport`; ARCHITECTURE › 6 | Design note in ARCHITECTURE; traits with doc comments; tests against the recording transport |
| C4.2 | **USB spike** (desktop, Windows first). **Done without the device (2026-10-28):** the Cameo 4 is a USB printer-class device on Microsoft's `usbprint.sys` (ARCHITECTURE › R3), so the transport opens its USB-printer device interface (`CreateFile` / `WriteFile` / `ReadFile`, `windows-sys`) rather than a user-space USB library; no driver change. **With the device:** find it by VID/PID, open it, write a status query, read the reply; measure write sizes and timeouts | Spike report in `docs/cutting/hardware-validation.md`: opens with the in-box driver, status read back; Silhouette Studio still works afterwards; no shipped code yet |
| C4.3 | **Protocol note** — **done** (to be confirmed on the machine): [`protocol-silhouette.md`](protocol-silhouette.md), facts from inkscape-silhouette (`Graphtec.py` at `f376a083`, read for facts only), Windows' device record and Silhouette's specifications, each cited, in our own words; with the mapping from the cut job and the checks to make on the machine | Reviewed; no copied code, tables or comments |
| C4.4 | **Encoder** — **done** (to be confirmed on the machine): `vectorcraft-cutter-silhouette` (L3), the `Silhouette` driver written from the protocol notes: models by USB id with their cut widths, ESC ENQ status, a job as initialise, mat (`TG`), per operation `J1`/`FX`/`!`/`FC`, `M`/`D` moves in 1/20 mm vertical first, passes, overcut, end (`SO0`); chunks of at most 4,096 bytes split between commands; an unknown mat is "no mat" with a warning, a job wider than the machine an error, speeds approximate (10 mm/s a step) with a warning | Golden bytes written from the notes; the moves read back from the bytes equal the simulation; chunks; a full send to a recording transport answering ready |
| C4.5 | **Send / monitor / cancel** — **done, to be checked on the machine**: the desktop app's USB transport (`apps/vectorcraft/src/cutters.rs`: SetupDi listing of the USB-printer interface, overlapped `CreateFile`/`WriteFile`/`ReadFile`, no driver change; Windows only for now), `cutting.devices`, `cutting.send {device?}`, `cutting.status`, `cutting.cancel` (engine, through the registry; one send at a time), and the Cutting panel's Cutter section (machine, Send to Cutter, Cancel, progress, outcome, Find Cutters). The first checks on the Cameo 4 are [`hardware-validation.md`](hardware-validation.md) | Against a fake Cameo 4 in CI: listing, a send to done (status, initialise, job, `SO0`), no mat stops it with its message, unknown device and empty job refused; app: device path ids, listing doesn't fail |
| C4.6 | **Hardware validation** (person in the loop): cut the test sheet (100 mm square, 50 mm circle, 10 mm text, nested "O") on one Cameo model | Measured within ±0.3 mm (target to confirm); results recorded in `docs/cutting/hardware-validation.md` |
| C4.7 | **Tool holders and Sketch** — each operation chooses its holder (the Cameo 4 has two: `J1`/`J2`), and a Sketch (pen) operation draws instead of cutting; after the machine checks, since it depends on how holder 2 behaves | Encoder golden bytes per holder; checked on the machine with a pen in holder 2 |

## Phase 5 — Usability and reliability (outline; size later)

Material presets and saved device profiles · operation grouping by colour or layer (multi-colour cutting) ·
blade-offset compensation and overcut (R10) · better ordering (travel optimisation with a time budget) ·
Print & Cut with registration marks (marks generated in code, no vendor artwork) · job history and error
reports · more cutter families as their own `cutter-*` crates · upstream PRs for command and overlay
extension points (ARCHITECTURE §7).

## How agents should work on this

- One task per branch (`cutting/C2.3-op-commands`) and per PR; the PR updates this file's status column (to be
  added once work starts) and ARCHITECTURE when a decision changes.
- Edit only the crates the task names; hook points in upstream files are single lines (ARCHITECTURE §7).
- Tests use hand-written inputs (small SVGs written from the SVG spec); never vendor sample files or files made
  with Silhouette Studio.
- Merge `upstream/main` into the fork's `main` at least weekly; resolve conflicts at the hooks first.
