# Cutting with VectorCraft: user guide

> **For:** anyone using this fork to prepare and cut designs on a Silhouette cutter · **Machines:** the Cameo 4
> family first (Cameo 3, 4, 4 Plus, 4 Pro, 5, 5 Plus are recognised) · **Status:** written 2026-10-30, before the
> first cut on a real machine: some steps may change once the [hardware checks](hardware-validation.md) are done

VectorCraft can send a design straight to a cutting machine. This guide walks through it in the order you'd
usually work: set up the mat, choose what is cut and how, make cut lines, arrange the pieces, check the job,
send it.

## 1. The Cutting workspace

Window › Workspace › **Cutting**. It turns on Cutting View (the mat drawn behind your artboard) and arranges the
panels on the right:

- **Layers** at the top. Cutting by layer is set up here.
- **Cut Settings** and **Cutting Mat**, as two tabs below it.

If your Cutting workspace looks different (you moved its panels, or saved it with an older version), Window ›
Workspace › Reset Cutting brings back this layout.

The two panels:

- **Cut Settings:** what you change for each design and each cut. At the top, a summary of the job ("2 paths ·
  392 mm · 5 s") and **Send to Cutter…**. Below it: what the selection is cut as, the default, groups, weeding,
  contour, the material and tool settings, and **Advanced**.
- **Cutting Mat:** what you set once per document. The mat and where the artboard sits on it, nesting, and Print
  & Cut.

## 2. Setting up the mat

In the **Cutting Mat** tab:

- **Mat:** Silhouette 12 × 12 in, 12 × 24 in, or Custom (type its size). A **≈** beside it means the cut area
  was measured rather than taken from the maker's specifications: hover it, and check the area on your own mat.
- **Orientation:** Portrait or Landscape, as the mat goes into the machine.
- **Artboard:** with more than one artboard, which one goes on the mat.
- **Offset:** moves the artboard on the mat, in mm from the cut area's top-left corner.
- **Clip to Mat:** lines that run off the cut area are cut up to its edge. Without it, a job that runs off the
  mat isn't sent.

If the artboard runs off the mat, the panel says so in red. It says nothing while all is well.

### Is it the right size?

An SVG that gives its size in mm, cm or inches opens at that size. One with no units, or in pixels (most SVGs
from the web and from icon sites), opens with a pixel as 1/72 in, and is cut at that size. Most of them were drawn
at 96 pixels to the inch, so they would cut a third larger than meant.

For such a document, Cut Settings says **Measured in pixels: cut at 72 per inch.** under the job line:

- **Scale from 96 px/in** shrinks the whole document to three quarters (art, line widths, artboards) and measures
  it in mm from then on. Undo puts it back.
- If 72 per inch is what you meant, change the document's units in File › Document Setup instead, and the warning
  goes.

## 3. What gets cut, and how

Every object is cut in one of these ways:

| Operation | What the blade does |
|---|---|
| **Cut** | Cuts along every line. |
| **Cut Outline** | Shapes set to it are joined, and only the outline of the joined shape is cut: overlapping letters come out as one piece. |
| **Score** | A light line that doesn't cut through, for folds. Scores are cut before cuts. |
| **No Cut** | Not cut at all: printed art, notes, guides. |

- **Default** (Cut Settings) is what everything is cut as unless something says otherwise.
- **Selected** appears when something is selected. It sets the selected objects' operation, or **As Its Group or
  Layer** to follow their group or layer. Setting a layer to No Cut turns off everything on it.

### Along the path or round the stroke

A line drawn with a thick stroke is cut down its middle at first: the blade follows the path, whatever the
stroke's width. **Cut Along** (shown under Selected) changes that for the selected objects:

- **Path:** along the path itself. Right for shapes whose edge *is* the path: filled shapes, thin lines.
- **Stroke Edge:** round the edge of what the object paints, its stroke as wide as it is drawn (with its caps,
  corners and dashes) and its fill. A 3 mm line becomes a 3 mm strip, and outlined lettering is cut as it looks.
  An object with no stroke is cut along its path still.

Set it on a layer (select the layer's objects) to change everything on it. It's kept in the document and in SVG
(`cut:edge`).

Inside shapes are cut before the shapes around them, so a letter's counter (the hole in an O) is cut before the
O, while the sheet still holds.

### Cutting in groups

**Groups** › By Layer or By Colour cuts the job in groups: each top-level layer, or each stroke colour (else fill
colour), one after the other. The machine **pauses before each group** after the first. Change the blade or the
material, then press **Continue** in the Send window.

While cutting in groups, Cut Settings lists each group in cutting order. Give a group its own **Material**, or its
own **Force** and **Passes**. A group left on "Document's setup" uses the tool settings below. When a group has a
material, the pause says what to load ("Paused before Gold. Load Cardstock, then Continue.").

## 4. Tools and materials

### The tool table

Under **Tool Settings**, a table with a **Cut** column and a **Score** column. Hover a row's name for its range.

| Row | What it sets |
|---|---|
| **Tool** | Which tool cuts. It decides the holder: see below. |
| **Force** | Blade pressure, in the machine's own steps (1–33 on the Cameo 4). |
| **Speed** | In the machine's own steps (1–30). |
| **Passes** | How many times each line is cut (1–10). |
| **Overcut** | How far a closed shape's cut runs on past its start, so it lifts out cleanly. |
| **Blade depth** | The AutoBlade's depth (1–10; 0 leaves the machine's own). Other tools show "—". |

**Advanced** (closed at first) holds what most cuts leave alone: **Accel.** (gentler or faster moves), **Corners**
(the blade runs a little past each sharp corner), **Blade offset** (how far the blade's tip trails its axis; the
machine turns the blade round corners with it; a pen shows "—"), and **Track Enhancing**.

### Tools and holders

The Cameo 4 has two tool holders:

| Holder | Tools |
|---|---|
| **1 (left)** | AutoBlade (comes with the machine), Pen, and older Silhouette tools in their adapters: Ratchet blade, Deep-cut blade, Kraft 2 mm |
| **2 (right, much more force)** | Kraft 3 mm (leather, balsa, chipboard), Rotary blade (fabric), Punch tool (marks what to weed) |

Choose the tool in the **Tool** row; the holder follows from it. When one is a holder 2 tool, the panel says which
holder each column uses. The Cameo 3 can't use holder 2's tools.

The tool belongs to the machine, not the material: choosing a material leaves your tool as it is.

### Materials

**Material** picks a set of cut and score settings. The built-in ones (Paper (light), Cardstock, Vinyl, …) are
starting points only: make a test cut before trusting them.

- Change a setting after choosing a material and the panel says "Changed since the material was chosen".
- **Save as Material…** keeps the current settings under a name of your own. For a built-in material it offers
  "name (copy)".
- **Update "name"** saves your changes into one of your own materials in one click.
- **Delete** removes one of your own.

Your materials are kept in `%APPDATA%\VectorCraft\Cutting\Materials.json` and are there in every document.

### The test cut

In the Send window, **Test at** (a corner of the mat) and **Test Cut** cut a 10 mm square with a triangle inside
it, 3 mm in from that corner. The test uses the Cut column's settings, or a group's own setup. If the design is in
that corner, the window warns before cutting.

To tune a material: test, then raise or lower Force until the triangle lifts out cleanly without cutting the
backing. Then **Save as Material…** (or **Update**).

## 5. Making cut lines

### Contour: a cut line round your art

For stickers and shapes printed and cut out. Select the art, then Cut Settings › **Contour**:

- **Offset:** how far out the line runs (default 2 mm). Corners are rounded.
- **Keep Holes:** also cut round the holes inside the art. Off, only the outside is cut.
- **Add Contour:** adds the line.

Pieces closer together than twice the offset share one line. The line goes on a **Cut Contour** layer (thin
magenta, set to Cut), and the art itself is set to No Cut, so only the line round it is cut. Undo takes the
contour away and puts the art's operation back.

### Weeding

For vinyl and heat-transfer material, where the waste around the design is peeled away:

- **Weed Box:** a box cut round the design, **Margin** mm out.
- **Weed Lines:** lines from the box in towards the design, stopping just short of it, so the waste comes off in
  strips. **Every** sets their spacing (0: one from the middle of each side).

When cutting in groups, each group gets its own box and lines.

## 6. Arranging the pieces: nesting

Cutting Mat › **Nesting** packs pieces close together on the mat by their real shapes, not their bounding boxes,
so a small piece can sit in a larger one's hole:

- **Gap:** the least space between pieces.
- **Turn Pieces:** pieces may be turned by quarter turns where that fits better.
- **Onto New Sheets:** what doesn't fit goes onto new artboards ("Sheet 2", "Sheet 3", …) to the right, each cut
  on its own.
- **Nest Selected** / **Nest All:** each top-level object or group is one piece. Pieces too large for the mat stay
  where they were.

With more than one sheet, the Send window shows "Sheet 1 of 3", with **Previous Sheet** and **Next Sheet**. Cut a
sheet, load the next, go to the next sheet, Send.

## 7. Print & Cut

To print a design and then cut round the print:

1. Cutting Mat › **Print & Cut** on, then **Add Marks**. Registration marks are added round the artboard on a
   locked layer of their own. **Marks inset** sets how far in from the edges they go.
2. Print the artboard at 100 % (no "fit to page").
3. Put the sheet on the mat at its top-left corner, the square mark at the top left. Load it.
4. Send. The machine finds the marks, then cuts where the print is. If it can't find them (a sheet turned round,
   marks too faint), the job stops before anything is cut.

