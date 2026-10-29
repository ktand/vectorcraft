# Cutting on the machine: first checks

> **Status:** not yet run · **Machine:** the owner's Silhouette Cameo 4 (USB `0x0B4D:0x1137`) · **For:** C4.2 and
> C4.6

The steps to confirm on a real Cameo 4 what the encoder takes from the
[protocol notes](protocol-silhouette.md), safest first. Use scrap paper on the mat and the lightest settings
until step 3 has passed. Write what happened under each step, then mark the matching facts in the protocol notes
as confirmed (or correct them).

## Before you start

1. Connect the Cameo 4 by USB, switch it on, and load a mat with a sheet of scrap paper.
2. Close Silhouette Studio, so it doesn't hold the machine.
3. Start VectorCraft from the repo: `cargo run -p vectorcraft -- crates/cutting/tests/golden/nested.svg`.
4. Window › Workspace › Cutting, then **Send to Cutter…** at the top of the Cut Settings panel (or File › Send to
   Cutter…, Ctrl+Alt+Shift+C). The Send window should name the machine ("Cameo 4"). If it says no cutter is
   connected, press **Find Cutters**. Sending, pausing and the test cut are all in that window.

## Step 1: the machine answers (C4.2)

- With an empty document (File › New), the Send window's **Send** is greyed out (nothing to cut). That's
  right.
- Draw one small rectangle, 10 × 10 mm. In Cut Settings set **Default** to Score, and under **Tool Settings ›
  Score** set Force to 1 (the machine's lightest step) and Passes to 1.
- Press **Send**. Expected: the window shows "Sending: …", the machine moves, then "Sent.".
- If instead it shows "load the mat into the cutter first", the status answer works but the mat isn't loaded.
- If it shows "the cutter didn't answer in time", write it down: the status read through `usbprint.sys` needs a
  look.

Result:

## Step 2: which way the axes go

- Look at the scored square on the sheet. It should be 10 mm from the mat's top-left corner of the grid in both
  directions, as the Cutting view shows it.
- If it is at the mirrored or swapped place, the coordinate order (vertical first) is wrong: note where it went.

Result:

## Step 3: size

- Measure the square: 10.0 mm a side (± 0.3 mm).
- Measure its distance from the grid's corner against the Cutting view.

Result:

## Step 4: speed (for the speed mapping)

- Draw a 100 mm line. Score it with **Tool Settings › Score › Speed** at steps 1, 10 and 30 and time each
  (the panel takes a step as 10 mm/s until this measures it).

Result:

## Step 4b: the new settings (C5.2)

- With the AutoBlade, set **Blade depth** to 1 and score a line, then 3: the deeper one marks more. (`TF` n)
- **Accel.** 1 and 3 on the same square: 1 should move more gently. (`TJ` n)
- **Track Enhancing** on: the rollers should grip differently, or the machine reports it. Which of `FY0`/`FY1` turns
  it on is a guess (`FY1` now): if on and off behave the other way round, note it.
- **Corners** 0.5 mm on a scored square: each corner gets a small spur past it.

Result:

## Step 4c: cutting in groups (C5.3)

- Two squares on two layers, Groups **By Layer**: the first square is cut, then the Send window says "Paused
  before" the second layer while the machine waits with the mat loaded. Continue: the second square is cut where it
  should be (the mat hasn't moved). Cancel while paused: nothing more is cut.

Result:

## Step 4d: blade offset (C5.4)

- A 20 mm square scored on paper with Blade offset 0.9 (the default), then 0.0: at 0.9 the corners should be
  sharp; at 0.0 they come out rounded or with small loops. If 0.9 looks wrong, try 0.5 and 1.2 and note which
  gives the sharpest corners for the blade fitted.

Result:

## Step 4e: Print & Cut (C6.1, C6.2)

- A Letter or A4 artboard with a 50 mm circle in the middle; Print & Cut on, Add Marks; print it at 100 % (no
  "fit to page"). Put the sheet on the mat at its top-left corner, the square mark at the top left; load it.
- Send: the machine should look for the marks (the head moves to each), then cut the circle round the print.
  Measure how far the cut is from the printed circle at four points (target: within 0.5 mm).
- Then with the sheet turned round (square at the bottom right): the job should stop with "the cutter didn't
  find the registration marks", nothing cut. Note what the machine did.
- If the search fails on a good sheet: note the marks inset, and try `TB52,0` instead of `TB52,2` (the other
  mark type in the notes). If the cut is offset by a fixed amount, measure it: the anchor may be the line's
  centre rather than its outer edge.

Result:

## Step 4f: each group's own setup (C7.4, C7.5)

- The two layers of step 4c; in Cut Settings › Groups give the second layer a heavier force (or a material such
  as Cardstock). The first square should be cut light, then after the pause ("Load Cardstock, then Continue"
  when a material is set) the second heavier. Feel or look at the cut depth of each.

Result:

## Step 4g: the test cut (C8.1)

- Send window › **Test at** Bottom Right, **Test Cut**: a 10 mm square with a triangle inside it is cut 3 mm in
  from that corner of the cut area, the triangle first. The triangle should lift out and the square stay on the
  backing. Then each other corner: check they land where the preview's mat says (a test at the corner the design
  is in should warn before cutting).
- Use it to tune a material: the force where the triangle lifts out cleanly without cutting the backing is the
  one to save as the material.

Result:

## Step 4h: weeding (C8.2)

- On vinyl (or paper as a stand-in): a few letters, Cut Settings › Weeding › **Weed Box** and **Weed Lines**.
  The letters are cut, then the lines from the box in towards them, then the box. Check that no line nicks a
  letter (they stop 0.3 mm short; if the blade runs on further, note how far) and that the waste comes off in
  strips.

Result:

## Step 4i: job history (C8.4)

- After the steps above, open the Send window's **History**: every send and test cut should be there with its
  time, material and how it ended, and the failed ones (the turned Print & Cut sheet) with a report in
  `Cutting/Reports/` next to the preferences. **Use Settings** on a good one puts its settings back.

Result:

## Step 5: a real cut

- Only after steps 1–3 pass: the golden test file (`nested.svg`) cut through on paper with the cut force set as
  for the material. Check the order (the fold line scored first, the O's counter before the O) and that the
  pieces come free.

Result:
