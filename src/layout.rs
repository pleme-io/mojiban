//! Lay a [`Document`] out at a width: renderer-neutral styled rows.
//!
//! The output is what every pleme-io text surface draws: rows of
//! [`egaku::Span`]s whose `style` indexes [`Rendered::styles`]. Each style
//! carries a semantic [`Role`] (so a host maps roles to its own palette
//! tokens) plus the resolved inline attributes and the [`Theme`]'s default
//! colour (so a host with no palette can draw it as-is).
//!
//! Spacing is a property of the sequence, not of a block: exactly
//! `block_gap` blank rows between two blocks, none before the first or after
//! the last, so nothing is ever doubled. Every prefix — a list marker, a
//! quote gutter, a code frame — is applied to already-wrapped rows, so a
//! wrapped or continued line keeps its hanging indent at any nesting depth.

use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthStr;

use egaku::{Span, TextView, Wrap};

use crate::colors;
use crate::doc::{Align, Block, Document, Flow, Item, MathRows};
use crate::highlight::SyntaxHighlighter;
use crate::span::{RichLine, TextStyle, TextWeight};

/// What a styled run IS, for a host that styles by meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Role {
    Text,
    Heading(u8),
    HeadingRule,
    InlineCode,
    Link,
    Math,
    ListMarker,
    QuoteBar,
    Quote,
    Code,
    CodeKeyword,
    CodeString,
    CodeComment,
    CodeNumber,
    CodeFrame,
    CodeLabel,
    TableHead,
    TableRule,
    Rule,
}

/// One entry of the style table a row's span indexes.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct CellStyle {
    pub role: Role,
    /// Inline attributes plus the theme's default colour for this role.
    pub text: TextStyle,
    pub background: Option<[f32; 4]>,
}

/// A laid-out document.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Rendered {
    pub rows: Vec<Vec<Span>>,
    pub styles: Vec<CellStyle>,
}

impl Rendered {
    /// The style a span's index names (the first entry when out of range).
    #[must_use]
    pub fn style(&self, index: u8) -> Option<&CellStyle> {
        self.styles.get(usize::from(index)).or_else(|| self.styles.first())
    }

    /// Rows as plain text, trailing spaces trimmed, one per line — the
    /// golden-test view.
    #[must_use]
    pub fn plain(&self) -> String {
        let mut out = String::new();
        for row in &self.rows {
            let line: String = row.iter().map(Span::text).collect();
            out.push_str(line.trim_end());
            out.push('\n');
        }
        out
    }
}

/// How a source newline inside a paragraph is drawn.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SoftBreak {
    /// Keep it: chat text is written line by line.
    #[default]
    Newline,
    /// CommonMark: fold it to a space and reflow.
    Space,
}

/// Code block chrome.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodeFrame {
    /// A rounded box with the language in its top edge.
    #[default]
    Box,
    /// A left bar, the language on its own row above.
    Bar,
    /// Indented only.
    None,
}

/// What happens to a code line wider than the frame.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CodeOverflow {
    /// Continue on the next row (column-exact, no word breaking).
    #[default]
    Wrap,
    /// Cut it and end the row with `…`.
    Clip,
}

/// Code block settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CodeTheme {
    pub frame: CodeFrame,
    pub overflow: CodeOverflow,
    pub highlight: bool,
    pub label: bool,
    pub background: Option<[f32; 4]>,
}

impl Default for CodeTheme {
    fn default() -> Self {
        Self {
            frame: CodeFrame::Box,
            overflow: CodeOverflow::Wrap,
            highlight: true,
            label: true,
            background: None,
        }
    }
}

/// Default colours per role. A host with its own tokens overrides these or
/// ignores them and styles by [`Role`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Styles {
    pub text: TextStyle,
    pub heading: TextStyle,
    pub heading1: TextStyle,
    pub muted: TextStyle,
    pub marker: TextStyle,
    pub code_label: TextStyle,
    pub quote: TextStyle,
}

impl Default for Styles {
    fn default() -> Self {
        let muted = TextStyle::colored(colors::COMMENT);
        Self {
            text: TextStyle::default(),
            heading: TextStyle::bold().with_color(colors::KEYWORD),
            heading1: TextStyle::bold().with_color(colors::CODE),
            muted,
            marker: TextStyle::colored(colors::KEYWORD),
            code_label: TextStyle::colored(colors::QUOTE).with_italic(),
            quote: TextStyle::colored(colors::QUOTE),
        }
    }
}

