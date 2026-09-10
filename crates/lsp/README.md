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
- `$/cancelRequest`: cancel pending or running document and workspace queries.
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
- `workspace/symbol`: search headings in open buffers and unopened Markdown files
  under the local workspace roots. Results are flat `SymbolInformation` entries
  with heading selection ranges and parent names, including nested headings.
  Matching uses a trimmed query and Unicode lowercase substrings; an empty query
  matches every heading. Results are ordered by URI and source position. In Neovim,
  use `vim.lsp.buf.workspace_symbol('intro')` to search and navigate from the list.
- `textDocument/foldingRange`: heading sections, code, math, admonitions,
  blockquotes, lists, tables, HTML, and multiline definitions. Heading sections
  end at the next heading of equal or lower depth in the same block container,
  or at that container's end. Ranges use inclusive line numbers and respect the
  client's range limit.
- `textDocument/selectionRange`: expand selections from words and inline nodes to
  containing blocks, heading sections, and the document. Positions use UTF-16;
  empty lines and EOF remain selectable. Each request accepts up to 128 positions
  and returns one chain per input position, in input order. Each parent strictly
  contains its child; chains retain at most 32 ranges, including the document,
  to bound memory and JSON nesting for deeply nested input.
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
- `workspace/didChangeWorkspaceFolders`: update local file access and indexing
  roots, and refresh link diagnostics.
- `workspace/didChangeWatchedFiles`: invalidate pending and running workspace
  queries and refresh link diagnostics when the client supplies file changes.
  When the client supports dynamic registration and relative patterns, the
  server registers watches under explicit local roots. Event-based reuse starts
  only after a successful registration response. Unsupported or rejected watches
  retain query-time discovery and content validation. Without notifications,
  external changes reach diagnostics when those diagnostics next run.
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
  At a top-level heading or a direct link/image pointing to one, the same request
  finds heading references across the workspace. It includes direct links,
  images, written link definitions, and bound reference occurrences, including
  those in admonition titles and footnotes. `includeDeclaration` adds only the
  heading's selection range; a link definition is a use of that heading.
  Results are deduplicated and ordered by URI and source position. IDs, duplicate
  suffixes, prefixes, and fragment decoding match navigation. Label occurrences
  and definitions retain their document-local reference semantics; an inner
  direct resource takes precedence over an enclosing declaration.
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
  At heading text, these methods also rename ATX and setext headings, preserving
  their markers. The new name is plain text, at most 1,000 characters, nonempty
  and single-line without surrounding whitespace; Markdown punctuation is escaped
  automatically. The server updates same-document and workspace links to every
  heading whose ID changes, including duplicate-suffix shifts. For example,
  renaming the first `Intro` heading to `Other` changes IDs
  `intro, intro-2, other` to `other, intro, other-2`; links continue to identify
  the same headings. References within heading text retain label-rename priority.
- `textDocument/codeAction`: clients advertising `codeActionLiteralSupport`
  receive complete, versioned workspace edits when they also support
  `workspaceEdit.documentChanges`; other literal-capable clients receive `changes`.
  The server advertises two kinds and honors hierarchical `context.only` filters:
  - `source.organizeLinkDefinitions`: collect top-level link definitions at the
    end of the document, sorted by normalized identifier. Original definition
    spellings, titles, unused entries, and duplicate order are preserved. Nested
    definitions remain in their containers; an action that would change binding
    priority or other Markdown structure is omitted. Already organized documents
    produce no action.
  - `refactor.extract.linkDefinition`: extract a selected inline link, image, or
    autolink to a fresh `link`, `link2`, ... definition. Matching inline occurrences
    with the same destination **and title** share the definition. Display text,
    image alt text, other titles, and existing references retain their semantics.
    The generated label must not activate unrelated plain-text references.
  Both actions validate the complete preview before offering edits; the client
  applies them. Code, existing references, and unsafe extraction contexts do not
  produce extract actions. A request permits 32 filter kinds of 256 bytes each,
  10,000 edits per action, a conservative 4 MiB edit budget per action, and 128 MiB
  of total preview parse input. Extraction attempts at most eight safe-label
  previews. Formatting and unused-definition removal are separate concerns.
- `workspace/willRenameFiles`: update links when regular files or directories move
  within explicit local workspace roots, including Markdown files and linked assets.
  Incoming links and moved documents' relative outgoing links are rewritten;
  query strings, fragments, and trailing directory slashes are preserved.
  Directory moves rebase descendant buffers and links without enumerating assets
  into individual operations. Up to 128 file/directory paths can move in one
  request, including exact swaps. Overlapping source/destination subtrees, moving
  a workspace root, and moving a directory into itself are rejected. The
  file-operation filter accepts files and directories; existing source-scan,
  byte, edit, canonical-path, and destination-collision limits still apply.
  `workspace/didRenameFiles` invalidates workspace analysis and refreshes link
  diagnostics; buffer identities still follow `didClose`/`didOpen`.
- `textDocument/publishDiagnostics`: warnings for duplicate link and footnote
  definitions, following the same identifier normalization and declaration scope
  as navigation. The first definition remains active; later declarations produce
  `duplicate-link-definition` or `duplicate-footnote-definition`. Locations use
  the parser's definition ranges. Clients supporting `relatedInformation` receive
  a location pointing to the active definition; clients supporting `versionSupport`
  receive the document version. Written local destinations also report
  `missing-file` for nonexistent targets and `missing-anchor` for absent heading
  IDs in known Markdown text. Links, images, and reference definitions are checked;
  a definition's destination is diagnosed once, irrespective of its reference count.
  Target buffers use unsaved text. Unreadable, out-of-sync, invalid UTF-8,
  oversized, and over-budget targets are treated as unknown and produce no link
  warning. Fragments in closed non-Markdown files, external URLs, and disallowed
  paths are skipped.
  Up to 1,000 combined warnings are published in source order.
  Publications are debounced by 150 ms per document, so intermediate edit versions
  can be skipped during continuous typing. Editing a target refreshes its known
  referrers without changing their source versions; opening/closing buffers and
  file/scope notifications also refresh sources with local file destinations.

Document queries start from open buffers, including unsaved documents. Target buffers use
their unsaved text; closed Markdown files are read on demand for anchor navigation.
Formatting and semantic tokens are outside the current capabilities.
Unresolved reference labels remain ordinary text, following the parser's fallback
behavior.

## Implementation contracts

The server owns each document's text, version, line index, and cached AST/outline. Edits
are processed serially and each batch is applied atomically. Opening a document
or accepting a change schedules diagnostics with a 150 ms delay. Further edits to
that document reset its deadline. Eligible queries run immediately, without a
fixed debounce interval. Queued cancellation and revision checks still retire
obsolete work before parsing or publication. An accepted newer change invalidates
pending and running queries for that URI, including when its edit batch is malformed. Stale
changes leave waiting queries intact. Reopening also invalidates them when the
version number is reused. Queries and diagnostics reuse the same AST until the
next edit invalidates it.
Open, accepted changes, close, workspace-folder changes, and valid watched-file
or file-rename notifications also retire workspace queries with `ContentModified`.
Text, line indexes, and parsed ASTs are shared through `Arc`. Each analysis gets
a snapshot of open buffers, workspace roots, and client capabilities. Workers
only update their snapshot's AST and outline caches; the server adopts those caches only
when text identity and version still match the live document. Text identity also
distinguishes close/reopen when the client reuses a version number.
Workers encode query results once. Hierarchical outlines reuse immutable JSON
bytes for the same document revision, with at most 1 MiB of cached outline bytes
per buffer. The server attaches each request's ID after freshness checks and
writes the encoded response without rebuilding its JSON tree. Large outlines
remain available when they exceed the cache limit; only their reuse is skipped.
The line index keeps sparse UTF-16 checkpoints on long lines, bounding coordinate
scans for dense reference edits. Rename constructs the proposed source in one pass
before validating its semantics. Descending `didChange` edits with strictly
separated ranges also build the text in one pass and rebuild the index once.
Other batches retain sequential coordinates, including touching ranges, clamped
positions, and edits that join CR/LF boundaries. Every intermediate document must
fit the size limit, even when a later edit would shrink it.

