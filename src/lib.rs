//! Mojiban (文字盤) — rich text rendering for pleme-io applications.
//!
//! Converts structured text (markdown, code) into styled spans
//! ready for GPU text rendering.
//!
//! - [`MarkdownParser`]: pulldown-cmark to styled spans
//! - [`SyntaxHighlighter`]: simple keyword-based syntax coloring
//! - [`RichLine`]: line of styled spans
//! - [`StyledSpan`]: text + color + weight + decoration

pub mod highlight;
pub mod markdown;
pub mod span;

pub use highlight::SyntaxHighlighter;
pub use markdown::MarkdownParser;
pub use span::{RichLine, StyledSpan, TextStyle, TextWeight};
