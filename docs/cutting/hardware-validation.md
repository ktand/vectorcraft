# Cutting on the machine: first checks

> **Status:** not yet run · **Machine:** the owner's Silhouette Cameo 4 (USB `0x0B4D:0x1137`) · **For:** C4.2 and
> C4.6 and every task marked "to confirm on the machine" · **Revised:** 2026-10-30 (C9.3: reordered safest
> first, the controls named as the panels show them since C9.1, what to note at each step)

The steps to confirm on a real Cameo 4 what the encoder takes from the
[protocol notes](protocol-silhouette.md) and what Phases 5–8 added, safest first. Use scrap paper and the lightest
settings until step 3 has passed. Each step says what to note; a word or a number is enough. Afterwards, mark the
matching facts in the protocol notes as confirmed (or correct them), or pass the notes on.

Steps 1–3 must pass before the rest; after that, any step can be skipped or done on another day. Where each
control is and what it does: the [user guide](user-guide.md).

## Results at a glance

| Step | What | Pass / fail | Note |
|---|---|---|---|
| 1 | The machine answers | | |
| 2 | Which way the axes go | | |
| 3 | Size | | |
| 4 | Test cut at each corner | | |
| 5 | Speed | | |
| 6 | Blade depth, acceleration, track enhancing, corners | | |
| 7 | Blade offset | | |
| 8 | Cutting in groups, with a pause | | |
| 9 | Each group's own setup | | |
| 10 | Weeding | | |
| 11 | Nesting | | |
| 12 | More than one sheet | | |
| 13 | Print & Cut | | |
| 14 | Job history, Send Again | | |
| 15 | A real cut | | |
| 16 | Tools and holders | | |

## Before you start

1. Connect the Cameo 4 by USB, switch it on, and load a mat with a sheet of scrap paper.
2. Close Silhouette Studio, so it doesn't hold the machine.
3. Start VectorCraft from the repo: `cargo run -p vectorcraft` (a dev build; `cargo run --release -p vectorcraft`
   draws faster but takes longer to build). This run uses your own preferences: the sends are kept in your job
   history, which step 14 looks at.
4. Window › Workspace › **Cutting**. On the right: Layers, then **Cut Settings** and **Cutting Mat** as two tabs.
   If your Cutting workspace looks different (saved before C9.1), Window › Workspace › Reset Cutting.
5. **Send to Cutter…** at the top of Cut Settings (or File › Send to Cutter…, Ctrl+Alt+Shift+X) opens the Send
   window: the mat preview on the left; the job, the machine, **Send**, the test cut and the history on the
   right. It should name the machine ("Cameo 4"); if it says no cutter is connected, press **Find Cutters**. The
   window can stay open beside the art; drag its corner to make the preview larger.

Where things are in Cut Settings: **Selected** (only with a selection: how the selected objects are cut),
**Default** (how everything else is cut), **Groups**, **Weeding**, **Tool Settings** (the material, then a table
with a **Cut** column and a **Score** column: Force, Speed, Passes, Overcut, Blade depth; hover a row's name for
its range), and **Advanced** (closed at first: Accel., Corners, Blade offset, Track Enhancing).

