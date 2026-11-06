// Fork (cutting workspace): the page design of the cutting manual. `src/main.rs` writes the main
// file that imports this and passes it the guide's text.

#let accent = rgb("#1d6f8a")
#let dim = luma(95)

// The guide's block quotes.
#let note(body) = block(
  fill: luma(245),
  stroke: (left: 3pt + accent),
  inset: (x: 10pt, y: 8pt),
  width: 100%,
  body,
)

// A screenshot, framed, with its caption.
#let shot(path, caption, width) = figure(
  box(stroke: 0.5pt + luma(185), image(path, width: 100%), width: width),
  caption: caption,
)

// Where a screenshot goes that hasn't been taken (run without --no-shots, or its recipe failed).
#let missing(name) = block(
  stroke: (paint: luma(170), dash: "dashed"),
  inset: 10pt,
  width: 100%,
  text(fill: luma(120), size: 9pt)[Screenshot “#name” not taken],
)

#let manual(title: "", subtitle: "", built: "", body) = {
  set document(title: title)
  set text(font: ("Noto Sans", "Segoe UI", "Libertinus Serif"), size: 10pt, lang: "en")
  set par(leading: 0.62em, spacing: 0.95em)
  show raw: set text(font: ("Cascadia Mono", "Consolas", "DejaVu Sans Mono"), size: 0.92em)
  show link: set text(fill: accent)
  set list(indent: 0.6em, spacing: 0.7em)
  set enum(indent: 0.6em, spacing: 0.7em)
  set table(
    stroke: 0.5pt + luma(205),
    inset: (x: 6pt, y: 5pt),
    fill: (_, y) => if y == 0 { luma(238) },
  )
  show table: set text(size: 9pt)
  show table.cell.where(y: 0): set text(weight: "bold")
  show figure: set block(breakable: false, above: 1.2em, below: 1.2em)
  show figure.caption: set text(size: 8.5pt, fill: dim)

  show heading.where(level: 2): it => {
    pagebreak(weak: true)
    block(below: 1em, text(size: 19pt, weight: "bold", fill: accent, it.body))
  }
  show heading.where(level: 3): it => block(above: 1.5em, below: 0.7em, sticky: true, text(size: 12.5pt, weight: "bold", it.body))
  show heading.where(level: 4): it => block(above: 1.2em, below: 0.6em, sticky: true, text(size: 10.5pt, weight: "bold", it.body))

  // The cover.
  page(paper: "a4", margin: 3cm)[
    #v(18%)
    #image("logo.svg", width: 3.2cm)
    #v(1.2em)
    #text(size: 32pt, weight: "bold", fill: accent, title)
    #v(0.4em)
    #text(size: 15pt, fill: dim, subtitle)
    #v(1fr)
    #text(size: 8.5pt, fill: dim)[
      #built \
      Based on VectorCraft. The cutting workspace is part of the ktand/vectorcraft fork.
    ]
  ]

  set page(
    paper: "a4",
    margin: (x: 2.2cm, top: 2.3cm, bottom: 2.3cm),
    header: context {
      if counter(page).get().first() > 1 {
        align(right, text(size: 8pt, fill: dim, title))
      }
    },
    footer: context align(center, text(size: 8pt, fill: dim, counter(page).display())),
  )
  counter(page).update(1)

  // The contents.
  {
    show outline.entry.where(level: 2): it => { v(0.5em, weak: true); strong(it) }
    outline(title: text(size: 19pt, fill: accent)[Contents], depth: 3, indent: 1.2em)
  }
  v(1.5em)

  body
}
