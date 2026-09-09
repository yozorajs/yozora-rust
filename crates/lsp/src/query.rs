use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use serde::Serialize;
use serde_json::{json, Value};
use yozora_ast::Root;
use yozora_parser::YozoraParser;

use crate::cancellation::Cancellation;
use crate::document::{open_document, Document, Snapshot};
use crate::files::{self, FileScope, Target, Workspace};
use crate::protocol::{
    parse_params, DocumentSymbol, FileRename, Json, Location, Position, Range, ReferencesParams,
    RenameFilesParams, RenameParams, ResponseError, TextDocumentParams, TextDocumentPositionParams,
    TextEdit, WorkspaceSymbolParams,
};
use crate::workspace_index::Index;
use crate::{
    analysis, completion, diagnostics, heading_references, link_diagnostics, links, refactor,
    rename, resource_completion,
};

/// The server owns live text and versions. Clones share immutable document data
/// while each analysis task owns its AST cache updates.
#[derive(Clone, Default)]
pub struct State {
    pub documents: HashMap<String, Document>,
    pub workspace: Workspace,
    /// Set only after the client confirms a filesystem watch registration.
    /// None retains content-based validation on every query.
    pub file_revision: Option<u64>,
    pub file_events: Arc<files::FileEvents>,
    pub refactor_file_event_cache: bool,
    pub heading_id_prefix: String,
    pub hierarchical_symbols: bool,
    pub folding_range_limit: Option<usize>,
    pub hover_markdown: bool,
    pub versioned_edits: bool,
    pub diagnostic_related_information: bool,
    pub diagnostic_version: bool,
}

#[derive(Default)]
pub struct Dependencies {
    pub targets: HashSet<String>,
    pub files: bool,
}

/// Decode the supported parameters at the input boundary. Queues and workers
/// retain only data used by the request, never client extension fields.
pub enum Request {
    Document(DocumentRequest),
    WorkspaceSymbols(String),
    RenameFiles(Vec<FileRename>),
}

pub struct DocumentRequest {
    uri: String,
    kind: RequestKind,
}

enum RequestKind {
    DocumentSymbol,
    FoldingRange,
    Definition(Position),
    DocumentLink,
    Hover(Position),
    References {
        position: Position,
        include_declaration: bool,
    },
    Completion(Position),
    PrepareRename(Position),
    Rename {
        position: Position,
        new_name: String,
    },
}

impl Request {
    pub fn parse(method: &str, params: Value) -> Result<Self, ResponseError> {
        if method == "workspace/symbol" {
            let params: WorkspaceSymbolParams = parse_params(params)?;
            return Ok(Self::WorkspaceSymbols(params.query));
        }
        if method == "workspace/willRenameFiles" {
            let params: RenameFilesParams = parse_params(params)?;
            if params.files.len() > 128 {
                return Err(ResponseError::invalid_params(
                    "a file rename batch permits at most 128 files",
                ));
            }
            return Ok(Self::RenameFiles(params.files));
        }
        let (uri, kind) = match method {
            "textDocument/documentSymbol"
            | "textDocument/foldingRange"
            | "textDocument/documentLink" => {
                let params: TextDocumentParams = parse_params(params)?;
                let kind = match method {
                    "textDocument/documentSymbol" => RequestKind::DocumentSymbol,
                    "textDocument/foldingRange" => RequestKind::FoldingRange,
                    _ => RequestKind::DocumentLink,
                };
                (params.text_document.uri, kind)
            }
            "textDocument/definition"
            | "textDocument/hover"
            | "textDocument/completion"
            | "textDocument/prepareRename" => {
                let params: TextDocumentPositionParams = parse_params(params)?;
                let kind = match method {
                    "textDocument/definition" => RequestKind::Definition(params.position),
                    "textDocument/hover" => RequestKind::Hover(params.position),
                    "textDocument/completion" => RequestKind::Completion(params.position),
                    _ => RequestKind::PrepareRename(params.position),
                };
                (params.text_document.uri, kind)
            }
            "textDocument/references" => {
                let ReferencesParams {
                    position_params: params,
                    context,
                } = parse_params(params)?;
                (
                    params.text_document.uri,
                    RequestKind::References {
                        position: params.position,
                        include_declaration: context.include_declaration,
                    },
                )
            }
            "textDocument/rename" => {
                let RenameParams {
                    position_params: params,
                    new_name,
                } = parse_params(params)?;
                (
                    params.text_document.uri,
                    RequestKind::Rename {
                        position: params.position,
                        new_name,
                    },
                )
            }
            _ => return Err(ResponseError::new(-32601, "Method not found")),
        };
        Ok(Self::Document(DocumentRequest { uri, kind }))
    }

    /// None means the request has no source document.
    pub fn source_uri(&self) -> Option<&str> {
        match self {
            Self::Document(request) => Some(&request.uri),
            Self::WorkspaceSymbols(_) | Self::RenameFiles(_) => None,
        }
    }

    /// Rename and references may need workspace reads after a worker resolves
    /// their subject. Reserve this scope before dispatch.
    pub fn uses_workspace(&self) -> bool {
        matches!(
            self,
            Self::WorkspaceSymbols(_)
                | Self::RenameFiles(_)
                | Self::Document(DocumentRequest {
                    kind: RequestKind::Rename { .. } | RequestKind::References { .. },
                    ..
                })
        )
    }

    pub fn owned_bytes(&self) -> usize {
        match self {
            Self::WorkspaceSymbols(query) => query.capacity(),
            Self::RenameFiles(files) => {
                files.capacity() * std::mem::size_of::<FileRename>()
                    + files
                        .iter()
                        .map(|file| file.old_uri.capacity() + file.new_uri.capacity())
                        .sum::<usize>()
            }
            Self::Document(request) => {
                request.uri.capacity()
                    + match &request.kind {
                        RequestKind::Rename { new_name, .. } => new_name.capacity(),
                        _ => 0,
                    }
            }
        }
    }
}

impl State {
    /// Only source-local immutable results may bypass worker dispatch. Their
    /// cache is owned and invalidated by the live document's writer.
    pub fn cached_request(&self, request: &Request) -> Option<crate::protocol::Json> {
        match request {
            Request::Document(DocumentRequest {
                uri,
                kind: RequestKind::DocumentSymbol,
            }) if self.hierarchical_symbols => self.documents.get(uri)?.cached_symbols(),
            _ => None,
        }
    }

