# Mojiban (文字盤)

Rich text rendering library for pleme-io applications. Converts markdown, code, and structured text into styled glyph runs for GPU rendering via garasu.

## Components

| Module | Purpose |
|--------|---------|
| `markdown` | pulldown-cmark → styled spans (bold, italic, code, links) |
| `highlight` | tree-sitter syntax highlighting for code blocks |
| `span` | `RichLine`, `StyledSpan`, `TextStyle` — styled text primitives |

## Usage

```toml
[dependencies]
mojiban = { git = "https://github.com/pleme-io/mojiban" }
```

```rust
use mojiban::{MarkdownParser, SyntaxHighlighter};

let lines = MarkdownParser::parse("**hello** _world_");
let code = SyntaxHighlighter::new().highlight("fn main() {}", "rust");
```

## Build

```bash
cargo build
cargo test --lib
```
