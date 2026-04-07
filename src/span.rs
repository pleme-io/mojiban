use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use unicode_width::UnicodeWidthStr;

/// Font weight for a text span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[non_exhaustive]
pub enum TextWeight {
    /// Normal / regular weight.
    #[default]
    Normal,
    /// Bold weight.
    Bold,
    /// Light / thin weight.
    Light,
}

impl fmt::Display for TextWeight {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Self::Normal => "Normal",
            Self::Bold => "Bold",
            Self::Light => "Light",
            _ => "Unknown",
        };
        f.write_str(label)
    }
}

/// Error returned when parsing an invalid [`TextWeight`] string.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown text weight: {0:?}")]
pub struct ParseTextWeightError(String);

impl FromStr for TextWeight {
    type Err = ParseTextWeightError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Normal" => Ok(Self::Normal),
            "Bold" => Ok(Self::Bold),
            "Light" => Ok(Self::Light),
            _ => Err(ParseTextWeightError(s.to_owned())),
        }
    }
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

impl From<Vec<StyledSpan>> for RichLine {
    fn from(spans: Vec<StyledSpan>) -> Self {
        Self { spans }
    }
}

impl IntoIterator for RichLine {
    type Item = StyledSpan;
    type IntoIter = std::vec::IntoIter<StyledSpan>;

    fn into_iter(self) -> Self::IntoIter {
        self.spans.into_iter()
    }
}

impl<'a> IntoIterator for &'a RichLine {
    type Item = &'a StyledSpan;
    type IntoIter = std::slice::Iter<'a, StyledSpan>;

    fn into_iter(self) -> Self::IntoIter {
        self.spans.iter()
    }
}

impl Extend<StyledSpan> for RichLine {
    fn extend<I: IntoIterator<Item = StyledSpan>>(&mut self, iter: I) {
        self.spans.extend(iter);
    }
}

impl fmt::Display for RichLine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for span in &self.spans {
            f.write_str(&span.text)?;
        }
        Ok(())
    }
}

