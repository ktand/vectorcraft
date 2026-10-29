# Cutting workspace — architecture

> **Status:** proposal for review, nothing implemented · **Written:** 2026-10-27 against upstream `8622d0c7`
> (v0.17.0) · **Scope:** the `ktand/vectorcraft` fork

The fork adds a cutting workspace: you prepare an ordinary VectorCraft document for a cutting machine, preview
the toolpaths and their order, and later send the job to a Silhouette Cameo or another cutter. This page says
where that work fits into VectorCraft and which seams it uses. The task list is in
[IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md).

The repository rules in [`AGENTS.md`](../../AGENTS.md) apply unchanged: never crash, everything is a command,
layering, Rust only, never break wasm, clean-room, assets.

## 1. What VectorCraft already gives us

| Concern | Where it lives | What cutting reuses |
|---|---|---|
| Document model | `crates/doc` (L1): a persistent tree of `Arc<Node>`; `Node.kind` is a `Layer`, `Group`, `Path`, text, image, symbol… Geometry is in **document points, y down**, with no transform per node (paths hold document coordinates). `NodeId` is stable and never reused. | Read-only input to job generation. |
| Physical units | `vectorcraft_doc::Unit` (display only; the model is always in points). `Unit::Millimeters.points()` = 72 / 25.4. | The conversion from points to millimetres. |
| Geometry as drawn | `vectorcraft_effects::apply_geometry` (geometry effects in stack order), `reshape` (type, symbols and live objects through their outlines), `clip_outline`; `vectorcraft_geom::flatten`; `vectorcraft_pathops` (booleans, offset, `outline_stroke`). `crates/cad` (DXF) already walks the document this way. | The cut geometry is the object's path after its geometry effects. |
| Commands, undo | `CommandSpec` in `crates/engine/src/cmd/*`, listed in `command_specs()` (`crates/engine/src/cmd.rs`). `Session::edit(label, f)` makes one undo step; history is snapshots of the shared document. | Every cutting action is a `cutting.*` command; edits go through `Session::edit`. |
| Agents | MCP (`crates/mcp`, `vectorcraft-cli mcp`), the control channel (`crates/ui-egui/src/control.rs`), the CLI. Each reaches the whole command registry. | `cutting.*` commands are reachable from all three with no extra work. |
| Per-object data | `NodeExtras.attrs.foreign: Vec<ForeignAttribute>`: attributes in another application's XML namespace. They are saved natively, kept from SVG import to SVG export, and copied with the object; nothing in VectorCraft reads them. The doc comment's own example is "a cutter's passes". | Each object's cut operation (§4.2). |
| Per-document data | `Document::unknown` (foreign data the native format keeps as is). `pluginData` within it, used by the plotter and cutter exporter plug-ins. | The document's cutting settings (§4.1). |
| UI layouts | `crates/ui-egui/src/workspaces.rs`: Window › Workspace presets are panel layouts (Essentials, Painting…). | A built-in **Cutting** workspace. |
| Canvas overlays | `crates/ui-egui/src/canvas/tool_overlays.rs`, e.g. `print_tiling_overlay` draws physical pages over the art. | The mat and toolpath overlays. |
| Host services | `PrintService` is a trait in `ui-egui`; the desktop app (`apps/vectorcraft/src/printing.rs`) implements it with craft-print. OS I/O stays in the app. | The same split for USB cutters (§6). |
| Exporter plug-ins | `crates/plugins` (sandboxed wasm); `docs/plugins.md` names "pen plotters, cutting machines" as a use. | Not the main route: a plug-in can't reach a device and has no UI. It stays an option for third-party file formats. |

`plan/` (the upstream session notes) isn't in this checkout; it's gitignored and local to upstream.

## 2. Design decisions

1. **The document stays the source of truth.** A cut job is derived on demand from the document and the
   cutting settings. It's never stored as geometry in the document, and making it never changes editable art.
2. **Millimetres are canonical** from the cutting core down. The document stays in points; the conversion happens
   once, in `vectorcraft-cutting`.
3. **Store data in the slots that already exist.** Use foreign attributes and `Document::unknown`; add no new
   field to `vectorcraft-doc` and no new section to the native format. Files made by the fork open in upstream
   VectorCraft, which keeps the cutting data.
