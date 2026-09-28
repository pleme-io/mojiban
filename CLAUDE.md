# Mojiban (文字盤) — Rich Text Rendering

> **★★★ CSE / Knowable Construction.** This repo operates under **Constructive Substrate Engineering** — canonical specification at [`pleme-io/theory/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md`](https://github.com/pleme-io/theory/blob/main/CONSTRUCTIVE-SUBSTRATE-ENGINEERING.md). The Compounding Directive (operational rules: solve once, load-bearing fixes only, idiom-first, models stay current, direction beats velocity) is in the org-level pleme-io/CLAUDE.md ★★★ section. Read both before non-trivial changes.


## Build & Test

```bash
cargo build
cargo test --lib
```

## Architecture

Converts structured text into styled spans for GPU rendering via garasu.

### Modules

| Module | Purpose |
|--------|---------|
| `span.rs` | `RichLine`, `StyledSpan`, `TextStyle`, `TextWeight` — core types |
| `markdown.rs` | `MarkdownParser` — pulldown-cmark to styled spans |
| `highlight.rs` | `SyntaxHighlighter` — tree-sitter token coloring |
| `doc.rs` | `Document::parse` — markdown + TeX to a typed block tree (headings, paragraphs with display-math flow, nested lists/tasks, code+lang, quotes, tables+align, aligned math rows, rules) |
| `layout.rs` | `layout(doc, width, &Theme)` / `render_markdown` — the fleet's terminal document engine: egaku `Span` rows + a `CellStyle` table tagged with a semantic `Role` (hosts map roles to their own tokens). One `block_gap` between blocks, never doubled; hanging indents; quote gutters; framed, highlighted code; wrapping table cells; `&`-aligned math. `Theme` is a serde-default spec (partial YAML is valid) |
| `math.rs` | `tex_to_unicode` (flat lines) and `tex_to_rows` (rows of `&` cells) |

### Layer Position

```
Application (chat messages, terminal, browser)
       ↓
    mojiban (markdown → spans, code → highlighted spans)
       ↓
    garasu TextRenderer (spans → GPU glyphs)
```

### Consumers

New consumers use `render_markdown` / `layout` (rows + role table), not
`MarkdownParser` (no wrapping, no width). TTY hosts map roles through
`egaku_term::markdown` (feature `markdown`); do not write a second role map.

- **fumi**: chat message bodies via `render_markdown` at `message_cols` (no GPU draw yet)
- **hikki**: note preview via `render_markdown` at pane width, spans to glyphon `Attrs` (the one GPU adapter so far; lift into garasu when a second GPU host draws rows)
- **mill** (typemill): `mill docs` via `render_markdown` + `egaku_term::markdown::write_ansi`
- **arnes**: assistant transcript (its own role map; candidate to adopt `egaku_term::markdown::role_style`)
- **nami**: HTML content rendering
- **mado**: terminal escape sequence styling (future)
- **hibiki**: lyrics display

Golden layouts: `tests/golden.rs` (every block kind at 80/40/24 + the arnes math transcript). `MOJIBAN_BLESS=1 cargo test --test golden` rewrites them after a deliberate change.

## Design Decisions

- **pulldown-cmark** for markdown: fast, CommonMark compliant, pure Rust
- **tree-sitter** for highlighting: incremental, language-aware, used by editors
- **Style stack**: nested styles compose (bold + italic + monospace)
- **No rendering**: produces styled data; garasu handles GPU rendering