Query requests are decoded into typed parameters before scheduling; unused
client extension fields are discarded. The pending queue admits at most 128
requests with a combined 1 MiB accounting budget for request records and their
owned strings, including IDs, URIs, rename text, file-operation paths, and workspace
query text. The two active worker tasks
are outside this pending budget. An over-budget request receives JSON-RPC error
`-32000` immediately and may be retried after outstanding queries complete.
Dispatch, cancellation, document invalidation, and shutdown release queue capacity.
Cancellation and shutdown remain available when the queue is full.

The message loop checks deadlines after each input or completed analysis and
while idle. Two bounded workers execute queries and diagnostics, including their
parsing and filesystem access. Each worker owns its parser. One task per source
document prevents repeated requests for one slow buffer from occupying both
workers. Workspace symbol searches, rename, and references requests share a
separate scope with at most one active operation, so other document queries can
use the second worker. Rename and references reserve this scope before a worker
resolves whether their subject needs workspace access, including label queries.
Queries and diagnostics share deadline order among eligible tasks. Editing a
buffer resets its own diagnostic deadline and those of its known link referrers.

Cancellation and shutdown do not wait for workers. Each task has an independent
cancellation flag written by the server when its response is retired, its source
changes or closes, or the server stops. Workers check it before analysis, after
parsing a document, during workspace traversal and symbol collection, between
completion context/candidate probes, and while building and validating rename edits.
Cancelled work stops at the next check without
returning partial results. Completed document ASTs can still be adopted when their
live revisions match. Running parser calls and filesystem operations are not
preempted; when both workers are busy, further analysis waits. Results are published only
while their request remains active and the source plus any target buffers actually
read by the query still match their snapshots. File queries also check an epoch
for workspace-folder changes, watched-file hints, and open/close events that could change URI alias
selection. Stale query results return `ContentModified`; stale diagnostics are
discarded and rescheduled if the source remains synchronized. Only current
diagnostic results replace the source's target dependencies, including unavailable
target revisions so recovery can refresh referrers. A worker catches an analysis panic, returns `InternalError` for the
query, recreates its parser, and clears its workspace cache before taking more work. Failed diagnostics are
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
same-buffer anchors, external document links, and absolute local paths or file URIs
inside explicit workspace roots; no relative base is inferred. Paths and fragments support
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
for navigation and completion, while link diagnostics omit uncertain anchors;
neither falls back to disk. Anchor lookup reads closed targets afresh, using
only regular UTF-8 `.md`, `.markdown`, `.mdown`, `.mkd`, `.mkdn`, or `.yozora` files,
case-insensitively, with the same 16 MiB limit as open buffers. Navigation without
a fragment only checks file metadata. No extensions, directory index files, or
website routes are inferred. External URLs are never fetched.

Workspace indexing scans only explicit local roots; without them it searches open
buffers, including untitled and remote buffers. Open buffers also participate
outside the configured roots, using their client-supplied text. Local buffer URIs
follow the same sensitive-path and URI restrictions as navigation. Open text
overrides disk text. Aliases inside the roots are grouped by canonical path,
preferring the canonical URI, then an equivalent lexical path, then the first URI
in lexical order. Results preserve the selected buffer's original URI. An
out-of-sync selected buffer makes the query return `ContentModified`.

Disk discovery deduplicates overlapping roots and symlink cycles. The navigation
access boundary applies, and paths are checked again before reading in case they
changed after discovery. In addition to sensitive paths, indexing skips `target`,
`node_modules`, `.cache`, and `.venv` directory trees below each explicit root.
A root placed inside one of these directories remains searchable. It does not interpret
`.gitignore`. Each query rediscovers files and reads eligible contents when
`initializationOptions.fileEventCache` is `false`, or no watch was confirmed.
Creation, modification, renaming, and deletion are then visible without notifications.
Otherwise, inventory and semantic summaries reuse the client's file-event
revision. Content changes to known regular files refresh only those sources;
creation, deletion, directory/alias changes, unavailable event history, and root
changes rebuild the inventory. The event journal retains at most 256 paths and
64 KiB of path bytes before requiring a full refresh. Open buffers always use
their current text identity and version.

`fileEventCache` defaults to automatic watch negotiation. Event caching assumes
the client delivers changes under the registered roots; missed events can leave
read-query results stale. Setting it to `false` forces complete discovery and
byte comparison on every workspace query, including refactors. Cached contents
avoid repeated parsing in either mode; timestamp equality alone never validates
changed content. Each worker releases closed ASTs after extracting summaries.
Workspace symbols also cache open-buffer summaries by immutable text identity.
Closed-target definition and completion share heading summaries after checking
the target's current contents and access scope.

One workspace query permits up to 128 roots, 20,000 directory entries (including
skipped names), 2,000 candidate files/buffers, 32 MiB of source text and attempted
reads (including invalid UTF-8), and 8 MiB of
heading summary payload. The existing 16 MiB per-document reader limit applies;
ineligible, unreadable, oversized, or non-UTF-8 files are omitted. Unreadable
directories and exceeded scan budgets return error `-32000`; narrow the roots or
close buffers before retrying. Each worker's workspace-symbol cache has a 40 MiB
payload budget across disk and buffer entries. Reference and refactor summaries
have separate 40 MiB budgets, and closed-target heading summaries have 20 MiB.
These caches may evict entries without affecting results. A response is limited to 1,000
symbols and a conservative 1 MiB budget for result strings, structure, and JSON
escaping. Exceeding a response budget returns `-32000` with a request to narrow
the query, never a silently truncated success response.

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
receive versioned edits for open buffers and null versions for closed files;
other clients receive `changes`. File-operation edits address the old URIs: apply
the edits before moving files, then send `didRenameFiles` and synchronize renamed
buffers through `didClose`/`didOpen`. The server never writes or moves files itself.
Before returning edits, the server validates AST structure, display content,
resources, and reference bindings through parsing or proven literal replacements.
Conflicting names,
unintended activation of plain text references, syntax changes, and edits exceeding
the document size limit are rejected as a whole.

Heading ID changes in local files require an explicit root containing the source;
changes that retain IDs, including nested headings, can remain local. Untitled
headings update same-buffer anchors. Renames reject new IDs that would activate
existing unresolved anchor links, and reject removal of an addressable ID.
Workspace refactors share index discovery and open-buffer overlay rules, but
reject ambiguous open URI aliases and any unreadable eligible Markdown source.
File operations require canonical paths, reject symlink operations, and reject
overwriting a destination outside the same batch. File autolinks whose visible
text would change are also rejected. A rejected operation returns no partial edits.
Refactors always recheck roots, resolved aliases, participating buffer revisions,
and the file-event epoch before publishing. By default, they also reread every
closed source at the end, detecting changed text with `ContentModified` even when
its file event has not arrived. On Linux, these reads check the opened file's
canonical descriptor path before reading; other platforms retain path-based
validation. Checks of at least 128 closed sources use up to four readers,
including the owning query worker and at most three short-lived filesystem
threads, limited by available CPU parallelism. Every edit is planned before these
checks start, and all readers finish before publication. Their byte budgets
partition the original source size; changed or cancelled reads yield no edits.
Inventory follows the `fileEventCache` mode above.

`initializationOptions.refactorFileEventCache: true` also lets refactors trust
the acknowledged file-event revision for closed source text, matching the usual
event-cache consistency model. This option defaults to `false` and has no effect
without a confirmed watch. It removes the final whole-workspace text read; missed
events can therefore produce edits for an older disk snapshot. Set
`fileEventCache: false` for complete discovery and content revalidation regardless
of this option. Changes after any final check remain the client's responsibility.

