# Fude (筆)

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
fude = { git = "https://github.com/pleme-io/fude" }
```

```rust
use fude::{MarkdownParser, SyntaxHighlighter};

let lines = MarkdownParser::parse("**hello** _world_");
let code = SyntaxHighlighter::new().highlight("fn main() {}", "rust");
```

## Build

```bash
cargo build
cargo test --lib
```
