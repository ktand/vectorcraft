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
4. Window › Workspace › Cutting. The Cutting panel's **Cutter** section should name the machine ("Cameo 4"). If it
   says no cutter is connected, press **Find Cutters**.

## Step 1: the machine answers (C4.2)

- With an empty document (File › New), press **Send to Cutter**: it is greyed out (nothing to cut). That's
  right.
- Draw one small rectangle, 10 × 10 mm. In the panel set **Default** to Score, and under **Tool Settings ›
  Score** set Force to 0 % (the machine's lightest) and Passes to 1.
- Press **Send to Cutter**. Expected: the panel shows "Sending: …", the machine moves, then "Sent.".
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

- Draw a 100 mm line. Score it with **Tool Settings › Score › Speed** at 10, 100 and 300 mm/s (the machine's
  steps 1, 10 and 30) and time each.

Result:

## Step 5: a real cut

- Only after steps 1–3 pass: the golden test file (`nested.svg`) cut through on paper with the cut force set as
  for the material. Check the order (the fold line scored first, the O's counter before the O) and that the
  pieces come free.

Result:
