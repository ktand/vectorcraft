# Cutting with VectorCraft: user guide

> **For:** anyone using this fork to prepare and cut designs on a Silhouette cutter · **Machines:** the Cameo 4
> family first (Cameo 3, 4, 4 Plus, 4 Pro, 5, 5 Plus and Portrait 3, 4 are recognised) · **Status:** written 2026-10-30, before the
> first cut on a real machine: some steps may change once the [hardware checks](hardware-validation.md) are done

VectorCraft can send a design straight to a cutting machine. This guide walks through it in the order you'd
usually work: set up the mat, choose what is cut and how, make cut lines, arrange the pieces, check the job,
send it.

## 1. The Cutting workspace

Window › Workspace › **Cutting**. It turns on Cutting View (the mat drawn behind your artboard) and arranges the
panels on the right:

- **Layers** at the top. Cutting by layer is set up here.
- **Cut Settings** below it: what you change for each cut, and Send to Cutter….
- Beside them, a strip of icons, one for each feature: **Cutting Mat** first, then **Contour**, **Stencil**,
  **Weeding**, **Sketch Fill**, **Rhinestones**, **Puzzle**, **Pop Up**, **Templates**, **Tiles**, **Nesting** and **Print & Cut**. Click an icon to
  open that feature's panel beside it, and again (or anywhere else) to close it. Hover an icon for its name.

If your Cutting workspace looks different (you moved its panels, or saved it with an older version), Window ›
Workspace › Reset Cutting brings back this layout. Any panel can also be dragged out of the strip and docked open.

The panels (each also on the Window › Cutting menu):

- **Cut Settings:** how things are cut. At the top, a summary of the job ("2 paths · 392 mm · 5 s") and **Send
  to Cutter…**. Below it: what the selection is cut as and along, the default, groups, and **Advanced**. The
  material and tool settings are in the Send window, beside the test cut that tunes them (section 4).
- **Contour**, **Stencil** and **Weeding:** what adds cut lines to cut the art out: a contour round it, stencil
  bridges, weeding's box and lines (section 5).
- **Sketch Fill**, **Rhinestones**, **Puzzle** and **Pop Up:** designs made from the art: lines for the pen, a
  rhinestone template, a puzzle, a pop-up card (section 5).
- **Templates:** boxes and other nets made to the size you give, ready to cut and fold (section 5).
- **Cutting Mat:** the mat and where the artboard sits on it. **Tiles** cuts a design larger than the mat in parts,
  **Nesting** packs the pieces onto it (section 2).
- **Print & Cut:** printing with registration marks and cutting round the print.

The line above Cut Settings can be dragged to give it or Layers more room.

## 2. Setting up the mat

In the **Cutting Mat** panel:

- **Mat:** Silhouette 12 × 12 in, 12 × 24 in, the Portrait's 8 × 12 in, 8.5 × 12 in, 15 × 15 in and 24 × 24 in
  (the last two for the wider models; a mat wider than your machine is refused when you send), or Custom (type
  its size). A **≈** beside it means the cut area
  was measured rather than taken from the maker's specifications: hover it, and check the area on your own mat.
- **Roll (no mat):** for vinyl fed straight into the machine from the roll. Set the roll's **Width** and the
  **Length** to cut (up to 3,000 mm); the canvas shows a strip instead of a mat, and the machine is told there
  is no mat. What the machine does at the end (feeding the material out, cutting it off) is still to be checked
  on the machine.
- **Orientation:** Portrait or Landscape, as the mat goes into the machine.
- **Artboard:** with more than one artboard, which one goes on the mat.
- **Offset:** moves the artboard on the mat, in mm from the cut area's top-left corner.
- **Clip to Mat:** lines that run off the cut area are cut up to its edge. Without it, a job that runs off the
  mat isn't sent.

If the artboard runs off the mat, the panel says so in red. It says nothing while all is well.

### Designs larger than the mat: tiles

**Tiles** cuts a design too large for the mat in parts, each on its own sheet:

1. Select the design (or nothing, for the whole artboard) and set **Overlap**: how far neighbouring tiles overlap,
   so the pieces can be joined (default 5 mm).
2. Press **Tile onto Mats**. Artboards named Tile 1, Tile 2… are added over the design, each the size the mat can
   cut, row by row from the top left. **Clip to Mat** is turned on and the cutting artboard set to Tile 1. Undo
   takes it all back.
3. Cut Tile 1, then choose Tile 2 as the **Artboard**, load the next sheet and cut again, and so on. Each tile cuts
   what lies on it, up to its edges.

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
| **Perforate** | Cuts in dashes, so the sheet tears along the line: the object's own dash pattern if its stroke is dashed, else 3 mm cuts with 1 mm gaps (Cut Settings › Advanced › Perforate). |
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

**Groups** › By Layer, By Line Colour or By Fill Colour cuts the job in groups: each top-level layer, each stroke
colour (else fill colour), or each fill colour (else stroke colour), one after the other. The machine **pauses
before each group** after the first: change the blade or the material, then press **Continue** in the Send window.
Each group's row has **On** (switch it off and it isn't cut) and **Pause** (switch it off and the group is cut
straight after the one before, with its own settings).

While cutting in groups, Cut Settings lists each group in cutting order. Give a group its own **Material**, or its
own **Force** and **Passes**. A group left on "Document's setup" uses the tool settings below. When a group has a
material, the pause says what to load ("Paused before Gold. Load Cardstock, then Continue.").

**Knockout** (shown once the job is cut in groups) is for layered vinyl, heat transfer vinyl above all: each colour
is cut only where it shows, the colours painted above it taken out of it, so the layers don't pile up thick. The
**Overlap** beside it (default 0.5 mm) is how far a lower colour still runs under the edge of the one above, so no
gap shows when the layers are pressed a little off. The document doesn't change, only what's cut; a colour that
is wholly hidden isn't cut, and the Send window says so.

**Align Marks** (also shown once the job is cut in groups) cuts a small cross in the same place on every colour,
just outside the design's corner. Weed the cross with the design; when pressing or applying, lay each colour's
cross exactly over the one before, and the layers line up.

## 4. Tools and materials

### The tool table

In the Send window (**Send to Cutter…**), under **Tool Settings**: the **Material**, then a table with a **Cut**
column and a **Score** column, then **Cut Mirrored**. Hover a row's name for its range. They are there, not in Cut Settings, because
they belong to what is on the mat today: test, adjust, test again, then send, without leaving the window.

| Row | What it sets |
|---|---|
| **Tool** | Which tool cuts. It decides the holder: see below. |
| **Force** | Blade pressure, in the machine's own steps (1–33 on the Cameo 4). |
| **Speed** | In the machine's own steps (1–30). |
| **Passes** | How many times each line is cut (1–10). |
| **Overcut** | How far a closed shape's cut runs on past its start, so it lifts out cleanly. |
| **Blade depth** | The AutoBlade's depth (1–10; 0 leaves the machine's own). Other tools show "—". |

**Cut Mirrored** cuts the design flipped left to right, for material cut from the back such as heat transfer
vinyl (iron-on). Your art stays as you drew it; Show Toolpaths shows the mirrored lines. Print & Cut is never
cut mirrored (the marks are read as printed): print the art mirrored instead.