4. **Keep upstream edits to a few lines at known hook points** (§7). The cutting code lives in new crates and new
   files.
5. **Separate the machine from the core.** The job model knows no device. A driver turns a job into device bytes
   and talks to an abstract transport. Only the app touches USB.
6. **Use no new dependencies until Phase 4.** Phases 1–3 need nothing beyond `kurbo`, `serde` and `serde_json`,
   which the workspace already has.

## 3. Crates

```
crates/
  cutjob/              NEW  L0 standalone  job model, validation, statistics, simulation timeline (serde only)
  cutting/             NEW  L3             document → CutJob: extract, flatten, transform, clip, order, validate
  cutter/              NEW  L2             Device and Transport traits, VirtualCutter, device registry
  cutter-silhouette/   NEW  L3             Silhouette protocol encoder and model table (no I/O)
  engine/              EDIT L5             + cmd/cutting.rs (the cutting.* commands), one line in cmd.rs
  ui-egui/             EDIT L6             + panels/cutting.rs, canvas/cutting_overlay.rs, Cutting workspace, UI commands
apps/vectorcraft/      EDIT L7             + cutters.rs: USB transport (Phase 4)
apps/vectorcraft-web/  EDIT L7             + WebUSB transport (Phase 4, optional)
xtask/src/layers.rs    EDIT                + four rows at the end of the append-only TABLE
```

Dependency graph (arrows point at dependencies):

```
           ui-egui ──► engine ──► cutting ──► doc, geom, effects, text, pathops
              │           │          │
              │           │          └──────► cutjob
              │           └─────────────────► cutter ──► cutjob
              │                               ▲
apps ─────────┴──► cutter-silhouette ─────────┘
(USB transport)
```

- **`vectorcraft-cutjob` (Standalone, L0)** depends on no workspace crate, and on nothing beyond `serde` and
  `serde_json`. Both sides share it: the core produces a job and the drivers consume one. Being standalone keeps
  it trivial to fuzz and to build for wasm, and an outside tool could reuse it.
- **`vectorcraft-cutting` (L3)** sits at the layer of `cad`, whose job is much the same (walk the document and emit
  machine-ready geometry). It depends on `doc`, `geom`, `effects`, `text` (outlines of type), `pathops` and
  `cutjob`. It has no UI and does no I/O.
- **`vectorcraft-cutter` (L2)** depends only on `cutjob`. It defines the device traits and the virtual cutter,
  which simulates execution and records it.
- **`vectorcraft-cutter-silhouette` (L3)** depends on `cutter` and `cutjob` (L3 rather than L2: `cutter` is L2 itself). It's pure encoding (job →
  bytes, status bytes → state), so the tests run without hardware. Each future machine family gets a crate of
  its own, which isolates protocol code and its provenance.
- **Engine glue** is one module, `crates/engine/src/cmd/cutting.rs`. It reads settings and per-object ops,
  calls `vectorcraft_cutting::build_job`, and writes edits through `Session::edit`. The registry is a static
  list, so it takes one `v.extend(cutting::specs())` line. A crate above L5 can't register commands today
  (see §7 on proposing an upstream hook).

Name clash: VectorCraft already has *Cutting tools* (Mirror & Cut, Line Cut, Rectangle Cut: `cmd/cut.rs`,
`tools/cut.rs`, `path.lineCut`…). Everything here is `cutting.*` / `cutjob` / `cutter`, and user-facing text says
"Cutting Machine" where there could be confusion.

## 4. Data stored in the document

### 4.1 Document settings: `Document::unknown["cutting"]`

```json
{
  "version": 1,
  "mat": "silhouette.12x12",
  "customMatMm": [304.8, 304.8],
  "orientation": "portrait",
  "artboard": 0,
  "offsetMm": [0.0, 0.0],
  "defaultOp": "cut",
  "toleranceMm": 0.025,
  "operations": {
    "cut":   {"force": 0.3, "speedMmS": 100, "passes": 1, "overcutMm": 0.0},
    "score": {"force": 0.1, "speedMmS": 100, "passes": 1, "overcutMm": 0.0}
  },
  "ordering": "insideFirst"
}
```

