//! Mojiban (文字盤) — rich text rendering for pleme-io applications.
//!
//! Converts structured text (markdown, code) into styled spans
//! ready for GPU text rendering.
//!
//! - [`MarkdownParser`]: pulldown-cmark to styled spans
//! - [`Document`]: markdown to a typed block tree (lists, code, quotes, tables, aligned math)
//! - [`layout`]: a [`Document`] at a width and [`Theme`] to egaku rows + a role-tagged style table
//! - [`tex_to_unicode`]: TeX math to readable Unicode (markdown uses it)
//! - [`SyntaxHighlighter`]: simple keyword-based syntax coloring
//! - [`RichLine`]: line of styled spans
//! - [`StyledSpan`]: text + color + weight + decoration
//! - [`TextProcessor`]: trait for text-to-styled-spans processors

pub mod colors;
pub mod doc;
pub mod highlight;
pub mod layout;
pub mod markdown;
pub mod math;
pub mod span;

pub use doc::{Block, Document, Flow, Item};
pub use highlight::SyntaxHighlighter;
pub use layout::{CellStyle, Rendered, Role, Theme, layout, render_markdown};
pub use markdown::MarkdownParser;
pub use math::{tex_to_rows, tex_to_unicode};
pub use span::{ParseTextWeightError, RichLine, StyledSpan, TextStyle, TextWeight};

/// A processor that converts source text into styled lines.
///
/// Implementors take some form of text input and produce a sequence of
/// [`RichLine`]s with appropriate styling applied. Both [`MarkdownParser`]
/// and [`SyntaxHighlighter`] implement this trait.
pub trait TextProcessor {
    /// Process source text into styled lines.
    #[must_use]
    fn process(&self, input: &str) -> Vec<RichLine>;
}
