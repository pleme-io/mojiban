//! Nord-inspired color palette shared across mojiban modules.
//!
//! All colors are RGBA with each component in `0.0..=1.0`.

/// Frost accent — used for inline code in markdown.
pub const CODE: [f32; 4] = [0.537, 0.737, 0.804, 1.0];

/// Muted gray — used for block quotes in markdown.
pub const QUOTE: [f32; 4] = [0.616, 0.635, 0.659, 1.0];

/// Blue — used for keywords in syntax highlighting.
pub const KEYWORD: [f32; 4] = [0.506, 0.631, 0.757, 1.0];

/// Green — used for string literals in syntax highlighting.
pub const STRING: [f32; 4] = [0.651, 0.761, 0.580, 1.0];

/// Gray — used for comments in syntax highlighting.
pub const COMMENT: [f32; 4] = [0.424, 0.443, 0.467, 1.0];

/// Purple — used for number literals in syntax highlighting.
pub const NUMBER: [f32; 4] = [0.706, 0.557, 0.678, 1.0];
