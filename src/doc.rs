//! Markdown (+ TeX math) as a typed block tree.
//!
//! [`MarkdownParser`](crate::MarkdownParser) flattens a document into styled
//! lines, which loses what a renderer needs to draw it well: where one block
//! ends and the next begins, which lines belong to a list item, what a code
//! block's language is, which rows of a display equation align. [`Document`]
//! keeps that structure. Inline styling (bold, italic, code, math, links) is
//! already resolved into [`StyledSpan`]s; block-level presentation (heading
//! tint, quote gutters, code frames, spacing) is left to
//! [`layout`](crate::layout), so one tree renders at any width and theme.

use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};

use crate::colors;
use crate::span::{RichLine, StyledSpan, TextStyle, TextWeight};

/// A parsed markdown document: its top-level blocks in order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Document {
    pub blocks: Vec<Block>,
}

/// One block-level element.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Block {
    /// `#`…`######`; `level` is 1–6.
    Heading { level: u8, lines: Vec<RichLine> },
    /// Prose. Display math written inside a paragraph stays in its flow, so
    /// `Evaluate\n\[…\]` draws the equation directly under its sentence.
    Paragraph(Vec<Flow>),
    /// Ordered when `start` is set. `loose` lists put a gap between items.
    List { start: Option<u64>, loose: bool, items: Vec<Item> },
    /// Fenced or indented code; `lang` is the info string's first word.
    Code { lang: String, lines: Vec<String> },
    Quote(Vec<Block>),
    Table { align: Vec<Align>, head: Vec<RichLine>, rows: Vec<Vec<RichLine>> },
    /// A paragraph that was nothing but display math.
    Math(MathRows),
    /// A thematic break (`---`).
    Rule,
}

/// A piece of a paragraph's flow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Flow {
    /// A run of inline text up to a line break. `soft` is a source newline
    /// (CommonMark folds it to a space; chat usually means a newline).
    Line { line: RichLine, soft: bool },
    Math(MathRows),
}

/// Display math as rows of `&`-separated cells (see [`crate::math::tex_to_rows`]).
pub type MathRows = Vec<Vec<String>>;

/// A list item: its marker state and the blocks inside it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Item {
    /// `Some(checked)` for a GFM task item.
    pub task: Option<bool>,
    pub blocks: Vec<Block>,
}

/// A table column's alignment.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
}

impl From<Alignment> for Align {
    fn from(a: Alignment) -> Self {
        match a {
            Alignment::Center => Self::Center,
            Alignment::Right => Self::Right,
            Alignment::Left | Alignment::None => Self::Left,
        }
    }
}

/// An open container while the event stream is walked.
enum Frame {
    Quote(Vec<Block>),
    List { start: Option<u64>, loose: bool, items: Vec<Item> },
    Item(Item),
}

#[derive(Default)]
struct Para {
    flow: Vec<Flow>,
    line: RichLine,
    heading: Option<u8>,
}

impl Para {
    fn end_line(&mut self, soft: bool) {
        let line = std::mem::take(&mut self.line);
        self.flow.push(Flow::Line { line, soft });
    }

    fn into_block(mut self) -> Option<Block> {
        if !self.line.is_empty() {
            self.end_line(false);
        }
        while matches!(self.flow.last(), Some(Flow::Line { line, .. }) if line.is_empty()) {
            self.flow.pop();
        }
        if let Some(level) = self.heading {
            let lines = self
                .flow
                .into_iter()
                .filter_map(|f| match f {
                    Flow::Line { line, .. } => Some(line),
                    Flow::Math(_) => None,
                })
                .collect();
            return Some(Block::Heading { level, lines });
        }
        match self.flow.as_slice() {
            [] => None,
            [Flow::Math(_)] => match self.flow.pop() {
                Some(Flow::Math(rows)) => Some(Block::Math(rows)),
                _ => None,
            },
            _ => Some(Block::Paragraph(self.flow)),
        }
    }
}

#[derive(Default)]
struct TableBuf {
    align: Vec<Align>,
    in_head: bool,
    head: Vec<RichLine>,
    rows: Vec<Vec<RichLine>>,
    row: Vec<RichLine>,
    cell: RichLine,
}

struct Builder {
    root: Vec<Block>,
    stack: Vec<Frame>,
    para: Option<Para>,
    styles: Vec<TextStyle>,
    code: Option<(String, String)>,
    table: Option<TableBuf>,
}

impl Builder {
    fn style(&self) -> TextStyle {
        self.styles.last().copied().unwrap_or_default()
    }

    fn push_style(&mut self, f: impl FnOnce(&mut TextStyle)) {
        let mut s = self.style();
        f(&mut s);
        self.styles.push(s);
    }

    fn push_block(&mut self, block: Block) {
        match self.stack.last_mut() {
            Some(Frame::Quote(blocks)) => blocks.push(block),
            Some(Frame::Item(item)) => item.blocks.push(block),
            Some(Frame::List { items, .. }) => {
                // Blocks never land directly in a list; guard anyway.
                items.push(Item { task: None, blocks: vec![block] });
            }
            None => self.root.push(block),
        }
    }

    fn flush_para(&mut self) {
        if let Some(p) = self.para.take()
            && let Some(b) = p.into_block()
        {
            self.push_block(b);
        }
    }

    fn para(&mut self) -> &mut Para {
        self.para.get_or_insert_with(Para::default)
    }

