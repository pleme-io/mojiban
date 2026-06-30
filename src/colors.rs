//! Nord color palette shared across mojiban modules.
//!
//! All colors are RGBA with each component in `0.0..=1.0`.
//!
//! ## Fleet convergence
//!
//! mojiban emits colored spans consumed by the pleme-io GPU fleet, which has
//! ONE design framework — `ishou` (`ishou_tokens::ColorPalette`). The four
//! syntax/code accents below are the canonical Nord token values that
//! `ishou_tokens::ColorPalette::pleme()` resolves to, expressed here through
//! [`nord_rgba`] (the byte triple is the token's identity, not a hand-authored
//! colour). The [`tests::palette_matches_ishou`] drift test — the library
//! analog of `ishou_tokens::convergence::Guard` — pins each accent to its
//! ishou token, so the palette can never silently fork from the fleet.
//!
//! `COMMENT` and `QUOTE` are muted greys that do NOT correspond to an exact
//! ishou token (see the repo notes: they want a dedicated `text_muted` /
//! `code_comment` ishou token); they stay as literals until that token exists.

/// Builds an opaque RGBA value from an 8-bit-per-channel colour, the same
/// `u8 → f32 / 255.0` conversion `ishou`/`irodori` use. Lets the accent
/// constants name their canonical Nord byte triple instead of transcribing
/// float literals by hand (the historical source of the CODE/STRING drift).
#[must_use]
const fn nord_rgba(r: u8, g: u8, b: u8) -> [f32; 4] {
    [r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0]
}

/// Frost cyan — inline code in markdown. ishou `ColorPalette::pleme().frost_1`
/// (Nord `#88C0D0`).
pub const CODE: [f32; 4] = nord_rgba(0x88, 0xC0, 0xD0);

/// Muted grey — block quotes in markdown. No exact ishou token yet (see notes).
pub const QUOTE: [f32; 4] = [0.616, 0.635, 0.659, 1.0];

/// Frost blue — keywords in syntax highlighting. ishou
/// `ColorPalette::pleme().frost_2` (Nord `#81A1C1`).
pub const KEYWORD: [f32; 4] = nord_rgba(0x81, 0xA1, 0xC1);

/// Aurora green — string literals in syntax highlighting. ishou
/// `ColorPalette::pleme().aurora_green` (Nord `#A3BE8C`).
pub const STRING: [f32; 4] = nord_rgba(0xA3, 0xBE, 0x8C);

/// Muted grey — comments in syntax highlighting. No exact ishou token yet
/// (see notes).
pub const COMMENT: [f32; 4] = [0.424, 0.443, 0.467, 1.0];

/// Aurora purple — number literals in syntax highlighting. ishou
/// `ColorPalette::pleme().aurora_purple` (Nord `#B48EAD`).
pub const NUMBER: [f32; 4] = nord_rgba(0xB4, 0x8E, 0xAD);

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

    /// Fleet convergence guard (library analog of `ishou_tokens::convergence::Guard`):
    /// every Nord-derived accent must equal its `ishou` design token, so this
    /// palette can never silently drift from the fleet source of truth.
    #[test]
    fn palette_matches_ishou() {
        use ishou_tokens::ColorPalette;

        let p = ColorPalette::pleme();
        let rgba = |c: ishou_tokens::Rgb| {
            [
                f32::from(c.r) / 255.0,
                f32::from(c.g) / 255.0,
                f32::from(c.b) / 255.0,
                1.0,
            ]
        };

        for (name, ours, token) in [
            ("CODE", CODE, p.frost_1),
            ("KEYWORD", KEYWORD, p.frost_2),
            ("STRING", STRING, p.aurora_green),
            ("NUMBER", NUMBER, p.aurora_purple),
        ] {
            assert_eq!(
                ours,
                rgba(token),
                "{name} has drifted from its ishou Nord token"
            );
        }
    }
}
