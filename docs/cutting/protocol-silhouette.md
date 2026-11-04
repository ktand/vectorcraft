# Silhouette cutters: protocol notes

> **Status:** facts gathered for the encoder (C4.3); framing, units, axes, the origin, status and the tools query
> checked on the owner's Cameo 4 (2026-11-01, [HW]); Print & Cut facts from Silhouette Studio's interface replaced
> by public tutorials where they cover them (2026-11-03, R2); curve commands from Graphtec's GP-GL manual
> (2026-11-04, to confirm) · **Written:** 2026-10-28 ·
> **Scope:** Cameo 4 first; other models noted where the source gives them

These are facts about how a computer talks to a Silhouette cutter: what it sends, what it gets back, and in
which units. They are written in our own words, with the source of each. Encoders are written from this page,
not from the sources.

**Silhouette Studio.** Never capture or watch what Silhouette Studio sends to or gets from a machine: its licence forbids it, and no fact here comes from a capture of our own; facts others have published (inkscape-silhouette) are a source whatever their origin (owner, 2026-11-04). Studio's interface is a source (**[ST]**) only where public documentation doesn't cover the fact; see [ARCHITECTURE.md › R2](ARCHITECTURE.md) (owner decision, 2026-11-03).

**Sources.**

- **[IS]** inkscape-silhouette, `silhouette/Graphtec.py` at commit
  [`f376a083`](https://github.com/fablabnbg/inkscape-silhouette/blob/f376a0836b53826e57f50d7ec44658663e629f05/silhouette/Graphtec.py)
  (2026-10-13). It is GPL-2.0 ("Distribute under GPLv2 or ask"). Per the owner's decision (2026-10-28) it may
  be **read for facts** and nothing from it is copied: not its code, tables or comments, tests included.
- **[WIN]** What Windows records for the owner's Cameo 4 (read 2026-10-28 with the device absent; see
  [ARCHITECTURE.md › R3](ARCHITECTURE.md)).
- **[SA]** Silhouette's published Cameo 4 specifications, <https://www.silhouetteamerica.com/cameo-4>.
- **[SS]** Silhouette School (a user blog, read 2026-10-30) on the Cameo 4's tools and holders:
  [tools and blades](https://silhouetteschoolblog.com/2019/10/silhouette-cameo-4-tools-blades.html),
  [tools FAQ](https://www.silhouetteschoolblog.com/2019/10/pre-order-cameo-4-tools-faqs-on.html),
  [dual tool carriage](https://www.silhouetteschoolblog.com/2019/06/silhouette-cameo-4-dual-tool-carriage.html),
  [adapters](https://www.silhouetteschoolblog.com/2019/09/silhouette-cameo-4-adapters-explained.html),
  [Studio V5 Send panel, Tool 2](https://www.silhouetteschoolblog.com/2026/03/silhouette-studio-v5-send-panel-tool-2.html).
- **[ISC]** inkscape-silhouette's command list, `Commands.md`, and its pull request
  [#369](https://github.com/fablabnbg/inkscape-silhouette/pull/369) (merged as `f166edc`, 2026-10-08), read
  2026-11-01 for facts only, as [IS].
- **[GM]** Graphtec's GP-GL manual for its MiPlot MP4100–MP4400 pen plotters (Part 1, "GP-GL commands"), a scan
  of the printed manual published at <https://github.com/Snow4DV/graphtec-gp-gl-manual> (no licence stated),
  read 2026-11-04 for facts only: section 1-4, "Circle and curve commands", pages 1-17 to 1-23. Graphtec's own
  description of the language Silhouette machines speak, from before any Silhouette existed; what a Cameo does
  with a command is still **to confirm** on the machine.
- **[GS]** Graphtec's GP-GL command summary for the Craft Robo Pro and CE5000, which Graphtec support sent "as
  is" to the author of <https://www.ohthehugemanatee.net/2011/07/gpgl-reference-courtesy-of-graphtec/> (2011),
  who published it; the PDF survives only in the Wayback Machine. Its curve commands are known here through
  [ISC], which lists them from it; not yet read directly.
- **[HW]** Checked on the owner's Cameo 4 (USB `0x0B4D:0x1137`) on 2026-11-01, steps 1–3 of
  [hardware-validation.md](hardware-validation.md): 10 mm squares scored at 20 and 280 mm across, 20 mm down,
  measured against the 12 × 12 in mat's grid.

Each fact is marked **to confirm** until it has been checked on the owner's Cameo 4 (C4.6). Facts the encoder
relies on most are listed again in [Checks on the machine](#checks-on-the-machine).

## Connection

| Fact | Source |
|---|---|
| USB vendor id `0x0B4D` (Graphtec). Cameo 4 product id `0x1137`; Cameo 4 Plus `0x1138`, Cameo 4 Pro `0x1139`, Cameo 3 `0x112F`, Cameo 5 `0x1140`, Cameo 5 Plus `0x1141`, Portrait 3 `0x113A`, Portrait 4 `0x113F` (the Portrait and Portrait 2, `0x1123` and `0x1132`, take the older, unaddressed commands and aren't driven). The Cameo 5 Alpha family uses another vendor id, `0x3844`. | [IS]; Cameo 4 also [WIN] |
| The Portrait 3 and 4 are driven as the Cameo 4 family (holder-numbered settings, speed 1–30, the mat codes) but have one tool holder: no `J` is sent to them. Cut widths 203 mm and 216 mm in the reference; the maker sells an 8 × 12 in mat for the Portrait 3 and gives the Portrait 4 an 8.5 in cut width (<https://www.silhouetteamerica.com/CUT-MAT-LTR-3T>, <https://www.silhouetteamerica.com/portrait-4>), so we take the Portrait 3 as 8 in (203.2 mm). | [IS] (`PRODUCT_LINE_CAMEO4`, `PRODUCTS_WITH_TWO_TOOLS`), [SA] |
| The Cameo 4 presents a USB printer-class interface (class 7, subclass 1, protocol 2: bidirectional); Windows binds it to its own `usbprint.sys`, with no vendor driver. | [WIN] |
| Commands go to the bulk OUT endpoint and replies come from the bulk IN endpoint of interface 0 (`0x01` and `0x82`). Through `usbprint.sys` these are a file's writes and reads. | [IS]; the usbprint mapping is ours, **to confirm** |
| Writes are sent in pieces of at most 4,096 bytes; a careful mode sends 1,024 bytes and waits for the machine to be ready before the next. A write that times out is tried again, up to five times, a second apart. | [IS], **to confirm** |

## Tools and holders

| Fact | Source |
|---|---|
| The Cameo 4 has two holders. Holder 1 (left) is the fast one, about 210 g of force; holder 2 (right) is driven by another kind of motor and presses with up to 5 kg. | [SS] |
| Holder 1 takes the Cameo 4 AutoBlade and the Cameo 4 pen holder, and older Silhouette tools in the four adapters that come with the machine (marked "1": blade/premium, sketch pen/pen holder, deep-cut, 2 mm Kraft blade); the Cameo 3 AutoBlade doesn't fit. | [SS] |
| Holder 2 takes only the 3 mm Kraft blade (thick material: leather, balsa, mat board), the rotary blade (fabric without a stabiliser, crepe paper) and the punch tool (marks what to weed); the 2 mm Kraft blade can go in holder 1 with its adapter. | [SS] |
| Both holders have a tool sensor, as do the Cameo 4's tools and adapters: the machine can tell what is fitted, and the maker's software adjusts its suggested settings to it; the material is still chosen by hand, and for the 3 mm Kraft blade the settings are made by hand. | [SS] |
| The maker's software (V5) gives each cut line a holder (red lines holder 1, blue holder 2) and says "incompatible" when a line's action doesn't suit the tool fitted in its holder. | [SS] |
| The query for what is fitted, and its codes, are under [Status](#status); no code for the punch tool is known (**to confirm**). | [IS] |

## Framing and units

| Fact | Source |
|---|---|
| Each command is ASCII text ending in the ETX byte (`0x03`). | [IS], confirmed [HW] |
| Coordinates are whole numbers of the machine's unit, 20 per millimetre (0.05 mm; 508 per inch). | [IS], confirmed [HW]: a 10 mm square measured 10 × 10 mm |
| The origin is at the top left: one axis grows to the right, the other toward the user (as our mat coordinates do). | [IS], confirmed [HW] |
| On the owner's Cameo 4 with a Nicapa 12 × 12 in mat (not Silhouette's), the origin is **2 mm left of the grid's top-left corner**, level with it: squares sent for 20 and 280 mm across landed 18 and 278 mm from the grid's left line, both 20 mm down. A shift, not a scale. The mat lines up by its plastic edge, so the shift may be the mat's: that mat's grid is 10.8 mm from its left edge, 9.5 mm from its right, 19.4 mm from its top and 20.5 mm from its bottom (overall 324.5 × 344.5 mm, grid 304.8 mm square). A genuine Silhouette mat hasn't been measured. So the encoder takes a user-set origin adjustment (C4.6) rather than a fixed shift per model. | [HW] |
| A coordinate pair is written vertical value first, then a comma, then the horizontal value. | [IS], confirmed [HW] |

## Status

| Fact | Source |
|---|---|
| Status is asked for with ESC followed by ENQ (`0x1B 0x05`). The reply is one character and ETX: `0` ready, `1` moving, `2` no media loaded. | [IS]; ready with the mat loaded confirmed [HW] (the send's ready check passed); an answer asked for while the machine is busy can come late and be read with the next one's (a tools answer `2,0` before a status `0`), [HW] 2026-11-03, so each answer is told by its form |
| ESC followed by NAK (`0x1B 0x15`) asks which tools are in the holders: a comma-separated code per holder (0 empty, 1 ratchet blade, 2 AutoBlade, 3 deep-cut blade, 4 kraft blade, 5 rotary blade, 7 pen, 255 error). | [IS]; 2 (AutoBlade, holder 1) and 0 (holder 2 empty) confirmed [HW] |
| The firmware version is asked for with the `FG` query. | [IS], **to confirm** |
| The machine answers `0` (ready) only once it has cut all it was sent, and `1` while it has work. A sender that waits for `0` before each part of a job therefore stops the machine at the end of each part: a box cut on 2026-11-04 paused mid-job, at the end of the first 4 KB. So only the first part waits for ready (and the parts after a group's pause or a mark search); the rest are written straight after, the machine holding a write back until its buffer has room (USB flow control), so a write may take as long as the cutting that makes room. | [HW] (the pause); straight after, **to confirm** by cutting the box again |

## A job

In the order the encoder sends them. "n" is a whole number.

| Step | Command | Meaning | Source |
|---|---|---|---|
| Initialise | ESC + EOT (`0x1B 0x04`) | resets the machine to take a job | [IS] |
| Mat | `TG` n, then `FN0`, then `TB50` and boundary commands | which mat is loaded; the orientation; the area | [IS] |
| Tool holder | `J1` or `J2` | which holder cuts next (two-holder machines such as the Cameo 4): `J` and the holder's digit, nothing else | [IS] |
| Force | `FX` n`,`h | blade pressure, 1–33 on the Cameo 4, then a comma and the holder h it is for (C10.1) | [IS] |
| Speed | `!` n`,`h | 1–30 on the Cameo 4 family (1–10 on older models); the units of a step aren't given; then the holder | [IS] |
| Depth | `TF` n`,`h | AutoBlade depth 0–10, holder 1 only (not sent for holder 2) | [IS] |
| Acceleration | `TJ` n | 1–3; no holder | [IS] |
| Track enhancing | `FY0` | `FY0` turns it on; `FY1` turns it off on older models, and the Cameo 3 and later are sent nothing when it is off. On, the rollers run the mat three times forward and back before cutting; the reference adds that the machine does it on its own at a force of 19 or more, and that below that it may do nothing (**to confirm**). Until 2026-11-03 the encoder sent `FY1` for on, from a misreading | [IS] (`setup`, the track-enhancing part) |
| Blade offset | `FC` n`,1,`h | the blade's offset in machine units (0.9 mm, 18 units, by default; 0 for a pen), then a second value that is always 1 unit (0.05 mm), then the holder. The machine turns the blade round corners by it, so the paths sent are where the tip cuts (we send each operation's `bladeOffsetMm`, C5.4) | [IS] |
| Corners, lift | `FF` …, `FE` … | corner handling and lift between strokes, each ending in the holder; the reference sends them for every tool (lift off, corners off for a pen), we send neither and leave the machine's own (**to confirm** that it needs none) | [IS] |
| Move | `M` y,x | travel with the blade up | [IS] |
| Cut | `D` y,x | cut a straight line to the point, blade down | [IS] |
| End | a move past the cut, then `SO0` | leave the head out of the way; end of job | [IS] |

On the Cameo 3 and 4 family every tool setting but acceleration and track enhancing carries the holder it is for,
holder 1 included; older one-holder models take the bare forms (`FX` n, `!` n, `FC0` for a pen) [IS, `Graphtec.py`
at the commit above, the tool class and `setup()`]. A tool's settings go in this order: `J`h, `FX`, `!`, `FC`, `TF`
(holder 1 only); the reference also resets acceleration and sends `FC`, `FE` and `FF` once more around a second
`FX`, which we don't (**to confirm** on the machine that the shorter form is taken).

Mat codes for `TG`: 0 none, 1 the 12 × 12 in mat, 2 the 12 × 24 in mat, 3 the Portrait's 8 × 12 in, 8 a 15 × 15
in, 9 a 24 × 24 in, 11 an 8.5 × 12 in [IS]. Media codes 100–138 and 300 (custom) name materials for the machine's
own presets [IS]; the encoder sends settings directly and needs none.

### Curves

The machine is sent straight moves today; GP-GL also has curve commands, which the machine breaks into straight
segments itself. Whether a Cameo 4 or 5 takes them, and how finely it divides them, is **to confirm** with the
curve test (C4.8, [hardware-validation.md](hardware-validation.md)).

| Fact | Source |
|---|---|
| The driver sends only straight moves: `M` (blade up) and `D` (blade down) to each point of a path already flattened; it never sends a curve or circle. Ours does the same, to the document's tolerance (0.025 mm at first). | [IS] |
| **Circle** `W`x,y,r1,r2,θ1,θ2[,d]: a circle, arc or spiral round the centre x,y, from radius r1 at angle θ1 to radius r2 at θ2 (angles in tenths of a degree from the x axis, counterclockwise when θ1 < θ2; a full circle is r1 = r2, 0 to 3600). `d` divides it: above 0 it is the angle of each segment in tenths of a degree, below 0 the number of segments, 0 or left out lets the plotter choose an angle that gives "a smooth circle". | [GM] 1-17, 1-18 |
| **Relative circle** `]`r1,r2,θ1,θ2[,d]: the same, starting from where the tool is (its centre found from the start angle and radius). | [GM] 1-19, 1-20 |
| **Curve** `Y`a,x1,y1,…,xn,yn and **relative curve** `_`a,Δx1,Δy1,…: a smooth cubic curve *through* the points (it interpolates them; it is not a Bézier), `a` 0 open, 1 closed; at least three points, none repeated in a row, best evenly spaced; a terminator after the last. A closed curve starts and ends at its second point. | [GM] 1-21, 1-22 |
| **Ellipse** `)`a,x,y,r1,r2,θ1,θ2,θ3[,d]: `a` 0 goes to the start with the tool up, 1 with it down; centre x,y; the major and minor radii; start and end angles; θ3 the major axis's angle to the x axis; `d` as for `W`. | [GM] 1-23 |
| On the plotter, "during the cutter mode" curves are processed differently according to a DIP switch (S2-8); the manual doesn't say how. A cutter may treat curves otherwise than a pen plotter does (for instance in how it turns the blade round them). | [GM] 1-20 |
| **Bézier** `BZ`a,x1,y1,x2,y2,x3,y3,x4,y4[,d]: a cubic Bézier through four control points (start, two handles, end), listed in Graphtec's Craft Robo Pro / CE5000 summary; also a three-point circle `WP`x1,y1,x2,y2,x3,y3[,d]. The list leaves the meaning of `a` and `d` open. | [ISC] from [GS], **to confirm** |
| `BZ` with `a` = 1 (a cubic) and a trailing 0 was reported "physically tested" on a CAMEO Pro Mark II, firmware V1.09; the Cameo 4 and 5 are expected to take it, untested. The report doesn't say how smooth the result was. Claimed benefits, unmeasured: a smaller USB payload and no stalls from the machine's input buffer running dry. | [ISC] (#369), **to confirm** |
| Silhouette publishes no protocol documentation: asked in 2018, it considered an SDK and in January 2019 decided against it, with no documentation planned. | inkscape-silhouette [issue #72](https://github.com/fablabnbg/inkscape-silhouette/issues/72) |
| Open questions for the machine: whether it takes each command at all; how finely it divides a curve when `d` is left out (coarser than our 0.025 mm would be worse than today); whether its blade offset (`FC`) is applied along native curves as along straight moves; whether motion along a `BZ` is smoother or quicker than along our segments. | our questions, 2026-11-04 |
| Using them would be a choice for paths the job leaves whole: our overcut, clipping to the mat, perforation and dashes work on straight segments, so a path with any of those stays flattened. Our decision of 2026-11-01 ("not planned") is replaced by: decide after the curve test. | our decision, 2026-11-04 |

## Sizes

| Machine | Cut area, mm | Source |
|---|---|---|
| Cameo 4 | 304.8 wide, up to 3,000 long | [IS]; 12 in wide and 12 × 24 in "With a Cutting Mat" in [SA] |
| Cameo 4 Plus | 372 × 3,000 | [IS] |
| Cameo 4 Pro | 600 × 3,000 | [IS] |
| Cameo 5 | 330.2 × 3,000 | [IS] |

The Cameo 4 width agrees with our mat presets (`vectorcraft_cutjob::MAT_PRESETS`).

## Print & Cut: registration marks

The machine's optical sensor finds marks printed on the sheet, then cuts relative to them. Sources:
**[IS]** `silhouette/Graphtec.py` (as above) for the commands, and **[IS-R]** `render_silhouette_regmarks.py`
at the same commit ([`f376a083`](https://github.com/fablabnbg/inkscape-silhouette/blob/f376a0836b53826e57f50d7ec44658663e629f05/render_silhouette_regmarks.py))
for what the marks look like. User tutorials on the maker's software, Silhouette Studio, read 2026-11-03:
**[SEC]** Silhouette Secrets, [v4 registration marks](https://silhouette-secrets.com/2018/01/16/lets-explore-v4-registration-marks/)
(2018, updated 2023) and [v4.3 registration marks](https://silhouette-secrets.com/2020/01/21/lets-explore-registration-marks-v4-3/)
(2020); **[SS]** Silhouette School on [moving the marks](https://silhouetteschoolblog.com/2023/03/how-to-change-position-of-registration.html),
[the four-point marks](https://www.silhouetteschoolblog.com/2026/01/silhouette-registration-mark-types.html) and
[Print Bleed](https://silhouetteschoolblog.com/2018/08/silhouette-studio-print-bleed-tool-v42.html). **[ST]**: the
Print & Cut panel of Silhouette Studio, in screenshots the owner took on 2026-10-31 (seen, not kept), now only for
what those don't give (R2). All **to confirm** on the Cameo 4.

| Fact | Source |
|---|---|
| The usual mark set has three marks, all black: a filled square 5 mm a side at the top left, and L-shaped corners at the top right and bottom left, each arm 20 mm long and drawn 0.3 mm wide (wider lines were reported to register worse), the arms pointing in toward the page. There is no mark at the bottom right. A four-corner variant has L corners in all four corners. | [IS-R] |
| Each mark's anchor is its outer corner: the square's top-left corner, the L's corner. The top-left anchor is the origin; the top-right one is the mark width to its right and the bottom-left one the mark length below it (anchor to anchor). Left at their defaults, the marks are 10 mm in from each page edge and the width and length are what remains of the page. | [IS-R] |
| Nothing else is printed close to the marks: a clear (white) area 20 mm wide is kept round them. | [IS-R] |
| Before the search the driver sends, each ending in ETX: `TB50,0` (not landscape); `TB99`; `TB52,2` (the mark type: 2 is the Cameo's portrait marks, 0 the original/SD ones); `TB51,400` (mark length) and `TB53,10` (mark line width), both raw numbers whose units the source doesn't give; `TB55,1`. | [IS] |
| The automatic search is `TB123,h,w,t,l`: the length (top-left to bottom-left anchor), the width (top-left to top-right), and where to start looking, from the top and from the left, all in machine units (20 a mm). The driver starts looking 10 mm before the expected origin on each axis (never below 0). `TB124` with the same values is the four-corner search on the models with it (not the Cameo 4); `TB23,h,w` is a manual registration. | [IS] |
| The answer to the search is read with a long timeout (40 s): success is four spaces, `0`, ETX. Anything else, or no answer, means the marks weren't found and the job must not be cut. | [IS] |
| After registration the machine's origin is the top-left mark's anchor: cut positions are measured from it, and the cut area is the rectangle the marks span. | [IS] |
| The driver's defaults put the origin 15 mm from the left and 20 mm from the top, with marks 180 mm apart across and 230 mm down; the mark drawing's defaults are 10 mm from each edge. Ours come from the marks we draw (C6.1): whatever is printed is what is searched for. | [IS], [IS-R] |
| The maker's software lets the marks be set: **Length**, **Thickness** and **Inset** (all three marks at once), and under Advanced Options each edge's inset; Restore Defaults puts them back. Its defaults are a length of 0.787 in (20 mm), an inset of 0.625 in (15.88 mm) and a thickness of 0.50 (the post says "in"; read as mm, half a millimetre, since half an inch would be wider than the marks are long). The marks can be placed **Inverted**, on the opposite corner of the mat. | [SEC], [SS] |
| Its least and greatest values, **Length** 5–20 mm, **Thickness** 0.3–1 mm, **Inset** 10–84 mm; More Settings' edges named left, top, right and bottom; inverted meaning the set turned half round, the square at the bottom with the sheet fed the other way. No public source found. | [ST] |
| Those defaults, 20 mm and 0.5 mm at 20 machine units a mm, are the `TB51,400` and `TB53,10` above, so we take both numbers as the length and line width in those units and send what is drawn (C12.1). | [SEC], [IS] |
| It cross-hatches the areas where the sensor reads the sheet: the hatching itself doesn't print, and a design, cut line or background in it can stop the marks being read. A shorter or thinner mark makes the hatched area smaller; the four-point marks of newer models add a mark and a hatched area at the bottom right. | [SEC], [SS] |
| Where exactly the hatching goes: from each mark out to the page's edges at its corner and a little past the mark's arms (about 1–2 mm, measured off the screen), and a band across the whole page along the top (the square's side), down to the foot of the top marks' areas; in the inverted set the band is along the bottom; nothing marks the left, right or bottom margins. No public source found. | [ST] |
| It also offers a **Print Bleed** (since version 4.2; first under Page Setup, now under Registration Marks): a little printed colour spread past the cut line, the cut line staying where it is, the distance adjustable. | [SS] |
| Print Bleed's default distance, 1.27 mm (0.05 in). No public source found. | [ST] |

## Mapping from the cut job

How `vectorcraft-cutjob`'s machine-neutral settings become these commands (the encoder's choices, C4.4):

- **Coordinates:** mat mm × 20, rounded, written vertical first.
- **Force** 0–1 → `FX` 1–33, linearly, rounded, clamped.
- **Speed** mm/s → `!` 1–30. The source gives no unit for a speed step; the encoder takes one step as 10 mm/s
  and warns that the speed is approximate. **[HW] 2026-11-03** (hardware-validation step 5): a 100 mm line took
  10 s at 1 and 1 s at 10 on the Cameo 4, so 10 mm/s a step holds from 1 to 10; at 30 it took about 0.6 s, too
  short a line to show the top speed.
- **Passes:** the encoder repeats each path's strokes, as the simulator does.
- **Overcut:** the encoder extends closed paths, as the simulator does.
- **Score:** the same commands at the score operation's force and speed.

## Checks on the machine

To do when the Cameo 4 is attached (C4.2 and C4.6), each on a scrap sheet:

1. ~~The status query answers through `usbprint.sys` (opening the device interface, no driver change).~~ Done
   2026-11-01.
2. ~~A 10 mm square drawn with a pen in holder 2, if one is at hand, or cut at the lightest force: which way each
   axis goes, and whether the pair is vertical first.~~ Done 2026-11-01: right as written.
3. ~~Its size, measured: 10.0 mm at 20 units per mm.~~ Done 2026-11-01: exact; the origin 2 mm left of the grid
   on a Nicapa mat, set as an origin adjustment.
4. A 100 mm line at speed settings 1, 10 and 30: the time each takes, for the speed mapping.
5. The end of the job leaves the head where expected and the machine ready for the next.
