# Yozora Language Server

`yozora-lsp` provides editor support for the default Yozora Markdown dialect,
including admonitions, math, reference links, reference images, and footnotes.

## Run

Build from the workspace root:

```sh
cargo build --release -p yozora-lsp
```

Configure your editor's LSP client to run the absolute path to
`target/release/yozora-lsp` with `--stdio` for Markdown or Yozora buffers.
Running without arguments also selects stdio. The binary supports `--help` and
`--version`.

For example, with Neovim 0.11 or later:

```lua
vim.lsp.config('yozora', {
  cmd = { '/absolute/path/to/yozora-lsp', '--stdio' },
  filetypes = { 'markdown', 'yozora' },
})
vim.lsp.enable('yozora')
```

## Capabilities

- `textDocument/didOpen`, `didChange`, and `didClose`: in-memory document sync,
  with both incremental edits and full replacements. Positions use UTF-16.
- `textDocument/documentSymbol`: ATX and setext heading outlines. Clients that
  support hierarchical symbols receive heading sections and selection ranges;
  other clients receive flat symbols with parent names.
- `textDocument/foldingRange`: heading sections, code, math, admonitions,
  blockquotes, lists, tables, HTML, and multiline definitions. Heading sections
  end at the next heading of equal or lower depth in the same block container,
  or at that container's end. Ranges use inclusive line numbers and respect the
  client's range limit.
- `textDocument/definition`: document-local reference links, reference images,
  and footnotes, including references in admonition titles. Identifier matching
  and duplicate definitions follow the parser's rules.

Queries operate on open buffers, including unsaved documents. Direct URL links,
heading anchors, cross-file indexing, completion, diagnostics, hover, rename,
and formatting are outside the current capabilities. Unresolved references
remain ordinary text, following the parser's fallback behavior.

## Implementation contracts

The server owns each document's text, version, line index, and cached AST. Edits
are processed serially and each batch is applied atomically. ASTs are parsed in
full on the first query after a change, so consecutive edits can share one
parse. Analysis only reads the AST; the parser has no dependency on LSP code.

Versions must increase within an open session. Stale changes are ignored. An
invalid newer edit leaves the previous text intact but suspends queries with
`ContentModified` until a full replacement or reopening restores synchronization.

The stdio transport uses the existing `serde` and `serde_json` dependencies. It
supports the advertised LSP subset and the initialize/shutdown/exit lifecycle.
Stdout carries framed JSON-RPC messages; errors are written to stderr. Frames
and documents are limited to 16 MiB, and headers to 8 KiB.

## Validation

```sh
cargo test -p yozora-lsp
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The integration tests launch the binary and communicate over pipes, exercising
framing, lifecycle, client capabilities, Unicode edits, and live document queries.
