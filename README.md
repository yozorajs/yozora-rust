# Yozora Rust

Rust implementation of the Yozora Markdown parser and its tokenizer ecosystem.

The initial `0.1.0` release tracks the behavior of Yozora `2.4.0` at commit
`4bbc3d4f591a87efe7d7f474c1f9f07f0dcbfdc1`. Rust crate versions follow their
own SemVer lifecycle independently of the TypeScript reference implementation.

## Packages

- `yozora-parser`: parser with Yozora extensions and built-in tokenizers.
- `yozora-parser-gfm`: parser for GitHub Flavored Markdown.
- `yozora-parser-gfm-ex`: parser for GitHub Flavored Markdown extensions.
- `yozora-ast`, `yozora-ast-util`, and `yozora-character`: AST and text utilities.
- `yozora-core-parser` and `yozora-core-tokenizer`: parser and tokenizer infrastructure.
- `yozora-tokenizer-*`: individual tokenizer crates.
- `yozora-markup-weaver`: serialize Yozora AST nodes back into Markdown markup.
- `yozora-lsp`: language server with document sync, outlines, folding, smart selection,
  hover, completion, code actions, workspace navigation and references, rename,
  file/directory move edits, and diagnostics. See the [LSP guide](crates/lsp/README.md).

## Usage

```toml
[dependencies]
yozora-parser = "0.1.0"
```

```rust
use yozora_parser::YozoraParser;

let parser = YozoraParser::default();
let root = parser.parse("# Hello, Yozora!", None);
assert_eq!(root.children.len(), 1);
```

## License

MIT
