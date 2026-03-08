use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd};

use crate::span::{RichLine, StyledSpan, TextStyle, TextWeight};

/// Nord-inspired palette constants for markdown styling.
mod colors {
    /// Frost accent — used for inline code.
    pub const CODE: [f32; 4] = [0.537, 0.737, 0.804, 1.0];
    /// Muted gray — used for block quotes.
    pub const QUOTE: [f32; 4] = [0.616, 0.635, 0.659, 1.0];
}

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
                        let prefix = match list_stack.last() {
                            Some(ListKind::Unordered) => "\u{2022} ".to_string(),
                            Some(ListKind::Ordered(n)) => format!("{n}. "),
                            None => String::new(),
                        };
                        if !prefix.is_empty() {
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
                        let prefix = match list_stack.last() {
                            Some(ListKind::Unordered) => "\u{2022} ".to_string(),
                            Some(ListKind::Ordered(n)) => format!("{n}. "),
                            None => String::new(),
                        };
                        if !prefix.is_empty() {
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

/// Tracks whether a list is ordered or unordered.
#[derive(Debug, Clone)]
enum ListKind {
    Ordered(u64),
    Unordered,
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