/// The declarative layout spec: spacing, glyphs, chrome and default
/// colours. Every field defaults, so a partial YAML/Lisp spec is valid.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Theme {
    /// Blank rows between two blocks.
    pub block_gap: usize,
    pub soft_break: SoftBreak,
    /// Unordered markers by nesting depth, cycling.
    pub bullets: Vec<String>,
    pub task_open: String,
    pub task_done: String,
    pub quote_bar: String,
    /// Columns display math is indented by.
    pub math_indent: usize,
    /// Drawn under a level-1 heading, its width; `None` for no rule.
    pub h1_rule: Option<String>,
    pub h2_rule: Option<String>,
    pub rule: String,
    pub code: CodeTheme,
    pub styles: Styles,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            block_gap: 1,
            soft_break: SoftBreak::Newline,
            bullets: vec!["•".into(), "◦".into(), "▪".into()],
            task_open: "☐".into(),
            task_done: "☑".into(),
            quote_bar: "▎".into(),
            math_indent: 4,
            h1_rule: Some("━".into()),
            h2_rule: None,
            rule: "─".into(),
            code: CodeTheme::default(),
            styles: Styles::default(),
        }
    }
}

/// Parse and lay out in one call.
#[must_use]
pub fn render_markdown(markdown: &str, width: usize, theme: &Theme) -> Rendered {
    layout(&Document::parse(markdown), width, theme)
}

/// Lay `doc` out at `width` display columns.
#[must_use]
pub fn layout(doc: &Document, width: usize, theme: &Theme) -> Rendered {
    let mut l = Layouter { theme, styles: Vec::new(), highlighter: SyntaxHighlighter::new() };
    l.intern(Role::Text, theme.styles.text, None);
    let rows = l.blocks(&doc.blocks, width.max(1), Ctx::default());
    Rendered { rows, styles: l.styles }
}

type Row = Vec<Span>;

#[derive(Clone, Copy, Default)]
struct Ctx {
    depth: usize,
    quote: bool,
}

struct Layouter<'t> {
    theme: &'t Theme,
    styles: Vec<CellStyle>,
    highlighter: SyntaxHighlighter,
}

fn width_of(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}

fn row_width(row: &[Span]) -> usize {
    row.iter().map(Span::width).sum()
}

fn wrap(spans: Vec<Span>, width: usize, mode: Wrap) -> Vec<Row> {
    wrap_hang(spans, width, mode, 0)
}

fn wrap_hang(spans: Vec<Span>, width: usize, mode: Wrap, hang: usize) -> Vec<Row> {
    let mut v = TextView::new();
    v.set_wrap(mode);
    v.set_hang(hang);
    v.set_width(width.max(1));
    v.set_lines(vec![spans]);
    let rows: Vec<Row> = v.wrapped().iter().map(|w| w.spans().to_vec()).collect();
    if rows.is_empty() { vec![Vec::new()] } else { rows }
}

fn prefix(rows: Vec<Row>, first: &[Span], rest: &[Span]) -> Vec<Row> {
    rows.into_iter()
        .enumerate()
        .map(|(i, row)| {
            let mut p = if i == 0 { first.to_vec() } else { rest.to_vec() };
            if row.is_empty() && i > 0 && rest.iter().all(|s| s.text().trim().is_empty()) {
                return Vec::new();
            }
            p.extend(row);
            p
        })
        .collect()
}

fn pad(text: &str, width: usize, align: Align) -> (usize, usize) {
    let gap = width.saturating_sub(width_of(text));
    match align {
        Align::Left => (0, gap),
        Align::Right => (gap, 0),
        Align::Center => (gap / 2, gap - gap / 2),
    }
}

impl Layouter<'_> {
    fn intern(&mut self, role: Role, text: TextStyle, background: Option<[f32; 4]>) -> u8 {
        let cs = CellStyle { role, text, background };
        let i = self.styles.iter().position(|s| *s == cs).unwrap_or_else(|| {
            self.styles.push(cs);
            self.styles.len() - 1
        });
        u8::try_from(i).unwrap_or(0)
    }

    fn span(&mut self, text: impl Into<String>, role: Role, style: TextStyle) -> Span {
        let i = self.intern(role, style, None);
        Span::new(text, i)
    }

    /// An inline run from the document: its role read off its colour, the
    /// block's tint applied where the run has no colour of its own.
    fn inline(&mut self, text: &str, st: TextStyle, tint: Option<(Role, TextStyle)>) -> Span {
        let plain = st.color == TextStyle::default().color;
        let (role, mut out) = if st.color == colors::MATH {
            (Role::Math, st)
        } else if st.underline && st.color == colors::CODE {
            (Role::Link, st)
        } else if st.color == colors::CODE {
            (Role::InlineCode, st)
        } else if let (true, Some((role, base))) = (plain, tint) {
            (role, TextStyle { color: base.color, ..st })
        } else {
            (Role::Text, if plain { TextStyle { color: self.theme.styles.text.color, ..st } } else { st })
        };
        if let Some((Role::Heading(_), base)) = tint {
            if base.weight == TextWeight::Bold {
                out.weight = TextWeight::Bold;
            }
        }
        self.span(text, role, out)
    }

    fn line_spans(&mut self, line: &RichLine, tint: Option<(Role, TextStyle)>) -> Row {
        line.spans.iter().map(|s| self.inline(&s.text, s.style, tint)).collect()
    }

    fn tint(&self, ctx: Ctx) -> Option<(Role, TextStyle)> {
        ctx.quote.then_some((Role::Quote, self.theme.styles.quote))
    }

    fn blocks(&mut self, blocks: &[Block], width: usize, ctx: Ctx) -> Vec<Row> {
        self.blocks_gap(blocks, width, ctx, self.theme.block_gap)
    }

    fn blocks_gap(&mut self, blocks: &[Block], width: usize, ctx: Ctx, gap: usize) -> Vec<Row> {
        let mut out: Vec<Row> = Vec::new();
        for b in blocks {
            let rows = self.block(b, width, ctx);
            if rows.is_empty() {
                continue;
            }
            if !out.is_empty() {
                out.extend(std::iter::repeat_with(Vec::new).take(gap));
            }
            out.extend(rows);
        }
        out
    }

    fn block(&mut self, block: &Block, width: usize, ctx: Ctx) -> Vec<Row> {
        match block {
            Block::Heading { level, lines } => self.heading(*level, lines, width),
            Block::Paragraph(flow) => self.paragraph(flow, width, ctx),
            Block::Math(rows) => self.math(rows, width),
            Block::List { start, loose, items } => self.list(*start, *loose, items, width, ctx),
            Block::Code { lang, lines } => self.code(lang, lines, width),
            Block::Quote(blocks) => self.quote(blocks, width, ctx),
            Block::Table { align, head, rows } => self.table(align, head, rows, width, ctx),
            Block::Rule => {
                let n = width / width_of(&self.theme.rule).max(1);
                let st = self.theme.styles.muted;
                vec![vec![self.span(self.theme.rule.repeat(n), Role::Rule, st)]]
            }
        }
    }

    fn heading(&mut self, level: u8, lines: &[RichLine], width: usize) -> Vec<Row> {
        let base = if level == 1 { self.theme.styles.heading1 } else { self.theme.styles.heading };
        let tint = Some((Role::Heading(level), base));
        let mut out = Vec::new();
        for line in lines {
            let spans = self.line_spans(line, tint);
            out.extend(wrap(spans, width, Wrap::Word));
        }
        let rule = match level {
            1 => self.theme.h1_rule.clone(),
            2 => self.theme.h2_rule.clone(),
            _ => None,
        };
        if let Some(g) = rule {
            let w = out.iter().map(|r| row_width(r)).max().unwrap_or(0).min(width);
            let n = w / width_of(&g).max(1);
            let st = TextStyle { weight: TextWeight::Normal, ..base };
            out.push(vec![self.span(g.repeat(n), Role::HeadingRule, st)]);
        }
        out
    }

    fn paragraph(&mut self, flow: &[Flow], width: usize, ctx: Ctx) -> Vec<Row> {
        let tint = self.tint(ctx);
        let mut out = Vec::new();
        let mut pending: Option<Row> = None;
        for f in flow {
            match f {
                Flow::Line { line, soft } => {
                    let mut spans = self.line_spans(line, tint);
                    if let Some(mut acc) = pending.take() {
                        let sp = self.span(" ", Role::Text, self.theme.styles.text);
                        acc.push(sp);
                        acc.append(&mut spans);
                        spans = acc;
                    }
                    if *soft && self.theme.soft_break == SoftBreak::Space {
                        pending = Some(spans);
                    } else {
                        out.extend(wrap(spans, width, Wrap::Word));
                    }
                }
                Flow::Math(rows) => {
                    if let Some(acc) = pending.take() {
                        out.extend(wrap(acc, width, Wrap::Word));
                    }
                    out.extend(self.math(rows, width));
                }
            }
        }
        if let Some(acc) = pending {
            out.extend(wrap(acc, width, Wrap::Word));
        }
        out
    }

    /// Cells laid out as columns: in an `aligned` row the cells alternate
    /// right- and left-aligned, so every `=` after a `&` shares one column.
    fn math(&mut self, rows: &MathRows, width: usize) -> Vec<Row> {
        let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
        let widths: Vec<usize> =
            (0..cols).map(|c| rows.iter().filter_map(|r| r.get(c)).map(|s| width_of(s)).max().unwrap_or(0)).collect();
        let lines: Vec<String> = rows
            .iter()
            .map(|r| {
                if cols <= 1 {
                    return r.first().cloned().unwrap_or_default();
                }
                let mut s = String::new();
                for (c, w) in widths.iter().enumerate() {
                    let cell = r.get(c).map_or("", String::as_str);
                    if c > 0 {
                        s.push(' ');
                    }
                    let gap = w.saturating_sub(width_of(cell));
                    if c % 2 == 0 {
                        s.push_str(&" ".repeat(gap));
                        s.push_str(cell);
                    } else {
                        s.push_str(cell);
                        s.push_str(&" ".repeat(gap));
                    }
                }
                s.trim_end().to_owned()
            })
            .collect();
        let widest = lines.iter().map(|l| width_of(l)).max().unwrap_or(0);
        let indent = if widest + self.theme.math_indent <= width { self.theme.math_indent } else { 0 };
        let st = TextStyle::colored(colors::MATH);
        let mut out = Vec::new();
        for l in lines {
            let sp = self.span(l, Role::Math, st);
            let lead = [Span::plain(" ".repeat(indent))];
            out.extend(prefix(wrap_hang(vec![sp], width - indent, Wrap::Word, 2), &lead, &lead));
        }
        out
    }

    fn list(&mut self, start: Option<u64>, loose: bool, items: &[Item], width: usize, ctx: Ctx) -> Vec<Row> {
        let last = start.map(|s| s + items.len().saturating_sub(1) as u64);
        let num_w = last.map_or(0, |n| n.to_string().len() + 1);
        let gap = if loose { self.theme.block_gap } else { 0 };
        let marker_style = self.theme.styles.marker;
        let mut out: Vec<Row> = Vec::new();
        for (i, item) in items.iter().enumerate() {
            let marker = match (start, item.task) {
                (_, Some(done)) => (if done { &self.theme.task_done } else { &self.theme.task_open }).clone(),
                (Some(s), None) => format!("{:>num_w$}", format!("{}.", s + i as u64)),
                (None, None) => {
                    let b = &self.theme.bullets;
                    b.get(ctx.depth % b.len().max(1)).cloned().unwrap_or_else(|| "•".into())
                }
            };
            let mw = width_of(&marker) + 1;
            let inner = width.saturating_sub(mw).max(1);
            let rows = self.blocks_gap(&item.blocks, inner, Ctx { depth: ctx.depth + 1, ..ctx }, gap);
            let rows = if rows.is_empty() { vec![Vec::new()] } else { rows };
            let first = [self.span(format!("{marker} "), Role::ListMarker, marker_style)];
            let rest = [Span::plain(" ".repeat(mw))];
            if i > 0 {
                out.extend(std::iter::repeat_with(Vec::new).take(gap));
            }
            out.extend(prefix(rows, &first, &rest));
        }
        out
    }

    fn quote(&mut self, blocks: &[Block], width: usize, ctx: Ctx) -> Vec<Row> {
        let bar = format!("{} ", self.theme.quote_bar);
        let inner = width.saturating_sub(width_of(&bar)).max(1);
        let rows = self.blocks(blocks, inner, Ctx { quote: true, ..ctx });
        let st = self.theme.styles.muted;
        let b = [self.span(bar, Role::QuoteBar, st)];
        rows.into_iter()
            .map(|r| {
                let mut p = b.to_vec();
                p.extend(r);
                p
            })
            .collect()
    }

    fn code_line(&mut self, line: &str, lang: &str, bg: Option<[f32; 4]>) -> Row {
        let rich = if self.theme.code.highlight {
            self.highlighter.highlight_line(line, lang)
        } else {
            RichLine::from_spans(vec![crate::span::StyledSpan::plain(line)])
        };
        rich.spans
            .iter()
            .map(|s| {
                let role = match s.style.color {
                    c if c == colors::KEYWORD => Role::CodeKeyword,
                    c if c == colors::STRING => Role::CodeString,
                    c if c == colors::COMMENT => Role::CodeComment,
                    c if c == colors::NUMBER => Role::CodeNumber,
                    _ => Role::Code,
                };
                let st = if role == Role::Code { TextStyle { color: colors::CODE, ..s.style } } else { s.style };
                let i = self.intern(role, st, bg);
                Span::new(s.text.replace('\t', "    "), i)
            })
            .collect()
    }

    fn code(&mut self, lang: &str, lines: &[String], width: usize) -> Vec<Row> {
        let t = self.theme.code.clone();
        let bg = t.background;
        let frame_st = self.theme.styles.muted;
        let label_st = self.theme.styles.code_label;
        let (left, right) = match t.frame {
            CodeFrame::Box => ("│ ", " │"),
            CodeFrame::Bar => ("▌ ", ""),
            CodeFrame::None => ("  ", ""),
        };
        let inner = width.saturating_sub(width_of(left) + width_of(right)).max(1);
        let mut body: Vec<Row> = Vec::new();
        for line in lines {
            let spans = self.code_line(line, lang, bg);
            let mut rows = if t.overflow == CodeOverflow::Clip && row_width(&spans) > inner {
                let mut r = wrap(spans, inner.saturating_sub(1).max(1), Wrap::Grapheme);
                r.truncate(1);
                let ell = self.intern(Role::CodeFrame, frame_st, bg);
                if let Some(first) = r.first_mut() {
                    first.push(Span::new("…", ell));
                }
                r
            } else {
                wrap(spans, inner, Wrap::Grapheme)
            };
            body.append(&mut rows);
        }
        let fill = self.intern(Role::Code, TextStyle::colored(colors::CODE), bg);
        let l = self.span(left, Role::CodeFrame, frame_st);
        let r = self.span(right, Role::CodeFrame, frame_st);
        let mut out = Vec::new();
        let label = (t.label && !lang.is_empty()).then(|| lang.to_owned());
        match t.frame {
            CodeFrame::Box => {
                let mut top = vec![self.span("╭─", Role::CodeFrame, frame_st)];
                let mut used = 2;
                if let Some(lab) = &label {
                    top.push(self.span(format!(" {lab} "), Role::CodeLabel, label_st));
                    used += width_of(lab) + 2;
                }
                let n = width.saturating_sub(used + 1);
                top.push(self.span(format!("{}╮", "─".repeat(n)), Role::CodeFrame, frame_st));
                out.push(top);
            }
            CodeFrame::Bar | CodeFrame::None => {
                if let Some(lab) = &label {
                    out.push(vec![Span::plain(" ".repeat(width_of(left))), self.span(lab.clone(), Role::CodeLabel, label_st)]);
                }
            }
        }
        for mut row in body {
            let gap = inner.saturating_sub(row_width(&row));
            let mut full = vec![l.clone()];
            full.append(&mut row);
            if gap > 0 && (bg.is_some() || t.frame == CodeFrame::Box) {
                full.push(Span::new(" ".repeat(gap), fill));
            }
            if !right.is_empty() {
                full.push(r.clone());
            }
            out.push(full);
        }
        if t.frame == CodeFrame::Box {
            let n = width.saturating_sub(2);
            out.push(vec![self.span(format!("╰{}╯", "─".repeat(n)), Role::CodeFrame, frame_st)]);
        }
        out
    }

    /// Columns shrink widest-first until the table fits; each cell then
    /// word-wraps inside its column, so a row can span several lines.
    fn table(&mut self, align: &[Align], head: &[RichLine], rows: &[Vec<RichLine>], width: usize, ctx: Ctx) -> Vec<Row> {
        let cols = std::iter::once(head.len()).chain(rows.iter().map(Vec::len)).max().unwrap_or(0);
        if cols == 0 {
            return Vec::new();
        }
        let sep = " │ ";
        let sep_w = width_of(sep);
        let natural: Vec<usize> = (0..cols)
            .map(|c| {
                std::iter::once(head)
                    .chain(rows.iter().map(Vec::as_slice))
                    .filter_map(|r| r.get(c))
                    .map(RichLine::total_width)
                    .max()
                    .unwrap_or(0)
                    .max(1)
            })
            .collect();
        let avail = width.saturating_sub(sep_w * (cols - 1)).max(cols);
        // A column first gives up the room its longest word does not need,
        // widest first; only then do words themselves break.
        let longest_word: Vec<usize> = (0..cols)
            .map(|c| {
                std::iter::once(head)
                    .chain(rows.iter().map(Vec::as_slice))
                    .filter_map(|r| r.get(c))
                    .flat_map(|cell| cell.plain_text().split_whitespace().map(width_of).collect::<Vec<_>>())
                    .max()
                    .unwrap_or(1)
            })
            .collect();
        let mut w = natural;
        while w.iter().sum::<usize>() > avail {
            let slack = w.iter().enumerate().filter(|(i, x)| **x > longest_word[*i]).max_by_key(|(_, x)| **x);
            let Some((i, _)) = slack.or_else(|| w.iter().enumerate().filter(|(_, x)| **x > 3).max_by_key(|(_, x)| **x))
            else {
                break;
            };
            w[i] -= 1;
        }
        let tint = self.tint(ctx);
        let rule_st = self.theme.styles.muted;
        let sep_span = self.span(sep, Role::TableRule, rule_st);
        let mut out = Vec::new();
        let emit = |this: &mut Self, cells: &[RichLine], is_head: bool, out: &mut Vec<Row>| {
            let wrapped: Vec<Vec<Row>> = (0..cols)
                .map(|c| {
                    let spans = cells.get(c).map_or_else(Vec::new, |cell| {
                        cell.spans
                            .iter()
                            .map(|s| {
                                if is_head {
                                    let st = TextStyle { weight: TextWeight::Bold, ..s.style };
                                    let st = if st.color == TextStyle::default().color {
                                        TextStyle { color: this.theme.styles.text.color, ..st }
                                    } else {
                                        st
                                    };
                                    this.span(s.text.clone(), Role::TableHead, st)
                                } else {
                                    this.inline(&s.text, s.style, tint)
                                }
                            })
                            .collect()
                    });
                    wrap(spans, w[c], Wrap::Word)
                })
                .collect();
            let height = wrapped.iter().map(Vec::len).max().unwrap_or(1);
            for line in 0..height {
                let mut row: Row = Vec::new();
                for c in 0..cols {
                    if c > 0 {
                        row.push(sep_span.clone());
                    }
                    let part = wrapped[c].get(line).cloned().unwrap_or_default();
                    let text: String = part.iter().map(Span::text).collect();
                    let (l, r) = pad(&text, w[c], align.get(c).copied().unwrap_or_default());
                    if l > 0 {
                        row.push(Span::plain(" ".repeat(l)));
                    }
                    row.extend(part);
                    if r > 0 && c + 1 < cols {
                        row.push(Span::plain(" ".repeat(r)));
                    }
                }
                out.push(row);
            }
        };
        if !head.is_empty() {
            emit(self, head, true, &mut out);
            let rule: Vec<String> = w.iter().map(|x| "─".repeat(*x)).collect();
            out.push(vec![self.span(rule.join("─┼─"), Role::TableRule, rule_st)]);
        }
        for r in rows {
            emit(self, r, false, &mut out);
        }
        out
    }
}