    pub fn request(
        &mut self,
        request: Request,
        parser: &YozoraParser,
        index: &mut Index,
        cancellation: &Cancellation,
        dependencies: &mut Dependencies,
    ) -> Result<Json, ResponseError> {
        cancellation.check()?;
        if let Request::Document(DocumentRequest {
            uri,
            kind: RequestKind::DocumentSymbol,
        }) = &request
        {
            if self.hierarchical_symbols {
                let document = open_document(&mut self.documents, uri)?;
                if let Some(symbols) = document.cached_symbols() {
                    return Ok(symbols);
                }
                let root = document_ast(document, parser, cancellation)?;
                let symbols = Json::encode(&analysis::document_symbols(root));
                cancellation.check()?;
                document.cache_symbols(symbols.clone());
                return Ok(symbols);
            }
        }
        index.catalog.events = Arc::clone(&self.file_events);
        let DocumentRequest { uri, kind } = match request {
            Request::Document(request) => request,
            Request::WorkspaceSymbols(query) => {
                dependencies.files = true;
                return index
                    .symbols(
                        &query,
                        &self.workspace,
                        &mut self.documents,
                        parser,
                        cancellation,
                        &mut dependencies.targets,
                        self.file_revision,
                    )
                    .map(|symbols| Json::encode(&symbols));
            }
            Request::RenameFiles(files) => {
                dependencies.files = true;
                let edits = refactor::Context {
                    validate_disk: !self.refactor_file_event_cache || self.file_revision.is_none(),
                    index: &mut index.refactors,
                    catalog: &mut index.catalog,
                    revision: self.file_revision,
                    workspace: &self.workspace,
                    documents: &mut self.documents,
                    parser,
                    cancellation,
                    used_buffers: &mut dependencies.targets,
                    heading_id_prefix: &self.heading_id_prefix,
                }
                .files(&files)?;
                return Ok(workspace_edit(edits, self.versioned_edits));
            }
        };
        match kind {
            RequestKind::DocumentSymbol => {
                let document = open_document(&mut self.documents, &uri)?;
                let root = document_ast(document, parser, cancellation)?;
                let symbols = analysis::document_symbols(root);
                // Hierarchical outlines use the immutable cache above. Flat
                // symbols also contain the request URI, so build that view here.
                Ok(flat_symbols(&symbols, &uri).into())
            }
            RequestKind::FoldingRange => {
                let document = open_document(&mut self.documents, &uri)?;
                let root = document_ast(document, parser, cancellation)?;
                let mut ranges = analysis::folding_ranges(root);
                if let Some(limit) = self.folding_range_limit {
                    ranges.truncate(limit);
                }
                Ok(Json::encode(&ranges))
            }
            RequestKind::Definition(position) => self
                .definition(
                    &uri,
                    position,
                    parser,
                    cancellation,
                    dependencies,
                    &mut index.targets,
                )
                .map(Json::from),
            RequestKind::DocumentLink => {
                dependencies.files = true;
                let scope = self.workspace.scope(&uri);
                let open_uris = self.open_file_uris(&scope, cancellation)?;
                let document = open_document(&mut self.documents, &uri)?;
                let root = document_ast(document, parser, cancellation)?;
                let links = links::document_links(root, |destination| {
                    if cancellation.is_cancelled() {
                        return None;
                    }
                    match files::resolve(&uri, destination, &scope)? {
                        Target::External(uri) => Some(uri),
                        Target::Local {
                            file,
                            query,
                            fragment,
                        } => {
                            let target_uri = match file {
                                None => uri.clone(),
                                Some(file) => match files::open_file_uri(&open_uris, &file, &uri) {
                                    Some(uri) => uri.to_string(),
                                    None if files::is_file(&file.path) => {
                                        files::path_uri(&file.path)?
                                    }
                                    None => return None,
                                },
                            };
                            files::link_uri(&target_uri, query.as_deref(), fragment.as_deref())
                        }
                    }
                });
                cancellation.check()?;
                Ok(Json::encode(&links))
            }
            RequestKind::Hover(position) => {
                let document = open_document(&mut self.documents, &uri)?;
                document.validate_position(position)?;
                let root = document_ast(document, parser, cancellation)?;
                Ok(analysis::hover(root, position)
                    .map(|info| format_hover(info, self.hover_markdown))
                    .unwrap_or(Value::Null)
                    .into())
            }
            RequestKind::References {
                position,
                include_declaration,
            } => {
                let document = open_document(&mut self.documents, &uri)?;
                document.validate_position(position)?;
                let root = document_ast(document, parser, cancellation)?;
                if !analysis::resource_or_symbol_at(root, position)
                    .is_some_and(|node| analysis::symbol_key(node).is_some())
                {
                    dependencies.files = true;
                    return heading_references::Context {
                        index: &mut index.references,
                        catalog: &mut index.catalog,
                        revision: self.file_revision,
                        workspace: &self.workspace,
                        documents: &mut self.documents,
                        parser,
                        cancellation,
                        used_buffers: &mut dependencies.targets,
                        heading_id_prefix: &self.heading_id_prefix,
                    }
                    .find(&uri, position, include_declaration)
                    .map(|locations| Json::encode(&locations));
                }
                let locations: Vec<_> = analysis::references(root, position, include_declaration)
                    .into_iter()
                    .map(|range| Location {
                        uri: uri.clone(),
                        range,
                    })
                    .collect();
                Ok(Json::encode(&locations))
            }
            RequestKind::Completion(position) => self
                .complete(
                    &uri,
                    position,
                    parser,
                    cancellation,
                    dependencies,
                    &mut index.targets,
                )
                .map(Json::from),
            RequestKind::PrepareRename(position) => {
                let document = open_document(&mut self.documents, &uri)?;
                let snapshot = document_snapshot(document, parser, position, cancellation)?;
                Ok(Json::encode(
                    &rename::prepare(&snapshot, position)
                        .or_else(|| refactor::prepare(&snapshot, position)),
                ))
            }
            RequestKind::Rename { position, new_name } => {
                let document = open_document(&mut self.documents, &uri)?;
                let snapshot = document_snapshot(document, parser, position, cancellation)?;
                if rename::prepare(&snapshot, position).is_none() {
                    dependencies.files = true;
                    let edits = refactor::Context {
                        validate_disk: !self.refactor_file_event_cache
                            || self.file_revision.is_none(),
                        index: &mut index.refactors,
                        catalog: &mut index.catalog,
                        revision: self.file_revision,
                        workspace: &self.workspace,
                        documents: &mut self.documents,
                        parser,
                        cancellation,
                        used_buffers: &mut dependencies.targets,
                        heading_id_prefix: &self.heading_id_prefix,
                    }
                    .heading(&uri, position, &new_name)?;
                    return Ok(workspace_edit(edits, self.versioned_edits));
                }
                let edits = rename::rename(&snapshot, position, &new_name, parser, cancellation)?;
                Ok(workspace_edit(
                    vec![refactor::DocumentEdits {
                        uri,
                        version: Some(snapshot.version),
                        edits,
                    }],
                    self.versioned_edits,
                ))
            }
        }
    }

