use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthStr;

/// Font weight for a text span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum TextWeight {
    /// Normal / regular weight.
    #[default]
    Normal,
    /// Bold weight.
    Bold,
    /// Light / thin weight.
    Light,
}

/// Visual style applied to a text span.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TextStyle {
    /// RGBA color, each component in 0.0..=1.0.
    pub color: [f32; 4],
    /// Font weight.
    pub weight: TextWeight,
    /// Whether the text is italic.
    pub italic: bool,
    /// Whether the text has an underline.
    pub underline: bool,
    /// Whether the text has a strikethrough line.
    pub strikethrough: bool,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            color: [1.0, 1.0, 1.0, 1.0],
            weight: TextWeight::Normal,
            italic: false,
            underline: false,
            strikethrough: false,
        }
    }
}

impl TextStyle {
    /// Bold white text.
    #[must_use]
    pub fn bold() -> Self {
        Self {
            weight: TextWeight::Bold,
            ..Self::default()
        }
    }

    /// Text with a custom color, normal weight, no decoration.
    #[must_use]
    pub fn colored(color: [f32; 4]) -> Self {
        Self {
            color,
            ..Self::default()
        }
    }

    /// Set the font weight.
    #[must_use]
    pub fn with_weight(mut self, weight: TextWeight) -> Self {
        self.weight = weight;
        self
    }

    /// Set italic.
    #[must_use]
    pub fn with_italic(self) -> Self {
        Self {
            italic: true,
            ..self
        }
    }

    /// Set underline.
    #[must_use]
    pub fn with_underline(self) -> Self {
        Self {
            underline: true,
            ..self
        }
    }

    /// Set strikethrough.
    #[must_use]
    pub fn with_strikethrough(self) -> Self {
        Self {
            strikethrough: true,
            ..self
        }
    }
}

/// A contiguous run of text sharing the same style.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StyledSpan {
    /// The text content of this span.
    pub text: String,
    /// The visual style applied to this span.
    pub style: TextStyle,
}

impl StyledSpan {
    /// Create a new styled span.
    #[must_use]
    pub fn new(text: impl Into<String>, style: TextStyle) -> Self {
        Self {
            text: text.into(),
            style,
        }
    }

    /// Create a plain span with default (white, normal) style.
    #[must_use]
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: TextStyle::default(),
        }
    }

    /// Byte length of the text.
    #[must_use]
    pub fn len(&self) -> usize {
        self.text.len()
    }

    /// Whether the text is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.text.is_empty()
    }

    /// Unicode display width of the text.
    #[must_use]
    pub fn width(&self) -> usize {
        UnicodeWidthStr::width(self.text.as_str())
    }
}

/// A line of styled text spans, ready for rendering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RichLine {
    /// The styled spans that make up this line.
    pub spans: Vec<StyledSpan>,
}

impl RichLine {
    /// Create an empty line with no spans.
    #[must_use]
    pub fn new() -> Self {
        Self { spans: Vec::new() }
    }

    /// Create a line from existing spans.
    #[must_use]
    pub fn from_spans(spans: Vec<StyledSpan>) -> Self {
        Self { spans }
    }

    /// Append a styled span.
    pub fn push(&mut self, span: StyledSpan) {
        self.spans.push(span);
    }

    /// Concatenate all span text into a single plain string.
    #[must_use]
    pub fn plain_text(&self) -> String {
        self.spans.iter().map(|s| s.text.as_str()).collect()
    }

    /// Sum of unicode display widths across all spans.
    #[must_use]
    pub fn total_width(&self) -> usize {
        self.spans.iter().map(StyledSpan::width).sum()
    }

    /// Whether this line has no spans.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.spans.is_empty()
    }

    /// Number of spans in this line.
    #[must_use]
    pub fn len(&self) -> usize {
        self.spans.len()
    }
}

