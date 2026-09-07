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
  other clients receive flat symbols with parent names. Headings inside block
  containers stay under their enclosing section, regardless of their depth,
  and do not change the hierarchy of subsequent headings outside the container.
  Outline nesting is capped at 32 levels for client JSON compatibility; deeper
  sections are flattened while retaining every heading.
- `textDocument/foldingRange`: heading sections, code, math, admonitions,
  blockquotes, lists, tables, HTML, and multiline definitions. Heading sections
  end at the next heading of equal or lower depth in the same block container,
  or at that container's end. Ranges use inclusive line numbers and respect the
  client's range limit.
- `textDocument/definition`: document-local reference links, reference images,
  and footnotes, including references in admonition titles. Identifier matching
  and duplicate definitions follow the parser's rules.
- `textDocument/hover`: links and images show their destination and title;
  footnotes show a text summary. Resolved references and their active definitions
  use the same information. Previews are limited to 2,000 Unicode characters,
  followed by an ellipsis when shortened. The server respects the client's
  plaintext/Markdown preference and displays destinations and titles as literal text.
- `textDocument/references`: find reference links, reference images, and footnotes
  from either an occurrence or its active definition. Results are ordered by
  source position and honor `context.includeDeclaration`. Links and images share
  a label namespace; footnotes have a separate namespace. An inactive duplicate
  definition has no bound references. Locations use the parser's node ranges.
- `textDocument/completion`: complete document-local labels in `[text][label]`,
  `![alt][label]`, and `[^label]`, including unclosed labels. Matching follows the
  parser's case folding and ASCII whitespace normalization. An existing label is
  replaced up to its closing `]`; without a closing bracket, only the prefix up to
  the cursor is replaced and `]` is inserted, preserving following text. Code,
  math, HTML, declaration labels, and link destinations are excluded. Completion
  currently supports labels typed on one line; labels from multiline definitions
  are inserted with equivalent single-space separators. A closing bracket in code,
  math, HTML, another table cell, or beyond the 999-character label limit is treated
  as following text. Table pipes are escaped while preserving label identity.
  The server checks up to 200 matching definitions per request and omits spellings
  that cannot form the intended reference. Incomplete lists refresh as the user types.
- `textDocument/prepareRename` and `textDocument/rename`: rename document-local
  reference labels from an active definition or a resolved reference. Explicit
  references select only their label; implicit `[old]` and `[old][]` references
  become `[old][new]` to preserve display text, including image alt text. All
  declarations of the renamed label move together, preserving duplicate order
  and first-definition-wins behavior. Link and footnote namespaces remain separate.
  Existing multiline labels are supported; the new name must be a valid single-line
  Markdown label spelling, at most 999 characters, without surrounding whitespace.
  Required escapes are part of the spelling, for example `A\]B`.
- `textDocument/publishDiagnostics`: warnings for duplicate link and footnote
  definitions, following the same identifier normalization and declaration scope
  as navigation. The first definition remains active; later declarations produce
  `duplicate-link-definition` or `duplicate-footnote-definition`. Locations use
  the parser's definition ranges. Clients supporting `relatedInformation` receive
  a location pointing to the active definition; clients supporting `versionSupport`
  receive the document version. Up to 1,000 warnings are published in source order.
  Publications are debounced by 150 ms per document, so intermediate edit versions
  can be skipped during continuous typing.

Queries operate on open buffers, including unsaved documents. Navigation to
direct URLs, heading anchors, cross-file indexing, and formatting are outside
the current capabilities. Unresolved
references remain ordinary text, following the parser's fallback behavior.

## Implementation contracts

The server owns each document's text, version, line index, and cached AST. Edits
are processed serially and each batch is applied atomically. Opening a document
or accepting a change schedules diagnostics with a 150 ms delay. Further edits to
that document reset its deadline. Queries do not wait for the deadline: they parse
the latest synchronized text on demand. Queries and diagnostics reuse the same AST
until the next edit invalidates it.
The line index keeps sparse UTF-16 checkpoints on long lines, bounding coordinate
scans for dense reference edits. Rename constructs the proposed source in one pass
before validating its semantics.

The message loop checks deadlines both while idle and after each incoming message,
analyzing at most one due document before checking input again. Editing one buffer
does not reset another buffer's deadline. Full parsing still runs synchronously on
the server thread; debounce reduces repeated work during edit bursts, while a parse
already in progress can still delay subsequent messages.

Definition, hover, and reference queries share identifier resolution and the
first-definition-wins rule. Analysis only reads the AST; the parser has no
dependency on LSP code.

Completion reads the source line and AST through one immutable snapshot of the
document version. Its lexical context handles unfinished syntax, while AST
ranges exclude non-reference regions. Label identity is reused from
`yozora-core-tokenizer`; this adds an internal workspace dependency.
Candidate spelling is validated with a short reference and a preset association,
using the same parser. Table candidates also pass through table escaping rules.

Rename returns a WorkspaceEdit and leaves server text unchanged until the client
sends `didChange`. Clients advertising `workspace.workspaceEdit.documentChanges`
receive edits tied to the document version; other clients receive `changes`.
Before returning any edits, the server reparses the proposed text and checks AST
structure, display content, resources, and reference bindings. Conflicting names,
unintended activation of plain text references, syntax changes, and edits exceeding
the document size limit are rejected as a whole.

Versions must increase within an open session. Stale changes are ignored. An
invalid newer edit leaves the previous text intact but suspends queries with
`ContentModified` until a full replacement or reopening restores synchronization.
An invalid newer edit cancels scheduled diagnostics and clears the previous results
immediately. Restoring synchronization schedules fresh diagnostics. Stale changes
neither publish diagnostics nor reset an existing deadline. Closing a document
cancels its scheduled work, releases its state, and immediately publishes an empty
diagnostic list without a version. Shutdown discards all scheduled work.
Once a notification identifies an open document and a newer version, an undecodable
or missing edit batch also suspends synchronization. Invalid document metadata is
rejected before changing document state.

The stdio transport uses the existing `serde` and `serde_json` dependencies. It
supports the advertised LSP subset and the initialize/shutdown/exit lifecycle.
Stdout carries framed JSON-RPC messages; errors are written to stderr. Frames
and documents are limited to 16 MiB, and headers to 8 KiB.
A dedicated blocking reader forwards input through a channel holding at most one
queued frame. This lets diagnostic timers run even while stdin is idle or a frame
is incomplete. Document state, analysis, and all stdout writes remain on the server
thread. The reader has process lifetime and is not joined on exit, so shutdown does
not depend on the client closing stdin. Input errors propagate to the server loop.

## Validation

```sh
cargo test -p yozora-lsp
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

The integration tests launch the binary and communicate over pipes, exercising
framing, lifecycle, client capabilities, Unicode edits, and live document queries.
They also check diagnostic publications on open, edit, resynchronization, and close,
including notifications interleaved with responses, coalesced edits, and incomplete
input frames. Scheduling tests use explicit timestamps to verify deadlines without
wall-clock sleeps.
