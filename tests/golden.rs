//! Golden layouts: every block kind at three widths, plus the arnes math
//! transcript. `MOJIBAN_BLESS=1 cargo test --test golden` rewrites the
//! files under tests/golden/ after a deliberate change; review the diff.

use mojiban::{Role, Theme, render_markdown};

const WIDTHS: [usize; 3] = [80, 40, 24];

const CASES: &[(&str, &str)] = &[
    ("heading", "# Title of the document\n\nIntro.\n\n## Section two\n\nBody.\n\n### Third"),
    ("paragraphs", "First paragraph with enough words to wrap when the terminal is narrow.\n\nSecond paragraph.\nA soft break kept as a line."),
    ("list", "- a bullet item long enough that it wraps onto a second row at narrow widths\n- second\n  - nested item that also runs long enough to wrap\n    - third level\n\n1. **Bold lead.**\n   Continuation line under the marker.\n\n2. Two\n\n10. Ten"),
    ("tasks", "- [x] done thing\n- [ ] open thing that is long enough to wrap around"),
    ("code", "```rust\nfn main() { println!(\"{}\", 2.0_f64.ln() * 2.0 - 1.0); }\n// a comment\n```\n\n```\nplain\n```"),
    ("quote", "> A blockquote whose text is long enough to wrap in a narrow terminal.\n>\n> > nested quote\n\nafter"),
    ("table", "| method | result | notes |\n|:--|--:|:-:|\n| by parts | 2 ln 2 - 1 | the textbook route through the antiderivative |\n| numeric | 0.386294 | quadrature |"),
    ("math", "Evaluate\n\\[\n\\begin{aligned}\nF(1) &= (1+1)\\ln(1+1) - 1 \\\\\n     &= 2\\ln 2 - 1, \\\\\nF(0) &= 0.\n\\end{aligned}\n\\]\n\n$$\\int_0^1 \\ln(1+x)\\,dx = 2\\ln 2 - 1$$"),
    ("rule", "above\n\n---\n\nbelow"),
];

fn check(name: &str, got: &str) {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden").join(format!("{name}.txt"));
    if std::env::var_os("MOJIBAN_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, got).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing golden {}; run with MOJIBAN_BLESS=1", path.display()));
    assert_eq!(got, want, "golden {name} differs:\n{got}");
}

fn show(md: &str, width: usize) -> String {
    render_markdown(md, width, &Theme::default()).plain()
}

#[test]
fn goldens() {
    for (name, md) in CASES {
        for w in WIDTHS {
            let got = show(md, w);
            for line in got.lines() {
                assert!(unicode_width::UnicodeWidthStr::width(line) <= w, "{name}@{w} overflows: {line:?}");
            }
            check(&format!("{name}.{w}"), &got);
        }
    }
    for w in [88, 48] {
        check(&format!("math-transcript.{w}"), &show(include_str!("fixtures/math-transcript.md"), w));
    }
}

#[test]
fn never_two_blank_rows_and_none_at_the_edges() {
    let got = show(include_str!("fixtures/math-transcript.md"), 60);
    assert!(!got.contains("\n\n\n"), "{got}");
    assert!(!got.starts_with('\n'));
    assert!(!got.trim_end_matches('\n').ends_with('\n'));
}

#[test]
fn list_continuations_hang_under_the_text() {
    let got = show("- alpha beta gamma delta epsilon zeta eta theta\n", 20);
    let rows: Vec<&str> = got.lines().collect();
    assert!(rows[0].starts_with("• "));
    assert!(rows[1..].iter().all(|r| r.starts_with("  ") && !r.starts_with("   ")), "{got}");
}

#[test]
fn aligned_equals_share_a_column() {
    let got = show("$$\n\\begin{aligned}\nF(1) &= a \\\\\n&= b \\\\\nG &= c\n\\end{aligned}\n$$", 40);
    let cols: Vec<usize> = got.lines().map(|l| l.chars().position(|c| c == '=').unwrap()).collect();
    assert!(cols.windows(2).all(|w| w[0] == w[1]), "{got}");
}

#[test]
fn code_is_framed_with_its_language_and_roles() {
    let r = render_markdown("```rust\nfn x() {}\n```", 30, &Theme::default());
    let text = r.plain();
    assert!(text.starts_with("╭─ rust ─"), "{text}");
    assert!(text.trim_end().ends_with('╯'));
    let roles: Vec<Role> = r.rows.iter().flatten().filter_map(|s| r.style(s.style())).map(|c| c.role).collect();
    assert!(roles.contains(&Role::CodeKeyword) && roles.contains(&Role::CodeLabel));
}

#[test]
fn quote_rows_carry_the_gutter() {
    let got = show("> one two three four five six seven\n", 16);
    assert!(got.lines().all(|l| l.starts_with('▎')), "{got}");
}

#[test]
fn table_cells_wrap_to_fit() {
    let got = show(CASES[6].1, 40);
    assert!(got.lines().all(|l| unicode_width::UnicodeWidthStr::width(l) <= 40));
    assert!(got.lines().count() > 4, "{got}");
}

#[test]
fn theme_is_a_partial_spec() {
    let t: Theme = serde_json::from_str(r#"{"block_gap": 2, "code": {"frame": "bar"}}"#).unwrap();
    assert_eq!(t.block_gap, 2);
    assert!(t.code.highlight);
    assert!(show("a\n\nb", 10).lines().count() == 3);
    assert_eq!(render_markdown("a\n\nb", 10, &t).plain().lines().count(), 4);
}
