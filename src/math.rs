//! TeX math → readable Unicode text, for terminals and any renderer with no
//! math layout.
//!
//! Two passes. A small structural pass here handles what a symbol table
//! cannot: `\frac{a}{b}` → `a/b`, `\sqrt{x}` → `√x`, `\boxed{x}` → `[x]`,
//! `\text{…}`, dropped layout commands (`\displaystyle`, `\left`, `\,`),
//! function names (`\ln` → `ln`), and `aligned`-style environments (`\\` a
//! new line, `&` dropped). What remains — Greek, operators, relations and
//! `_{…}`/`^{…}` — goes to [`unicodeit`], the Rust port of the established
//! unicodeit tables, which only substitutes where a Unicode form exists and
//! otherwise leaves the source readable.

/// Commands that only steer TeX layout; they carry no content.
const DROPPED: &[&str] = &[
    "displaystyle", "textstyle", "scriptstyle", "left", "right", "big", "Big", "bigg", "Bigg", "bigl", "bigr",
    "Bigl", "Bigr", "biggl", "biggr", "!", "limits", "nolimits",
];

/// Commands whose one argument is shown as-is (no further TeX meaning).
const VERBATIM_ARG: &[&str] = &["text", "textrm", "textbf", "textit", "mathrm", "operatorname", "mbox"];

/// Commands whose one argument is rendered, wrapper dropped.
const TRANSPARENT_ARG: &[&str] = &["mathbf", "mathit", "mathsf", "boldsymbol", "bm", "mathbb", "mathcal", "mathfrak"];

/// Function names TeX sets upright; shown as the plain word.
const FUNCTIONS: &[&str] = &[
    "ln", "log", "lg", "exp", "sin", "cos", "tan", "cot", "sec", "csc", "arcsin", "arccos", "arctan", "sinh",
    "cosh", "tanh", "lim", "liminf", "limsup", "max", "min", "sup", "inf", "det", "dim", "ker", "deg", "gcd",
    "arg", "Pr", "mod",
];

/// Render one TeX math source (without its `$`/`\[` delimiters) as lines of
/// Unicode text. More than one line only for `\\` row breaks.
#[must_use]
pub fn tex_to_unicode(src: &str) -> Vec<String> {
    tex_to_rows(src)
        .iter()
        .map(|cells| collapse_spaces(&cells.join(" ")))
        .filter(|l| !l.is_empty())
        .collect()
}

/// Like [`tex_to_unicode`], but each row keeps its `&` alignment points:
/// one row per `\\` break, each split into its cells. `aligned`, `align`,
/// `cases` and `array` cells can then be laid out as columns (the `=` of
/// every row under the one above). A row with no `&` is one cell.
#[must_use]
pub fn tex_to_rows(src: &str) -> Vec<Vec<String>> {
    let chars: Vec<char> = src.chars().collect();
    let mut pos = 0;
    let raw = render_seq(&chars, &mut pos, false);
    raw.split('\n')
        .map(|l| {
            l.split(CELL)
                .map(|c| collapse_spaces(&tidy_scripts(&unicodeit::replace(c))).trim_start().to_owned())
                .collect::<Vec<_>>()
        })
        .filter(|cells| cells.iter().any(|c| !c.is_empty()))
        .collect()
}

/// Stands for a `&` alignment point between the structural pass and the
/// split into cells; never survives into output.
const CELL: char = '\u{1F}';

fn collapse_spaces(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = true;
    for c in s.chars() {
        if c == ' ' {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out.trim_end().to_owned()
}

/// Render until end of input, or until the closing `}` when `in_group`.
fn render_seq(s: &[char], pos: &mut usize, in_group: bool) -> String {
    let mut out = String::new();
    while *pos < s.len() {
        let c = s[*pos];
        match c {
            '}' if in_group => {
                *pos += 1;
                return out;
            }
            '{' => {
                *pos += 1;
                out.push_str(&render_seq(s, pos, true));
            }
            '&' => {
                *pos += 1;
                out.push(CELL);
            }
            '~' => {
                *pos += 1;
                out.push(' ');
            }
            '_' | '^' => {
                *pos += 1;
                let arg = take_arg(s, pos);
                // unicodeit converts `_{…}`/`^{…}` when every character has
                // a sub/superscript form, and leaves them otherwise.
                out.push(c);
                out.push('{');
                out.push_str(&arg);
                out.push('}');
            }
            '\\' => {
                *pos += 1;
                out.push_str(&render_command(s, pos));
            }
            '\n' | '\r' | '\t' => {
                *pos += 1;
                out.push(' ');
            }
            _ => {
                *pos += 1;
                out.push(c);
            }
        }
    }
    out
}

/// A command name after `\`: a run of letters, or one other character.
fn read_name(s: &[char], pos: &mut usize) -> String {
    let start = *pos;
    while *pos < s.len() && s[*pos].is_ascii_alphabetic() {
        *pos += 1;
    }
    if *pos == start && *pos < s.len() {
        *pos += 1;
    }
    s[start..*pos].iter().collect()
}

fn skip_spaces(s: &[char], pos: &mut usize) {
    while *pos < s.len() && s[*pos] == ' ' {
        *pos += 1;
    }
}

/// One argument: a `{group}`, a `\command`, or a single character.
fn take_arg(s: &[char], pos: &mut usize) -> String {
    skip_spaces(s, pos);
    match s.get(*pos) {
        Some('{') => {
            *pos += 1;
            render_seq(s, pos, true)
        }
        Some('\\') => {
            *pos += 1;
            render_command(s, pos)
        }
        Some(&c) => {
            *pos += 1;
            c.to_string()
        }
        None => String::new(),
    }
}

/// The raw text of a `{group}` argument, braces balanced, nothing rendered.
fn take_raw_arg(s: &[char], pos: &mut usize) -> String {
    skip_spaces(s, pos);
    if s.get(*pos) != Some(&'{') {
        return take_arg(s, pos);
    }
    *pos += 1;
    let mut depth = 1;
    let start = *pos;
    while *pos < s.len() {
        match s[*pos] {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let t: String = s[start..*pos].iter().collect();
                    *pos += 1;
                    return t;
                }
            }
            _ => {}
        }
        *pos += 1;
    }
    s[start..].iter().collect()
}

/// A rendered operand that needs parentheses to stay unambiguous around `/`.
fn compound(t: &str) -> bool {
    t.trim().chars().any(|c| matches!(c, ' ' | '+' | '-' | '−' | '=' | '/' | '·' | '×' | ','))
}

fn paren(t: &str) -> String {
    let t = t.trim();
    if compound(t) { format!("({t})") } else { t.to_owned() }
}

fn render_command(s: &[char], pos: &mut usize) -> String {
    let name = read_name(s, pos);
    match name.as_str() {
        "\\" => "\n".into(),
        "," | ":" | ";" | " " => " ".into(),
        "quad" => "  ".into(),
        "qquad" => "    ".into(),
        "{" | "}" | "$" | "%" | "#" | "&" | "_" => name,
        n if DROPPED.contains(&n) => {
            // `\left(` keeps its delimiter; `\left.` is an invisible one.
            if matches!(n, "left" | "right" | "bigl" | "bigr" | "Bigl" | "Bigr" | "biggl" | "biggr" | "big" | "Big" | "bigg" | "Bigg") {
                match s.get(*pos) {
                    Some('.') => {
                        *pos += 1;
                        String::new()
                    }
                    Some('\\') => {
                        *pos += 1;
                        render_command(s, pos)
                    }
                    Some(&c) => {
                        *pos += 1;
                        c.to_string()
                    }
                    None => String::new(),
                }
            } else {
                String::new()
            }
        }
        n if FUNCTIONS.contains(&n) => format!("{n}{}", word_gap(s, *pos)),
        n if VERBATIM_ARG.contains(&n) => take_raw_arg(s, pos),
        n if TRANSPARENT_ARG.contains(&n) => take_arg(s, pos),
        "frac" | "dfrac" | "tfrac" | "cfrac" => {
            let a = take_arg(s, pos);
            let b = take_arg(s, pos);
            format!("{}/{}", paren(&a), paren(&b))
        }
        "binom" | "dbinom" => {
            let a = take_arg(s, pos);
            let b = take_arg(s, pos);
            format!("C({}, {})", a.trim(), b.trim())
        }
        "sqrt" => {
            skip_spaces(s, pos);
            let index = if s.get(*pos) == Some(&'[') {
                let start = *pos + 1;
                while *pos < s.len() && s[*pos] != ']' {
                    *pos += 1;
                }
                let i: String = s[start..(*pos).min(s.len())].iter().collect();
                *pos += 1;
                Some(i)
            } else {
                None
            };
            let x = take_arg(s, pos);
            let root = match index.as_deref() {
                Some("3") => "∛",
                Some("4") => "∜",
                _ => "√",
            };
            let x = x.trim();
            if x.chars().count() == 1 { format!("{root}{x}") } else { format!("{root}({x})") }
        }
        "boxed" | "fbox" => format!("[{}]", take_arg(s, pos).trim()),
        "begin" | "end" => {
            let env = take_raw_arg(s, pos);
            // `array`/`tabular` carry a column spec argument.
            if name == "begin" && matches!(env.trim_end_matches('*'), "array" | "tabular") {
                let _ = take_raw_arg(s, pos);
            }
            "\n".into()
        }
        "cdots" | "ldots" | "dots" | "dotsc" | "dotsb" => "…".into(),
        "iint" => "∬".into(),
        "iiint" => "∭".into(),
        "oint" => "∮".into(),
        "neq" => "≠".into(),
        "to" => "→".into(),
        // Left for unicodeit. A following letter would read as part of the
        // name, so only then is a separating space kept.
        _ => format!("\\{name}{}", word_gap(s, *pos)),
    }
}

/// A space after a command word only when the next visible character is a
/// letter or digit (`\ln x`, `\alpha b`); none before `(`, `_`, `^`.
fn word_gap(s: &[char], pos: usize) -> &'static str {
    match s[pos..].iter().find(|c| **c != ' ') {
        Some(c) if c.is_alphanumeric() => " ",
        _ => "",
    }
}

/// unicodeit leaves `_{…}` / `^{…}` it cannot convert. A single character
/// loses its braces (`^∞`); a longer run is parenthesised (`_(max)`).
fn tidy_scripts(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < chars.len() {
        if matches!(chars[i], '_' | '^') && chars.get(i + 1) == Some(&'{') {
            if let Some(len) = chars[i + 2..].iter().position(|c| *c == '}') {
                let body: String = chars[i + 2..i + 2 + len].iter().collect::<String>().trim().to_owned();
                out.push(chars[i]);
                if body.chars().count() == 1 {
                    out.push_str(&body);
                } else {
                    out.push('(');
                    out.push_str(&body);
                    out.push(')');
                }
                i += len + 3;
                continue;
            }
        }
        out.push(chars[i]);
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one(src: &str) -> String {
        tex_to_unicode(src).join(" | ")
    }

    #[test]
    fn definite_integral_from_the_transcript() {
        assert_eq!(one(r"\int_{0}^{1} \ln(1+x)\,dx"), "∫₀¹ ln(1+x) dx");
    }

    #[test]
    fn boxed_result_from_the_transcript() {
        assert_eq!(
            one(r"\boxed{\displaystyle \int_{0}^{1} \ln(1+x)\,dx = 2\ln 2 - 1 \approx 0.386294}"),
            "[∫₀¹ ln(1+x) dx = 2ln 2 − 1 ≈ 0.386294]"
        );
    }

    #[test]
    fn aligned_block_becomes_lines_without_alignment_points() {
        let lines = tex_to_unicode(
            "\\begin{aligned}\nF(1) &= (1+1)\\ln(1+1) - 1 \\\\\n&= 2\\ln 2 - 1, \\\\\nF(0) &= (1+0)\\ln(1+0) - 0 \\\\\n&= 1\\cdot \\ln 1 = 0.\n\\end{aligned}",
        );
        assert_eq!(
            lines,
            vec!["F(1) = (1+1)ln(1+1) − 1", "= 2ln 2 − 1,", "F(0) = (1+0)ln(1+0) − 0", "= 1⋅ ln 1 = 0."]
        );
    }

    #[test]
    fn aligned_rows_keep_their_cells() {
        let rows = tex_to_rows("\\begin{aligned}\nF(1) &= 2\\ln 2 - 1 \\\\\n&= 0.386\n\\end{aligned}");
        assert_eq!(
            rows,
            vec![vec!["F(1)".to_owned(), "= 2ln 2 − 1".to_owned()], vec![String::new(), "= 0.386".to_owned()]]
        );
    }

    #[test]
    fn integration_by_parts_step() {
        assert_eq!(
            one(r"= (1+x)\ln(1+x) - \int \frac{1+x}{1+x}\,dx"),
            "= (1+x)ln(1+x) − ∫ (1+x)/(1+x) dx"
        );
    }

    #[test]
    fn greek_and_relations() {
        assert_eq!(one(r"\alpha \le \beta \ne \gamma \to \infty"), "α ≤ β ≠ γ → ∞");
        assert_eq!(one(r"a \times b \cdot c \ge d"), "a × b ⋅ c ≥ d");
    }

    #[test]
    fn sums_and_products() {
        assert_eq!(one(r"\sum_{n=1}^{\infty} \frac{1}{n^2} = \frac{\pi^2}{6}"), "∑ₙ₌₁^∞ 1/n² = π²/6");
        assert!(one(r"\prod_{i} x_i").starts_with('∏'));
    }

    #[test]
    fn sqrt_forms() {
        assert_eq!(one(r"\sqrt{2}"), "√2");
        assert_eq!(one(r"\sqrt{x+1}"), "√(x+1)");
        assert_eq!(one(r"\sqrt[3]{x}"), "∛x");
    }

    #[test]
    fn unrepresentable_subscript_stays_readable() {
        assert_eq!(one(r"x_{\text{max}}"), "xₘₐₓ");
        assert_eq!(one(r"x_{\text{Max}}"), "x_(Max)");
        assert_eq!(one(r"e^{\infty}"), "e^∞");
    }

    #[test]
    fn text_is_verbatim_and_left_right_keep_delimiters() {
        assert_eq!(one(r"\left( \frac{a}{b} \right) \text{ if } x"), "( a/b ) if x");
    }
}