impl Default for RichLine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- TextWeight ----

    #[test]
    fn text_weight_default_is_normal() {
        assert_eq!(TextWeight::default(), TextWeight::Normal);
    }

    #[test]
    fn text_weight_equality() {
        assert_eq!(TextWeight::Bold, TextWeight::Bold);
        assert_ne!(TextWeight::Bold, TextWeight::Light);
        assert_ne!(TextWeight::Normal, TextWeight::Light);
    }

    // ---- TextStyle ----

    #[test]
    fn text_style_default_values() {
        let s = TextStyle::default();
        assert_eq!(s.color, [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(s.weight, TextWeight::Normal);
        assert!(!s.italic);
        assert!(!s.underline);
        assert!(!s.strikethrough);
    }

    #[test]
    fn text_style_bold() {
        let s = TextStyle::bold();
        assert_eq!(s.weight, TextWeight::Bold);
        assert_eq!(s.color, [1.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn text_style_colored() {
        let color = [0.5, 0.6, 0.7, 1.0];
        let s = TextStyle::colored(color);
        assert_eq!(s.color, color);
        assert_eq!(s.weight, TextWeight::Normal);
    }

    #[test]
    fn text_style_builder_chain() {
        let s = TextStyle::colored([1.0, 0.0, 0.0, 1.0])
            .with_weight(TextWeight::Bold)
            .with_italic()
            .with_underline()
            .with_strikethrough();
        assert_eq!(s.color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(s.weight, TextWeight::Bold);
        assert!(s.italic);
        assert!(s.underline);
        assert!(s.strikethrough);
    }

    #[test]
    fn text_style_with_weight() {
        let s = TextStyle::default().with_weight(TextWeight::Light);
        assert_eq!(s.weight, TextWeight::Light);
    }

    #[test]
    fn text_style_with_italic() {
        let s = TextStyle::default().with_italic();
        assert!(s.italic);
        assert!(!s.underline);
    }

    #[test]
    fn text_style_with_underline() {
        let s = TextStyle::default().with_underline();
        assert!(s.underline);
        assert!(!s.strikethrough);
    }

    #[test]
    fn text_style_with_strikethrough() {
        let s = TextStyle::default().with_strikethrough();
        assert!(s.strikethrough);
    }

    // ---- StyledSpan ----

    #[test]
    fn styled_span_new() {
        let style = TextStyle::bold();
        let span = StyledSpan::new("hello", style);
        assert_eq!(span.text, "hello");
        assert_eq!(span.style.weight, TextWeight::Bold);
    }

    #[test]
    fn styled_span_plain() {
        let span = StyledSpan::plain("world");
        assert_eq!(span.text, "world");
        assert_eq!(span.style, TextStyle::default());
    }

    #[test]
    fn styled_span_len() {
        let span = StyledSpan::plain("hello");
        assert_eq!(span.len(), 5);
    }

    #[test]
    fn styled_span_is_empty() {
        assert!(StyledSpan::plain("").is_empty());
        assert!(!StyledSpan::plain("x").is_empty());
    }

    #[test]
    fn styled_span_width_ascii() {
        let span = StyledSpan::plain("hello");
        assert_eq!(span.width(), 5);
    }

    #[test]
    fn styled_span_width_cjk() {
        // CJK characters are width 2
        let span = StyledSpan::plain("\u{7B46}"); // 筆
        assert_eq!(span.width(), 2);
    }

    #[test]
    fn styled_span_width_mixed() {
        // "A筆" = 1 + 2 = 3
        let span = StyledSpan::plain("A\u{7B46}");
        assert_eq!(span.width(), 3);
    }

    // ---- RichLine ----

    #[test]
    fn rich_line_new_is_empty() {
        let line = RichLine::new();
        assert!(line.is_empty());
        assert_eq!(line.len(), 0);
    }

    #[test]
    fn rich_line_push() {
        let mut line = RichLine::new();
        line.push(StyledSpan::plain("hello"));
        assert_eq!(line.len(), 1);
        assert!(!line.is_empty());
    }

    #[test]
    fn rich_line_plain_text() {
        let line = RichLine::from_spans(vec![
            StyledSpan::plain("hello "),
            StyledSpan::new("world", TextStyle::bold()),
        ]);
        assert_eq!(line.plain_text(), "hello world");
    }

    #[test]
    fn rich_line_total_width() {
        let line = RichLine::from_spans(vec![
            StyledSpan::plain("AB"),
            StyledSpan::plain("\u{7B46}"), // width 2
        ]);
        assert_eq!(line.total_width(), 4); // 2 + 2
    }

    #[test]
    fn rich_line_from_spans() {
        let spans = vec![StyledSpan::plain("a"), StyledSpan::plain("b")];
        let line = RichLine::from_spans(spans);
        assert_eq!(line.len(), 2);
        assert_eq!(line.plain_text(), "ab");
    }

    #[test]
    fn rich_line_default() {
        let line = RichLine::default();
        assert!(line.is_empty());
    }
}
