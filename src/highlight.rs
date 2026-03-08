use crate::span::{RichLine, StyledSpan, TextStyle};

/// Nord-inspired palette for syntax highlighting.
mod colors {
    /// Blue — keywords.
    pub const KEYWORD: [f32; 4] = [0.506, 0.631, 0.757, 1.0];
    /// Green — strings.
    pub const STRING: [f32; 4] = [0.651, 0.761, 0.580, 1.0];
    /// Gray — comments.
    pub const COMMENT: [f32; 4] = [0.424, 0.443, 0.467, 1.0];
    /// Purple — numbers.
    pub const NUMBER: [f32; 4] = [0.706, 0.557, 0.678, 1.0];
}

/// Rust language keywords.
const RUST_KEYWORDS: &[&str] = &[
    "fn", "let", "mut", "pub", "struct", "enum", "impl", "use", "mod", "if", "else", "match",
    "return", "for", "while", "loop", "break", "continue", "const", "static", "type", "trait",
    "where", "async", "await", "self", "super", "crate",
];

/// Nix language keywords.
const NIX_KEYWORDS: &[&str] = &[
    "let", "in", "rec", "with", "inherit", "if", "then", "else", "import", "builtins", "true",
    "false", "null",
];

/// Simple keyword-based syntax highlighter.
///
/// Applies word-boundary matching to color keywords, strings, comments,
/// and numbers. No tree-sitter or grammar files required.
pub struct SyntaxHighlighter;

impl SyntaxHighlighter {
    /// Create a new highlighter.
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Highlight a single line of source code.
    ///
    /// Applies simple keyword coloring for known languages. Unknown
    /// languages are returned as plain text.
    #[must_use]
    pub fn highlight_line(&self, line: &str, language: &str) -> RichLine {
        let keywords: &[&str] = match language {
            "rust" | "rs" => RUST_KEYWORDS,
            "nix" => NIX_KEYWORDS,
            _ => return RichLine::from_spans(vec![StyledSpan::plain(line)]),
        };

        Tokenizer::new(line, keywords).tokenize()
    }
}

impl Default for SyntaxHighlighter {
    fn default() -> Self {
        Self::new()
    }
}

/// Internal tokenizer that walks a line character-by-character.
struct Tokenizer<'a> {
    chars: Vec<char>,
    keywords: &'a [&'a str],
    spans: Vec<StyledSpan>,
    pos: usize,
    plain_start: usize,
}