The settings are read leniently (`vectorcraft_cutjob::CuttingSettings::from_value`, which never fails): a
missing, damaged or not-understood value means its default, a finite number out of range is clamped, and unknown
keys are kept at every level (top, `operations`, each tool). A `version` newer than the fork knows is kept, and so
is a mat id it doesn't know, laid out meanwhile on the default mat. The native
format keeps `unknown` as it is, and older VectorCraft versions keep it when they save.

`pluginData` was the alternative. It is documented as "the host never reads them", so a host feature
shouldn't live there.

### 4.2 Per-object operation: foreign attribute

```
ns     = "urn:vectorcraft:cutting:1"
prefix = "cut"
name   = "op"
value  = "cut" | "score" | "none" | "outline"
```

Exported to SVG it reads `<path cut:op="score" xmlns:cut="urn:vectorcraft:cutting:1" …/>`.

- **Inheritance:** an object without the attribute takes the operation of its nearest ancestor (group or layer)
  that has one, and otherwise `defaultOp`. So "No Cut" on a layer turns off everything on it.
- Copies, duplicates and pastes keep the operation; deleting the object removes it. Undo works because the
  change is a normal document edit.
- The object-level attribute is the only place an operation is stored. There's no parallel map keyed by
  `NodeId`, so there's nothing to go stale.

**The registration marks layer (C6.1).** Print & Cut's marks are ordinary art on a top-level layer, locked and
set to `cut:op="none"`, so they print with the artboard and are never cut. The layer is found again by a second
attribute in the same namespace, `cut:marks="1"`, whatever it has been renamed to; `cutting.marks.add` replaces
it, `cutting.marks.remove` takes it out. Nothing else marks it.

### 4.3 What is never stored

The cut job itself, flattened geometry, cut order, simulation state, and device connection state. All of them are
derived, or they belong to the session.

## 5. The machine-independent cut job (`vectorcraft-cutjob`)

```rust
/// A complete, validated, device-independent job: what to cut, in which order, with which settings.
pub struct CutJob {
    pub version: u32,                 // schema version of this struct (1)
    pub mat: MatGeometry,             // the usable area, mm
    pub operations: Vec<Operation>,   // settings per operation; paths refer to them by index
    pub paths: Vec<Toolpath>,         // in execution order
    pub warnings: Vec<JobWarning>,    // what was skipped or approximated, with source ids
}

pub struct MatGeometry {
    pub id: String,                   // "silhouette.cameo.12x12", "custom"
    pub size_mm: [f64; 2],            // physical mat
    pub cut_area_mm: RectMm,          // where the blade may go, in mat coordinates
}

pub struct Operation {
    pub kind: OpKind,                 // Cut | Score   (later: Sketch/Pen, Perforate, Emboss…)
    pub tool: ToolSettings,
}

pub struct ToolSettings {
    pub force: f64,                   // 0.0..=1.0 of the machine's range; the driver maps it to its own steps
    pub speed_mm_s: f64,              // requested speed; the driver clamps it to what the machine supports
    pub passes: u8,                   // 1..=10
    pub overcut_mm: f64,              // closed paths only
    pub tool_slot: Option<u8>,        // dual-carriage machines
}

pub struct Toolpath {
    pub op: usize,                    // index into operations
    pub source: SourceRef,            // node id + subpath index, for the preview's hover and errors
    pub closed: bool,
    pub points: Vec<[f64; 2]>,        // mm, mat coordinates: origin at the cut area's top-left, +x right,
                                      // +y toward the user (feed direction), as the document's y down
}

pub struct JobStats { pub cut_mm: f64, pub travel_mm: f64, pub path_count: usize, pub point_count: usize }
```

- **Polylines only in v1.** Flattening happens once, to `toleranceMm`. A driver that can send curves may get a
  Bézier variant later without changing the core.
- **The job is built from data and never panics.** `CutJob::validate` checks that coordinates are finite and
  inside `cut_area_mm`, that the settings are in range, and that counts stay under the caps (paths ≤ 200 000,
  points ≤ 5 000 000, each a `const`). It returns a `Result`. It's also the deserialisation gate for jobs coming
  from MCP or files.