    pub fn diagnostics(
        &mut self,
        uri: &str,
        parser: &YozoraParser,
        cancellation: &Cancellation,
        dependencies: &mut Dependencies,
    ) -> Result<Value, ResponseError> {
        cancellation.check()?;
        let document = open_document(&mut self.documents, uri)?;
        let root = document_ast(document, parser, cancellation)?;
        let duplicates = diagnostics::duplicate_definitions(root);
        let resources = link_diagnostics::resources(root, cancellation)?;
        let mut publication = format_diagnostics(
            uri,
            self.diagnostic_version.then_some(document.version()),
            duplicates,
            self.diagnostic_related_information,
        );
        if !resources.is_empty() {
            let scope = self.workspace.scope(uri);
            let links = link_diagnostics::Context {
                source_uri: uri,
                documents: &mut self.documents,
                scope: &scope,
                parser,
                cancellation,
                heading_id_prefix: &self.heading_id_prefix,
                used_buffers: &mut dependencies.targets,
                uses_files: &mut dependencies.files,
            }
            .inspect(resources)?;
            let diagnostics = publication["params"]["diagnostics"]
                .as_array_mut()
                .expect("diagnostics are an array");
            diagnostics.extend(links);
            diagnostics.sort_by_key(|diagnostic| {
                (
                    diagnostic["range"]["start"]["line"].as_u64(),
                    diagnostic["range"]["start"]["character"].as_u64(),
                    diagnostic["range"]["end"]["line"].as_u64(),
                    diagnostic["range"]["end"]["character"].as_u64(),
                )
            });
            diagnostics.truncate(diagnostics::MAX_DIAGNOSTICS);
        }
        cancellation.check()?;
        Ok(publication)
    }