The Send window's **Advanced** (closed at first, under the table) holds what most cuts leave alone: **Accel.**
(gentler or faster moves), **Corners** (the blade runs a little past each sharp corner), **Blade offset** (how far
the blade's tip trails its axis; the machine turns the blade round corners with it; a pen shows "—") and **Track
Enhancing**. Cut Settings' own **Advanced** holds the design's: **Perforate** (the cut and gap lengths for
objects set to Perforate) and **Cut Order**.

**Cut Order** decides the order the lines are cut in. Scoring always comes before cutting, and with the first two
choices a shape inside another (the counter of an O) is cut before it, so the piece around it stays held:

- **Shortest Travel** (at first): the blade's moves between lines kept short.
- **Fewest Roller Moves:** the mat is cut in bands 30 mm deep from the top, each finished before the next, so
  the rollers take the material one way instead of back and forth. Good for long designs and for thin material
  that the rollers might shift or crease.
- **As in the Document:** the order the objects are stacked in, bottom first.

After the last line the head goes home, to the top left (with Print & Cut, to the top-left mark), and the mat
stays loaded.

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

Some materials are cut from the back, such as **Heat Transfer Vinyl** (iron-on): choosing one turns **Cut
Mirrored** on, and choosing another turns it off. When you press **Send** with such a material, a dialog asks
**Cut Mirrored** (the default, also Enter) or **As Is** before anything moves; **Cancel** sends nothing. It
doesn't ask with Print & Cut (never cut mirrored) or for a test cut. A material you save while Cut Mirrored is
on is marked as cut from the back too.

Your materials are kept in `%APPDATA%\VectorCraft\Cutting\Materials.json` and are there in every document.

### The test cut

In the Send window, **Test at** (a corner of the mat) and **Test Cut** cut a 10 mm square with a triangle inside
it, 3 mm in from that corner. The test uses the Cut column's settings, or a group's own setup. If the design is in
that corner, the window warns before cutting.

To tune a material: test, then raise or lower Force until the triangle lifts out cleanly without cutting the
backing. Then **Save as Material…** (or **Update**).

**Origin Adjustment** (under the test cut, closed at first): if the cuts land a little off the mat's grid, the
same amount everywhere, move them back. Cut a test, measure from the grid's corner where it landed against where
the preview shows it, and set the difference: **Across** + moves the cuts to the right, **Down** + toward you
(at most 10 mm each way, in steps of 0.1 mm). It belongs to your machine and mat, not to the document, so it is
kept with your preferences and used for every send until you change it; the heading shows it while it isn't 0.
A mat that isn't Silhouette's may put its grid a little differently: check it when you change mats. With Print &
Cut the machine measures from the marks, so the adjustment only moves where it starts looking for them.

## 5. Making cut lines

### Contour: a cut line round your art

For stickers and shapes printed and cut out. Select the art, then open **Contour**:

- **Offset:** how far out the line runs (default 2 mm). Corners are rounded.
- **Keep Holes:** also cut round the holes inside the art. Off, only the outside is cut.
- **Add Contour:** adds the line.

Pieces closer together than twice the offset share one line. The line goes on a **Cut Contour** layer (thin
magenta, set to Cut), and the art itself is set to No Cut, so only the line round it is cut. Undo takes the
contour away and puts the art's operation back.

### Stencils: bridges

Cut a letter O out of a stencil sheet and its middle falls out with it. **Stencil** › **Add Bridges** keeps such
middles joined to the sheet by narrow bridges (the **Stencil** panel):

- **Bridges:** how wide each bridge is (default 1.5 mm). Wider holds better; narrower shows less.
- **Direction:** **Vertical** puts a bridge above and below each hole, **Horizontal** one either side.

The art is taken as filled shapes, as it is cut: a closed shape counts as filled whatever its paint, an object set
to Cut Along › Stroke Edge counts as its stroke, and open lines add nothing. The bridged
shape, named **Stencil**, is added above the art and is what's cut. The art is hidden and set to No Cut, so you
see what will be cut. Undo takes the stencil away and brings the art back. If the panel says holes were missed
(a bar through a hole's middle doesn't cross the art beside it, as in some odd shapes), try the other direction.

### Rhinestones

A rhinestone template is a sheet with a hole for each stone. Select the art, then open **Rhinestones**:

- **Stone:** the stones' size, SS6 to SS30; the hole is made 0.2 mm larger than the stone. Choose Custom and set
  **Hole** for any other size.
- **Hole:** each hole's diameter (default 3 mm, for SS10 stones).
- **Gap:** the material left between two holes (default 1 mm).
- **Fill Inside:** Outline Only (holes along the outline); Grid (a staggered grid inside too); or Radial (rows
  following the outline inward, ring after ring). Holes that would come too close where a row turns sharply are
  left out. The note under the button says how many holes there are, so you know how many stones you'll need.
- **Add Template:** the holes are added as a path named Rhinestones, and that's what is cut. The art is hidden and
  set to No Cut. Undo brings it back.

### Puzzles

The **Puzzle** panel cuts a jigsaw over the selected art (a printed picture, say), or over the whole artboard
when nothing is selected:

- **Rows** and **Columns:** how many pieces (up to 50 each way).
- **Curvature:** knobs and waves; off, a plain grid of rectangles.
- **Curve:** the knobs' size. **Ripple:** how much each side waves either side of its knob. **Wobble:** how far
  the corners and knobs stray from even, so no two pieces are alike (all 100 % at first).
- **Add Puzzle:** adds the cut lines on top as one path. The art stays as it is, to be printed (Print & Cut).
- With the puzzle selected, **Apply to Selected Puzzle** makes it again with the settings, and **Randomize
  Selected Puzzle** with another layout.

### Pop-up cards

The **Pop Up** panel makes the selected design (text converted to outlines, shapes, or several together) into a
card that folds in half, the design standing up when the card is opened at a right angle:

- **Base Width:** a bar under the design, as a share of its height, so every part stands on one straight fold.
- **Strut Width:** a narrow strip rises from the top of each part of the design to a fold on the back of the card,
  holding it upright.
- **Dash Pitch:** the folds are cut as dashes this long, with gaps as long, so the card bends along them.
- **Margin:** how much larger than the pop-up the card is on each side.
- **Make Pop Up:** adds a group on top: the card's outline, the design's cut (open where it joins the card) and
  the folds, each set to Cut. The design itself is set to No Cut, left to be printed (Print & Cut) or as a guide.
  With the pop-up alone selected, the button reads **Apply to Selected Pop Up** and makes it again with the new
  settings. If the card is larger than the mat's cut area where it lies, the panel says so.

Each selected object stands up as a layer of its own, on a fold of its own: a design lower on the sheet stands
further forward, in front of the ones above it. Group objects (or convert text to outlines as one) to stand them
up together. Designs whose struts would run into each other are refused: move them apart.

Fold the card along its middle fold first, then push each design forward along its folds.

### Templates: boxes made to size

The **Templates** panel makes a flat net you cut out and fold up, at the size you ask for. So far: the **Tuck End
Box** (a card-deck box, both ends tucked in from the back), the **Reverse Tuck End Box** (the bottom tucked in
from the front), the **Open Tray**, the **Tray with Lid** (the lid sized to slide over the tray, with the
**Clearance** you give), the **Shipping Carton (0201)** (flaps that meet in the middle; give the board's
**Thickness**), the **Drawer Box** (a drawer with a finger hole and the sleeve it slides into), the **Gable Box** (a peaked box with a carry handle; its top locks shut) and the **Pillow Box** (curved ends that fold in; **Depth** is how far they bow).
Under **Cards & Envelopes**: an **Envelope** and a **Pointed Envelope** (give the card's size and the **Ease**
round it), a **Place Card**, a **Gift Tag**, an **Earring Card** (with a **Hang Slot** you can switch off) and a
**Gift Card Holder**. Under **Solids**: the **Cube**, **Tetrahedron**, **Octahedron**, **Dodecahedron** and
**Icosahedron** (give the **Edge**), a **Pyramid** and a **Prism** of 3 to 24 sides, a **Cylinder** and a **Cone**
(a **Top Diameter** above 0 cuts it off flat). Solids come out in one piece with a glue tab on every edge that
is cut, the ends of a cylinder or a cone as pieces of their own. Under **Party**: **Bunting** (any number of
pennants, pointed or **Swallowtail**), a **Paper Lantern** and a **Party Hat**.

- **Template:** which one. A picture of its net shows under it (the outline, the folds dashed in blue, the glue
  tabs shaded), with the net's size, and changes as you change the sizes.
- **Length**, **Width**, **Height:** the inside sizes, what has to fit in it.
- **Thickness:** the material's. Each fold takes up some of it, so the panels are made slightly larger for what
  you put in to fit. Turn on **Outside Sizes** to give the finished size instead.
- **Glue Tab:** how far each corner's tab reaches along the wall it is glued inside.
- **Size to Selected Art:** with art selected (a label, a picture), the template's face (a box's front, a tray's
  base, an envelope's or a card's face) is made as large as the art plus 2 mm all round, and the net is placed
  so that face lies under the art: print it with Print & Cut, then cut, fold and glue. Solids have no one face to
  size.