    fn span(&mut self, span: StyledSpan) {
        if let Some(t) = self.table.as_mut() {
            t.cell.push(span);
        } else {
            self.para().line.push(span);
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.flush_para(),
            Tag::Heading { level, .. } => {
                self.flush_para();
                self.para = Some(Para { heading: Some(level as u8), ..Para::default() });
            }
            Tag::BlockQuote(_) => {
                self.flush_para();
                self.stack.push(Frame::Quote(Vec::new()));
            }
            Tag::CodeBlock(kind) => {
                self.flush_para();
                let lang = match kind {
                    CodeBlockKind::Fenced(info) => info.split_whitespace().next().unwrap_or("").to_owned(),
                    CodeBlockKind::Indented => String::new(),
                };
                self.code = Some((lang, String::new()));
            }
            Tag::List(start) => {
                self.flush_para();
                self.stack.push(Frame::List { start, loose: false, items: Vec::new() });
            }
            Tag::Item => {
                self.flush_para();
                self.stack.push(Frame::Item(Item::default()));
            }
            Tag::Table(align) => {
                self.flush_para();
                self.table = Some(TableBuf { align: align.into_iter().map(Align::from).collect(), ..TableBuf::default() });
            }
            Tag::TableHead => {
                if let Some(t) = self.table.as_mut() {
                    t.in_head = true;
                }
            }
            Tag::Emphasis => self.push_style(|s| s.italic = true),
            Tag::Strong => self.push_style(|s| s.weight = TextWeight::Bold),
            Tag::Strikethrough => self.push_style(|s| s.strikethrough = true),
            Tag::Link { .. } => self.push_style(|s| {
                s.underline = true;
                if s.color == TextStyle::default().color {
                    s.color = colors::CODE;
                }
            }),
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                // A paragraph inside an item makes the list loose.
                let in_item = matches!(self.stack.last(), Some(Frame::Item(_)));
                if in_item && let Some(Frame::List { loose, .. }) = self.stack.iter_mut().rev().nth(1) {
                    *loose = true;
                }
                self.flush_para();
            }
            TagEnd::Heading(_) => self.flush_para(),
            TagEnd::BlockQuote(_) => {
                self.flush_para();
                if let Some(Frame::Quote(blocks)) = self.stack.pop() {
                    self.push_block(Block::Quote(blocks));
                }
            }
            TagEnd::CodeBlock => {
                if let Some((lang, text)) = self.code.take() {
                    let lines = text.strip_suffix('\n').unwrap_or(&text).split('\n').map(str::to_owned).collect();
                    self.push_block(Block::Code { lang, lines });
                }
            }
            TagEnd::Item => {
                self.flush_para();
                if let Some(Frame::Item(item)) = self.stack.pop()
                    && let Some(Frame::List { items, .. }) = self.stack.last_mut()
                {
                    items.push(item);
                }
            }
            TagEnd::List(_) => {
                self.flush_para();
                if let Some(Frame::List { start, loose, items }) = self.stack.pop() {
                    self.push_block(Block::List { start, loose, items });
                }
            }
            TagEnd::TableCell => {
                if let Some(t) = self.table.as_mut() {
                    let cell = std::mem::take(&mut t.cell);
                    t.row.push(cell);
                }
            }
            TagEnd::TableHead => {
                if let Some(t) = self.table.as_mut() {
                    t.head = std::mem::take(&mut t.row);
                    t.in_head = false;
                }
            }
            TagEnd::TableRow => {
                if let Some(t) = self.table.as_mut() {
                    let row = std::mem::take(&mut t.row);
                    t.rows.push(row);
                }
            }
            TagEnd::Table => {
                if let Some(t) = self.table.take() {
                    self.push_block(Block::Table { align: t.align, head: t.head, rows: t.rows });
                }
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough | TagEnd::Link => {
                if self.styles.len() > 1 {
                    self.styles.pop();
                }
            }
            _ => {}
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => {
                if let Some((_, code)) = self.code.as_mut() {
                    code.push_str(&text);
                } else {
                    let st = self.style();
                    self.span(StyledSpan::new(text.to_string(), st));
                }
            }
            Event::Code(code) => {
                let mut st = self.style();
                st.color = colors::CODE;
                self.span(StyledSpan::new(code.to_string(), st));
            }
            Event::InlineMath(tex) => {
                let mut st = self.style();
                st.color = colors::MATH;
                self.span(StyledSpan::new(crate::math::tex_to_unicode(&tex).join(" "), st));
            }
            Event::DisplayMath(tex) => {
                let rows = crate::math::tex_to_rows(&tex);
                if self.table.is_some() {
                    let text = rows.iter().map(|r| r.join(" ")).collect::<Vec<_>>().join(" ");
                    let st = TextStyle { color: colors::MATH, ..self.style() };
                    self.span(StyledSpan::new(text, st));
                    return;
                }
                let p = self.para();
                if !p.line.is_empty() {
                    p.end_line(false);
                }
                // The source newline before `\[` was already a soft break.
                if let Some(Flow::Line { line, .. }) = p.flow.last()
                    && line.is_empty()
                {
                    p.flow.pop();
                }
                p.flow.push(Flow::Math(rows));
            }
            Event::Html(html) | Event::InlineHtml(html) => {
                let st = self.style();
                for (i, l) in html.trim_end_matches('\n').split('\n').enumerate() {
                    if i > 0 {
                        self.para().end_line(false);
                    }
                    self.span(StyledSpan::new(l.to_owned(), st));
                }
            }
            Event::FootnoteReference(name) => {
                let st = self.style();
                self.span(StyledSpan::new(format!("[^{name}]"), st));
            }
            Event::SoftBreak | Event::HardBreak => {
                let soft = matches!(event, Event::SoftBreak);
                if let Some(t) = self.table.as_mut() {
                    t.cell.push(StyledSpan::plain(" "));
                } else {
                    let p = self.para();
                    // A break straight after display math is the math's own line end.
                    if p.line.is_empty() && matches!(p.flow.last(), Some(Flow::Math(_))) {
                        return;
                    }
                    p.end_line(soft);
                }
            }
            Event::Rule => {
                self.flush_para();
                self.push_block(Block::Rule);
            }
            Event::TaskListMarker(checked) => {
                if let Some(Frame::Item(item)) = self.stack.last_mut() {
                    item.task = Some(checked);
                }
            }
        }
    }
}

