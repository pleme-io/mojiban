//! Mojiban (文字盤) — rich text rendering for pleme-io applications.
//!
//! Converts structured text (markdown, code) into styled spans
//! ready for GPU text rendering.
//!
//! - [`MarkdownParser`]: pulldown-cmark to styled spans
//! - [`SyntaxHighlighter`]: simple keyword-based syntax coloring
//! - [`RichLine`]: line of styled spans
//! - [`StyledSpan`]: text + color + weight + decoration
//! - [`TextProcessor`]: trait for text-to-styled-spans processors

pub(crate) mod colors;
pub mod highlight;
pub mod markdown;
pub mod span;

pub use highlight::SyntaxHighlighter;
pub use markdown::MarkdownParser;
pub use span::{ParseTextWeightError, RichLine, StyledSpan, TextStyle, TextWeight};

/// A processor that converts source text into styled lines.
///
/// Implementors take some form of text input and produce a sequence of
/// [`RichLine`]s with appropriate styling applied. Both [`MarkdownParser`]
/// and [`SyntaxHighlighter`] implement this trait.
pub trait TextProcessor {
    /// Process source text into styled lines.
    fn process(&self, input: &str) -> Vec<RichLine>;
}