- **Folds:** **Score** (the default) or **Perforate**, cut in dashes with the perforation settings, for card too
  stiff to score or a fold that should also tear.
- **Add Template:** adds a group, centred on the artboard: the outline set to Cut, the folds set to Score or
  Perforate (in a blue of their own, so cutting by colour can send them to a pen or an embosser instead), and the glue tabs
  shaded and not cut. With the template alone selected, the button reads **Apply to Selected Template** and makes
  it again where it is with the new sizes. If the net is larger than the mat's cut area, the panel says so.

Fold up along the scored lines, then glue each tab inside the wall next to it.

**Papercraft from a 3D model.** Under the templates, set the **Model Size** (the model's longest side) and click
**Papercraft from 3D Model…** to choose an `.obj` or `.stl` file. The model is unfolded into flat pieces laid out
in rows the mat's width: the outlines are cut, the **Mountain Folds** (blue) and **Valley Folds** (green) scored,
and the glue tabs shaded. Every edge that's cut has a small number printed on both sides: glue the tab on one to
the matching number. Fold mountains away from you and valleys toward you, with the printed side out. Large
models with many faces come out in many small pieces; simplify a model to a few hundred faces first (in a 3D
program) for something you'd want to build. Models of more than 2,000 faces are refused.

### Sketch fill: shading for the pen

A pen draws lines, not areas. The **Sketch Fill** panel fills the selected shapes with lines for the pen to draw:

- **Pattern:** Lines (straight lines across); Concentric (rings following the outline inward); or Spiral (one line
  from the middle out).
- **Spacing:** how far apart the lines, rings or turns are (default 2 mm).
- **Angle:** the lines' slope (default 45°; Lines only).
- **Cross-Hatch:** a second set of lines across the first (Lines only).
- **Add Sketch Fill:** adds the lines as a path just above the art. Holes are left empty.

To draw them, choose the **Pen** as the tool in Cut Settings (or cut by colour, with the lines in a colour of their
own and the pen in that group's setup).

### Weeding

In the **Weeding** panel, for vinyl and heat-transfer material, where the waste around the design is peeled away:

- **Weed Box:** a box cut round the design, **Margin** mm out.
- **Weed Lines:** lines from the box in towards the design, stopping just short of it, so the waste comes off in
  strips. **Every** sets their spacing (0: one from the middle of each side).

When cutting in groups, each group gets its own box and lines.

## 6. Arranging the pieces: nesting

The **Nesting** panel packs pieces close together on the mat by their real shapes, not their bounding boxes,
so a small piece can sit in a larger one's hole:

- **Gap:** the least space between pieces.
- **Turn Pieces:** **Quarter Turns** (the usual) lets each piece be turned by 90° steps where that fits better;
  **Every 15°** tries more angles, which packs odd shapes (stars, leaves, crescents) tighter but takes longer;
  **Off** keeps every piece as drawn.
- **Onto New Sheets:** what doesn't fit goes onto new artboards ("Sheet 2", "Sheet 3", …) to the right, each cut
  on its own.
- **Copies:** Replicate: this many more of each piece are made and nested with them, to fill a sheet with
  stickers or labels. Copies that don't fit are left out, or with Onto New Sheets go onto more sheets.
- **Nest Selected** / **Nest All:** each top-level object or group is one piece. Pieces too large for the mat stay
  where they were.

With more than one sheet, the Send window shows "Sheet 1 of 3", with **Previous Sheet** and **Next Sheet**. Cut a
sheet, load the next, go to the next sheet, Send.

## 7. Print & Cut

To print a design and then cut round the print:

1. In the **Print & Cut** panel, tick **Print & Cut**, then **Add Marks**. Registration marks are added round the artboard on a
   locked layer of their own. Their settings, best left as they are:
   - **Mark length** (5–20 mm, default 20) and **Thickness** (0.3–1 mm, default 0.5);
   - **Inset**, how far in from the edges they go (10–84 mm, default 15.88), or under **Each Edge** one for each
     side;
   - **Restore Defaults** puts the usual marks back.

   The marks follow the artboard: change its size, or a mark setting, and they are drawn again (one undo puts
   back both). A sheet printed before that has to be printed again.
2. Print the artboard at 100 % (no "fit to page").
3. Put the sheet on the mat at its top-left corner, the square mark at the top left. Load it.
4. Send. The machine finds the marks, then cuts where the print is. If it can't find them (a sheet turned round,
   marks too faint), the job stops before anything is cut.

**Print bleed.** A cut a little off the print leaves a thin white edge. In the Print & Cut panel, **Print
Bleed** › **Add Bleed** spreads each printed object's outer colour (its outline's colour, else its fill's) past
its edge, by **Bleed** mm (default 1.27), on a locked layer at the bottom called Print Bleed. Behind the art only
the spread shows, and it isn't cut. The bleed follows the art: move or change it and the bleed is made again (one
undo puts back both). A thin outline (narrower than the bleed) doesn't colour it: the fill's colour is spread. An
image bleeds its own edge pixels, stretched outward, inside its clipping mask when it has one. With more than 200
printed objects the bleed isn't remade on every change: the job says when to press **Update Bleed**.

**Keep the hatched areas clear.** With Print & Cut on, the page shows hatched areas: a band across the top, down
to the marks' line, and a square round each mark. The machine's sensor looks there for the marks, so anything printed or cut there can stop it
finding them. The job warns about each object that comes into one, and nesting keeps pieces out of them.

A contour (section 5) is the usual cut line for printed stickers.

## 8. Checking the job before you cut

- **Cutting View** (View menu): the mat behind the art, with a millimetre grid.
- **Show Cut Border** (View menu, on at first): what can be cut, as a thin red line: between the marks for Print
  & Cut, otherwise the mat's cut area.
- **Show Toolpaths** (View menu): the job's lines in cutting order, numbered, with the moves between them dashed.
- **Playback:** the **Play**, **Previous** and **Next** buttons at the foot of the canvas play the blade's run, ten
  times faster than the machine. They step from line to line, or open a slider to scrub along it. **Done** puts
  it away.

The summary at the top of Cut Settings gives the number of lines, the cut length and the estimated time.
Warnings show there and in the Send window, such as lines off the mat.

### Slow details

Cut Settings › Advanced › **Slow Details**: paths no larger than the size given (default 10 mm) either way, the
letters of small text, a star's points, are cut at the slower speed beside it (default 30 mm/s), the rest at
full speed. Fine vinyl lifts and tears less in tight curves cut slowly.