impl Document {
    /// Parse markdown (GFM tables, strikethrough, task lists, `$`/`$$` and
    /// `\(`/`\[` TeX math) into a block tree.
    #[must_use]
    pub fn parse(markdown: &str) -> Self {
        let options = Options::ENABLE_STRIKETHROUGH
            | Options::ENABLE_TABLES
            | Options::ENABLE_MATH
            | Options::ENABLE_TASKLISTS
            | Options::ENABLE_FOOTNOTES;
        let src = crate::markdown::normalize_math_delimiters(markdown);
        let mut b = Builder {
            root: Vec::new(),
            stack: Vec::new(),
            para: None,
            styles: vec![TextStyle::default()],
            code: None,
            table: None,
        };
        for event in Parser::new_ext(&src, options) {
            b.event(event);
        }
        b.flush_para();
        while let Some(frame) = b.stack.pop() {
            let block = match frame {
                Frame::Quote(blocks) => Block::Quote(blocks),
                Frame::List { start, loose, items } => Block::List { start, loose, items },
                Frame::Item(item) => Block::List { start: None, loose: false, items: vec![item] },
            };
            b.push_block(block);
        }
        Self { blocks: b.root }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_are_kept_apart() {
        let d = Document::parse("# T\n\npara\n\n```rust\nfn x() {}\n```\n\n> q\n\n---\n");
        assert!(matches!(d.blocks[0], Block::Heading { level: 1, .. }));
        assert!(matches!(d.blocks[1], Block::Paragraph(_)));
        assert!(matches!(&d.blocks[2], Block::Code { lang, lines } if lang == "rust" && lines.len() == 1));
        assert!(matches!(d.blocks[3], Block::Quote(_)));
        assert!(matches!(d.blocks[4], Block::Rule));
    }

    #[test]
    fn list_items_hold_their_blocks_and_nesting() {
        let d = Document::parse("1. one\n   more\n   - inner\n2. two\n");
        let Block::List { start: Some(1), loose: false, items } = &d.blocks[0] else { panic!("{d:?}") };
        assert_eq!(items.len(), 2);
        assert!(matches!(items[0].blocks[1], Block::List { start: None, .. }));
    }

    #[test]
    fn loose_list_is_marked() {
        let d = Document::parse("- a\n\n- b\n");
        assert!(matches!(d.blocks[0], Block::List { loose: true, .. }));
    }

    #[test]
    fn display_math_stays_in_its_paragraph_flow() {
        let d = Document::parse("Evaluate\n\\[\nx^2\n\\]\nafter");
        let Block::Paragraph(flow) = &d.blocks[0] else { panic!("{d:?}") };
        assert!(matches!(flow[0], Flow::Line { .. }));
        assert!(matches!(flow[1], Flow::Math(_)));
        assert!(matches!(&flow[2], Flow::Line { line, .. } if line.plain_text() == "after"));
    }

    #[test]
    fn math_alone_is_a_math_block() {
        let d = Document::parse("$$\na &= b \\\\\n  &= c\n$$\n");
        assert_eq!(d.blocks, vec![Block::Math(vec![vec!["a".into(), "= b".into()], vec![String::new(), "= c".into()]])]);
    }

    #[test]
    fn task_items_and_tables() {
        let d = Document::parse("- [x] done\n\n| a | b |\n|:-|-:|\n| 1 | 2 |\n");
        let Block::List { items, .. } = &d.blocks[0] else { panic!() };
        assert_eq!(items[0].task, Some(true));
        let Block::Table { align, head, rows } = &d.blocks[1] else { panic!() };
        assert_eq!(align, &[Align::Left, Align::Right]);
        assert_eq!(head.len(), 2);
        assert_eq!(rows[0][1].plain_text(), "2");
    }
}
