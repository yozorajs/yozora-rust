use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde_json::{json, Value};
use yozora_ast::Root;
use yozora_parser::YozoraParser;

use crate::cancellation::Cancellation;
use crate::document::{open_document, Document, Snapshot};
use crate::files::{self, FileScope, Target, Workspace};
use crate::protocol::{
    parse_params, DocumentSymbol, FileRename, Position, Range, ReferencesParams, RenameFilesParams,
    RenameParams, ResponseError, TextDocumentParams, TextDocumentPositionParams,
    WorkspaceSymbolParams,
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
    pub fn request(
        &mut self,
        request: Request,
        parser: &YozoraParser,
        index: &mut Index,
        cancellation: &Cancellation,
        dependencies: &mut Dependencies,
    ) -> Result<Value, ResponseError> {
        cancellation.check()?;
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
                    )
                    .map(|symbols| json!(symbols));
            }
            Request::RenameFiles(files) => {
                dependencies.files = true;
                let edits = refactor::Context {
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
                if self.hierarchical_symbols {
                    Ok(json!(symbols))
                } else {
                    Ok(flat_symbols(&symbols, &uri))
                }
            }
            RequestKind::FoldingRange => {
                let document = open_document(&mut self.documents, &uri)?;
                let root = document_ast(document, parser, cancellation)?;
                let mut ranges = analysis::folding_ranges(root);
                if let Some(limit) = self.folding_range_limit {
                    ranges.truncate(limit);
                }
                Ok(json!(ranges))
            }
            RequestKind::Definition(position) => {
                self.definition(&uri, position, parser, cancellation, dependencies)
            }
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
                Ok(json!(links))
            }
            RequestKind::Hover(position) => {
                let document = open_document(&mut self.documents, &uri)?;
                document.validate_position(position)?;
                let root = document_ast(document, parser, cancellation)?;
                Ok(analysis::hover(root, position)
                    .map(|info| format_hover(info, self.hover_markdown))
                    .unwrap_or(Value::Null))
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
                        workspace: &self.workspace,
                        documents: &mut self.documents,
                        parser,
                        cancellation,
                        used_buffers: &mut dependencies.targets,
                        heading_id_prefix: &self.heading_id_prefix,
                    }
                    .find(&uri, position, include_declaration)
                    .map(|locations| json!(locations));
                }
                let locations: Vec<_> = analysis::references(root, position, include_declaration)
                    .into_iter()
                    .map(|range| json!({ "uri": uri, "range": range }))
                    .collect();
                Ok(json!(locations))
            }
            RequestKind::Completion(position) => {
                self.complete(&uri, position, parser, cancellation, dependencies)
            }
            RequestKind::PrepareRename(position) => {
                let document = open_document(&mut self.documents, &uri)?;
                let snapshot = document_snapshot(document, parser, position, cancellation)?;
                Ok(json!(rename::prepare(&snapshot, position)
                    .or_else(|| refactor::prepare(&snapshot, position))))
            }
            RequestKind::Rename { position, new_name } => {
                let document = open_document(&mut self.documents, &uri)?;
                let snapshot = document_snapshot(document, parser, position, cancellation)?;
                if rename::prepare(&snapshot, position).is_none() {
                    dependencies.files = true;
                    let edits = refactor::Context {
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
                if self.versioned_edits {
                    Ok(json!({ "documentChanges": [{
                        "textDocument": { "uri": uri, "version": snapshot.version }, "edits": edits
                    }] }))
                } else {
                    Ok(json!({ "changes": { uri: edits } }))
                }
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
        let mut disk_document;
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
                let Some(text) = files::read_markdown(&file.path) else {
                    return Ok(Value::Null);
                };
                cancellation.check()?;
                disk_document = Document::new(0, text)?;
                &mut disk_document
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
                        } else if let Some(text) = files::read_markdown(&file.path) {
                            cancellation.check()?;
                            let mut document = Document::new(0, text)?;
                            let root = document_ast(&mut document, parser, cancellation)?;
                            context.anchors(root, &self.heading_id_prefix)
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

fn workspace_edit(documents: Vec<refactor::DocumentEdits>, versioned: bool) -> Value {
    if versioned {
        json!({ "documentChanges": documents.into_iter().map(|document| json!({
            "textDocument": { "uri": document.uri, "version": document.version }, "edits": document.edits
        })).collect::<Vec<_>>() })
    } else {
        let changes: serde_json::Map<_, _> = documents
            .into_iter()
            .map(|document| (document.uri, json!(document.edits)))
            .collect();
        json!({ "changes": changes })
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