impl<'a> Tokenizer<'a> {
    fn new(line: &str, keywords: &'a [&'a str]) -> Self {
        Self {
            chars: line.chars().collect(),
            keywords,
            spans: Vec::new(),
            pos: 0,
            plain_start: 0,
        }
    }

    fn len(&self) -> usize {
        self.chars.len()
    }

    fn tokenize(mut self) -> RichLine {
        while self.pos < self.len() {
            let ch = self.chars[self.pos];
            if self.try_line_comment(ch)
                || self.try_string(ch)
                || self.try_number(ch)
                || self.try_keyword(ch)
            {
                continue;
            }
            self.pos += 1;
        }
        self.flush_plain(self.len());
        if self.spans.is_empty() {
            self.spans.push(StyledSpan::plain(String::new()));
        }
        RichLine::from_spans(self.spans)
    }

    /// Flush accumulated plain text up to `end`.
    fn flush_plain(&mut self, end: usize) {
        if self.plain_start < end {
            let text: String = self.chars[self.plain_start..end].iter().collect();
            self.spans.push(StyledSpan::plain(text));
        }
    }

    /// Try to consume a line comment (`//` or `#`). Returns true if consumed.
    fn try_line_comment(&mut self, ch: char) -> bool {
        let is_double_slash = ch == '/' && self.pos + 1 < self.len() && self.chars[self.pos + 1] == '/';
        let is_hash = ch == '#';
        if !is_double_slash && !is_hash {
            return false;
        }
        self.flush_plain(self.pos);
        let comment: String = self.chars[self.pos..].iter().collect();
        self.spans
            .push(StyledSpan::new(comment, TextStyle::colored(colors::COMMENT)));
        self.pos = self.len();
        self.plain_start = self.len();
        true
    }

    /// Try to consume a string literal (`"..."` or `'...'`). Returns true if consumed.
    fn try_string(&mut self, ch: char) -> bool {
        if ch != '"' && ch != '\'' {
            return false;
        }
        self.flush_plain(self.pos);
        let start = self.pos;
        let quote = ch;
        self.pos += 1;
        while self.pos < self.len() {
            if self.chars[self.pos] == '\\' && self.pos + 1 < self.len() {
                self.pos += 2;
            } else if self.chars[self.pos] == quote {
                self.pos += 1;
                break;
            } else {
                self.pos += 1;
            }
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        self.spans
            .push(StyledSpan::new(text, TextStyle::colored(colors::STRING)));
        self.plain_start = self.pos;
        true
    }

    /// Try to consume a number literal. Returns true if consumed.
    fn try_number(&mut self, ch: char) -> bool {
        if !ch.is_ascii_digit() || self.preceded_by_word_char() {
            return false;
        }
        self.flush_plain(self.pos);
        let start = self.pos;
        while self.pos < self.len()
            && (self.chars[self.pos].is_ascii_digit()
                || self.chars[self.pos] == '.'
                || self.chars[self.pos] == '_')
        {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        self.spans
            .push(StyledSpan::new(text, TextStyle::colored(colors::NUMBER)));
        self.plain_start = self.pos;
        true
    }

    /// Try to consume a keyword (word that matches the keyword list). Returns true if consumed.
    fn try_keyword(&mut self, ch: char) -> bool {
        if !is_word_start(ch) || self.preceded_by_word_char() {
            return false;
        }
        let word_start = self.pos;
        while self.pos < self.len() && is_word_continue(self.chars[self.pos]) {
            self.pos += 1;
        }
        let word: String = self.chars[word_start..self.pos].iter().collect();
        if self.keywords.contains(&word.as_str()) && !self.followed_by_word_char() {
            self.flush_plain(word_start);
            self.spans
                .push(StyledSpan::new(word, TextStyle::colored(colors::KEYWORD)));
            self.plain_start = self.pos;
        }
        // Word consumed either way (keyword or not)
        true
    }

    /// Whether the character immediately before `self.pos` is a word character.
    fn preceded_by_word_char(&self) -> bool {
        self.pos > 0 && is_word_continue(self.chars[self.pos - 1])
    }

    /// Whether the character at `self.pos` is a word-continuation character.
    fn followed_by_word_char(&self) -> bool {
        self.pos < self.len() && is_word_continue(self.chars[self.pos])
    }
}

/// Characters that start a word (for keyword detection).
fn is_word_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

/// Characters that continue a word.
fn is_word_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    fn highlighter() -> SyntaxHighlighter {
        SyntaxHighlighter::new()
    }

    #[test]
    fn unknown_language_returns_plain() {
        let line = highlighter().highlight_line("some code here", "brainfuck");
        assert_eq!(line.plain_text(), "some code here");
        assert_eq!(line.len(), 1);
        assert_eq!(line.spans[0].style, TextStyle::default());
    }

    #[test]
    fn empty_input() {
        let line = highlighter().highlight_line("", "rust");
        assert_eq!(line.plain_text(), "");
    }

    #[test]
    fn rust_keyword_fn() {
        let line = highlighter().highlight_line("fn main() {", "rust");
        let kw = line.spans.iter().find(|s| s.text == "fn");
        assert!(kw.is_some(), "should find 'fn' keyword span");
        assert_eq!(kw.unwrap().style.color, colors::KEYWORD);
    }

    #[test]
    fn rust_keyword_let() {
        let line = highlighter().highlight_line("let x = 5;", "rust");
        let kw = line.spans.iter().find(|s| s.text == "let");
        assert!(kw.is_some(), "should find 'let' keyword span");
        assert_eq!(kw.unwrap().style.color, colors::KEYWORD);
    }

    #[test]
    fn rust_keyword_not_in_identifier() {
        // "letter" contains "let" but should NOT color it as a keyword
        let line = highlighter().highlight_line("letter", "rust");
        let kw = line.spans.iter().find(|s| s.text == "let");
        assert!(kw.is_none(), "'let' should not be extracted from 'letter'");
    }

    #[test]
    fn rust_string_colored() {
        let line = highlighter().highlight_line("let s = \"hello\";", "rust");
        let string_span = line.spans.iter().find(|s| s.text.contains("hello"));
        assert!(string_span.is_some(), "should find string span");
        assert_eq!(string_span.unwrap().style.color, colors::STRING);
    }

    #[test]
    fn rust_comment_colored() {
        let line = highlighter().highlight_line("// a comment", "rust");
        let comment_span = line.spans.iter().find(|s| s.text.contains("comment"));
        assert!(comment_span.is_some(), "should find comment span");
        assert_eq!(comment_span.unwrap().style.color, colors::COMMENT);
    }

    #[test]
    fn rust_number_colored() {
        let line = highlighter().highlight_line("let x = 42;", "rust");
        let num_span = line.spans.iter().find(|s| s.text == "42");
        assert!(num_span.is_some(), "should find number span");
        assert_eq!(num_span.unwrap().style.color, colors::NUMBER);
    }

    #[test]
    fn nix_keyword_let() {
        let line = highlighter().highlight_line("let x = 1;", "nix");
        let kw = line.spans.iter().find(|s| s.text == "let");
        assert!(kw.is_some(), "should find 'let' keyword in nix");
        assert_eq!(kw.unwrap().style.color, colors::KEYWORD);
    }

    #[test]
    fn nix_keyword_inherit() {
        let line = highlighter().highlight_line("inherit pkgs;", "nix");
        let kw = line.spans.iter().find(|s| s.text == "inherit");
        assert!(kw.is_some());
        assert_eq!(kw.unwrap().style.color, colors::KEYWORD);
    }

    #[test]
    fn nix_hash_comment() {
        let line = highlighter().highlight_line("# nix comment", "nix");
        let comment_span = line.spans.iter().find(|s| s.text.contains("nix comment"));
        assert!(comment_span.is_some());
        assert_eq!(comment_span.unwrap().style.color, colors::COMMENT);
    }

    #[test]
    fn rust_multiple_keywords() {
        let line = highlighter().highlight_line("pub fn foo() {}", "rust");
        let pub_kw = line.spans.iter().find(|s| s.text == "pub");
        let fn_kw = line.spans.iter().find(|s| s.text == "fn");
        assert!(pub_kw.is_some());
        assert!(fn_kw.is_some());
        assert_eq!(pub_kw.unwrap().style.color, colors::KEYWORD);
        assert_eq!(fn_kw.unwrap().style.color, colors::KEYWORD);
    }

    #[test]
    fn plain_text_reconstructed() {
        let line = highlighter().highlight_line("fn main() { let x = 42; }", "rust");
        assert_eq!(line.plain_text(), "fn main() { let x = 42; }");
    }

    #[test]
    fn default_trait() {
        let h = SyntaxHighlighter::default();
        let line = h.highlight_line("test", "rust");
        assert_eq!(line.plain_text(), "test");
    }

    #[test]
    fn rs_alias_works() {
        let line = highlighter().highlight_line("fn test()", "rs");
        let kw = line.spans.iter().find(|s| s.text == "fn");
        assert!(kw.is_some());
    }

    #[test]
    fn escaped_string() {
        let line = highlighter().highlight_line(r#"let s = "he\"llo";"#, "rust");
        assert_eq!(line.plain_text(), r#"let s = "he\"llo";"#);
        let string_span = line.spans.iter().find(|s| s.text.contains("he\\\"llo"));
        assert!(string_span.is_some());
        assert_eq!(string_span.unwrap().style.color, colors::STRING);
    }
}