A contour (section 5) is the usual cut line for printed stickers.

## 8. Checking the job before you cut

- **Cutting View** (View menu): the mat behind the art, with a millimetre grid.
- **Show Toolpaths** (View menu): the job's lines in cutting order, numbered, with the moves between them dashed.
- **Playback:** the **Play**, **Previous** and **Next** buttons at the foot of the canvas play the blade's run, ten
  times faster than the machine. They step from line to line, or open a slider to scrub along it. **Done** puts
  it away.

The summary at the top of Cut Settings gives the number of lines, the cut length and the estimated time.
Warnings show there and in the Send window, such as lines off the mat.

## 9. Sending the job

**Send to Cutter…** (top of Cut Settings, or File › Send to Cutter…, **Ctrl+Alt+Shift+X**) opens the Send
window. It doesn't block the rest of the app: you can keep it open beside your art, and drag its corner to make
it larger.

- **Left:** the mat as the machine takes it, with the job's lines on it.
- **Right, from the top:**
  - the job's numbers and any warnings, with its own playback;
  - which sheet, when there are several;
  - the machine. **Find Cutters** looks again after you plug it in.
  - what is in each holder, as the machine reports it ("Holder 1: AutoBlade · Holder 2: empty"), with **Check
    Tools**. The window warns when the job needs a tool that isn't fitted.
  - **Send**, then the progress, **Continue** at a pause, and **Cancel** (the machine finishes the piece it's on);
  - the test cut (section 4);
  - **History** (section 10).

Closing the window doesn't stop a cut. Cut Settings shows how far it has got, and opening the window again shows
it in full.

Before you press Send: load the mat in the machine, and close Silhouette Studio so it doesn't hold the machine.

## 10. History

Your last 50 sends and test cuts are kept. Send window › **History** lists the latest of them: when, which
document, the material, how long it took and how it ended.

- **Use Settings:** puts that send's tool settings back on the document.
- **Send Again:** cuts this document again exactly as that send was, with a fresh sheet loaded. A test cut is cut
  again at the same corner. Shown only for sends of the document that's open.
- **Clear History:** empties the list.

A send that failed also writes a short report to `%APPDATA%\VectorCraft\Cutting\Reports\`: the machine, the
settings and how far it got, with nothing of the art. It's worth passing on when something goes wrong.

## 11. Saving

- **Save as `.vectorcraft`** to keep everything: the mat, the tool settings, groups, weeding, each object's cut
  operation and the contour and marks layers. Older `.drawcraft` files open too.
- **SVG** keeps each object's cut operation (as a `cut:op` attribute), but not the document's mat and tool
  settings.
- Your materials and history are kept with your preferences, not in the document.

## 12. Trying it without a machine

Start VectorCraft with `--virtual-cutter` and a simulated Cameo 4 is listed as "Cameo 4 (simulated)". It takes
jobs, pauses, cancels, answers the tool check (AutoBlade in holder 1) and finds Print & Cut marks, so the whole
workflow can be tried before the machine is plugged in. Nothing is cut, of course.

## 13. When something goes wrong

| What you see | What to do |
|---|---|
| "No cutter is connected" | Check the cable and that the machine is on; close Silhouette Studio; press **Find Cutters**. |
| "load the mat into the cutter first" | Load the mat with the machine's load button, then Send again. |
| "the cutter didn't answer in time" | Switch the machine off and on, press Find Cutters, and try again. If it keeps happening, pass on the report and the log. |
| "the cutter didn't find the registration marks" | Check the sheet's position (square mark at the top left), print at 100 %, and that the marks printed dark. |
| The artboard goes off the mat | Move it with **Offset**, choose a larger mat, turn on **Clip to Mat**, or nest the pieces. |
| A tool warning in the Send window | Fit the tool the job needs in the holder it names, or choose the fitted tool in the Tool row, then **Check Tools**. |

The app's log is in `%APPDATA%\VectorCraft\logs\`. With a failed send's report from
`%APPDATA%\VectorCraft\Cutting\Reports\`, it's what a bug report needs.
