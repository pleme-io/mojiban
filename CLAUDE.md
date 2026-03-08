# Mojiban (文字盤) — Rich Text Rendering

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

### Layer Position

```
Application (chat messages, terminal, browser)
       ↓
    mojiban (markdown → spans, code → highlighted spans)
       ↓
    garasu TextRenderer (spans → GPU glyphs)
```

### Consumers

- **fumi**: chat message markdown rendering
- **nami**: HTML content rendering
- **mado**: terminal escape sequence styling (future)
- **hibiki**: lyrics display

## Design Decisions

- **pulldown-cmark** for markdown: fast, CommonMark compliant, pure Rust
- **tree-sitter** for highlighting: incremental, language-aware, used by editors
- **Style stack**: nested styles compose (bold + italic + monospace)
- **No rendering**: produces styled data; garasu handles GPU rendering