    fn definition(
        &mut self,
        uri: &str,
        position: Position,
        parser: &YozoraParser,
        cancellation: &Cancellation,
        dependencies: &mut Dependencies,
        index: &mut crate::heading_index::Index,
    ) -> Result<Value, ResponseError> {
        let document = open_document(&mut self.documents, uri)?;
        document.validate_position(position)?;
        let root = document_ast(document, parser, cancellation)?;
        if let Some(range) = analysis::definition(root, position) {
            return Ok(json!({ "uri": uri, "range": range }));
        }
        let Some(destination) = links::destination_at(root, position).map(str::to_string) else {
            return Ok(Value::Null);
        };
        dependencies.files = true;
        let scope = self.workspace.scope(uri);
        let Some(Target::Local { file, fragment, .. }) = files::resolve(uri, &destination, &scope)
        else {
            return Ok(Value::Null);
        };
        let fragment = fragment.as_deref().filter(|value| !value.is_empty());
        let target_uri;
        let document = if let Some(file) = file {
            if let Some(open_uri) =
                files::open_file_uri(&self.open_file_uris(&scope, cancellation)?, &file, uri)
            {
                target_uri = open_uri.to_string();
                dependencies.targets.insert(target_uri.clone());
                open_document(&mut self.documents, &target_uri)?
            } else {
                let Some(file_uri) = files::path_uri(&file.path) else {
                    return Ok(Value::Null);
                };
                target_uri = file_uri;
                if fragment.is_none() {
                    return Ok(if files::is_file(&file.path) {
                        json!({ "uri": target_uri, "range": document_start() })
                    } else {
                        Value::Null
                    });
                }
                let headings = index.headings(
                    &file.path,
                    &scope,
                    &self.heading_id_prefix,
                    parser,
                    cancellation,
                )?;
                return Ok(headings
                    .as_ref()
                    .and_then(|headings| {
                        headings
                            .iter()
                            .find(|(identifier, _)| Some(identifier.as_str()) == fragment)
                    })
                    .map(|(_, range)| json!({ "uri": target_uri, "range": range }))
                    .unwrap_or(Value::Null));
            }
        } else {
            target_uri = uri.to_string();
            open_document(&mut self.documents, uri)?
        };
        // Never fall back to disk when an open target has lost synchronization.
        document.validate_position(Position::default())?;
        let range = match fragment {
            Some(fragment) => {
                let root = document_ast(document, parser, cancellation)?;
                links::heading(root, fragment, &self.heading_id_prefix)
            }
            None => Some(document_start()),
        };
        Ok(range
            .map(|range| json!({ "uri": target_uri, "range": range }))
            .unwrap_or(Value::Null))
    }

    fn complete(
        &mut self,
        uri: &str,
        position: Position,
        parser: &YozoraParser,
        cancellation: &Cancellation,
        dependencies: &mut Dependencies,
        index: &mut crate::heading_index::Index,
    ) -> Result<Value, ResponseError> {
        let document = open_document(&mut self.documents, uri)?;
        let snapshot = document_snapshot(document, parser, position, cancellation)?;
        let Some(context) =
            resource_completion::Context::new(&snapshot, position, parser, cancellation)?
        else {
            return Ok(json!(completion::complete(
                &snapshot,
                position,
                parser,
                cancellation
            )?));
        };
        cancellation.check()?;
        dependencies.files = true;
        let scope = self.workspace.scope(uri);
        let open_uris = self.open_file_uris(&scope, cancellation)?;
        let candidates = match context.target() {
            Some(resource_completion::Target::Path(prefix)) => context.paths(
                files::path_candidates(uri, prefix, &scope, open_uris.keys().map(PathBuf::as_path)),
            ),
            Some(resource_completion::Target::Anchor { resource, .. }) => {
                match files::resolve(uri, resource, &scope) {
                    Some(Target::Local { file: None, .. }) => {
                        let document = open_document(&mut self.documents, uri)?;
                        let root = document_ast(document, parser, cancellation)?;
                        context.anchors(root, &self.heading_id_prefix)
                    }
                    Some(Target::Local {
                        file: Some(file), ..
                    }) => {
                        if let Some(uri) = files::open_file_uri(&open_uris, &file, uri) {
                            dependencies.targets.insert(uri.to_string());
                            let document = open_document(&mut self.documents, uri)?;
                            let root = document_ast(document, parser, cancellation)?;
                            context.anchors(root, &self.heading_id_prefix)
                        } else if let Some(headings) = index.headings(
                            &file.path,
                            &scope,
                            &self.heading_id_prefix,
                            parser,
                            cancellation,
                        )? {
                            context.identifiers(
                                headings.iter().map(|(identifier, _)| identifier.as_str()),
                            )
                        } else {
                            Vec::new()
                        }
                    }
                    _ => Vec::new(),
                }
            }
            None => Vec::new(),
        };
        Ok(json!(context.complete(candidates, parser, cancellation)?))
    }

    fn open_file_uris(
        &self,
        scope: &FileScope,
        cancellation: &Cancellation,
    ) -> Result<HashMap<PathBuf, Vec<String>>, ResponseError> {
        scope.open_file_uris(self.documents.keys().map(String::as_str), cancellation)
    }
}