- **Deterministic:** the same document and settings give a byte-identical job (no `HashMap` order, no time). This
  is what makes golden tests and "reproducible job" work.
- `Simulation::new(&job)` steps through the moves (knife down/up, line to) for the virtual cutter and the
  preview's playback. Both share the same timeline, so the preview shows exactly what a driver would get.

### 5.1 The pipeline in `vectorcraft-cutting`

```
Document + CuttingSettings
  1. select     the visible objects on the chosen artboard (locked ones and non-printing layers too:
                print-and-cut keeps cut lines on one); skip guides, clipping paths, template layers
                and hidden layers; resolve each object's op (attribute → ancestors → defaultOp);
                drop "none"; note the clip groups around each
  2. geometry   paths and compound paths after geometry effects (apply_geometry); type through its outlines,
                symbols expanded (reshape); a stroke is cut on its centre line, not its outline;
                images are skipped with a warning
  3b. weld      the closed shapes set to Cut Outline united; only the union's outline is cut (C3.10)
  3. clip       clip groups and layer clips: closed paths clipped with pathops, open paths split; v1 may warn
                and cut unclipped (the task decides)
  4. transform  document pt → mm (× 25.4 / 72), artboard top-left → mat origin + offsetMm, orientation
  5. flatten    to toleranceMm; drop zero-length paths; merge collinear points
  6. bounds     paths outside cut_area_mm are an error by default (clipping to the mat is an option)
  7. order      a containment-aware order (insideFirst: a path inside another closed path is cut before it, so
                the piece doesn't shift); nearest neighbour inside each level; a closed path starts at the
                point nearest the previous end; ops in a fixed order (score before cut)
  8. validate   CutJob::validate
```

Each stage is a pure function with its own unit tests. Stages 2 and 3 reuse what `cad` and the renderer use, so
the cut follows what the user sees.

## 6. Devices (Phase 4)

```rust
pub trait Transport: Send {                         // bytes in, bytes out; implemented by the app
    fn write(&mut self, bytes: &[u8]) -> Result<(), TransportError>;
    fn read(&mut self, buf: &mut [u8], timeout_ms: u32) -> Result<usize, TransportError>;
}

pub trait CutterDriver: Send {                      // implemented per machine family, no I/O of its own
    fn identify(&self, reply: &[u8]) -> Option<DeviceInfo>;
    fn capabilities(&self, info: &DeviceInfo) -> Capabilities;   // mats, force/speed ranges, tools
    fn encode(&self, job: &CutJob, caps: &Capabilities) -> Result<Vec<Chunk>, DriverError>;
    fn parse_status(&self, reply: &[u8]) -> DeviceStatus;        // ready, moving, paused, error
}
```

- `VirtualCutter` implements `Transport` in memory. It records the bytes, and for the virtual driver it replays
  the `Simulation`. It's the hardware double for every test above the driver.
- **The USB transport lives in the app**, as printing does. Desktop gets a new dependency, a USB crate, chosen
  in task P4.2: `nusb` (pure Rust, MIT/Apache-2.0, no libusb) is the first candidate, and `rusb` (binds the C
  libusb) is the fallback. The web build could use WebUSB through `web-sys` (unstable APIs, Chromium only).
  The core crates stay I/O-free, so wasm never breaks.
- Sending runs on a background worker with progress and cancellation, like Background Export and print
  submission. The status line reports the outcome, and a failure is an error message, never a panic.
- **Decided (C4.1, 2026-10-28): a registry per process.** `vectorcraft_cutter::registry` holds one
  `DeviceProvider`, which the app installs at start-up (the desktop app: USB through the system driver; the CLI
  and the MCP server can install the same one), and the `Driver`s this version has. The engine's `cutting.*`
  device commands ask it, so sending works from the UI, the control channel, MCP and the CLI alike, with no new
  field in the engine's session; plug-ins are registered per process the same way. A send runs on a thread of
  its own (`send::spawn`): each chunk goes once the machine answers ready, `Running` reports progress and
  cancels between chunks, and the outcome (done, no mat, still busy, unplugged, cancelled) is a message, never a
  panic. The web build refuses to send until WebUSB is added. `MemoryTransport` records and answers from a
  script: every test above the USB write runs without hardware.

## 7. Where the fork touches upstream code

Each hook is a few lines. Keep them in this list so a merge from `upstream` knows where to look.

| File | Change |
|---|---|
| `AGENTS.md`, `CLAUDE.md` | the "Fork: cutting workspace" section at the end of each (keep the two identical) |
| `Cargo.toml` (workspace) | `[workspace.dependencies]` path entries (`cutjob`, `cutting`, `cutter`, `cutter-silhouette` so far); members are already `crates/*` |
| `xtask/src/layers.rs` | rows appended to `TABLE` (`cutjob` Standalone, `cutting` L3, `cutter` L2, `cutter-silhouette` L3); `["svg", "cutting"]` in `INTRA_LAYER_ORDER` (the golden tests open SVGs) |
| `crates/engine/tests/it/main.rs` | `mod cutting;` (every engine-level cutting test lives under `tests/it/cutting/`) |
| `crates/engine/Cargo.toml`, `src/cmd.rs` | dependencies on `vectorcraft-cutjob`, `vectorcraft-cutting`, `vectorcraft-cutter` and `vectorcraft-cutter-silhouette`; `mod cutting;` + `v.extend(cutting::specs())` (the device commands are `cmd/cutting/devices.rs`, a submodule) |
| `crates/ui-egui/…` | `Cargo.toml`: the two cutting dependencies; `state.rs`: `ViewFlags::cutting` and its default; `menus/ui_commands.rs`, `menus/run.rs`, `menus/state.rs`, `menus/tree.rs`: `view.cutting`'s entry, toggle, checkmark and View-menu item; `canvas.rs`: `pub(crate) mod cutting_overlay;` (the Send window draws its mat preview with it), the mat plate's call after the pasteboard fill (behind the art) and the overlay's call (over it); `tests.rs`: `mod cutting;`; `lib.rs`: `mod cutting;` (the shared job cache); `panels.rs`: `pub mod cutting;` and its `show` arm; `state.rs`: the panel's `ICON_PANELS` row and its `ICON_PANEL_GROUPS` group; `menus/tree.rs`: Window › Cutting and File › Send to Cutter…; `menus/ui_commands.rs`, `menus/run.rs`: `cutting.sendWindow`'s entry and arm; `app.rs`: the Send window's `show` after the dialogs'; `workspaces.rs`: the built-in Cutting workspace, `CUTTING`, and one line in `apply` (the workspace turns Cutting View on, the others off) |
| `apps/vectorcraft/…` | `src/main.rs`: `mod cutters;`, the `--virtual-cutter` flag (one match arm, one usage line) and `cutters::install(virtual_cutter)` after the arguments; `Cargo.toml`: the `vectorcraft-cutter` and `vectorcraft-cutter-silhouette` dependencies and five more `windows` features (SetupDi, file I/O, overlapped I/O, events, security attributes); `src/cutters.rs` is the fork's |
| `crates/pathops/src/boolean.rs` | a bug fix, not a hook: `normalize_bez` turns a panic of the sweep into `PathOpsError::Degenerate`, as `Arrangement::new` already does (found by the cutting fuzz test; a crafted clip mask could otherwise panic the renderer too) — **send upstream** |

Upstream PR candidates: the `pathops` sweep guard above (a crash fix, worth sending soon). Ones that would shrink this list (optional, later): an extension point for registering command
sets from crates other than `engine`, and one for canvas overlays. They're generic, and upstream might accept
them on their own merits.

The upstream progress docs (`ROADMAP.md`, `docs/target-app-parity.md`, `docs/gaps.md`…) track Illustrator parity.
The fork leaves them alone, which avoids merge conflicts, and tracks cutting in `docs/cutting/`.

## 8. Risks and gaps

