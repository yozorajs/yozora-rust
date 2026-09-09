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
vim.filetype.add({ extension = { yozora = 'markdown' } })
vim.lsp.config('yozora', {
  cmd = { '/absolute/path/to/yozora-lsp', '--stdio' },
  filetypes = { 'markdown', 'yozora' },
  root_markers = { '.git' },
})
vim.lsp.enable('yozora')
```

The filetype mapping gives `.yozora` files Neovim's Markdown support and lets the
client attach automatically. The root marker supplies a workspace root, so links
such as `docs/page.md` to `../guide.md` can resolve within the project. For projects
without Git, set suitable `root_markers` or an explicit `root_dir`; without a root,
local navigation is limited to the source file's directory.

## Capabilities

- `textDocument/didOpen`, `didChange`, and `didClose`: in-memory document sync,
  with both incremental edits and full replacements. Positions use UTF-16.
- `$/cancelRequest`: cancel pending or running document queries.
  Each cancelled request receives `RequestCancelled`; unknown or completed IDs
  are ignored. Newer edits, close, and reopen invalidate pending and running queries
  for that document with `ContentModified`. A running
  analysis may finish in the background; its response is discarded after cancellation.
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
  and duplicate definitions follow the parser's rules. Direct links, images, and
  link definitions also navigate to local files and heading anchors, for example
  `[guide](./guide.md#intro)` or `[section](#intro-2)`. Navigation selects the
  innermost resource or reference at the cursor. Reference occurrences navigate
  to their declaration first. Files without a fragment open at the start;
  nonempty fragments select the matching heading text. Missing targets return null.
- `textDocument/documentLink`: clickable destinations for direct links, images,
  link definitions, and resolved link/image references, including admonition
  titles. Ranges cover the parser's resource nodes. The server inspects at most
  1,000 resources per request, with a 1 MiB combined budget for destination input
  and resolved URI output, and omits missing or disallowed local paths. HTTP,
  HTTPS, and mailto links are returned without fetching their contents. Local
  fragments are preserved as URI fragments for the client; heading validation
  and exact source positions are provided by `textDocument/definition`.
- `workspace/didChangeWorkspaceFolders`: update the local navigation roots.
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
  the cursor is replaced and `]` is inserted, preserving following text. Label
  completion excludes code, math, HTML, declaration labels, and link destinations.
  It supports labels typed on one line; labels from multiline definitions
  are inserted with equivalent single-space separators. A closing bracket in code,
  math, HTML, another table cell, or beyond the 999-character label limit is treated
  as following text. Table pipes are escaped while preserving label identity.
  The server checks up to 200 matching definitions per request and omits spellings
  that cannot form the intended reference in the surrounding block. Large blocks
  or definition sets reduce the number checked. Incomplete lists refresh as the
  user types.
  Destinations in links, images, and reference definitions also complete local
  file/directory names and heading anchors, such as `[go](./docs/gu)` and
  `[go](guide.md#in)`. Matching is case-sensitive, with raw Unicode, encoded
  characters, and partially typed percent escapes supported. Directories end in
  `/`; unsaved files and their parent directories participate. Heading candidates
  use the same IDs, prefix, and target-buffer precedence as navigation.
  Destination completion requires the destination and its `](`/`]:` introducer
  on the cursor's line; display text may span preceding lines. It edits the whole
  destination, preserving the display, title, remaining path components, and
  query/fragment semantics. Inserted destinations use URI encoding and Markdown
  escapes as needed. Unfinished destinations at line end are supported without
  inserting closing delimiters. Code, math, HTML, titles, query parameters, and
  remote destinations do not produce path or anchor candidates. A cursor inside
  a Markdown escape, character entity, or percent-encoded path separator does not
  produce destination edits. URI schemes and authorities are not path components.
  `(`, `/`, and `#`
  are completion triggers in addition to `[` and `^`.
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

Queries start from open buffers, including unsaved documents. Target buffers use
their unsaved text; closed Markdown files are read on demand for anchor navigation.
Workspace indexing, heading rename, and formatting are
outside the current capabilities. Unresolved references remain ordinary text,
following the parser's fallback behavior.

## Implementation contracts

The server owns each document's text, version, line index, and cached AST. Edits
are processed serially and each batch is applied atomically. Opening a document
or accepting a change schedules diagnostics with a 150 ms delay. Further edits to
that document reset its deadline. Document queries enter a short 5 ms queue so
already-arriving edits and cancellations can retire obsolete work before parsing.
They do not wait for the diagnostic deadline. An accepted newer change invalidates
pending and running queries for that URI, including when its edit batch is malformed. Stale
changes leave waiting queries intact. Reopening also invalidates them when the
version number is reused. Queries and diagnostics reuse the same AST until the
next edit invalidates it.
Text, line indexes, and parsed ASTs are shared through `Arc`. Each analysis gets
a snapshot of open buffers, workspace roots, and client capabilities. Workers
only update their snapshot's AST caches; the server adopts those caches only
when text identity and version still match the live document. Text identity also
distinguishes close/reopen when the client reuses a version number.
The line index keeps sparse UTF-16 checkpoints on long lines, bounding coordinate
scans for dense reference edits. Rename constructs the proposed source in one pass
before validating its semantics. Descending `didChange` edits with strictly
separated ranges also build the text in one pass and rebuild the index once.
Other batches retain sequential coordinates, including touching ranges, clamped
positions, and edits that join CR/LF boundaries. Every intermediate document must
fit the size limit, even when a later edit would shrink it.

Document requests are decoded into typed parameters before scheduling; unused
client extension fields are discarded. The pending queue admits at most 128
requests with a combined 1 MiB accounting budget for request records and their
owned strings, including IDs, URIs, and rename text. The two active worker tasks
are outside this pending budget. An over-budget request receives JSON-RPC error
`-32000` immediately and may be retried after outstanding queries complete.
Dispatch, cancellation, document invalidation, and shutdown release queue capacity.
Cancellation and shutdown remain available when the queue is full.

The message loop checks deadlines after each input or completed analysis and
while idle. Two bounded workers execute queries and diagnostics, including their
parsing and filesystem access. Each worker owns its parser. One task per source
document prevents repeated requests for one slow buffer from occupying both
workers; eligible work for other documents can proceed. Queries and diagnostics
share deadline order among eligible tasks. Editing one buffer does not reset
another buffer's deadline.

Cancellation and shutdown do not wait for workers. Each task has an independent
cancellation flag written by the server when its response is retired, its source
changes or closes, or the server stops. Workers check it before analysis, after
parsing a document, between completion context/candidate probes, and while building
and validating rename edits. Cancelled work stops at the next check without
returning partial results. Completed document ASTs can still be adopted when their
live revisions match. Running parser calls and filesystem operations are not
preempted; when both workers are busy, further analysis waits. Results are published only
while their request remains active and the source plus any target buffers actually
read by the query still match their snapshots. File queries also check an epoch
for workspace-folder changes and open/close events that could change URI alias
selection. Stale query results return `ContentModified`; stale diagnostics are
discarded. A worker catches an analysis panic, returns `InternalError` for the
query, and recreates its parser before taking more work. Failed diagnostics are
cleared and logged to stderr.

Built-in inline containers yield child token lists to an explicit parse stack,
so nested images and references do not consume the worker's call stack. Each
token list keeps its original parse-hook scope and depth-first callback order.
Temporary image children and partial ASTs are released iteratively as well.

Definition, hover, and reference queries share identifier resolution and the
first-definition-wins rule. Analysis only reads the AST; the parser has no
dependency on LSP code.

Heading anchors use `yozora-ast-util`'s TOC identifiers, including Unicode case
folding, inline content, and duplicate suffixes (`intro`, `intro-2`, ...).
Only top-level headings participate, matching the existing TOC contract; nested
headings still appear in outlines. The optional `initializationOptions.headingIdPrefix`
uses the same literal prefix as `calc_heading_toc`; it defaults to an empty string
and accepts up to 256 UTF-8 bytes without control characters. Fragments are
percent-decoded once and matched exactly, without further case folding.

Local navigation accepts file URIs with an empty authority or `localhost`.
Relative paths use the source file's directory. Untitled/non-file buffers support
same-buffer anchors and external document links. Paths and fragments support
percent-encoded Unicode and delimiters; `+` remains a literal plus. File queries
are separated from the path and preserved in document links. Paths must remain
within `workspaceFolders` (or `rootUri` when folders are absent); without a
workspace, access is limited to the source file's directory. Remote workspace
roots do not grant local file access. Folder changes are validated before replacing
the roots. Canonical paths are checked against the roots, including symlinks and
the existing ancestors of unsaved files. Sensitive paths such as `.ssh`, `.env*`,
`local/env.*`, and credential/request/response files are excluded.

Open file buffers are matched by requested URI first, then normalized lexical
path, then canonical aliases, retaining the selected buffer's original URI.
An out-of-sync target returns `ContentModified`
without falling back to disk. Closed targets are never cached: anchor lookup reads
only regular UTF-8 `.md`, `.markdown`, `.mdown`, `.mkd`, `.mkdn`, or `.yozora` files,
case-insensitively, with the same 16 MiB limit as open buffers. Navigation without
a fragment only checks file metadata. No extensions, directory index files, or
website routes are inferred. External URLs are never fetched.

Completion reads the source text and AST through one immutable snapshot of the
document version. Its lexical context handles unfinished syntax, while AST
ranges exclude non-reference regions. Label identity is reused from
`yozora-core-tokenizer`; this adds an internal workspace dependency.
Each candidate is validated by reparsing its containing top-level block with all
active definition associations. It must produce the intended reference at the
exact edited range. This preserves multiline delimiter pairing, table escaping,
and container syntax. Validation has a 1 MiB input budget per request, counting
both source and association strings; at least one candidate is checked even if
its block exceeds the budget. The 200-candidate limit still applies.

Destination completion first checks its context by reparsing the containing block
with URL markers. It then reparses candidate edits against that same block and
definition associations. The server retains an owned probe while reading a target
buffer, keeping source and target borrows separate. Context probes, candidate
probes, association strings, and returned text share a strict 1 MiB budget; large
blocks or long destinations can yield fewer candidates or no candidates. At most
200 candidates are returned. Directory completion examines up to 1,000 entries in
one directory and merges scoped open file paths; it never scans recursively.
Names are sorted, and directory contents are read anew on each request. All local
access restrictions used by navigation also apply to completion. Anchor lookups
use the existing bounded Markdown reader and return `ContentModified` for an
out-of-sync open target instead of using disk contents.

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
diagnostic list without a version. Shutdown replies to all pending and running queries with
`RequestCancelled` before its own response and discards scheduled diagnostics.
Once a notification identifies an open document and a newer version, an undecodable
or missing edit batch also suspends synchronization. Invalid document metadata is
rejected before changing document state.

The stdio transport uses the existing `serde` and `serde_json` dependencies. It
supports the advertised LSP subset and the initialize/shutdown/exit lifecycle.
Stdout carries framed JSON-RPC messages; errors are written to stderr. Incoming
frames and documents are limited to 16 MiB, and input headers to 8 KiB.
A dedicated blocking reader and the analysis workers publish to one bounded event
channel. This lets diagnostic timers run even while stdin is idle or a frame is
incomplete. Live document mutations, scheduling, result validation, and all stdout
writes remain on the server thread. The reader and workers have process lifetime
and are not joined on exit, so shutdown does not depend on the client closing
stdin or on a slow analysis finishing. Input errors propagate to the server loop.

## Validation

```sh
cargo test -p yozora-lsp
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

An optional acceptance test runs Neovim's native LSP client in headless mode.
It requires Neovim 0.11 or later in `PATH` and is ignored by default:

```sh
cargo test -p yozora-lsp --test neovim -- --ignored
```

The test uses factory defaults and a temporary workspace with spaces and Unicode
in its path. It covers `.yozora` attachment, workspace roots, unsaved target
navigation, UTF-16 completion edits, native rename and incremental synchronization,
diagnostic updates, a 2,001-edit rename, deep documents, cancellation, close/reopen,
and graceful shutdown. It exercises native client APIs and edit application;
interactive UI rendering and user plugin configurations are outside its coverage.

The integration tests launch the binary and communicate over pipes, exercising
framing, lifecycle, client capabilities, Unicode edits, and live document queries.
They exercise automatic diagnostics for deeply nested images and a large rename
round trip through incremental `didChange`. Edit tests compare batch processing
against sequential application across Unicode, CRLF, clamping, and invalid ranges.
They also check diagnostic publications on open, edit, resynchronization, and close,
including notifications interleaved with responses, coalesced edits, and incomplete
input frames. Scheduling tests use explicit timestamps to verify deadlines without
wall-clock sleeps. Worker tests use channel synchronization to verify progress
while another parser is blocked and recovery after an analysis panic. A message-loop
test uses the same synchronization with real framed messages to check cancellation,
other-buffer queries, reuse of a cancelled task's AST, and shutdown/exit while both
workers are blocked. Completion tests block individual context and candidate probes
to verify that cancellation, edits, and close/reopen stop subsequent probes and
allow fresh queries for the same buffer. Worker tests also cover cancellation before
parsing and during a rename's initial parse, including reuse of the completed AST.
Snapshot tests cover cancellation, request ID reuse,
target-buffer edits, workspace changes, close/reopen, stale diagnostics, and cache
adoption across document versions.
Queue tests cover unknown parameter fields, count and byte budgets, and capacity
release. A blocked-worker test fills the queue and verifies overload responses,
cancellation, recovery for another buffer, and shutdown. Panic recovery also
includes already-materialized deep inline and block results.