If something goes wrong anywhere: note what the Send window said, word for word. A send that fails writes a
report to `%APPDATA%\VectorCraft\Cutting\Reports\`, and the app's log is in `%APPDATA%\VectorCraft\logs\`; both
are worth passing on.

## Step 1: the machine answers (C4.2)

- File › New. The Send window's **Send** is greyed out (nothing to cut). That's right.
- Draw a rectangle 10 × 10 mm (type the size in Properties) near the top left of the artboard. In Cut Settings
  set **Default** to Score; in the tool table's **Score** column set **Force** to 1 (the lightest step) and
  **Passes** to 1.
- Press **Send**. Expected: the window shows "Sending: …", the machine moves, then "Sent.".
- "load the mat into the cutter first": the status answer works, but the mat isn't loaded. Load it and send again.
- "the cutter didn't answer in time": the status read through `usbprint.sys` needs a look.
- If the machine moves but seems to ignore the force or speed (a very light or a very heavy line whatever is set),
  note it: since C10.1 each setting names its holder (`FX`n`,1`), as the protocol notes say the Cameo 4 takes.
- Under the machine's name the window should say what is in each holder, e.g. "Holder 1: AutoBlade · Holder 2:
  empty". Step 16 checks this in full.

Note: what the window said at the end; did the machine move? What the holders line said.

Result:

## Step 2: which way the axes go

- Look at the scored square on the sheet. It should be where the Send window's preview shows it, the same
  distances from the grid's top-left corner across and down.
- If it is mirrored or the two distances are swapped, the coordinate order (vertical first) is wrong.

Note: where it landed compared with the preview (right / mirrored left–right / mirrored top–bottom / swapped).

Result:

## Step 3: size

- Measure the square: 10.0 mm a side (± 0.3 mm).
- Measure its distance from the grid's corner and compare with the preview (the grid is in inches on a 12 × 12 in
  mat).

Note: the two sides, mm; the distance from the corner, mm, against what the preview shows.

Result:

## Step 4: the test cut at each corner (C8.1)

- A design anywhere on the artboard (the square from step 1 will do). Send window › **Test at** Bottom Right,
  **Test Cut**: a 10 mm square with a triangle inside, cut 3 mm in from that corner of the cut area, the triangle
  first. It uses the Cut column's settings: set **Force** for the paper first.
- Try the other three corners. Each should land where the preview's mat says. At the corner the design is in, the
  window should warn ("lands on the design") before anything is cut.
- From now on, use the test cut to tune each material: the force where the triangle lifts out cleanly without
  cutting the backing is the one to keep (Tool Settings › **Save as Material…**).

Note: did each corner land where the preview showed? The force that cut the paper cleanly.

Result:

## Step 5: speed (for the speed mapping)

- Draw a 100 mm line. Score it with the **Score** column's **Speed** at 1, 10 and 30 and time each with a
  stopwatch (the app takes a step as 10 mm/s until this measures it, so its time estimates are rough).

Note: seconds for the 100 mm at speeds 1, 10 and 30.

Result:

## Step 6: blade depth, acceleration, track enhancing, corners (C5.2)

- With the AutoBlade: **Blade depth** 1 in the Score column, score a line; then 3. The deeper one should mark
  more. (`TF` n)
- Open **Advanced**. **Accel.** 1 and then 3 in the Score column on the same square: 1 should move more gently.
  (`TJ` n)
- **Track Enhancing** on: the rollers should grip differently, or the machine reports it. Which of `FY0`/`FY1`
  turns it on is a guess (`FY1` now): if on and off behave the other way round, note it.
- **Corners** 0.5 mm in the Score column, on a scored square: each corner gets a small spur past it.

Note: for each, yes / no / the other way round.

Result:

## Step 7: blade offset (C5.4)

- A 20 mm square scored on paper with **Advanced › Blade offset** 0.9 (the default), then 0.0: at 0.9 the corners
  should be sharp; at 0.0 they come out rounded or with small loops. If 0.9 looks wrong, try 0.5 and 1.2.

Note: which offset gives the sharpest corners for the blade fitted.

Result:

## Step 8: cutting in groups, with a pause (C5.3)

- Two squares on two layers (Layers › new layer, then draw the second square). Cut Settings › **Groups** By
  Layer. Send: the first square is cut, then the window says "Paused before" the second layer while the machine
  waits with the mat loaded. **Continue**: the second square is cut where it should be (the mat hasn't moved).
- Send again and press **Cancel** while it is paused: nothing more is cut.

Note: did the machine wait at the pause? Was the second square in its place? Did Cancel stop it?

Result:

## Step 9: each group's own setup (C7.4, C7.5)

- The two layers of step 8. Cut Settings › Groups lists each layer: give the second a heavier **Force**, or a
  **Material** such as Cardstock. Send: the first square should be cut light; at the pause the window says "Load
  Cardstock, then Continue." when a material is set; then the second is cut heavier.

Note: was the second cut deeper than the first?

Result:

## Step 10: weeding (C8.2)

- On vinyl (or paper as a stand-in): a few letters typed with the Type tool (they are cut along their outlines as
  they are), Cut Settings › Weeding › **Weed Box** and **Weed Lines**. The letters are cut, then the lines from
  the box in towards them, then the box. Check that no line nicks a letter (they stop 0.3 mm short) and that the
  waste comes off in strips.

Note: any nicks (how far the blade ran on, mm)? Did the waste come off in strips?

Result:

## Step 11: nesting (C8.5)

- A few shapes of different sizes (one with a hole), spread over and off the artboard. Cutting Mat tab ›
  Nesting › **Nest All**: they come onto the mat close together from the top left, none closer than **Gap**. Cut
  them on paper.

Note: did the pieces come free whole? The smallest gap between neighbours, mm, against the Gap set.

Result:

## Step 12: more than one sheet (C8.6)

- More pieces than one mat holds. Nesting › **Onto New Sheets**, **Nest All**: Sheet 2 (and more) appear to the
  right of the artboard. Send window: "Sheet 1 of 2"; cut it, unload, load a fresh sheet, **Next Sheet**, Send:
  the second sheet's pieces are cut where the preview shows them, from the mat's origin as for the first.

Note: did sheet 2 cut where its preview showed?

Result:

## Step 13: Print & Cut (C6.1, C6.2)

- A Letter or A4 artboard with a 50 mm circle in the middle. Cutting Mat tab › Print & Cut › **Print & Cut** on,
  **Add Marks**; print it at 100 % (no "fit to page"). Put the sheet on the mat at its top-left corner, the
  square mark at the top left; load it.
- Send: the machine should look for the marks (the head moves to each), then cut the circle round the print.
  Measure how far the cut is from the printed circle at four points (target: within 0.5 mm).
- Then with the sheet turned round (square at the bottom right): the job should stop with "the cutter didn't
  find the registration marks", nothing cut.
- If the search fails on a good sheet: note the **Marks inset**, and try `TB52,0` instead of `TB52,2` (the other
  mark type in the notes). If the cut is offset by a fixed amount, measure it: the anchor may be the line's
  centre rather than its outer edge.

Note: found the marks (yes / no); the four distances, mm; what the machine did with the turned sheet.

Result:

## Step 14: job history, Send Again (C8.4, C8.8)

- Send window › **History** (click to open): every send and test cut above should be there with its time,
  material and how it ended; the failed one (the turned Print & Cut sheet) with a report in
  `%APPDATA%\VectorCraft\Cutting\Reports\`. **Use Settings** on a good one puts its settings back.
- **Send Again** on a good cut, with a fresh sheet loaded: the same cut as before, with the settings it was cut
  with. On a test cut: the test again at the same corner.

Note: was everything listed? Did Send Again cut the same thing?

Result:

## Step 15: a real cut

- Only after steps 1–3 pass: File › Open `crates/cutting/tests/golden/nested.svg` from the repo, cut through on
  paper with the force from step 4. Check the order (the fold line scored first, the O's counter before the O)
  and that the pieces come free.

Note: the order as cut; did every piece come free?

Result:

## Step 16: tools and holders (C10.1–C10.3)

- For each tool you have, fit it in its holder (holder 1: the AutoBlade, the pen holder, older tools in their
  adapters; holder 2: the 3 mm Kraft blade, rotary blade, punch tool), then **Check Tools** in the Send window. The
  holders line should name it. The punch tool's code isn't known: note what the window shows for it ("unknown tool
  n").
- With nothing in a holder, the line should say "empty" for it.
- The pen: Cut Settings › Tool Settings › **Tool** Pen in the Cut column, a pen in holder 1, a 20 mm square
  drawn: the corners should be sharp, with no little loops (a pen has no blade offset).
- If you have a holder 2 tool: choose it for the Cut column (the window warns while holder 2 is empty), fit it,
  and cut a 20 mm square in card. The machine should use holder 2. If it uses holder 1 or ignores the settings,
  note it.

Note: what the window showed for each tool and holder; the pen's corners; which holder the holder 2 cut used.

Result:
