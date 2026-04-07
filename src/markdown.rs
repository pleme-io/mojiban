use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::colors;
use crate::span::{RichLine, StyledSpan, TextStyle, TextWeight};

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
    /// - Plain text — default style
    #[must_use]
    pub fn parse(&self, markdown: &str) -> Vec<RichLine> {
        let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
        let parser = Parser::new_ext(markdown, options);

        let mut lines: Vec<RichLine> = Vec::new();
        let mut current_line = RichLine::new();
        let mut style_stack: Vec<TextStyle> = vec![TextStyle::default()];
        let mut list_stack: Vec<ListKind> = Vec::new();
        let mut need_list_prefix = false;

        for event in parser {
            match event {
                Event::Start(tag) => {
                    let mut style = style_stack.last().copied().unwrap_or_default();
                    match &tag {
                        Tag::Emphasis => style.italic = true,
                        Tag::Strikethrough => style.strikethrough = true,
                        Tag::Strong | Tag::Heading { .. } => style.weight = TextWeight::Bold,
                        Tag::BlockQuote(_) => style.color = colors::QUOTE,
                        Tag::List(start) => {
                            let kind = if let Some(n) = start {
                                ListKind::Ordered(*n)
                            } else {
                                ListKind::Unordered
                            };
                            list_stack.push(kind);
                        }
                        Tag::Item => {
                            need_list_prefix = true;
                        }
                        _ => {}
                    }
                    style_stack.push(style);
                }
                Event::End(tag_end) => {
                    style_stack.pop();
                    match tag_end {
                        TagEnd::Paragraph
                        | TagEnd::Heading(_)
                        | TagEnd::BlockQuote(_) => {
                            lines.push(std::mem::take(&mut current_line));
                        }
                        TagEnd::Item => {
                            lines.push(std::mem::take(&mut current_line));
                            // Increment ordered list counter for next item
                            if let Some(ListKind::Ordered(start)) = list_stack.last_mut() {
                                *start += 1;
                            }
                        }
                        TagEnd::List(_) => {
                            list_stack.pop();
                        }
                        _ => {}
                    }
                }
                Event::Text(text) => {
                    let style = style_stack.last().copied().unwrap_or_default();
                    if need_list_prefix {
                        if let Some(prefix) = list_prefix(&list_stack) {
                            current_line.push(StyledSpan::new(prefix, style));
                        }
                        need_list_prefix = false;
                    }
                    current_line.push(StyledSpan::new(text.to_string(), style));
                }
                Event::Code(code) => {
                    let mut style = style_stack.last().copied().unwrap_or_default();
                    style.color = colors::CODE;
                    if need_list_prefix {
                        if let Some(prefix) = list_prefix(&list_stack) {
                            let base_style = style_stack.last().copied().unwrap_or_default();
                            current_line.push(StyledSpan::new(prefix, base_style));
                        }
                        need_list_prefix = false;
                    }
                    current_line.push(StyledSpan::new(code.to_string(), style));
                }
                Event::SoftBreak | Event::HardBreak => {
                    lines.push(std::mem::take(&mut current_line));
                }
                _ => {}
            }
        }

        // Flush any remaining content
        if !current_line.is_empty() {
            lines.push(current_line);
        }

        lines
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
        let quote_line = lines.iter().find(|l| l.plain_text().contains("quoted text"));
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
        let p = MarkdownParser::default();
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
        let all_text: String = lines.iter().map(RichLine::plain_text).collect::<Vec<_>>().join(" ");
        assert!(all_text.contains("line one"));
        assert!(all_text.contains("line two"));
    }

    #[test]
    fn hard_break_creates_new_line() {
        // Two trailing spaces followed by newline = hard break
        let lines = parser().parse("first  \nsecond");
        let all_text: String = lines.iter().map(RichLine::plain_text).collect::<Vec<_>>().join(" ");
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
        let all_text: String = lines.iter().map(RichLine::plain_text).collect::<Vec<_>>().join(" ");
        assert!(all_text.contains("line one"));
        assert!(all_text.contains("line two"));
        // All content spans should have QUOTE color
        for line in &lines {
            for span in &line.spans {
                if !span.text.trim().is_empty() {
                    assert_eq!(span.style.color, colors::QUOTE, "blockquote span should have QUOTE color");
                }
            }
        }
    }

    #[test]
    fn blockquote_with_bold() {
        let lines = parser().parse("> **bold quote**");
        let bold_span = lines.iter()
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
        let code_spans: Vec<_> = lines[0].spans.iter()
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
        // Code block content should appear in some form
        let all_text: String = lines.iter().map(RichLine::plain_text).collect::<Vec<_>>().join("\n");
        assert!(all_text.contains("let x = 1;") || all_text.contains("let x = 1"),
                "code block content should be present: {all_text:?}");
    }

    // ---- Long document ----

    #[test]
    fn long_document_many_paragraphs() {
        let mut input = String::new();
        for i in 0..50 {
            input.push_str(&format!("Paragraph {i}.\n\n"));
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
        let bold_spans: Vec<_> = lines[0].spans.iter()
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
        let all_text: String = lines.iter().map(RichLine::plain_text).collect::<Vec<_>>().join("\n");
        assert!(all_text.contains("outer"));
        assert!(all_text.contains("inner"));
    }

    #[test]
    fn ordered_inside_unordered() {
        let input = "- item\n  1. sub one\n  2. sub two";
        let lines = parser().parse(input);
        let all_text: String = lines.iter().map(RichLine::plain_text).collect::<Vec<_>>().join("\n");
        assert!(all_text.contains("item"));
        assert!(all_text.contains("sub one"));
        assert!(all_text.contains("sub two"));
    }

    // ---- Tables ----

    #[test]
    fn table_cell_text_preserved() {
        let input = "| A | B |\n|---|---|\n| 1 | 2 |";
        let lines = parser().parse(input);
        let all_text: String = lines.iter().map(RichLine::plain_text).collect::<Vec<_>>().join(" ");
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
        let all_text: String = lines.iter().map(RichLine::plain_text).collect::<Vec<_>>().join("\n");
        assert!(all_text.contains("fn main()"));
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
        let all_text: String = lines.iter().map(RichLine::plain_text).collect::<Vec<_>>().join(" ");
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
        let all_text: String = lines.iter().map(RichLine::plain_text).collect::<Vec<_>>().join(" ");
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
}