### What it takes and costs

Under the job's numbers the Send window says how much material the job takes (a sheet on a mat; on a roll,
the length to the end of the last path and 10 mm more) and, when the material has a price, what that costs.
Give one of your own materials a **Price** in Cut Settings, per sheet or per metre (a roll's length gets 10%
added for waste). A built-in material can't be priced: save it as your own first. Each group of a job cut in
groups counts its own material.

### What's left of a roll

Cutting from a roll (mat **Roll**), tick **Roll Left** under one of your own materials in Cut Settings and give
the length left on the roll. Each send takes off what it cuts (when it starts, so a cancelled send counts), and
the Send window warns when a job needs more than is left.

### Pressing and the job sheet

For heat transfer vinyl, tick **Pressing** under one of your own materials in Cut Settings and give the
heat press's temperature and time and when to peel the carrier. **Job Sheet…** in the Send window saves a page
(SVG, to print) listing each group in cutting order with its material, cut settings and pressing.

### Checks before cutting

The job is looked over for what usually spoils a cut, and the Send window says so (click a warning to select
the object): an open path set to Cut (it cuts a slit, not a piece), pieces or holes 2 mm or smaller (hard to
weed), the same line twice (two copies on top of each other cut through the backing), parts narrower than
0.5 mm (the blade tears them), and an object whose pieces overlap (script letters that cut through each other:
set it to Cut Outline to weld them). They are warnings: the job can still be sent.

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
| The artboard goes off the mat | Move it with **Offset**, choose a larger mat, turn on **Clip to Mat**, nest the pieces, or cut it in **Tiles**. |
| A tool warning in the Send window | Fit the tool the job needs in the holder it names, or choose the fitted tool in the Tool row, then **Check Tools**. |

The app's log is in `%APPDATA%\VectorCraft\logs\`. With a failed send's report from
`%APPDATA%\VectorCraft\Cutting\Reports\`, it's what a bug report needs.
