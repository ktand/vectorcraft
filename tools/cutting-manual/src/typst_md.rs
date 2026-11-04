//! The user guide's Markdown as Typst markup: headings, paragraphs, emphasis, code, links,
//! lists, tables and block quotes, and `<!-- shot: name -->` markers as screenshots.
//!
//! Everything is written as Typst function calls (`#strong[…];`, `#list([…], […])`), never as
//! Typst's own markup shorthands, so the guide's text can't be read as markup: every character
//! Typst gives a meaning to is escaped.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// What the converter calls for each `<!-- shot: name -->` marker: the Typst to put there.
pub type ShotFn<'a> = dyn Fn(&str) -> String + 'a;

#[derive(Debug)]
enum Kind {
    Root,
    Para,
    Heading(u8),
    Strong,
    Emph,
    Strike,
    Link(String),
    Quote,
    List(Option<u64>),
    Item,
    Table(usize),
    Head,
    Row,
    Cell,
    Code,
}

struct Frame {
    kind: Kind,
    buf: String,
    items: Vec<String>,
    head: Vec<String>,
}

impl Frame {
    fn new(kind: Kind) -> Self {
        Frame { kind, buf: String::new(), items: Vec::new(), head: Vec::new() }
    }
}

/// `s` as Typst markup text: every character with a meaning in markup escaped (quotes are left to
/// become typographic ones; `.` so that "1." never starts a numbered list).
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        if matches!(c, '\\' | '*' | '_' | '`' | '#' | '$' | '<' | '>' | '@' | '[' | ']' | '~' | '/' | '=' | '-' | '+' | '.') {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// `s` as a Typst string literal.
pub fn string(s: &str) -> String {
    let mut out = String::from("\"");
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn level(l: HeadingLevel) -> u8 {
    match l {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// The name in a `<!-- shot: name -->` marker.
pub fn shot_name(html: &str) -> Option<&str> {
    let inner = html.trim().strip_prefix("<!--")?.strip_suffix("-->")?.trim();
    let name = inner.strip_prefix("shot:")?.trim();
    (!name.is_empty()).then_some(name)
}

/// Every screenshot the guide asks for, in order.
pub fn shots(md: &str) -> Vec<String> {
    Parser::new_ext(md, options())
        .filter_map(|e| match e {
            Event::Html(h) | Event::InlineHtml(h) => shot_name(&h).map(str::to_string),
            _ => None,
        })
        .collect()
}

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH
}

/// The guide as Typst. The guide's own title (its first level-1 heading) and the block quote
/// right after it are left out: the cover has them.
pub fn convert(md: &str, shot: &ShotFn) -> String {
    let mut stack = vec![Frame::new(Kind::Root)];
    let mut skip_title = true;
    let mut skip_quote = false;
    let mut skipping = 0usize;
    for event in Parser::new_ext(md, options()) {
        if skipping > 0 {
            match &event {
                Event::Start(_) => skipping += 1,
                Event::End(_) => skipping -= 1,
                _ => {}
            }
            continue;
        }
        match event {
            Event::Start(Tag::Heading { level: HeadingLevel::H1, .. }) if skip_title => {
                skip_title = false;
                skip_quote = true;
                skipping = 1;
            }
            Event::Start(Tag::BlockQuote(_)) if skip_quote => {
                skip_quote = false;
                skipping = 1;
            }
            Event::Start(tag) => {
                if !matches!(tag, Tag::BlockQuote(_)) {
                    skip_quote = false;
                }
                let kind = match tag {
                    Tag::Paragraph => Kind::Para,
                    Tag::Heading { level: l, .. } => Kind::Heading(level(l)),
                    Tag::Strong => Kind::Strong,
                    Tag::Emphasis => Kind::Emph,
                    Tag::Strikethrough => Kind::Strike,
                    Tag::Link { dest_url, .. } => Kind::Link(dest_url.to_string()),
                    Tag::BlockQuote(_) => Kind::Quote,
                    Tag::List(start) => Kind::List(start),
                    Tag::Item => Kind::Item,
                    Tag::Table(aligns) => Kind::Table(aligns.len()),
                    Tag::TableHead => Kind::Head,
                    Tag::TableRow => Kind::Row,
                    Tag::TableCell => Kind::Cell,
                    Tag::CodeBlock(_) => Kind::Code,
                    // Images, footnotes and the rest the guide doesn't use: their text kept.
                    _ => Kind::Para,
                };
                stack.push(Frame::new(kind));
            }
            Event::End(end) => {
                let Some(frame) = stack.pop() else { continue };
                let Some(parent) = stack.last_mut() else { continue };
                close(frame, parent, matches!(end, TagEnd::Paragraph));
            }
            Event::Text(t) => {
                if let Some(top) = stack.last_mut() {
                    if matches!(top.kind, Kind::Code) {
                        top.buf.push_str(&t);
                    } else {
                        top.buf.push_str(&escape(&t));
                    }
                }
            }
            Event::Code(c) => {
                if let Some(top) = stack.last_mut() {
                    top.buf.push_str(&format!("#raw({});", string(&c)));
                }
            }
            Event::SoftBreak => {
                if let Some(top) = stack.last_mut() {
                    top.buf.push(' ');
                }
            }
            Event::HardBreak => {
                if let Some(top) = stack.last_mut() {
                    top.buf.push_str("#linebreak();");
                }
            }
            Event::Rule => {
                if let Some(top) = stack.last_mut() {
                    top.buf.push_str("#line(length: 100%, stroke: 0.5pt + luma(180))\n\n");
                }
            }
            Event::Html(h) | Event::InlineHtml(h) => {
                if let (Some(name), Some(top)) = (shot_name(&h), stack.last_mut()) {
                    top.buf.push_str(&shot(name));
                    top.buf.push_str("\n\n");
                }
            }
            _ => {}
        }
    }
    stack.into_iter().next().map(|f| f.buf).unwrap_or_default()
}

/// A finished frame's Typst added to its parent.
fn close(frame: Frame, parent: &mut Frame, _para: bool) {
    let body = frame.buf.trim().to_string();
    match frame.kind {
        Kind::Root => {}
        Kind::Para => {
            parent.buf.push_str(&body);
            parent.buf.push_str("\n\n");
        }
        Kind::Heading(l) => parent.buf.push_str(&format!("#heading(level: {l})[{body}]\n\n")),
        Kind::Strong => parent.buf.push_str(&format!("#strong[{}];", frame.buf)),
        Kind::Emph => parent.buf.push_str(&format!("#emph[{}];", frame.buf)),
        Kind::Strike => parent.buf.push_str(&format!("#strike[{}];", frame.buf)),
        Kind::Link(url) => {
            if url.starts_with("http://") || url.starts_with("https://") {
                parent.buf.push_str(&format!("#link({})[{}];", string(&url), frame.buf));
            } else {
                // A link to another file in the repository: its text only, the PDF stands alone.
                parent.buf.push_str(&frame.buf);
            }
        }
        Kind::Quote => parent.buf.push_str(&format!("#note[{body}]\n\n")),
        Kind::Item => parent.items.push(body),
        Kind::List(start) => {
            let items: Vec<String> = frame.items.iter().map(|i| format!("[{i}]")).collect();
            let call = match start {
                Some(n) => format!("#enum(start: {n}, {})\n\n", items.join(", ")),
                None => format!("#list({})\n\n", items.join(", ")),
            };
            parent.buf.push_str(&call);
        }
        Kind::Cell => parent.items.push(body),
        Kind::Head => parent.head = frame.items,
        Kind::Row => parent.items.extend(frame.items),
        Kind::Table(columns) => {
            let cell = |c: &String| format!("[{c}]");
            let head: Vec<String> = frame.head.iter().map(cell).collect();
            let cells: Vec<String> = frame.items.iter().map(cell).collect();
            parent.buf.push_str(&format!("#table(columns: {}, table.header({}), {})\n\n", columns.max(1), head.join(", "), cells.join(", ")));
        }
        Kind::Code => parent.buf.push_str(&format!("#raw(block: true, {})\n\n", string(&frame.buf))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn conv(md: &str) -> String {
        convert(md, &|n| format!("SHOT({n})"))
    }

    #[test]
    fn markup_characters_are_escaped() {
        assert_eq!(escape("a*b_c #1 [x] // 2."), "a\\*b\\_c \\#1 \\[x\\] \\/\\/ 2\\.");
        assert_eq!(string("say \"hi\" \\"), "\"say \\\"hi\\\" \\\\\"");
    }

    #[test]
    fn the_title_and_its_note_go_to_the_cover() {
        let t = conv("# Title\n\n> **For:** you\n\nIntro.\n\n## 1. First\n\nText.");
        assert!(!t.contains("Title") && !t.contains("For"), "{t}");
        assert!(t.contains("Intro\\.") && t.contains("#heading(level: 2)[1\\. First]"), "{t}");
    }

    #[test]
    fn inline_calls_end_with_a_semicolon() {
        let t = conv("# T\n\nPress **Send**(now) and `cutting.job`.");
        assert!(t.contains("#strong[Send];(now)"), "{t}");
        assert!(t.contains("#raw(\"cutting.job\");\\."), "{t}");
    }

    #[test]
    fn lists_tables_links_and_shots() {
        let md = "# T\n\n- one\n- **two**\n\n| A | B |\n|---|---|\n| 1 | 2 |\n\nSee [the site](https://example.com) and [notes](notes.md).\n\n<!-- shot: mat -->\n";
        let t = conv(md);
        assert!(t.contains("#list([one], [#strong[two];])"), "{t}");
        assert!(t.contains("#table(columns: 2, table.header([A], [B]), [1], [2])"), "{t}");
        assert!(t.contains("#link(\"https://example.com\")[the site];") && t.contains(" notes\\."), "{t}");
        assert!(t.contains("SHOT(mat)"), "{t}");
        assert_eq!(shots(md), ["mat"]);
    }
}
