use crate::colors;
use crate::span::{RichLine, StyledSpan, TextStyle};

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
        let (keywords, hash_comments): (&[&str], bool) = match language {
            "rust" | "rs" => (RUST_KEYWORDS, false),
            "nix" => (NIX_KEYWORDS, true),
            _ => return RichLine::from_spans(vec![StyledSpan::plain(line)]),
        };

        Tokenizer::new(line, keywords, hash_comments).tokenize()
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
    hash_comments: bool,
    spans: Vec<StyledSpan>,
    pos: usize,
    plain_start: usize,
}

impl<'a> Tokenizer<'a> {
    fn new(line: &str, keywords: &'a [&'a str], hash_comments: bool) -> Self {
        Self {
            chars: line.chars().collect(),
            keywords,
            hash_comments,
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
        let is_hash = self.hash_comments && ch == '#';
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

    // ---- Number edge cases ----

    #[test]
    fn number_with_decimal_point() {
        let line = highlighter().highlight_line("let x = 3.14;", "rust");
        let num = line.spans.iter().find(|s| s.text.contains("3.14"));
        assert!(num.is_some(), "should find decimal number");
        assert_eq!(num.unwrap().style.color, colors::NUMBER);
    }

    #[test]
    fn number_with_underscores() {
        let line = highlighter().highlight_line("let x = 1_000_000;", "rust");
        let num = line.spans.iter().find(|s| s.text.contains("1_000_000"));
        assert!(num.is_some(), "should find number with underscores");
        assert_eq!(num.unwrap().style.color, colors::NUMBER);
    }

    #[test]
    fn number_not_in_identifier() {
        // "x42" should NOT extract "42" as a separate number
        let line = highlighter().highlight_line("x42", "rust");
        let num = line.spans.iter().find(|s| s.text == "42" && s.style.color == colors::NUMBER);
        assert!(num.is_none(), "42 inside identifier should not be a separate number span");
    }

    #[test]
    fn number_at_start_of_line() {
        let line = highlighter().highlight_line("42", "rust");
        let num = line.spans.iter().find(|s| s.text == "42");
        assert!(num.is_some());
        assert_eq!(num.unwrap().style.color, colors::NUMBER);
    }

    #[test]
    fn number_zero() {
        let line = highlighter().highlight_line("let x = 0;", "rust");
        let num = line.spans.iter().find(|s| s.text == "0");
        assert!(num.is_some());
        assert_eq!(num.unwrap().style.color, colors::NUMBER);
    }

    // ---- String edge cases ----

    #[test]
    fn single_quoted_string() {
        let line = highlighter().highlight_line("let c = 'a';", "rust");
        let string_span = line.spans.iter().find(|s| s.text.contains('a') && s.style.color == colors::STRING);
        assert!(string_span.is_some(), "single-quoted string should be highlighted");
    }

    #[test]
    fn empty_string() {
        let line = highlighter().highlight_line(r#"let s = "";"#, "rust");
        let string_span = line.spans.iter().find(|s| s.text == "\"\"");
        assert!(string_span.is_some(), "empty string literal should be highlighted");
        assert_eq!(string_span.unwrap().style.color, colors::STRING);
    }

    #[test]
    fn multiple_strings_on_line() {
        let line = highlighter().highlight_line(r#"let a = "hello"; let b = "world";"#, "rust");
        let string_spans: Vec<_> = line.spans.iter()
            .filter(|s| s.style.color == colors::STRING)
            .collect();
        assert_eq!(string_spans.len(), 2, "should find two string spans");
        assert!(string_spans[0].text.contains("hello"));
        assert!(string_spans[1].text.contains("world"));
    }

    #[test]
    fn string_with_escaped_backslash() {
        let line = highlighter().highlight_line(r#"let s = "path\\to";"#, "rust");
        let string_span = line.spans.iter().find(|s| s.style.color == colors::STRING);
        assert!(string_span.is_some());
        assert!(string_span.unwrap().text.contains("path\\\\to"));
    }

    // ---- Comment edge cases ----

    #[test]
    fn comment_consumes_rest_of_line() {
        let line = highlighter().highlight_line("let x = 1; // comment with code fn", "rust");
        let comment = line.spans.iter().find(|s| s.style.color == colors::COMMENT);
        assert!(comment.is_some());
        assert!(comment.unwrap().text.contains("comment with code fn"));
        // "fn" after // should NOT be a keyword
        let fn_kw = line.spans.iter().find(|s| s.text == "fn" && s.style.color == colors::KEYWORD);
        assert!(fn_kw.is_none(), "keyword in comment should not be highlighted separately");
    }

    #[test]
    fn comment_only_line() {
        let line = highlighter().highlight_line("// entire line is comment", "rust");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].style.color, colors::COMMENT);
        assert_eq!(line.spans[0].text, "// entire line is comment");
    }

    #[test]
    fn hash_comment_only_line_nix() {
        let line = highlighter().highlight_line("# nix comment line", "nix");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].style.color, colors::COMMENT);
    }

    // ---- Keyword boundary detection ----

    #[test]
    fn keyword_not_prefix_of_identifier() {
        // "letting" contains "let" as prefix
        let line = highlighter().highlight_line("letting", "rust");
        let kw = line.spans.iter().find(|s| s.text == "let" && s.style.color == colors::KEYWORD);
        assert!(kw.is_none(), "'let' should not be extracted from 'letting'");
    }

    #[test]
    fn keyword_not_suffix_of_identifier() {
        // "outlet" contains "let" as suffix
        let line = highlighter().highlight_line("outlet", "rust");
        let kw = line.spans.iter().find(|s| s.text == "let" && s.style.color == colors::KEYWORD);
        assert!(kw.is_none(), "'let' should not be extracted from 'outlet'");
    }

    #[test]
    fn keyword_at_end_of_line() {
        let line = highlighter().highlight_line("x = fn", "rust");
        let kw = line.spans.iter().find(|s| s.text == "fn");
        assert!(kw.is_some(), "'fn' at end of line should be detected");
        assert_eq!(kw.unwrap().style.color, colors::KEYWORD);
    }

    #[test]
    fn keyword_after_punctuation() {
        let line = highlighter().highlight_line("(let x)", "rust");
        let kw = line.spans.iter().find(|s| s.text == "let");
        assert!(kw.is_some(), "'let' after parenthesis should be detected");
        assert_eq!(kw.unwrap().style.color, colors::KEYWORD);
    }

    #[test]
    fn keyword_before_punctuation() {
        let line = highlighter().highlight_line("fn()", "rust");
        let kw = line.spans.iter().find(|s| s.text == "fn");
        assert!(kw.is_some(), "'fn' before parenthesis should be detected");
    }

    // ---- All Rust keywords recognized ----

    #[test]
    fn all_rust_keywords_recognized() {
        for &kw_str in RUST_KEYWORDS {
            let input = format!(" {kw_str} ");
            let line = highlighter().highlight_line(&input, "rust");
            let found = line.spans.iter().find(|s| s.text == kw_str && s.style.color == colors::KEYWORD);
            assert!(found.is_some(), "Rust keyword '{kw_str}' should be highlighted");
        }
    }

    // ---- All Nix keywords recognized ----

    #[test]
    fn all_nix_keywords_recognized() {
        for &kw_str in NIX_KEYWORDS {
            let input = format!(" {kw_str} ");
            let line = highlighter().highlight_line(&input, "nix");
            let found = line.spans.iter().find(|s| s.text == kw_str && s.style.color == colors::KEYWORD);
            assert!(found.is_some(), "Nix keyword '{kw_str}' should be highlighted");
        }
    }

    // ---- Mixed content preservation ----

    #[test]
    fn mixed_keywords_strings_numbers_comments() {
        let input = r#"pub fn add(a: 42, b: "hello") // sum"#;
        let line = highlighter().highlight_line(input, "rust");
        assert_eq!(line.plain_text(), input, "plain text must match original");

        let pub_kw = line.spans.iter().find(|s| s.text == "pub");
        assert!(pub_kw.is_some());
        assert_eq!(pub_kw.unwrap().style.color, colors::KEYWORD);

        let fn_kw = line.spans.iter().find(|s| s.text == "fn");
        assert!(fn_kw.is_some());
        assert_eq!(fn_kw.unwrap().style.color, colors::KEYWORD);

        let num = line.spans.iter().find(|s| s.text == "42");
        assert!(num.is_some());
        assert_eq!(num.unwrap().style.color, colors::NUMBER);

        let string_span = line.spans.iter().find(|s| s.text.contains("hello"));
        assert!(string_span.is_some());
        assert_eq!(string_span.unwrap().style.color, colors::STRING);

        let comment = line.spans.iter().find(|s| s.text.contains("sum"));
        assert!(comment.is_some());
        assert_eq!(comment.unwrap().style.color, colors::COMMENT);
    }

    // ---- Plain text reconstruction for complex lines ----

    #[test]
    fn plain_text_preserved_nix() {
        let input = "let x = import ./foo.nix; # import config";
        let line = highlighter().highlight_line(input, "nix");
        assert_eq!(line.plain_text(), input);
    }

    #[test]
    fn plain_text_preserved_with_strings_and_escapes() {
        let input = r#"let msg = "say \"hi\"";"#;
        let line = highlighter().highlight_line(input, "rust");
        assert_eq!(line.plain_text(), input);
    }

    // ---- Whitespace-only and special inputs ----

    #[test]
    fn whitespace_only_input() {
        let line = highlighter().highlight_line("   ", "rust");
        assert_eq!(line.plain_text(), "   ");
    }

    #[test]
    fn tab_characters_preserved() {
        let line = highlighter().highlight_line("\tlet x = 1;", "rust");
        assert_eq!(line.plain_text(), "\tlet x = 1;");
        let kw = line.spans.iter().find(|s| s.text == "let");
        assert!(kw.is_some());
    }

    #[test]
    fn unicode_identifiers_not_keywords() {
        // Unicode chars should not be treated as keywords
        let line = highlighter().highlight_line("\u{6587}\u{5B57}", "rust");
        assert_eq!(line.plain_text(), "\u{6587}\u{5B57}");
        // Should be plain text, no keyword coloring
        for span in &line.spans {
            assert_ne!(span.style.color, colors::KEYWORD);
        }
    }

    // ---- Nix-specific: keywords shared with Rust ----

    #[test]
    fn nix_let_in_combination() {
        let line = highlighter().highlight_line("let x = 1; in x", "nix");
        let let_kw = line.spans.iter().find(|s| s.text == "let");
        let in_kw = line.spans.iter().find(|s| s.text == "in");
        assert!(let_kw.is_some());
        assert!(in_kw.is_some());
        assert_eq!(let_kw.unwrap().style.color, colors::KEYWORD);
        assert_eq!(in_kw.unwrap().style.color, colors::KEYWORD);
    }

    #[test]
    fn nix_true_false_null() {
        for &kw in &["true", "false", "null"] {
            let input = format!(" {kw} ");
            let line = highlighter().highlight_line(&input, "nix");
            let found = line.spans.iter().find(|s| s.text == kw);
            assert!(found.is_some(), "Nix keyword '{kw}' should be found");
            assert_eq!(found.unwrap().style.color, colors::KEYWORD);
        }
    }

    // ---- Multiple comments/hashes ----

    #[test]
    fn double_slash_in_string_not_comment() {
        let line = highlighter().highlight_line(r#"let url = "http://example.com";"#, "rust");
        // The "//" inside the string should be part of the string, not a comment
        let comment_spans: Vec<_> = line.spans.iter()
            .filter(|s| s.style.color == colors::COMMENT)
            .collect();
        assert!(comment_spans.is_empty(), "// inside string should not create comment span");
    }

    // ---- Consecutive keywords ----

    #[test]
    fn consecutive_keywords_separated_by_space() {
        let line = highlighter().highlight_line("pub async fn", "rust");
        let keywords: Vec<_> = line.spans.iter()
            .filter(|s| s.style.color == colors::KEYWORD)
            .collect();
        assert_eq!(keywords.len(), 3);
        assert_eq!(keywords[0].text, "pub");
        assert_eq!(keywords[1].text, "async");
        assert_eq!(keywords[2].text, "fn");
    }

    // ---- Edge: keyword followed immediately by number ----

    #[test]
    fn keyword_followed_by_number_no_space() {
        // "let42" is not a keyword — it's an identifier
        let line = highlighter().highlight_line("let42", "rust");
        let kw = line.spans.iter().find(|s| s.text == "let" && s.style.color == colors::KEYWORD);
        assert!(kw.is_none(), "'let' should not be extracted from 'let42'");
    }

    // ---- Struct/enum/impl coverage ----

    #[test]
    fn rust_struct_enum_impl() {
        let line = highlighter().highlight_line("pub struct Foo { impl Bar for Foo {} enum Baz {}", "rust");
        for kw in &["struct", "impl", "enum"] {
            let found = line.spans.iter().find(|s| s.text == *kw);
            assert!(found.is_some(), "keyword '{kw}' should be found");
            assert_eq!(found.unwrap().style.color, colors::KEYWORD);
        }
    }

    // ---- Empty line returns a span ----

    #[test]
    fn empty_line_returns_single_empty_span() {
        let line = highlighter().highlight_line("", "rust");
        assert_eq!(line.len(), 1);
        assert!(line.spans[0].is_empty());
    }

    // ---- Rust attribute (regression: hash must not start comment in Rust) ----

    #[test]
    fn rust_attribute_not_treated_as_comment() {
        let line = highlighter().highlight_line("#[derive(Debug)]", "rust");
        let comment_spans: Vec<_> = line.spans.iter()
            .filter(|s| s.style.color == colors::COMMENT)
            .collect();
        assert!(comment_spans.is_empty(), "#[derive] should not be a comment in Rust");
        assert_eq!(line.plain_text(), "#[derive(Debug)]");
    }

    #[test]
    fn rust_hash_in_macro_not_comment() {
        let line = highlighter().highlight_line("#![allow(unused)]", "rust");
        let comment_spans: Vec<_> = line.spans.iter()
            .filter(|s| s.style.color == colors::COMMENT)
            .collect();
        assert!(comment_spans.is_empty());
    }

    #[test]
    fn nix_hash_still_is_comment() {
        let line = highlighter().highlight_line("# nix comment", "nix");
        assert_eq!(line.spans.len(), 1);
        assert_eq!(line.spans[0].style.color, colors::COMMENT);
    }

    // ---- Unterminated string ----

    #[test]
    fn unterminated_string_no_panic() {
        let line = highlighter().highlight_line(r#"let s = "unterminated"#, "rust");
        assert_eq!(line.plain_text(), r#"let s = "unterminated"#);
    }

    #[test]
    fn unterminated_single_quote_no_panic() {
        let line = highlighter().highlight_line("let c = 'x", "rust");
        assert_eq!(line.plain_text(), "let c = 'x");
    }

    // ---- Only punctuation ----

    #[test]
    fn punctuation_only() {
        let line = highlighter().highlight_line("(){};,.", "rust");
        assert_eq!(line.plain_text(), "(){};,.");
        for span in &line.spans {
            assert_eq!(span.style, TextStyle::default());
        }
    }

    // ---- Code with trailing whitespace ----

    #[test]
    fn trailing_whitespace_preserved() {
        let line = highlighter().highlight_line("fn main()   ", "rust");
        assert_eq!(line.plain_text(), "fn main()   ");
    }

    // ---- Leading whitespace before keyword ----

    #[test]
    fn leading_whitespace_before_keyword() {
        let line = highlighter().highlight_line("    fn main()", "rust");
        let kw = line.spans.iter().find(|s| s.text == "fn");
        assert!(kw.is_some());
        assert_eq!(kw.unwrap().style.color, colors::KEYWORD);
        assert_eq!(line.plain_text(), "    fn main()");
    }

    // ---- String containing keyword ----

    #[test]
    fn keyword_inside_string_not_highlighted_separately() {
        let line = highlighter().highlight_line(r#"let s = "fn let pub";"#, "rust");
        let fn_kw = line.spans.iter().find(|s| s.text == "fn" && s.style.color == colors::KEYWORD);
        assert!(fn_kw.is_none(), "'fn' inside string should not be a keyword");
    }

    // ---- Number followed by keyword ----

    #[test]
    fn number_then_keyword() {
        let line = highlighter().highlight_line("42 fn", "rust");
        let num = line.spans.iter().find(|s| s.text == "42");
        assert!(num.is_some());
        assert_eq!(num.unwrap().style.color, colors::NUMBER);
        let kw = line.spans.iter().find(|s| s.text == "fn");
        assert!(kw.is_some());
        assert_eq!(kw.unwrap().style.color, colors::KEYWORD);
    }

    // ---- Multiple lines independently ----

    #[test]
    fn highlight_line_is_stateless() {
        let h = highlighter();
        let l1 = h.highlight_line("fn foo()", "rust");
        let l2 = h.highlight_line("let x = 1;", "rust");
        assert!(l1.spans.iter().any(|s| s.text == "fn"));
        assert!(l2.spans.iter().any(|s| s.text == "let"));
    }

    // ---- Nix if-then-else ----

    #[test]
    fn nix_if_then_else() {
        let line = highlighter().highlight_line("if x then y else z", "nix");
        for kw in &["if", "then", "else"] {
            let found = line.spans.iter().find(|s| s.text == *kw);
            assert!(found.is_some(), "Nix keyword '{kw}' should be found");
            assert_eq!(found.unwrap().style.color, colors::KEYWORD);
        }
    }

    // ---- Nix import with path ----

    #[test]
    fn nix_import_path() {
        let line = highlighter().highlight_line("import ./default.nix", "nix");
        let kw = line.spans.iter().find(|s| s.text == "import");
        assert!(kw.is_some());
        assert_eq!(kw.unwrap().style.color, colors::KEYWORD);
        assert_eq!(line.plain_text(), "import ./default.nix");
    }

    // ---- Complex Rust line ----

    #[test]
    fn complex_rust_line() {
        let input = "pub async fn process(data: &str) -> Result<String, Error> {";
        let line = highlighter().highlight_line(input, "rust");
        assert_eq!(line.plain_text(), input);
        for kw in &["pub", "async", "fn"] {
            let found = line.spans.iter().find(|s| s.text == *kw);
            assert!(found.is_some(), "keyword '{kw}' should be found");
            assert_eq!(found.unwrap().style.color, colors::KEYWORD);
        }
    }

    // ---- Highlight the SyntaxHighlighter::default() returns usable instance ----

    #[test]
    fn default_instance_works_for_all_languages() {
        let h = SyntaxHighlighter::default();
        let _ = h.highlight_line("test", "rust");
        let _ = h.highlight_line("test", "rs");
        let _ = h.highlight_line("test", "nix");
        let _ = h.highlight_line("test", "unknown");
    }
}
