# Silhouette cutters: protocol notes

> **Status:** facts gathered for the encoder (C4.3), not yet checked on a machine · **Written:** 2026-10-28 ·
> **Scope:** Cameo 4 first; other models noted where the source gives them

These are facts about how a computer talks to a Silhouette cutter: what it sends, what it gets back, and in
which units. They are written in our own words, with the source of each. Encoders are written from this page,
not from the sources.

**Sources.**

- **[IS]** inkscape-silhouette, `silhouette/Graphtec.py` at commit
  [`f376a083`](https://github.com/fablabnbg/inkscape-silhouette/blob/f376a0836b53826e57f50d7ec44658663e629f05/silhouette/Graphtec.py)
  (2026-10-13). It is GPL-2.0 ("Distribute under GPLv2 or ask"). Per the owner's decision (2026-10-28) it may
  be **read for facts** and nothing from it is copied: not its code, tables or comments, tests included.
- **[WIN]** What Windows records for the owner's Cameo 4 (read 2026-10-28 with the device absent; see
  [ARCHITECTURE.md › R3](ARCHITECTURE.md)).
- **[SA]** Silhouette's published Cameo 4 specifications, <https://www.silhouetteamerica.com/cameo-4>.

Each fact is marked **to confirm** until it has been checked on the owner's Cameo 4 (C4.6). Facts the encoder
relies on most are listed again in [Checks on the machine](#checks-on-the-machine).

## Connection

| Fact | Source |
|---|---|
| USB vendor id `0x0B4D` (Graphtec). Cameo 4 product id `0x1137`; Cameo 4 Plus `0x1138`, Cameo 4 Pro `0x1139`, Cameo 3 `0x112F`, Cameo 5 `0x1140`, Cameo 5 Plus `0x1141`. The Cameo 5 Alpha family uses another vendor id, `0x3844`. | [IS]; Cameo 4 also [WIN] |
| The Cameo 4 presents a USB printer-class interface (class 7, subclass 1, protocol 2: bidirectional); Windows binds it to its own `usbprint.sys`, with no vendor driver. | [WIN] |
| Commands go to the bulk OUT endpoint and replies come from the bulk IN endpoint of interface 0 (`0x01` and `0x82`). Through `usbprint.sys` these are a file's writes and reads. | [IS]; the usbprint mapping is ours, **to confirm** |
| Writes are sent in pieces of at most 4,096 bytes; a careful mode sends 1,024 bytes and waits for the machine to be ready before the next. A write that times out is tried again, up to five times, a second apart. | [IS], **to confirm** |

## Framing and units

| Fact | Source |
|---|---|
| Each command is ASCII text ending in the ETX byte (`0x03`). | [IS], **to confirm** |
| Coordinates are whole numbers of the machine's unit, 20 per millimetre (0.05 mm; 508 per inch). | [IS], **to confirm** |
| The origin is the top left of the cut area: one axis grows to the right, the other toward the user (as our mat coordinates do). | [IS], **to confirm** |
| A coordinate pair is written vertical value first, then a comma, then the horizontal value. | [IS], **to confirm** — easy to get backwards; the first cut test shows it |

## Status

| Fact | Source |
|---|---|
| Status is asked for with ESC followed by ENQ (`0x1B 0x05`). The reply is one character and ETX: `0` ready, `1` moving, `2` no media loaded. | [IS], **to confirm** |
| ESC followed by NAK (`0x1B 0x15`) asks which tools are in the holders: a comma-separated code per holder (0 empty, 1 ratchet blade, 2 AutoBlade, 3 deep-cut blade, 4 kraft blade, 5 rotary blade, 7 pen, 255 error). | [IS], **to confirm** |
| The firmware version is asked for with the `FG` query. | [IS], **to confirm** |

## A job

In the order the encoder sends them. "n" is a whole number.

| Step | Command | Meaning | Source |
|---|---|---|---|
| Initialise | ESC + EOT (`0x1B 0x04`) | resets the machine to take a job | [IS] |
| Mat | `TG` n, then `FN0`, then `TB50` and boundary commands | which mat is loaded; the orientation; the area | [IS] |
| Tool holder | `J1` or `J2` | which holder cuts next (two-holder machines such as the Cameo 4) | [IS] |
| Force | `FX` n | blade pressure, 1–33 on the Cameo 4 | [IS] |
| Speed | `!` n | 1–30 on the Cameo 4 family (1–10 on older models); the units of a step aren't given | [IS] |
| Depth | `TF` n | AutoBlade depth 0–10, holder 1 only | [IS] |
| Acceleration | `TJ` n | 1–3 | [IS] |
| Track enhancing | `FY0` / `FY1` | one turns it on, the other off; which is which isn't stated (**to confirm**; the encoder sends `FY1` for on) | [IS] |
| Blade offset | `FC` n | the blade's offset in machine units (0.9 mm, 18 units, by default); `FC0` for a pen. The machine turns the blade round corners by it, so the paths sent are where the tip cuts (we send each operation's `bladeOffsetMm`, C5.4) | [IS] |
| Corners, lift | `FF` …, `FE` … | corner handling and lift between strokes | [IS] |
| Move | `M` y,x | travel with the blade up | [IS] |
| Cut | `D` y,x | cut a straight line to the point, blade down | [IS] |
| End | a move past the cut, then `SO0` | leave the head out of the way; end of job | [IS] |

Mat codes for `TG`: 0 none, 1 the 12 × 12 in mat, 2 the 12 × 24 in mat, 3 the Portrait's 8 × 12 in, 8 a 15 × 15
in, 9 a 24 × 24 in, 11 an 8.5 × 12 in [IS]. Media codes 100–138 and 300 (custom) name materials for the machine's
own presets [IS]; the encoder sends settings directly and needs none.

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
for what the marks look like. All **to confirm** on the Cameo 4.

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

## Mapping from the cut job

How `vectorcraft-cutjob`'s machine-neutral settings become these commands (the encoder's choices, C4.4):

- **Coordinates:** mat mm × 20, rounded, written vertical first.
- **Force** 0–1 → `FX` 1–33, linearly, rounded, clamped.
- **Speed** mm/s → `!` 1–30. **Unknown mapping:** the source gives no unit for a speed step. Until measured, the
  encoder maps the job's speed linearly onto the range, warns that the speed is approximate, and C4.6 times a
  known line at several settings.
- **Passes:** the encoder repeats each path's strokes, as the simulator does.
- **Overcut:** the encoder extends closed paths, as the simulator does.
- **Score:** the same commands at the score operation's force and speed.

## Checks on the machine

To do when the Cameo 4 is attached (C4.2 and C4.6), each on a scrap sheet:

1. The status query answers through `usbprint.sys` (opening the device interface, no driver change).
2. A 10 mm square drawn with a pen in holder 2, if one is at hand, or cut at the lightest force: which way each
   axis goes, and whether the pair is vertical first.
3. Its size, measured: 10.0 mm at 20 units per mm.
4. A 100 mm line at speed settings 1, 10 and 30: the time each takes, for the speed mapping.
5. The end of the job leaves the head where expected and the machine ready for the next.