Refactor caches store source text and destination spans, independently of the
requested target and new name. Unambiguous literal URI replacements preserve
their Markdown delimiters without reparsing the full preview; nonliteral or
ambiguous destinations retain full structural validation. A plain top-level ATX
heading can similarly be validated in isolation, preserving the global heading
ID and duplicate-suffix calculation. Resource, edit, source-size, and analysis
budgets apply to cached and uncached paths alike.

Workspace refactors allow 128 roots, 20,000 directory entries, 2,000 candidate
files/buffers, and 32 MiB of source text. They also limit total parse input to
128 MiB, written resources to 20,000, top-level headings in the renamed document
to 20,000, edits to 10,000, and conservative edit output to 4 MiB. Exceeding a
budget rejects the whole operation. Link diagnostics separately inspect at most
1,000 written destinations with 1 MiB of URL input and 32 MiB of target text per
source analysis. Each target is parsed at most once per analysis; targets with
more than 20,000 top-level headings are unknown for anchor validation.

Heading-reference searches use explicit local workspace roots and the index's
directory exclusions. Without local roots, or for a heading outside their access
scope, only same-buffer anchors are searched. Open buffers override disk text.
Every scoped open URI alias participates with its own unsaved contents, and each
link selects its target by the same exact-URI, lexical-path, and canonical-alias
precedence as navigation. A closed target is parsed once and reused while scanning
its self references; other closed ASTs are released after each file. Reference
summaries cache both raw destinations and resolved targets. Their reuse follows
the same acknowledged event revision and inventory generation as indexing;
without event caching, each query compares current disk bytes before reuse.

A heading-reference query permits 128 roots, 20,000 directory entries, 2,000
candidate files/buffers, 32 MiB of source/target reads, and 20,000 resource
occurrences with 8 MiB of destination input. Results are limited to 10,000
locations and a conservative 4 MiB output budget. Targets with more than 20,000
top-level headings or exceeded scan/result budgets return `-32000` without a
partial result. An out-of-sync participating buffer returns `ContentModified`.
Unreadable, oversized, and invalid UTF-8 closed candidates are omitted; an absent
or unreadable closed target, missing ID, or unaddressable heading returns an empty
list. Code, HTML, external destinations, and disallowed paths are not references.

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
cargo test --release -p yozora-lsp --test neovim -- --ignored
```

The test uses factory defaults and a temporary workspace with spaces and Unicode
in its path. It covers `.yozora` attachment, workspace roots, unsaved target
navigation, native workspace symbol lists and disk refresh, UTF-16 completion
edits, label and heading rename, file-operation edits followed by a native file
move, native heading-reference lists with unopened referrers, and incremental
synchronization. Smart selection and both code actions run through native editor
APIs; directory moves apply edits and rename descendant buffers before checking
navigation. It also checks link diagnostic refresh
after unsaved target edits and watched file creation/edit/deletion, a 2,001-edit
rename, deep documents, cancellation, close/reopen,
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
Workspace tests cover unopened and unsaved files, URI aliases, Unicode and nested
headings, content-cache reuse, edits with unchanged lengths, create/rename/delete,
root changes, symlink cycles and replacements, scan/cache/response budgets, and
out-of-sync buffers. A framed-message test blocks disk parsing to verify that
document requests, cancellation, buffer changes, and shutdown remain responsive.
Refactor tests cover duplicate heading ID shifts, Markdown preservation, nested
and multiline headings, file swaps, relative-link rebasing, canonical-path and
alias restrictions, cancellation, disk changes during analysis, and both
WorkspaceEdit formats. Diagnostic tests cover exact anchors, shared target parsing,
written definitions, uncertainty, target-buffer lifecycle, and watched files. A
framed-message test changes a target while the first source analysis is blocked,
verifying that stale warnings are discarded and diagnostics recover without a
source edit.
Heading-reference tests cover declaration inclusion, reference-label precedence,
Unicode and duplicate IDs, exact aliases with different unsaved content, closed
target parse reuse, open/close/resynchronization, disk refresh, and result budgets.
A framed-message test blocks a reference scan while other document queries,
cancellation, referrer edits, and shutdown/exit continue; retired requests receive
exactly one response and fresh queries use the changed referrer.