| # | Risk | Mitigation |
|---|---|---|
| R1 | **Size of unitless SVGs.** SVG import reads a file with physical units (`width="200mm"`) at its true size but a unitless one at 1 px = 1 pt (72 dpi). A unitless file drawn at 96 px/in cuts a third larger than its author expects (a 100 mm square of 378 px opens 133 mm wide; pinned by `crates/engine/tests/it/cutting/svg_size.rs`). | The cutting panel always shows the job's size in mm; a "scale" field in the cutting settings; a warning when the document has no physical unit. Never change SVG import itself. |
| R2 | **Licence of the protocol source.** inkscape-silhouette is GPL-2.0, and `AGENTS.md` forbids copying GPL code. | **Owner decision (2026-10-27):** inkscape-silhouette may be read for protocol facts. (a) `docs/cutting/protocol-silhouette.md` records facts only (commands, units, sequences, status codes), each cited to its source (inkscape-silhouette by file and commit, Graphtec GP-GL references, public posts, published specs), in our own words, with no code and no transcribed tables or comments; (b) the encoder is written from that note, not from the GPL source, and nothing from it is copied into this repo, tests included. No rule about Silhouette Studio for now (owner decision, 2026-10-27). |
| R3 | **USB on Windows.** The machine may bind to a vendor or printer-class driver that a user-space USB library can't open without WinUSB. | **Checked 2026-10-28 (Cameo 4, from the device record Windows keeps; the device itself wasn't attached):** USB `VID_0B4D&PID_1137`, interface class 7 / subclass 1 / protocol 2 (USB printer, bidirectional), bound to Microsoft's in-box `usbprint.sys` (`usbprint.inf`); no vendor driver and no print queue. A user-space USB library would need the driver swapped for WinUSB (Zadig), which we won't ask of users. **Plan:** on Windows, open the device through its USB-printer device interface (`GUID_DEVINTERFACE_USBPRINT`) with `CreateFile` and use `WriteFile`/`ReadFile` for commands and status (`windows-sys`, already in `Cargo.lock`); Linux `/dev/usb/lp*`; macOS through IOKit or `nusb`; the web through WebUSB later. To confirm on the device (C4.2). |
| R4 | **Hardware access.** Agents can't cut on a physical machine; real validation needs a person and a mat. | The virtual cutter and golden byte streams make everything up to the USB write testable; hardware checks are a short manual checklist in each Phase 4 task. |
| R5 | **Clip groups, compound paths, effects.** Cutting what the user *sees* needs the clipped, effected outline; cutting what the user *drew* needs the raw path. | v1 cuts the effected path, clipped (warning where not yet supported); a per-job "ignore effects" option later. Tests pin down each case. |
| R6 | **Fills vs strokes.** A stroked open path and a filled closed path both become one centre line; wide strokes and stroke-only art (outlined lettering) may surprise. | Document the rule in the panel; "Cut stroke outline" is a later per-object option (uses `outline_stroke`). |
| R7 | **Merge conflicts** in `cmd.rs`, `canvas.rs`, `workspaces.rs`, which change often upstream. | Hooks are single lines at list ends; the §7 table; merge `upstream/main` often (weekly). |
| R8 | **Rulers and units.** The canvas rulers follow the document's unit and ruler origin; a mat-origin mm ruler needs a ruler change in `ui-egui`. | v1 draws its own mm scale along the mat overlay and sets the document unit to mm only when the user asks; a ruler-origin hook is a later task. |
| R9 | **Mat geometry.** Mat sizes and the usable cut area per model must come from published product specifications, not guesses. | `vectorcraft_cutjob::MAT_PRESETS` cites its sources (C2.4): Silhouette's Cameo 4 specifications give 12 × 24 in "With a Cutting Mat"; the 12 × 12 in mat's full-area cut comes from retailers and a fab lab only, so it stays `approximate` and the UI says so. The mat's plastic beyond its grid isn't modelled (no published figure). Per-model tables move to `cutter-silhouette` in Phase 4. |
| R10 | **Drag-knife compensation** (blade offset), corner handling, and overcut change the geometry noticeably on small pieces. | Decided in C5.4: Silhouette machines compensate in firmware (`FC` n, [protocol notes](protocol-silhouette.md)), so the job keeps the tip's path and each operation's `bladeOffsetMm` (null: the usual 0.9 mm blade's) goes to the machine. Compensating in software as well would double the correction. A software stage in `cutting` (the axis path run on past each corner by the offset, with an arc round it) is only for a later cutter family that can't compensate itself; overcut and corner overcut are done (C5.2). |

Missing for the first milestone: nothing in VectorCraft blocks it. All of Phase 2–3 can be built on the
existing seams.
