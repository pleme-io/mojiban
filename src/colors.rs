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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_colors_have_full_alpha() {
        for (name, color) in [
            ("CODE", CODE),
            ("QUOTE", QUOTE),
            ("KEYWORD", KEYWORD),
            ("STRING", STRING),
            ("COMMENT", COMMENT),
            ("NUMBER", NUMBER),
        ] {
            assert!(
                (color[3] - 1.0).abs() < f32::EPSILON,
                "{name} alpha should be 1.0, got {}",
                color[3]
            );
        }
    }

    #[test]
    fn all_color_channels_in_unit_range() {
        for (name, color) in [
            ("CODE", CODE),
            ("QUOTE", QUOTE),
            ("KEYWORD", KEYWORD),
            ("STRING", STRING),
            ("COMMENT", COMMENT),
            ("NUMBER", NUMBER),
        ] {
            for (i, ch) in color.iter().enumerate() {
                assert!(
                    (0.0..=1.0).contains(ch),
                    "{name}[{i}] = {ch} is out of [0.0, 1.0]"
                );
            }
        }
    }

    #[test]
    fn all_colors_are_distinct() {
        let all = [CODE, QUOTE, KEYWORD, STRING, COMMENT, NUMBER];
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_ne!(
                    all[i], all[j],
                    "color at index {i} should differ from index {j}"
                );
            }
        }
    }

    #[test]
    fn comment_is_darkest() {
        // Comments should be the most muted (lowest luminance)
        let luminance = |c: [f32; 4]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
        let comment_lum = luminance(COMMENT);
        for (name, color) in [
            ("CODE", CODE),
            ("KEYWORD", KEYWORD),
            ("STRING", STRING),
            ("NUMBER", NUMBER),
        ] {
            assert!(
                comment_lum < luminance(color),
                "COMMENT should be darker than {name}"
            );
        }
    }
}
