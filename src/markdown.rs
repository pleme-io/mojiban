use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::colors;
use crate::highlight::SyntaxHighlighter;
use crate::span::{InlineKind, RichLine, StyledSpan, TextStyle, TextWeight};

/// Stateless markdown-to-styled-spans processor.
///
/// Uses pulldown-cmark to parse `CommonMark` markdown and produce
/// [`RichLine`]s with appropriate styling.
pub struct MarkdownParser;

impl MarkdownParser {
    /// Create a new parser instance.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Parse markdown source into a vector of styled lines.
    ///
    /// Supported formatting:
    /// - `**bold**` — bold weight
    /// - `*italic*` — italic style
    /// - `~~strike~~` — strikethrough
    /// - `` `code` `` — frost accent color
    /// - `# Heading` — bold weight
    /// - `> quote` — muted color
    /// - `- item` / `1. item` — plain with bullet/number prefix
    /// - `$…$` / `\\(…\\)` — inline TeX math, rendered to Unicode
    /// - `$$…$$` / `\\[…\\]` — display TeX math, its own indented lines
    /// - Plain text — default style
    #[must_use]
    pub fn parse(&self, markdown: &str) -> Vec<RichLine> {
        let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES | Options::ENABLE_MATH;
        let markdown = normalize_math_delimiters(markdown);
        let mut b = LineBuilder::new();
        for event in Parser::new_ext(&markdown, options) {
            if b.table.is_some() {
                b.table_event(&event);
            } else {
                b.event(event);
            }
        }
        b.finish()
    }
}

/// The running state of one [`MarkdownParser::parse`] pass.
struct LineBuilder {
    lines: Vec<RichLine>,
    current_line: RichLine,
    style_stack: Vec<TextStyle>,
    list_stack: Vec<ListKind>,
    need_list_prefix: bool,
    // Inside a fenced/indented code block: its language (possibly "").
    // Code text arrives as ONE event carrying embedded newlines; it is
    // split into one RichLine per source line and each line highlighted,
    // rather than emitted as a single span a renderer draws as one row.
    code_lang: Option<String>,
    highlighter: SyntaxHighlighter,
    // Inside a GFM table: its rows, laid out when the table ends. Cell
    // text is collected per cell instead of into `current_line`, which
    // is what flattened a whole table into one run-on line before.
    table: Option<TableBuf>,
}

impl LineBuilder {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            current_line: RichLine::new(),
            style_stack: vec![TextStyle::default()],
            list_stack: Vec::new(),
            need_list_prefix: false,
            code_lang: None,
            highlighter: SyntaxHighlighter::new(),
            table: None,
        }
    }

    fn style(&self) -> TextStyle {
        self.style_stack.last().copied().unwrap_or_default()
    }

    fn flush_line(&mut self) {
        if !self.current_line.spans.is_empty() {
            self.lines.push(std::mem::take(&mut self.current_line));
        }
    }

    fn finish(mut self) -> Vec<RichLine> {
        if !self.current_line.is_empty() {
            self.lines.push(self.current_line);
        }
        self.lines
    }

    fn table_event(&mut self, event: &Event<'_>) {
        let base = self.style();
        let Some(t) = self.table.as_mut() else { return };
        match event {
            Event::Start(Tag::TableHead) => t.in_head = true,
            Event::End(TagEnd::TableHead) => {
                t.finish_row();
                t.in_head = false;
            }
            Event::End(TagEnd::TableRow) => t.finish_row(),
            Event::End(TagEnd::TableCell) => t.finish_cell(),
            Event::Text(text) | Event::Code(text) => {
                let mut style = base;
                if matches!(event, Event::Code(_)) {
                    style.color = colors::CODE;
                    style.kind = InlineKind::Code;
                }
                if t.in_head {
                    style.weight = TextWeight::Bold;
                }
                t.cell.push(StyledSpan::new(text.to_string(), style));
            }
            Event::End(TagEnd::Table) => {
                if let Some(done) = self.table.take() {
                    self.lines.extend(done.layout());
                }
                return;
            }
            _ => {}
        }
        // Inline emphasis inside a cell still needs its style frame.
        match event {
            Event::Start(Tag::Emphasis) => self.style_stack.push(TextStyle {
                italic: true,
                ..base
            }),
            Event::Start(Tag::Strong) => {
                self.style_stack.push(TextStyle {
                    weight: TextWeight::Bold,
                    ..base
                });
            }
            Event::End(TagEnd::Emphasis | TagEnd::Strong) => {
                self.style_stack.pop();
            }
            _ => {}
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(Tag::Table(_)) => {
                self.flush_line();
                self.table = Some(TableBuf::default());
            }
            Event::Start(tag) => self.start(&tag),
            Event::End(tag_end) => self.end(tag_end),
            Event::Text(text) if self.code_lang.is_some() => {
                let lang = self.code_lang.as_deref().unwrap_or("");
                for line in text.lines() {
                    self.lines.push(self.highlighter.highlight_line(line, lang));
                }
            }
            Event::Text(text) => {
                let style = self.style();
                self.inline(text.to_string(), style);
            }
            Event::Code(code) => {
                let style = TextStyle {
                    color: colors::CODE,
                    kind: InlineKind::Code,
                    ..self.style()
                };
                self.inline(code.to_string(), style);
            }
            Event::InlineMath(tex) => {
                let style = TextStyle {
                    color: colors::MATH,
                    kind: InlineKind::Math,
                    ..self.style()
                };
                self.inline(crate::math::tex_to_unicode(&tex).join(" "), style);
            }
            Event::DisplayMath(tex) => self.display_math(&tex),
            Event::SoftBreak | Event::HardBreak => {
                self.lines.push(std::mem::take(&mut self.current_line));
            }
            _ => {}
        }
    }

    fn start(&mut self, tag: &Tag<'_>) {
        let mut style = self.style();
        match tag {
            Tag::Emphasis => style.italic = true,
            Tag::Strikethrough => style.strikethrough = true,
            Tag::Strong | Tag::Heading { .. } => style.weight = TextWeight::Bold,
            Tag::BlockQuote(_) => style.color = colors::QUOTE,
            Tag::List(start) => {
                self.list_stack
                    .push(start.map_or(ListKind::Unordered, ListKind::Ordered));
            }
            Tag::Item => self.need_list_prefix = true,
            Tag::CodeBlock(kind) => {
                self.flush_line();
                self.code_lang = Some(match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(info) => {
                        info.split_whitespace().next().unwrap_or("").to_owned()
                    }
                    pulldown_cmark::CodeBlockKind::Indented => String::new(),
                });
            }
            _ => {}
        }
        self.style_stack.push(style);
    }

    fn end(&mut self, tag_end: TagEnd) {
        self.style_stack.pop();
        match tag_end {
            TagEnd::Paragraph | TagEnd::Heading(_) | TagEnd::BlockQuote(_) => {
                self.lines.push(std::mem::take(&mut self.current_line));
            }
            TagEnd::Item => {
                self.lines.push(std::mem::take(&mut self.current_line));
                // Increment ordered list counter for next item
                if let Some(ListKind::Ordered(start)) = self.list_stack.last_mut() {
                    *start += 1;
                }
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
            }
            TagEnd::CodeBlock => self.code_lang = None,
            _ => {}
        }
    }

    /// Push an inline run, preceded by a pending list marker in the base style.
    fn inline(&mut self, text: String, style: TextStyle) {
        if self.need_list_prefix {
            if let Some(prefix) = list_prefix(&self.list_stack) {
                let base = self.style();
                self.current_line.push(StyledSpan::new(prefix, base));
            }
            self.need_list_prefix = false;
        }
        self.current_line.push(StyledSpan::new(text, style));
    }

    fn display_math(&mut self, tex: &str) {
        self.flush_line();
        let style = TextStyle {
            color: colors::MATH,
            kind: InlineKind::Math,
            ..TextStyle::default()
        };
        for l in crate::math::tex_to_unicode(tex) {
            let mut line = RichLine::new();
            line.push(StyledSpan::new(format!("{DISPLAY_MATH_INDENT}{l}"), style));
            self.lines.push(line);
        }
    }
}

impl Default for MarkdownParser {
    fn default() -> Self {
        Self::new()
    }
}

impl crate::TextProcessor for MarkdownParser {
    fn process(&self, input: &str) -> Vec<RichLine> {
        self.parse(input)
    }
}

/// A GFM table while it is being read: rows of cells, each cell its styled
/// spans. Laid out as aligned columns once the whole table is known.
#[derive(Debug, Default)]
struct TableBuf {
    in_head: bool,
    /// (is the header row, its cells)
    rows: Vec<(bool, Vec<Vec<StyledSpan>>)>,
    row: Vec<Vec<StyledSpan>>,
    cell: Vec<StyledSpan>,
}

impl TableBuf {
    fn finish_cell(&mut self) {
        self.row.push(std::mem::take(&mut self.cell));
    }

    fn finish_row(&mut self) {
        if !self.row.is_empty() {
            self.rows
                .push((self.in_head, std::mem::take(&mut self.row)));
        }
    }

    fn width(cell: &[StyledSpan]) -> usize {
        cell.iter()
            .map(|s| unicode_width::UnicodeWidthStr::width(s.text.as_str()))
            .sum()
    }

    /// One line per row, columns padded to their widest cell and joined by
    /// ` │ `, with a `─┼─` rule under the header.
    fn layout(self) -> Vec<RichLine> {
        let cols = self.rows.iter().map(|(_, r)| r.len()).max().unwrap_or(0);
        let widths: Vec<usize> = (0..cols)
            .map(|c| {
                self.rows
                    .iter()
                    .map(|(_, r)| r.get(c).map_or(0, |cell| Self::width(cell)))
                    .max()
                    .unwrap_or(0)
            })
            .collect();
        let rule_style = TextStyle {
            color: colors::QUOTE,
            ..TextStyle::default()
        };
        let mut out = Vec::new();
        for (is_head, row) in self.rows {
            let mut line = RichLine::new();
            for (c, w) in widths.iter().enumerate() {
                if c > 0 {
                    line.push(StyledSpan::new(" \u{2502} ", rule_style));
                }
                let cell = row.get(c).cloned().unwrap_or_default();
                let pad = w.saturating_sub(Self::width(&cell));
                for span in cell {
                    line.push(span);
                }
                if pad > 0 && c + 1 < widths.len() {
                    line.push(StyledSpan::new(" ".repeat(pad), TextStyle::default()));
                }
            }
            out.push(line);
            if is_head {
                let rule: Vec<String> = widths.iter().map(|w| "\u{2500}".repeat(*w)).collect();
                let mut r = RichLine::new();
                r.push(StyledSpan::new(
                    rule.join("\u{2500}\u{253c}\u{2500}"),
                    rule_style,
                ));
                out.push(r);
            }
        }
        out
    }
}

/// Display math sits indented under the prose around it.
const DISPLAY_MATH_INDENT: &str = "    ";

/// Rewrite LaTeX's `\\[…\\]` and `\\(…\\)` to the `$$…$$` / `$…$` pulldown-cmark
/// recognises. Without this, `CommonMark` reads `\\[` as an escaped bracket and
/// a model's display math renders as a bare `[` over raw TeX. Only a pair
/// with its closer converts; fenced code and inline code spans are skipped.
pub(crate) fn normalize_math_delimiters(src: &str) -> std::borrow::Cow<'_, str> {
    if !src.contains("\\[") && !src.contains("\\(") {
        return std::borrow::Cow::Borrowed(src);
    }
    std::borrow::Cow::Owned(convert_pairs(src))
}

fn convert_pairs(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    let mut line_start = true;
    let mut in_fence = false;
    let mut in_code: usize = 0;
    while i < bytes.len() {
        if line_start {
            let rest = &src[i..];
            let t = rest.trim_start_matches([' ', '\t']);
            if t.starts_with("```") || t.starts_with("~~~") {
                in_fence = !in_fence;
            }
        }
        let c = bytes[i];
        line_start = c == b'\n';
        if !in_fence && c == b'`' {
            let run = bytes[i..].iter().take_while(|&&b| b == b'`').count();
            if in_code == 0 {
                in_code = run;
            } else if in_code == run {
                in_code = 0;
            }
            out.push_str(&src[i..i + run]);
            i += run;
            continue;
        }
        if !in_fence
            && in_code == 0
            && c == b'\\'
            && i + 1 < bytes.len()
            && matches!(bytes[i + 1], b'[' | b'(')
        {
            let (close, delim) = if bytes[i + 1] == b'[' {
                ("\\]", "$$")
            } else {
                ("\\)", "$")
            };
            if let Some(end) = src[i + 2..].find(close) {
                let body = &src[i + 2..i + 2 + end];
                out.push_str(delim);
                out.push_str(body.trim());
                out.push_str(delim);
                i += 2 + end + 2;
                continue;
            }
        }
        let ch = src[i..].chars().next().unwrap_or(' ');
        out.push(ch);
        i += ch.len_utf8();
    }
    out
}

/// Tracks whether a list is ordered or unordered.
#[derive(Debug, Clone)]
enum ListKind {
    Ordered(u64),
    Unordered,
}

impl ListKind {
    /// Build the text prefix for a list item (bullet or number).
    fn prefix(&self) -> String {
        match self {
            Self::Unordered => "\u{2022} ".to_owned(),
            Self::Ordered(n) => format!("{n}. "),
        }
    }
}

/// Build the text prefix for the current list context.
fn list_prefix(stack: &[ListKind]) -> Option<String> {
    stack.last().map(ListKind::prefix)
}

#[cfg(test)]
#[allow(clippy::float_cmp)]
mod tests {
    use super::*;
    use crate::TextProcessor;

    fn parser() -> MarkdownParser {
        MarkdownParser::new()
    }

    #[test]
    fn plain_text_passthrough() {
        let lines = parser().parse("hello world");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].plain_text(), "hello world");
        // Should have default style
        assert_eq!(lines[0].spans[0].style, TextStyle::default());
    }

    #[test]
    fn bold_text() {
        let lines = parser().parse("**bold**");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.text, "bold");
        assert_eq!(span.style.weight, TextWeight::Bold);
    }

    #[test]
    fn italic_text() {
        let lines = parser().parse("*italic*");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.text, "italic");
        assert!(span.style.italic);
    }

    #[test]
    fn strikethrough_text() {
        let lines = parser().parse("~~strike~~");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.text, "strike");
        assert!(span.style.strikethrough);
    }

    #[test]
    fn inline_code() {
        let lines = parser().parse("`code`");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.text, "code");
        assert_eq!(span.style.color, colors::CODE);
    }

    #[test]
    fn heading() {
        let lines = parser().parse("# Heading");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.text, "Heading");
        assert_eq!(span.style.weight, TextWeight::Bold);
    }

    #[test]
    fn blockquote() {
        let lines = parser().parse("> quoted text");
        // pulldown-cmark wraps blockquote content in a paragraph,
        // so we may get multiple lines from End(Paragraph) + End(BlockQuote).
        // Find the line that contains the quoted text.
        let quote_line = lines
            .iter()
            .find(|l| l.plain_text().contains("quoted text"));
        assert!(quote_line.is_some(), "should find quoted text in output");
        let span = &quote_line.unwrap().spans[0];
        assert_eq!(span.text, "quoted text");
        assert_eq!(span.style.color, colors::QUOTE);
    }

    #[test]
    fn unordered_list() {
        let lines = parser().parse("- item one\n- item two");
        assert_eq!(lines.len(), 2);
        assert!(lines[0].plain_text().contains("item one"));
        assert!(lines[1].plain_text().contains("item two"));
        // Should have bullet prefix
        assert!(lines[0].plain_text().starts_with('\u{2022}'));
    }

    #[test]
    fn ordered_list() {
        let lines = parser().parse("1. first\n2. second");
        assert_eq!(lines.len(), 2);
        assert!(lines[0].plain_text().contains("first"));
        assert!(lines[1].plain_text().contains("second"));
        assert!(lines[0].plain_text().starts_with("1."));
    }

    #[test]
    fn multiple_paragraphs() {
        let lines = parser().parse("para one\n\npara two");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].plain_text(), "para one");
        assert_eq!(lines[1].plain_text(), "para two");
    }

    #[test]
    fn mixed_formatting() {
        let lines = parser().parse("normal **bold** *italic*");
        assert_eq!(lines.len(), 1);
        // Should have at least 3 spans (normal, bold, italic) plus whitespace
        assert!(lines[0].spans.len() >= 3);
        // Find the bold span
        let bold_span = lines[0].spans.iter().find(|s| s.text == "bold");
        assert!(bold_span.is_some());
        assert_eq!(bold_span.unwrap().style.weight, TextWeight::Bold);
        // Find the italic span
        let italic_span = lines[0].spans.iter().find(|s| s.text == "italic");
        assert!(italic_span.is_some());
        assert!(italic_span.unwrap().style.italic);
    }

    #[test]
    fn empty_input() {
        let lines = parser().parse("");
        assert!(lines.is_empty());
    }

    #[test]
    fn bold_and_italic_combined() {
        let lines = parser().parse("***both***");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.text, "both");
        assert_eq!(span.style.weight, TextWeight::Bold);
        assert!(span.style.italic);
    }

    #[test]
    fn parser_default_trait() {
        let p = MarkdownParser;
        let lines = p.parse("test");
        assert_eq!(lines.len(), 1);
    }

    // ---- Heading levels ----

    #[test]
    fn heading_level_2() {
        let lines = parser().parse("## Sub heading");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.text, "Sub heading");
        assert_eq!(span.style.weight, TextWeight::Bold);
    }

    #[test]
    fn heading_level_3() {
        let lines = parser().parse("### Third level");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].style.weight, TextWeight::Bold);
    }

    #[test]
    fn heading_level_6() {
        let lines = parser().parse("###### Deepest");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].spans[0].text, "Deepest");
        assert_eq!(lines[0].spans[0].style.weight, TextWeight::Bold);
    }

    #[test]
    fn multiple_headings() {
        let input = "# First\n\n## Second\n\n### Third";
        let lines = parser().parse(input);
        assert_eq!(lines.len(), 3);
        for line in &lines {
            assert_eq!(line.spans[0].style.weight, TextWeight::Bold);
        }
        assert_eq!(lines[0].plain_text(), "First");
        assert_eq!(lines[1].plain_text(), "Second");
        assert_eq!(lines[2].plain_text(), "Third");
    }

    // ---- Nested formatting ----

    #[test]
    fn bold_inside_italic() {
        // *italic **bold-italic** italic*
        let lines = parser().parse("*start **both** end*");
        assert_eq!(lines.len(), 1);
        // Find the span that has both bold and italic
        let both = lines[0].spans.iter().find(|s| s.text == "both");
        assert!(both.is_some(), "should find 'both' span");
        let both_style = &both.unwrap().style;
        assert_eq!(both_style.weight, TextWeight::Bold);
        assert!(both_style.italic);
    }

    #[test]
    fn code_in_bold_context() {
        // **bold `code` bold**
        let lines = parser().parse("**before `code` after**");
        assert_eq!(lines.len(), 1);
        let code_span = lines[0].spans.iter().find(|s| s.text == "code");
        assert!(code_span.is_some(), "should find inline code span");
        assert_eq!(code_span.unwrap().style.color, colors::CODE);
    }

    #[test]
    fn strikethrough_with_bold() {
        let lines = parser().parse("~~**both**~~");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.text, "both");
        assert!(span.style.strikethrough);
        assert_eq!(span.style.weight, TextWeight::Bold);
    }

    // ---- Whitespace and edge cases ----

    #[test]
    fn whitespace_only_input() {
        let lines = parser().parse("   ");
        // whitespace-only is not a paragraph — pulldown-cmark may return empty
        // or a single line; the key constraint is no panic
        for line in &lines {
            // If any line, its plain_text should be whitespace
            assert!(line.plain_text().trim().is_empty() || line.is_empty());
        }
    }

    #[test]
    fn newline_only_input() {
        let lines = parser().parse("\n\n\n");
        // Multiple blank lines should produce no meaningful content
        assert!(lines.is_empty() || lines.iter().all(|l| l.plain_text().is_empty()));
    }

    #[test]
    fn single_character() {
        let lines = parser().parse("x");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].plain_text(), "x");
    }

    #[test]
    fn unicode_content_preserved() {
        let lines = parser().parse("\u{6587}\u{5B57}\u{76E4}"); // 文字盤
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].plain_text(), "\u{6587}\u{5B57}\u{76E4}");
    }

    // ---- Soft break and hard break ----

    #[test]
    fn soft_break_creates_new_line() {
        // A single newline within a paragraph is a soft break
        let lines = parser().parse("line one\nline two");
        // pulldown-cmark emits SoftBreak between the two lines
        // Our parser flushes current_line on SoftBreak, so we get 2+ lines
        let all_text: String = lines
            .iter()
            .map(RichLine::plain_text)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(all_text.contains("line one"));
        assert!(all_text.contains("line two"));
    }

    #[test]
    fn hard_break_creates_new_line() {
        // Two trailing spaces followed by newline = hard break
        let lines = parser().parse("first  \nsecond");
        let all_text: String = lines
            .iter()
            .map(RichLine::plain_text)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(all_text.contains("first"));
        assert!(all_text.contains("second"));
    }

    // ---- Lists: deeper coverage ----

    #[test]
    fn unordered_list_bullet_prefix() {
        let lines = parser().parse("- alpha\n- beta\n- gamma");
        assert_eq!(lines.len(), 3);
        for line in &lines {
            assert!(
                line.plain_text().starts_with('\u{2022}'),
                "each unordered list item should start with bullet: {:?}",
                line.plain_text()
            );
        }
    }

    #[test]
    fn ordered_list_increments() {
        let lines = parser().parse("1. one\n2. two\n3. three");
        assert_eq!(lines.len(), 3);
        assert!(lines[0].plain_text().starts_with("1."));
        assert!(lines[1].plain_text().starts_with("2."));
        assert!(lines[2].plain_text().starts_with("3."));
    }

    #[test]
    fn ordered_list_auto_increments_from_start() {
        // pulldown-cmark normalizes ordered list start numbers;
        // our parser increments from whatever start it receives
        let lines = parser().parse("5. five\n6. six");
        // pulldown-cmark may renumber from 5 or from 1 depending on spec;
        // key: each item has a numeric prefix
        for line in &lines {
            let text = line.plain_text();
            assert!(
                text.chars().next().unwrap_or(' ').is_ascii_digit(),
                "ordered item should start with digit: {text:?}"
            );
        }
    }

    #[test]
    fn list_with_inline_code() {
        let lines = parser().parse("- `code item`");
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("code item"));
        // The code span should have CODE color
        let code_span = lines[0].spans.iter().find(|s| s.text == "code item");
        assert!(code_span.is_some(), "should find code span in list item");
        assert_eq!(code_span.unwrap().style.color, colors::CODE);
    }

    #[test]
    fn list_with_bold_item() {
        let lines = parser().parse("- **bold item**");
        assert_eq!(lines.len(), 1);
        let bold_span = lines[0].spans.iter().find(|s| s.text == "bold item");
        assert!(bold_span.is_some());
        assert_eq!(bold_span.unwrap().style.weight, TextWeight::Bold);
    }

    // ---- Block quote deeper coverage ----

    #[test]
    fn blockquote_multiple_lines() {
        let lines = parser().parse("> line one\n> line two");
        let all_text: String = lines
            .iter()
            .map(RichLine::plain_text)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(all_text.contains("line one"));
        assert!(all_text.contains("line two"));
        // All content spans should have QUOTE color
        for line in &lines {
            for span in &line.spans {
                if !span.text.trim().is_empty() {
                    assert_eq!(
                        span.style.color,
                        colors::QUOTE,
                        "blockquote span should have QUOTE color"
                    );
                }
            }
        }
    }

    #[test]
    fn blockquote_with_bold() {
        let lines = parser().parse("> **bold quote**");
        let bold_span = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .find(|s| s.text == "bold quote");
        assert!(bold_span.is_some());
        let style = &bold_span.unwrap().style;
        assert_eq!(style.weight, TextWeight::Bold);
        assert_eq!(style.color, colors::QUOTE);
    }

    // ---- Inline code edge cases ----

    #[test]
    fn inline_code_with_special_characters() {
        let lines = parser().parse("`fn main() {}`");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.text, "fn main() {}");
        assert_eq!(span.style.color, colors::CODE);
    }

    #[test]
    fn multiple_inline_codes() {
        let lines = parser().parse("`a` and `b`");
        assert_eq!(lines.len(), 1);
        let code_spans: Vec<_> = lines[0]
            .spans
            .iter()
            .filter(|s| s.style.color == colors::CODE)
            .collect();
        assert_eq!(code_spans.len(), 2);
        assert_eq!(code_spans[0].text, "a");
        assert_eq!(code_spans[1].text, "b");
    }

    // ---- Mixed content paragraphs ----

    #[test]
    fn paragraph_with_all_inline_styles() {
        let lines = parser().parse("normal **bold** *italic* ~~strike~~ `code`");
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("normal"));
        assert!(text.contains("bold"));
        assert!(text.contains("italic"));
        assert!(text.contains("strike"));
        assert!(text.contains("code"));

        let bold = lines[0].spans.iter().find(|s| s.text == "bold").unwrap();
        assert_eq!(bold.style.weight, TextWeight::Bold);

        let italic = lines[0].spans.iter().find(|s| s.text == "italic").unwrap();
        assert!(italic.style.italic);

        let strike = lines[0].spans.iter().find(|s| s.text == "strike").unwrap();
        assert!(strike.style.strikethrough);

        let code = lines[0].spans.iter().find(|s| s.text == "code").unwrap();
        assert_eq!(code.style.color, colors::CODE);
    }

    // ---- Multiple paragraphs with formatting ----

    #[test]
    fn multiple_paragraphs_with_formatting() {
        let input = "**Bold paragraph.**\n\n*Italic paragraph.*\n\nPlain paragraph.";
        let lines = parser().parse(input);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].spans[0].style.weight, TextWeight::Bold);
        assert!(lines[1].spans[0].style.italic);
        assert_eq!(lines[2].spans[0].style, TextStyle::default());
    }

    // ---- Heading with inline formatting ----

    #[test]
    fn heading_with_inline_code() {
        let lines = parser().parse("# Title with `code`");
        assert_eq!(lines.len(), 1);
        // "Title with " should be bold
        let title_span = lines[0].spans.iter().find(|s| s.text.contains("Title"));
        assert!(title_span.is_some());
        assert_eq!(title_span.unwrap().style.weight, TextWeight::Bold);
        // "code" should have CODE color
        let code_span = lines[0].spans.iter().find(|s| s.text == "code");
        assert!(code_span.is_some());
        assert_eq!(code_span.unwrap().style.color, colors::CODE);
    }

    // ---- Fenced code blocks ----

    #[test]
    fn fenced_code_block_produces_lines() {
        let input = "```\nlet x = 1;\nlet y = 2;\n```";
        let lines = parser().parse(input);
        // One RichLine per source line — not one span carrying a newline,
        // which a renderer draws as a single garbled row.
        let texts: Vec<String> = lines.iter().map(RichLine::plain_text).collect();
        assert_eq!(texts, ["let x = 1;", "let y = 2;"]);
        assert!(
            lines
                .iter()
                .all(|l| l.spans.iter().all(|s| !s.text.contains('\n')))
        );
    }

    // ---- Long document ----

    #[test]
    fn long_document_many_paragraphs() {
        let mut input = String::new();
        for i in 0..50 {
            let _ = std::fmt::Write::write_fmt(&mut input, format_args!("Paragraph {i}.\n\n"));
        }
        let lines = parser().parse(&input);
        assert_eq!(lines.len(), 50);
        for (i, line) in lines.iter().enumerate() {
            assert_eq!(line.plain_text(), format!("Paragraph {i}."));
        }
    }

    // ---- Consecutive bold spans ----

    #[test]
    fn consecutive_bold_spans() {
        let lines = parser().parse("**one** **two**");
        assert_eq!(lines.len(), 1);
        let bold_spans: Vec<_> = lines[0]
            .spans
            .iter()
            .filter(|s| s.style.weight == TextWeight::Bold)
            .collect();
        assert_eq!(bold_spans.len(), 2);
        assert_eq!(bold_spans[0].text, "one");
        assert_eq!(bold_spans[1].text, "two");
    }

    // ---- Plain text between formatted spans preserved ----

    #[test]
    fn plain_text_between_formatted_preserved() {
        let lines = parser().parse("a **b** c");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].plain_text(), "a b c");
        // "a " should be plain
        let first = &lines[0].spans[0];
        assert_eq!(first.style, TextStyle::default());
    }

    // ---- Links ----

    #[test]
    fn inline_link_text_preserved() {
        let lines = parser().parse("[click here](https://example.com)");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("click here"));
    }

    #[test]
    fn link_with_bold_text() {
        let lines = parser().parse("[**bold link**](https://example.com)");
        assert_eq!(lines.len(), 1);
        let bold = lines[0].spans.iter().find(|s| s.text == "bold link");
        assert!(bold.is_some(), "bold text inside link should be present");
        assert_eq!(bold.unwrap().style.weight, TextWeight::Bold);
    }

    #[test]
    fn autolink_produces_text() {
        let lines = parser().parse("<https://example.com>");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("https://example.com"));
    }

    // ---- Images ----

    #[test]
    fn image_alt_text_preserved() {
        let lines = parser().parse("![alt text](image.png)");
        assert_eq!(lines.len(), 1);
        assert!(lines[0].plain_text().contains("alt text"));
    }

    // ---- Nested lists ----

    #[test]
    fn nested_unordered_list() {
        let input = "- outer\n  - inner";
        let lines = parser().parse(input);
        let all_text: String = lines
            .iter()
            .map(RichLine::plain_text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(all_text.contains("outer"));
        assert!(all_text.contains("inner"));
    }

    #[test]
    fn ordered_inside_unordered() {
        let input = "- item\n  1. sub one\n  2. sub two";
        let lines = parser().parse(input);
        let all_text: String = lines
            .iter()
            .map(RichLine::plain_text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(all_text.contains("item"));
        assert!(all_text.contains("sub one"));
        assert!(all_text.contains("sub two"));
    }

    // ---- Tables ----

    #[test]
    fn table_cell_text_preserved() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let lines = parser().parse(input);
        let all_text: String = lines
            .iter()
            .map(RichLine::plain_text)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(all_text.contains('A'));
        assert!(all_text.contains('B'));
        assert!(all_text.contains('1'));
        assert!(all_text.contains('2'));
    }

    // ---- Fenced code block with language tag ----

    #[test]
    fn fenced_code_block_with_language() {
        let input = "```rust\nfn main() {}\n```";
        let lines = parser().parse(input);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].plain_text(), "fn main() {}");
        // The fence's language reaches the highlighter: `fn` is a keyword,
        // so it is styled differently from the identifier after it.
        let styles: Vec<_> = lines[0].spans.iter().map(|s| s.style).collect();
        assert!(
            styles.windows(2).any(|w| w[0] != w[1]),
            "rust keywords must be highlighted"
        );
    }

    // ---- Horizontal rule ----

    #[test]
    fn horizontal_rule_does_not_panic() {
        let lines = parser().parse("---");
        // May produce empty or non-empty output; key constraint is no crash
        let _ = lines;
    }

    // ---- Escape sequences ----

    #[test]
    fn escaped_asterisks_not_bold() {
        let lines = parser().parse(r"\*not bold\*");
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains("*not bold*"));
        for span in &lines[0].spans {
            assert_eq!(span.style.weight, TextWeight::Normal);
        }
    }

    // ---- Deeply nested formatting ----

    #[test]
    fn bold_italic_strikethrough_combined() {
        let lines = parser().parse("~~***all three***~~");
        assert_eq!(lines.len(), 1);
        let span = &lines[0].spans[0];
        assert_eq!(span.text, "all three");
        assert_eq!(span.style.weight, TextWeight::Bold);
        assert!(span.style.italic);
        assert!(span.style.strikethrough);
    }

    // ---- Empty list items ----

    #[test]
    fn empty_list_item() {
        let input = "- \n- text";
        let lines = parser().parse(input);
        let all_text: String = lines
            .iter()
            .map(RichLine::plain_text)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(all_text.contains("text"));
    }

    // ---- Paragraph after heading ----

    #[test]
    fn paragraph_after_heading() {
        let lines = parser().parse("# Title\n\nBody text here.");
        assert!(lines.len() >= 2);
        assert_eq!(lines[0].spans[0].style.weight, TextWeight::Bold);
        let body = lines.iter().find(|l| l.plain_text().contains("Body"));
        assert!(body.is_some());
        assert_eq!(body.unwrap().spans[0].style, TextStyle::default());
    }

    // ---- Only whitespace spans ----

    #[test]
    fn tab_only_paragraph() {
        let lines = parser().parse("\t");
        for line in &lines {
            assert!(line.plain_text().trim().is_empty() || line.is_empty());
        }
    }

    // ---- Very long line ----

    #[test]
    fn very_long_line() {
        let long = "x".repeat(10_000);
        let lines = parser().parse(&long);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].plain_text().len(), 10_000);
    }

    // ---- Multiple inline code spans adjacent ----

    #[test]
    fn adjacent_inline_code() {
        let lines = parser().parse("`a``b`");
        assert_eq!(lines.len(), 1);
        let text = lines[0].plain_text();
        assert!(text.contains('a'));
        assert!(text.contains('b'));
    }

    // ---- Blockquote with multiple paragraphs ----

    #[test]
    fn blockquote_with_multiple_paragraphs() {
        let input = "> first\n>\n> second";
        let lines = parser().parse(input);
        let all_text: String = lines
            .iter()
            .map(RichLine::plain_text)
            .collect::<Vec<_>>()
            .join(" ");
        assert!(all_text.contains("first"));
        assert!(all_text.contains("second"));
    }

    // ---- TextProcessor trait ----

    #[test]
    fn text_processor_trait_produces_same_output_as_parse() {
        let p = parser();
        let input = "**bold** and *italic*";
        let via_parse = p.parse(input);
        let via_trait = TextProcessor::process(&p, input);
        assert_eq!(via_parse, via_trait);
    }

    #[test]
    fn code_block_between_paragraphs_keeps_its_rows() {
        let input = "Before:\n\n```nix\n{ x = 1; }\n# note\n```\n\nAfter.";
        let texts: Vec<String> = parser()
            .parse(input)
            .iter()
            .map(RichLine::plain_text)
            .collect();
        assert_eq!(texts, ["Before:", "{ x = 1; }", "# note", "After."]);
    }

    #[test]
    fn indented_code_block_is_split_too() {
        let texts: Vec<String> = parser()
            .parse("    a\n    b\n")
            .iter()
            .map(RichLine::plain_text)
            .collect();
        assert_eq!(texts, ["a", "b"]);
    }

    // ---- tables ----

    const TABLE: &str =
        "Intro.\n\n| Item | Details |\n|---|---|\n| Hardware | 32 GB |\n| OS | macOS |\n\nAfter.";

    #[test]
    fn a_table_is_one_line_per_row_with_aligned_columns() {
        let texts: Vec<String> = parser()
            .parse(TABLE)
            .iter()
            .map(RichLine::plain_text)
            .collect();
        assert_eq!(
            texts,
            [
                "Intro.",
                "Item     \u{2502} Details",
                // 8 + 1 dashes, the cross under the `│`, then 1 + 7.
                "\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{253c}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}\u{2500}",
                "Hardware \u{2502} 32 GB",
                "OS       \u{2502} macOS",
                "After."
            ]
        );
    }

    #[test]
    fn a_table_header_is_bold_and_its_body_is_not() {
        let lines = parser().parse(TABLE);
        assert_eq!(lines[1].spans[0].style.weight, TextWeight::Bold);
        assert_ne!(lines[3].spans[0].style.weight, TextWeight::Bold);
    }

    #[test]
    fn emphasis_inside_a_cell_keeps_its_style() {
        let lines = parser().parse("| a |\n|---|\n| **b** |\n");
        let b = lines
            .iter()
            .flat_map(|l| l.spans.iter())
            .find(|s| s.text == "b")
            .unwrap();
        assert_eq!(b.style.weight, TextWeight::Bold);
    }

    #[test]
    fn wide_characters_align_by_display_width() {
        let texts: Vec<String> = parser()
            .parse("| k | v |\n|---|---|\n| 日本 | x |\n| a | y |\n")
            .iter()
            .map(RichLine::plain_text)
            .collect();
        // 日本 is 4 columns wide, so `a` is padded by 3 to match it.
        assert_eq!(texts[3], "a    \u{2502} y");
    }

    fn texts(md: &str) -> Vec<String> {
        MarkdownParser::new()
            .parse(md)
            .iter()
            .map(|l| l.spans.iter().map(|s| s.text.as_str()).collect())
            .collect()
    }

    #[test]
    fn bracket_display_math_is_not_eaten_as_an_escape() {
        let out =
            texts("Evaluate the definite integral\n\\[\n\\int_{0}^{1} \\ln(1+x)\\,dx.\n\\]\nnext");
        assert!(out.iter().any(|l| l == "    ∫₀¹ ln(1+x) dx."), "{out:?}");
        assert!(!out.iter().any(|l| l.trim() == "["), "{out:?}");
    }

    #[test]
    fn dollar_display_math_gets_its_own_styled_lines() {
        let lines = MarkdownParser::new().parse("$$\\boxed{\\displaystyle \\int_{0}^{1} \\ln(1+x)\\,dx = 2\\ln 2 - 1 \\approx 0.386294}$$");
        let m = lines
            .iter()
            .find(|l| l.spans.iter().any(|s| s.text.contains('≈')))
            .expect("math line");
        assert_eq!(m.spans[0].style.color, colors::MATH);
        assert_eq!(
            m.spans[0].text,
            "    [∫₀¹ ln(1+x) dx = 2ln 2 − 1 ≈ 0.386294]"
        );
    }

    #[test]
    fn inline_math_in_both_delimiters() {
        let out = texts("area under \\(y=\\ln(1+x)\\) is $2\\ln 2 - 1$.");
        assert_eq!(out, vec!["area under y=ln(1+x) is 2ln 2 − 1."]);
    }

    #[test]
    fn aligned_display_block_one_line_per_row() {
        let md =
            "\\[\n\\begin{aligned}\nF(1) &= 2\\ln 2 - 1, \\\\\nF(0) &= 0.\n\\end{aligned}\n\\]";
        let out = texts(md);
        assert!(
            out.contains(&"    F(1) = 2ln 2 − 1,".to_string()),
            "{out:?}"
        );
        assert!(out.contains(&"    F(0) = 0.".to_string()), "{out:?}");
    }

    #[test]
    fn math_delimiters_inside_code_are_left_alone() {
        let out = texts("```\n\\[x\\]\n```\nuse `\\(a\\)` here");
        assert!(out.iter().any(|l| l.contains("\\[x\\]")), "{out:?}");
        assert!(out.iter().any(|l| l.contains("\\(a\\)")), "{out:?}");
    }

    #[test]
    fn currency_is_not_math() {
        let out = texts("costs $5 and $10 today");
        assert_eq!(out, vec!["costs $5 and $10 today"]);
    }
}