impl fmt::Display for StyledSpan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
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

    #[test]
    fn text_weight_display() {
        assert_eq!(TextWeight::Normal.to_string(), "Normal");
        assert_eq!(TextWeight::Bold.to_string(), "Bold");
        assert_eq!(TextWeight::Light.to_string(), "Light");
    }

    #[test]
    fn text_weight_from_str_valid() {
        assert_eq!("Normal".parse::<TextWeight>().unwrap(), TextWeight::Normal);
        assert_eq!("Bold".parse::<TextWeight>().unwrap(), TextWeight::Bold);
        assert_eq!("Light".parse::<TextWeight>().unwrap(), TextWeight::Light);
    }

    #[test]
    fn text_weight_from_str_invalid() {
        let err = "nope".parse::<TextWeight>().unwrap_err();
        assert_eq!(err, ParseTextWeightError("nope".to_owned()));
        assert!(err.to_string().contains("nope"));
    }

    #[test]
    fn text_weight_display_from_str_round_trip() {
        for w in [TextWeight::Normal, TextWeight::Bold, TextWeight::Light] {
            let s = w.to_string();
            let parsed: TextWeight = s.parse().unwrap();
            assert_eq!(parsed, w);
        }
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

    // ---- Serde round-trip ----

    #[test]
    fn text_style_serde_round_trip() {
        let style = TextStyle::colored([0.1, 0.2, 0.3, 1.0])
            .with_weight(TextWeight::Bold)
            .with_italic()
            .with_underline()
            .with_strikethrough();
        let json = serde_json::to_string(&style).expect("serialize");
        let deser: TextStyle = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser, style);
    }

    #[test]
    fn styled_span_serde_round_trip() {
        let span = StyledSpan::new("hello world", TextStyle::bold().with_italic());
        let json = serde_json::to_string(&span).expect("serialize");
        let deser: StyledSpan = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser, span);
    }

    #[test]
    fn rich_line_serde_round_trip() {
        let line = RichLine::from_spans(vec![
            StyledSpan::plain("normal "),
            StyledSpan::new("bold", TextStyle::bold()),
            StyledSpan::new("italic", TextStyle::default().with_italic()),
        ]);
        let json = serde_json::to_string(&line).expect("serialize");
        let deser: RichLine = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(deser, line);
    }

    // ---- Clone and equality ----

    #[test]
    fn text_style_clone_is_equal() {
        let original = TextStyle::bold().with_italic().with_underline();
        let cloned = original;
        assert_eq!(original, cloned);
    }

    #[test]
    fn text_style_inequality() {
        let bold = TextStyle::bold();
        let italic = TextStyle::default().with_italic();
        assert_ne!(bold, italic);
    }

    #[test]
    fn styled_span_clone_is_equal() {
        let original = StyledSpan::new("test", TextStyle::bold());
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    #[test]
    fn styled_span_inequality_text() {
        let a = StyledSpan::plain("hello");
        let b = StyledSpan::plain("world");
        assert_ne!(a, b);
    }

    #[test]
    fn styled_span_inequality_style() {
        let a = StyledSpan::new("same", TextStyle::default());
        let b = StyledSpan::new("same", TextStyle::bold());
        assert_ne!(a, b);
    }

    #[test]
    fn rich_line_clone_is_equal() {
        let original = RichLine::from_spans(vec![
            StyledSpan::plain("a"),
            StyledSpan::plain("b"),
        ]);
        let cloned = original.clone();
        assert_eq!(original, cloned);
    }

    // ---- Multi-byte and unicode edge cases ----

    #[test]
    fn styled_span_len_vs_width_emoji() {
        // Emoji: multi-byte UTF-8 but typically width 2
        let span = StyledSpan::plain("\u{1F600}"); // grinning face
        assert!(span.len() > 1, "byte length of emoji should be > 1");
        // unicode-width returns 2 for most emoji
        assert_eq!(span.width(), 2);
    }

    #[test]
    fn styled_span_len_multibyte_utf8() {
        // e with acute accent: 2 bytes in UTF-8, width 1
        let span = StyledSpan::plain("\u{00E9}");
        assert_eq!(span.len(), 2);
        assert_eq!(span.width(), 1);
    }

    #[test]
    fn styled_span_empty_width() {
        let span = StyledSpan::plain("");
        assert_eq!(span.width(), 0);
        assert_eq!(span.len(), 0);
    }

    #[test]
    fn rich_line_total_width_empty_line() {
        let line = RichLine::new();
        assert_eq!(line.total_width(), 0);
    }

    #[test]
    fn rich_line_plain_text_empty_spans() {
        let line = RichLine::new();
        assert_eq!(line.plain_text(), "");
    }

    #[test]
    fn rich_line_total_width_multiple_cjk() {
        let line = RichLine::from_spans(vec![
            StyledSpan::plain("\u{6587}"), // 文 — width 2
            StyledSpan::plain("\u{5B57}"), // 字 — width 2
            StyledSpan::plain("\u{76E4}"), // 盤 — width 2
        ]);
        assert_eq!(line.total_width(), 6);
        assert_eq!(line.plain_text(), "\u{6587}\u{5B57}\u{76E4}");
    }

    // ---- Builder chain preservation ----

    #[test]
    fn text_style_builder_preserves_color_through_chain() {
        let color = [0.2, 0.4, 0.6, 0.8];
        let style = TextStyle::colored(color)
            .with_italic()
            .with_underline();
        assert_eq!(style.color, color);
        assert!(style.italic);
        assert!(style.underline);
        assert!(!style.strikethrough);
        assert_eq!(style.weight, TextWeight::Normal);
    }

    #[test]
    fn text_style_weight_overrides_previous_weight() {
        let style = TextStyle::bold().with_weight(TextWeight::Light);
        assert_eq!(style.weight, TextWeight::Light);
    }

    // ---- RichLine push multiple ----

    #[test]
    fn rich_line_push_multiple_spans() {
        let mut line = RichLine::new();
        line.push(StyledSpan::plain("a"));
        line.push(StyledSpan::new("b", TextStyle::bold()));
        line.push(StyledSpan::new("c", TextStyle::default().with_italic()));
        assert_eq!(line.len(), 3);
        assert_eq!(line.plain_text(), "abc");
        assert_eq!(line.spans[0].style, TextStyle::default());
        assert_eq!(line.spans[1].style.weight, TextWeight::Bold);
        assert!(line.spans[2].style.italic);
    }

    // ---- Into<String> for StyledSpan::new ----

    #[test]
    fn styled_span_new_accepts_string() {
        let owned = String::from("owned");
        let span = StyledSpan::new(owned, TextStyle::default());
        assert_eq!(span.text, "owned");
    }

    #[test]
    fn styled_span_plain_accepts_string() {
        let owned = String::from("owned");
        let span = StyledSpan::plain(owned);
        assert_eq!(span.text, "owned");
    }

    // ---- TextWeight Debug ----

    #[test]
    fn text_weight_debug_representation() {
        assert_eq!(format!("{:?}", TextWeight::Normal), "Normal");
        assert_eq!(format!("{:?}", TextWeight::Bold), "Bold");
        assert_eq!(format!("{:?}", TextWeight::Light), "Light");
    }

    // ---- From / IntoIterator ----

    #[test]
    fn rich_line_from_vec() {
        let spans = vec![StyledSpan::plain("a"), StyledSpan::plain("b")];
        let line: RichLine = spans.into();
        assert_eq!(line.len(), 2);
        assert_eq!(line.plain_text(), "ab");
    }

    #[test]
    fn rich_line_into_iter_owned() {
        let line = RichLine::from_spans(vec![
            StyledSpan::plain("x"),
            StyledSpan::plain("y"),
        ]);
        let texts: Vec<String> = line.into_iter().map(|s| s.text).collect();
        assert_eq!(texts, vec!["x", "y"]);
    }

    #[test]
    fn rich_line_into_iter_ref() {
        let line = RichLine::from_spans(vec![
            StyledSpan::plain("a"),
            StyledSpan::plain("b"),
        ]);
        let texts: Vec<&str> = (&line).into_iter().map(|s| s.text.as_str()).collect();
        assert_eq!(texts, vec!["a", "b"]);
    }

    #[test]
    fn rich_line_extend() {
        let mut line = RichLine::new();
        let extra = vec![StyledSpan::plain("a"), StyledSpan::plain("b")];
        line.extend(extra);
        assert_eq!(line.len(), 2);
        assert_eq!(line.plain_text(), "ab");
    }

    #[test]
    fn rich_line_display() {
        let line = RichLine::from_spans(vec![
            StyledSpan::plain("hello "),
            StyledSpan::new("world", TextStyle::bold()),
        ]);
        assert_eq!(line.to_string(), "hello world");
    }

    #[test]
    fn styled_span_display() {
        let span = StyledSpan::new("test", TextStyle::bold());
        assert_eq!(span.to_string(), "test");
    }

    // ---- TextStyle with all decorations off ----

    #[test]
    fn text_style_colored_has_no_decorations() {
        let style = TextStyle::colored([0.0, 0.0, 0.0, 1.0]);
        assert!(!style.italic);
        assert!(!style.underline);
        assert!(!style.strikethrough);
        assert_eq!(style.weight, TextWeight::Normal);
    }
}
