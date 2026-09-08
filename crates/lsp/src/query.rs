use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde_json::{json, Value};
use yozora_ast::Root;
use yozora_parser::YozoraParser;

use crate::cancellation::Cancellation;
use crate::document::{open_document, Document, Snapshot};
use crate::files::{self, FileScope, Target, Workspace};
use crate::protocol::{
    parse_params, DocumentSymbol, Position, Range, ReferencesParams, RenameParams, ResponseError,
    TextDocumentParams, TextDocumentPositionParams,
};
use crate::{analysis, completion, diagnostics, links, rename, resource_completion};

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
pub struct Request {
    pub uri: String,
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
        Ok(Self { uri, kind })
    }

    pub fn owned_bytes(&self) -> usize {
        self.uri.capacity()
            + match &self.kind {
                RequestKind::Rename { new_name, .. } => new_name.capacity(),
                _ => 0,
            }
    }
}

impl State {
    pub fn request(
        &mut self,
        request: Request,
        parser: &YozoraParser,
        cancellation: &Cancellation,
        dependencies: &mut Dependencies,
    ) -> Result<Value, ResponseError> {
        cancellation.check()?;
        let Request { uri, kind } = request;
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
                let open_uris = self.open_file_uris(&scope, &uri, cancellation)?;
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
                                Some(file) => match open_file_uri(&open_uris, &file) {
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
                Ok(json!(rename::prepare(&snapshot, position)))
            }
            RequestKind::Rename { position, new_name } => {
                let document = open_document(&mut self.documents, &uri)?;
                let snapshot = document_snapshot(document, parser, position, cancellation)?;
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
    ) -> Result<Value, ResponseError> {
        cancellation.check()?;
        let document = open_document(&mut self.documents, uri)?;
        let root = document_ast(document, parser, cancellation)?;
        let duplicates = diagnostics::duplicate_definitions(root);
        Ok(format_diagnostics(
            uri,
            self.diagnostic_version.then_some(document.version()),
            duplicates,
            self.diagnostic_related_information,
        ))
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
                open_file_uri(&self.open_file_uris(&scope, uri, cancellation)?, &file)
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
        let open_uris = self.open_file_uris(&scope, uri, cancellation)?;
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
                        if let Some(uri) = open_file_uri(&open_uris, &file) {
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
        source_uri: &str,
        cancellation: &Cancellation,
    ) -> Result<HashMap<PathBuf, Vec<String>>, ResponseError> {
        cancellation.check()?;
        let mut uris: Vec<_> = self.documents.keys().collect();
        // Preserve all aliases. Prefer the source only when no exact target
        // URI or lexical path selects a more specific buffer.
        uris.sort_by_key(|uri| (uri.as_str() != source_uri, *uri));
        let mut result: HashMap<PathBuf, Vec<String>> = HashMap::new();
        for uri in uris {
            cancellation.check()?;
            if let Some(path) = files::file_uri_path(uri).and_then(|path| scope.resolve(&path)) {
                result.entry(path).or_default().push(uri.clone());
            }
        }
        Ok(result)
    }
}

fn open_file_uri<'a>(
    open_uris: &'a HashMap<PathBuf, Vec<String>>,
    file: &files::LocalFile,
) -> Option<&'a str> {
    let aliases = open_uris.get(&file.path)?;
    // Buffer identity is the requested URI, then its normalized lexical path.
    // Canonical aliases are a fallback, never an override of a matching buffer.
    aliases
        .iter()
        .find(|uri| *uri == &file.uri)
        .or_else(|| {
            let lexical = files::file_uri_path(&file.uri)?;
            aliases
                .iter()
                .find(|uri| files::file_uri_path(uri).as_ref() == Some(&lexical))
        })
        .or_else(|| aliases.first())
        .map(String::as_str)
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