fn workspace_edit(documents: Vec<refactor::DocumentEdits>, versioned: bool) -> Json {
    #[derive(Serialize)]
    struct TextDocument<'a> {
        uri: &'a str,
        version: Option<i32>,
    }
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Edit<'a> {
        text_document: TextDocument<'a>,
        edits: &'a [TextEdit],
    }
    if versioned {
        let changes: Vec<_> = documents
            .iter()
            .map(|document| Edit {
                text_document: TextDocument {
                    uri: &document.uri,
                    version: document.version,
                },
                edits: &document.edits,
            })
            .collect();
        Json::encode(&BTreeMap::from([("documentChanges", changes)]))
    } else {
        let changes: BTreeMap<_, _> = documents
            .iter()
            .map(|document| (document.uri.as_str(), &document.edits))
            .collect();
        Json::encode(&BTreeMap::from([("changes", changes)]))
    }
}

pub(super) fn document_start() -> Range {
    Range {
        start: Position::default(),
        end: Position::default(),
    }
}

pub(super) fn format_diagnostics(
    uri: &str,
    version: Option<i32>,
    duplicates: Vec<diagnostics::DuplicateDefinition>,
    related_information: bool,
) -> Value {
    let diagnostics: Vec<_> = duplicates
        .into_iter()
        .map(|duplicate| {
            let mut diagnostic = json!({
                "range": duplicate.range,
                "severity": 2, // DiagnosticSeverity.Warning
                "code": duplicate.code,
                "source": "yozora-lsp",
                "message": duplicate.message,
            });
            if related_information {
                diagnostic["relatedInformation"] = json!([{
                    "location": { "uri": uri, "range": duplicate.first_definition },
                    "message": "The first definition is used here.",
                }]);
            }
            diagnostic
        })
        .collect();
    let mut params = json!({ "uri": uri, "diagnostics": diagnostics });
    if let Some(version) = version {
        params["version"] = json!(version);
    }
    json!({ "jsonrpc": "2.0", "method": "textDocument/publishDiagnostics", "params": params })
}

fn format_hover(info: analysis::HoverInfo, markdown: bool) -> Value {
    let (kind, value) = if markdown {
        let mut escaped = String::new();
        for character in info.text.chars() {
            if character.is_ascii_punctuation() {
                escaped.push('\\');
            }
            escaped.push(character);
        }
        ("markdown", escaped)
    } else {
        ("plaintext", info.text)
    };
    json!({ "contents": { "kind": kind, "value": value }, "range": info.range })
}

fn flat_symbols(symbols: &[DocumentSymbol], uri: &str) -> Value {
    let mut result = Vec::new();
    let mut stack: Vec<_> = symbols.iter().rev().map(|symbol| (symbol, None)).collect();
    while let Some((symbol, parent)) = stack.pop() {
        let mut entry = json!({
            "name": symbol.name,
            "kind": symbol.kind,
            "location": { "uri": uri, "range": symbol.selection_range },
        });
        if let Some(parent) = parent {
            entry["containerName"] = Value::String(parent);
        }
        result.push(entry);
        stack.extend(
            symbol
                .children
                .iter()
                .rev()
                .map(|child| (child, Some(symbol.name.clone()))),
        );
    }
    Value::Array(result)
}

// Check both sides of parsing so cancellation during a preceding filesystem
// query cannot start another parse. A parse already in progress keeps its cache.
fn document_ast<'a>(
    document: &'a mut Document,
    parser: &YozoraParser,
    cancellation: &Cancellation,
) -> Result<&'a Root, ResponseError> {
    cancellation.check()?;
    let root = document.ast(parser)?;
    cancellation.check()?;
    Ok(root)
}

fn document_snapshot<'a>(
    document: &'a mut Document,
    parser: &YozoraParser,
    position: Position,
    cancellation: &Cancellation,
) -> Result<Snapshot<'a>, ResponseError> {
    cancellation.check()?;
    let snapshot = document.snapshot(parser, position)?;
    cancellation.check()?;
    Ok(snapshot)
}
