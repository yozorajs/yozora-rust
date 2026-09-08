use std::collections::{HashMap, VecDeque};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::mpsc::RecvTimeoutError;
use std::time::{Duration, Instant};

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use yozora_parser::YozoraParser;

use crate::analysis;
use crate::completion;
use crate::diagnostics;
use crate::document::Document;
use crate::files::{self, FileScope, Target, Workspace};
use crate::links;
use crate::protocol::{
    DidChangeParams, DidChangeWorkspaceFoldersParams, DidOpenParams, DocumentSymbol,
    InitializeParams, Position, Range, ReferencesParams, RenameParams, ResponseError,
    TextDocumentParams, TextDocumentPositionParams,
};
use crate::rename;
use crate::resource_completion;
use crate::transport::{read_messages, write_message};

const DIAGNOSTICS_DELAY: Duration = Duration::from_millis(150);
const QUERY_DELAY: Duration = Duration::from_millis(5);

#[derive(Default, Eq, PartialEq)]
enum State {
    #[default]
    Uninitialized,
    Running,
    Shutdown,
}

struct PendingQuery {
    id: Value,
    method: String,
    params: Value,
    uri: String,
    deadline: Instant,
}

#[derive(Default)]
struct Server {
    state: State,
    parser: YozoraParser,
    documents: HashMap<String, Document>,
    workspace: Workspace,
    heading_id_prefix: String,
    hierarchical_symbols: bool,
    folding_range_limit: Option<usize>,
    hover_markdown: bool,
    versioned_edits: bool,
    diagnostic_related_information: bool,
    diagnostic_version: bool,
    diagnostics_due: HashMap<String, Instant>,
    pending_queries: VecDeque<PendingQuery>,
    // Notifications can invalidate queries and retract diagnostics together.
    // Drain these messages even when the notification returns an error.
    outgoing: Vec<Value>,
}

pub fn run(reader: impl BufRead + Send + 'static, mut writer: impl Write) -> io::Result<ExitCode> {
    let incoming = read_messages(reader)?;
    let mut server = Server::default();
    loop {
        let message = match server.next_deadline() {
            Some(deadline) => {
                incoming.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            }
            None => incoming.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        match message {
            Ok(body) => {
                if let Some(status) = server.handle_message(&body?, &mut writer)? {
                    return Ok(status);
                }
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        // Check timers after every message as well as on idle timeouts, so
        // traffic for one document cannot starve diagnostics for another.
        // At most one analysis runs before returning to incoming messages.
        match server.take_due_work(Instant::now()) {
            Ok(Some(message)) => write_message(&mut writer, &message)?,
            Ok(None) => {}
            Err(error) => eprintln!("yozora-lsp: diagnostics: {}", error.message),
        }
    }
    Ok(server.exit_status())
}

impl Server {
    fn handle_message(
        &mut self,
        body: &[u8],
        writer: &mut impl Write,
    ) -> io::Result<Option<ExitCode>> {
        let message: Value = match serde_json::from_slice(body) {
            Ok(message) => message,
            Err(_) => {
                write_message(
                    writer,
                    &ResponseError::new(-32700, "Parse error").response(Value::Null),
                )?;
                return Ok(None);
            }
        };
        let id = message.get("id").cloned();
        let valid_id = id
            .as_ref()
            .is_none_or(|id| id.is_string() || id.as_i64().is_some());
        let method = message.get("method").and_then(Value::as_str);
        if message.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
            || method.is_none()
            || !valid_id
        {
            write_message(
                writer,
                &ResponseError::new(-32600, "Invalid Request")
                    .response(id.filter(|_| valid_id).unwrap_or(Value::Null)),
            )?;
            return Ok(None);
        }

        let method = method.expect("method was validated");
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        let response = if let Some(id) = id {
            if self.state == State::Running && is_document_query(method) {
                self.queue_query(id.clone(), method, params, Instant::now())
                    .err()
                    .map(|error| error.response(id))
            } else {
                Some(match self.request(method, params) {
                    Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                    Err(error) => error.response(id),
                })
            }
        } else if method == "exit" {
            return Ok(Some(self.exit_status()));
        } else {
            if self.state == State::Running {
                if let Err(error) = self.notification(method, params, Instant::now()) {
                    eprintln!("yozora-lsp: {method}: {}", error.message);
                }
            }
            None
        };
        for message in self.outgoing.drain(..) {
            write_message(writer, &message)?;
        }
        if let Some(response) = response {
            write_message(writer, &response)?;
        }
        Ok(None)
    }

    fn exit_status(&self) -> ExitCode {
        if self.state == State::Shutdown {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
    }

    fn queue_query(
        &mut self,
        id: Value,
        method: &str,
        params: Value,
        now: Instant,
    ) -> Result<(), ResponseError> {
        let document: TextDocumentParams = parse_params(params.clone())?;
        let uri = document.text_document.uri;
        open_document(&mut self.documents, &uri)?;
        self.pending_queries.push_back(PendingQuery {
            id,
            method: method.to_string(),
            params,
            uri,
            deadline: now + QUERY_DELAY,
        });
        Ok(())
    }

    fn reject_queries(&mut self, reject: impl Fn(&PendingQuery) -> bool, error: ResponseError) {
        let outgoing = &mut self.outgoing;
        self.pending_queries.retain(|query| {
            if reject(query) {
                outgoing.push(error.response(query.id.clone()));
                false
            } else {
                true
            }
        });
    }

    fn invalidate_queries(&mut self, uri: &str) {
        self.reject_queries(
            |query| query.uri == uri,
            ResponseError::new(-32801, "document changed before the query started"),
        );
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.pending_queries
            .front()
            .map(|query| query.deadline)
            .into_iter()
            .chain(self.diagnostics_due.values().copied())
            .min()
    }

    fn take_due_work(&mut self, now: Instant) -> Result<Option<Value>, ResponseError> {
        // Serve the oldest deadline across both kinds of work, so a stream of
        // queries cannot starve diagnostics for another document.
        let query_deadline = self.pending_queries.front().map(|query| query.deadline);
        let diagnostic_deadline = self.diagnostics_due.values().min().copied();
        if query_deadline.is_some_and(|deadline| {
            deadline <= now && diagnostic_deadline.is_none_or(|other| deadline <= other)
        }) {
            let query = self.pending_queries.pop_front().expect("query was checked");
            return Ok(Some(match self.request(&query.method, query.params) {
                Ok(result) => json!({ "jsonrpc": "2.0", "id": query.id, "result": result }),
                Err(error) => error.response(query.id),
            }));
        }
        self.take_due_diagnostics(now)
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, ResponseError> {
        if method == "initialize" {
            if self.state != State::Uninitialized {
                return Err(ResponseError::new(-32600, "server is already initialized"));
            }
            if !params.get("capabilities").is_some_and(Value::is_object) {
                return Err(ResponseError::invalid_params(
                    "initialize requires client capabilities",
                ));
            }
            let initialization: InitializeParams = parse_params(params.clone())?;
            let folders: Vec<_> = initialization.workspace_folders.map_or_else(
                || initialization.root_uri.into_iter().collect(),
                |folders| folders.into_iter().map(|folder| folder.uri).collect(),
            );
            for uri in &folders {
                validate_uri(uri)?;
            }
            let prefix = initialization
                .initialization_options
                .unwrap_or_default()
                .heading_id_prefix;
            if prefix.len() > 256 || prefix.chars().any(char::is_control) {
                return Err(ResponseError::invalid_params(
                    "headingIdPrefix must be at most 256 UTF-8 bytes without control characters",
                ));
            }
            self.workspace = Workspace::new(folders);
            self.heading_id_prefix = prefix;
            self.hierarchical_symbols = params
                .pointer(
                    "/capabilities/textDocument/documentSymbol/hierarchicalDocumentSymbolSupport",
                )
                .and_then(Value::as_bool)
                .unwrap_or(false);
            self.folding_range_limit = params
                .pointer("/capabilities/textDocument/foldingRange/rangeLimit")
                .and_then(Value::as_u64)
                .and_then(|limit| usize::try_from(limit).ok());
            self.hover_markdown = params
                .pointer("/capabilities/textDocument/hover/contentFormat")
                .and_then(Value::as_array)
                .and_then(|formats| {
                    formats
                        .iter()
                        .filter_map(Value::as_str)
                        .find(|format| matches!(*format, "markdown" | "plaintext"))
                })
                == Some("markdown");
            self.versioned_edits = params
                .pointer("/capabilities/workspace/workspaceEdit/documentChanges")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            self.diagnostic_related_information = params
                .pointer("/capabilities/textDocument/publishDiagnostics/relatedInformation")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            self.diagnostic_version = params
                .pointer("/capabilities/textDocument/publishDiagnostics/versionSupport")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            self.state = State::Running;
            return Ok(json!({
                "capabilities": {
                    "positionEncoding": "utf-16",
                    "textDocumentSync": { "openClose": true, "change": 2 },
                    "documentSymbolProvider": true,
                    "foldingRangeProvider": true,
                    "definitionProvider": true,
                    "documentLinkProvider": { "resolveProvider": false },
                    "workspace": { "workspaceFolders": { "supported": true, "changeNotifications": true } },
                    "hoverProvider": true,
                    "referencesProvider": true,
                    "completionProvider": { "triggerCharacters": ["[", "^", "(", "/", "#"], "resolveProvider": false },
                    "renameProvider": { "prepareProvider": true },
                },
                "serverInfo": { "name": "yozora-lsp", "version": env!("CARGO_PKG_VERSION") },
            }));
        }
        match self.state {
            State::Uninitialized => {
                return Err(ResponseError::new(-32002, "Server not initialized"));
            }
            State::Shutdown => {
                return Err(ResponseError::new(-32600, "server has shut down"));
            }
            State::Running => {}
        }

        match method {
            "shutdown" => {
                if !params.is_null() {
                    return Err(ResponseError::invalid_params(
                        "shutdown takes no parameters",
                    ));
                }
                self.state = State::Shutdown;
                self.documents.clear();
                self.diagnostics_due.clear();
                self.outgoing.clear();
                self.reject_queries(
                    |_| true,
                    ResponseError::new(-32800, "server is shutting down"),
                );
                Ok(Value::Null)
            }
            "textDocument/documentSymbol" => {
                let params: TextDocumentParams = parse_params(params)?;
                let uri = params.text_document.uri;
                let document = open_document(&mut self.documents, &uri)?;
                let symbols = analysis::document_symbols(document.ast(&self.parser)?);
                if self.hierarchical_symbols {
                    Ok(json!(symbols))
                } else {
                    Ok(flat_symbols(&symbols, &uri))
                }
            }
            "textDocument/foldingRange" => {
                let params: TextDocumentParams = parse_params(params)?;
                let document = open_document(&mut self.documents, &params.text_document.uri)?;
                let mut ranges = analysis::folding_ranges(document.ast(&self.parser)?);
                if let Some(limit) = self.folding_range_limit {
                    ranges.truncate(limit);
                }
                Ok(json!(ranges))
            }
            "textDocument/definition" => {
                let params: TextDocumentPositionParams = parse_params(params)?;
                self.definition(&params.text_document.uri, params.position)
            }
            "textDocument/documentLink" => {
                let params: TextDocumentParams = parse_params(params)?;
                let uri = params.text_document.uri;
                let scope = self.workspace.scope(&uri);
                let open_uris = self.open_file_uris(&scope, &uri);
                let document = open_document(&mut self.documents, &uri)?;
                Ok(json!(links::document_links(
                    document.ast(&self.parser)?,
                    |destination| {
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
                    }
                )))
            }
            "textDocument/hover" => {
                let params: TextDocumentPositionParams = parse_params(params)?;
                let document = open_document(&mut self.documents, &params.text_document.uri)?;
                document.validate_position(params.position)?;
                Ok(
                    analysis::hover(document.ast(&self.parser)?, params.position)
                        .map(|info| format_hover(info, self.hover_markdown))
                        .unwrap_or(Value::Null),
                )
            }
            "textDocument/references" => {
                let ReferencesParams {
                    position_params: params,
                    context,
                } = parse_params(params)?;
                let uri = params.text_document.uri;
                let document = open_document(&mut self.documents, &uri)?;
                document.validate_position(params.position)?;
                let locations: Vec<_> = analysis::references(
                    document.ast(&self.parser)?,
                    params.position,
                    context.include_declaration,
                )
                .into_iter()
                .map(|range| json!({ "uri": uri, "range": range }))
                .collect();
                Ok(json!(locations))
            }
            "textDocument/completion" => {
                let params: TextDocumentPositionParams = parse_params(params)?;
                self.complete(&params.text_document.uri, params.position)
            }
            "textDocument/prepareRename" => {
                let params: TextDocumentPositionParams = parse_params(params)?;
                let document = open_document(&mut self.documents, &params.text_document.uri)?;
                let snapshot = document.snapshot(&self.parser, params.position)?;
                Ok(json!(rename::prepare(&snapshot, params.position)))
            }
            "textDocument/rename" => {
                let RenameParams {
                    position_params: params,
                    new_name,
                } = parse_params(params)?;
                let uri = params.text_document.uri;
                let document = open_document(&mut self.documents, &uri)?;
                let snapshot = document.snapshot(&self.parser, params.position)?;
                let edits = rename::rename(&snapshot, params.position, &new_name, &self.parser)?;
                if self.versioned_edits {
                    Ok(json!({ "documentChanges": [{
                        "textDocument": { "uri": uri, "version": snapshot.version }, "edits": edits
                    }] }))
                } else {
                    Ok(json!({ "changes": { uri: edits } }))
                }
            }
            _ => Err(ResponseError::new(-32601, "Method not found")),
        }
    }

    fn definition(&mut self, uri: &str, position: Position) -> Result<Value, ResponseError> {
        let document = open_document(&mut self.documents, uri)?;
        document.validate_position(position)?;
        let root = document.ast(&self.parser)?;
        if let Some(range) = analysis::definition(root, position) {
            return Ok(json!({ "uri": uri, "range": range }));
        }
        let Some(destination) = links::destination_at(root, position).map(str::to_string) else {
            return Ok(Value::Null);
        };
        let scope = self.workspace.scope(uri);
        let Some(Target::Local { file, fragment, .. }) = files::resolve(uri, &destination, &scope)
        else {
            return Ok(Value::Null);
        };
        let fragment = fragment.as_deref().filter(|value| !value.is_empty());
        let target_uri;
        let mut disk_document;
        let document = if let Some(file) = file {
            if let Some(open_uri) = open_file_uri(&self.open_file_uris(&scope, uri), &file) {
                target_uri = open_uri.to_string();
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
            Some(fragment) => links::heading(
                document.ast(&self.parser)?,
                fragment,
                &self.heading_id_prefix,
            ),
            None => Some(document_start()),
        };
        Ok(range
            .map(|range| json!({ "uri": target_uri, "range": range }))
            .unwrap_or(Value::Null))
    }

    fn complete(&mut self, uri: &str, position: Position) -> Result<Value, ResponseError> {
        let document = open_document(&mut self.documents, uri)?;
        let snapshot = document.snapshot(&self.parser, position)?;
        let Some(context) = resource_completion::Context::new(&snapshot, position, &self.parser)?
        else {
            return Ok(json!(completion::complete(
                &snapshot,
                position,
                &self.parser
            )?));
        };
        let scope = self.workspace.scope(uri);
        let open_uris = self.open_file_uris(&scope, uri);
        let candidates = match context.target() {
            Some(resource_completion::Target::Path(prefix)) => context.paths(
                files::path_candidates(uri, prefix, &scope, open_uris.keys().map(PathBuf::as_path)),
            ),
            Some(resource_completion::Target::Anchor { resource, .. }) => {
                match files::resolve(uri, resource, &scope) {
                    Some(Target::Local { file: None, .. }) => {
                        let document = open_document(&mut self.documents, uri)?;
                        context.anchors(document.ast(&self.parser)?, &self.heading_id_prefix)
                    }
                    Some(Target::Local {
                        file: Some(file), ..
                    }) => {
                        if let Some(uri) = open_file_uri(&open_uris, &file) {
                            let document = open_document(&mut self.documents, uri)?;
                            context.anchors(document.ast(&self.parser)?, &self.heading_id_prefix)
                        } else if let Some(text) = files::read_markdown(&file.path) {
                            let mut document = Document::new(0, text)?;
                            context.anchors(document.ast(&self.parser)?, &self.heading_id_prefix)
                        } else {
                            Vec::new()
                        }
                    }
                    _ => Vec::new(),
                }
            }
            None => Vec::new(),
        };
        Ok(json!(context.complete(candidates, &self.parser)))
    }

    fn open_file_uris(&self, scope: &FileScope, source_uri: &str) -> HashMap<PathBuf, Vec<String>> {
        let mut uris: Vec<_> = self.documents.keys().collect();
        // Preserve all aliases. Prefer the source only when no exact target
        // URI or lexical path selects a more specific buffer.
        uris.sort_by_key(|uri| (uri.as_str() != source_uri, *uri));
        let mut result: HashMap<PathBuf, Vec<String>> = HashMap::new();
        for uri in uris {
            if let Some(path) = files::file_uri_path(uri).and_then(|path| scope.resolve(&path)) {
                result.entry(path).or_default().push(uri.clone());
            }
        }
        result
    }

    fn notification(
        &mut self,
        method: &str,
        params: Value,
        now: Instant,
    ) -> Result<(), ResponseError> {
        match method {
            "workspace/didChangeWorkspaceFolders" => {
                let params: DidChangeWorkspaceFoldersParams = parse_params(params)?;
                let added: Vec<_> = params
                    .event
                    .added
                    .into_iter()
                    .map(|folder| folder.uri)
                    .collect();
                let removed: Vec<_> = params
                    .event
                    .removed
                    .into_iter()
                    .map(|folder| folder.uri)
                    .collect();
                for uri in added.iter().chain(&removed) {
                    validate_uri(uri)?;
                }
                self.workspace.change(&removed, added);
            }
            "textDocument/didOpen" => {
                let params: DidOpenParams = parse_params(params)?;
                let item = params.text_document;
                validate_uri(&item.uri)?;
                let document = Document::new(item.version, item.text)?;
                self.invalidate_queries(&item.uri);
                self.diagnostics_due
                    .insert(item.uri.clone(), now + DIAGNOSTICS_DELAY);
                self.documents.insert(item.uri, document);
            }
            "textDocument/didChange" => {
                let params: DidChangeParams = parse_params(params)?;
                let uri = params.text_document.uri;
                let document = open_document(&mut self.documents, &uri)?;
                let previous_version = document.version();
                let result = match parse_params(params.content_changes) {
                    Ok(changes) => document.change(params.text_document.version, changes),
                    Err(error) => document
                        .invalidate(params.text_document.version)
                        .and(Err(error)),
                };
                let version = document.version();
                if version != previous_version {
                    self.invalidate_queries(&uri);
                    if result.is_ok() {
                        self.diagnostics_due.insert(uri, now + DIAGNOSTICS_DELAY);
                    } else {
                        // Text is now out of sync; retract the previous snapshot.
                        self.diagnostics_due.remove(&uri);
                        self.outgoing.push(format_diagnostics(
                            &uri,
                            self.diagnostic_version.then_some(version),
                            Vec::new(),
                            false,
                        ));
                    }
                }
                result?;
            }
            "textDocument/didClose" => {
                let params: TextDocumentParams = parse_params(params)?;
                let uri = params.text_document.uri;
                if self.documents.remove(&uri).is_some() {
                    self.invalidate_queries(&uri);
                    self.diagnostics_due.remove(&uri);
                    self.outgoing
                        .push(format_diagnostics(&uri, None, Vec::new(), false));
                }
            }
            "$/cancelRequest" => {
                let id = params
                    .get("id")
                    .ok_or_else(|| ResponseError::invalid_params("cancelRequest requires an id"))?;
                self.reject_queries(
                    |query| query.id == *id,
                    ResponseError::new(-32800, "request cancelled"),
                );
            }
            // Unknown notifications and optional $/ messages have no response.
            _ => {}
        }
        Ok(())
    }

    fn take_due_diagnostics(&mut self, now: Instant) -> Result<Option<Value>, ResponseError> {
        let Some((uri, deadline)) = self
            .diagnostics_due
            .iter()
            .min_by_key(|(uri, deadline)| (**deadline, *uri))
        else {
            return Ok(None);
        };
        if *deadline > now {
            return Ok(None);
        }
        let uri = uri.clone();
        self.diagnostics_due.remove(&uri);
        let document = open_document(&mut self.documents, &uri)?;
        let diagnostics = diagnostics::duplicate_definitions(document.ast(&self.parser)?);
        Ok(Some(format_diagnostics(
            &uri,
            self.diagnostic_version.then_some(document.version()),
            diagnostics,
            self.diagnostic_related_information,
        )))
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

fn document_start() -> Range {
    Range {
        start: Position::default(),
        end: Position::default(),
    }
}

fn format_diagnostics(
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

fn is_document_query(method: &str) -> bool {
    matches!(
        method,
        "textDocument/documentSymbol"
            | "textDocument/foldingRange"
            | "textDocument/definition"
            | "textDocument/documentLink"
            | "textDocument/hover"
            | "textDocument/references"
            | "textDocument/completion"
            | "textDocument/prepareRename"
            | "textDocument/rename"
    )
}

fn parse_params<T: DeserializeOwned>(params: Value) -> Result<T, ResponseError> {
    serde_json::from_value(params).map_err(|error| ResponseError::invalid_params(error.to_string()))
}

fn open_document<'a>(
    documents: &'a mut HashMap<String, Document>,
    uri: &str,
) -> Result<&'a mut Document, ResponseError> {
    documents
        .get_mut(uri)
        .ok_or_else(|| ResponseError::invalid_params("document is not open"))
}

fn validate_uri(uri: &str) -> Result<(), ResponseError> {
    if uri.split_once(':').is_some_and(|(scheme, _)| {
        scheme
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic)
            && scheme
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'+' | b'-' | b'.'))
    }) {
        Ok(())
    } else {
        Err(ResponseError::invalid_params(
            "document URI requires a scheme",
        ))
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    const DUPLICATES: &str = "# 😀\n\n[ref]: /first\n[REF]: /second\n";

    fn server() -> Server {
        let mut server = Server::default();
        server
            .request(
                "initialize",
                json!({ "capabilities": { "textDocument": {
                "publishDiagnostics": { "versionSupport": true },
                "documentSymbol": { "hierarchicalDocumentSymbolSupport": true }
            } } }),
            )
            .unwrap();
        server
    }

    fn open(server: &mut Server, uri: &str, text: &str, now: Instant) {
        server
            .notification(
                "textDocument/didOpen",
                json!({ "textDocument": {
                "uri": uri, "version": 1, "text": text
            } }),
                now,
            )
            .unwrap();
    }

    fn replace(
        server: &mut Server,
        uri: &str,
        version: i32,
        text: &str,
        now: Instant,
    ) -> Result<(), ResponseError> {
        server.notification(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": text }]
            }),
            now,
        )
    }

    fn published(server: &mut Server, now: Instant) -> Value {
        server.take_due_diagnostics(now).unwrap().unwrap()["params"].clone()
    }

    fn queue(server: &mut Server, id: Value, uri: &str, now: Instant) {
        server
            .queue_query(
                id,
                "textDocument/documentSymbol",
                json!({
                    "textDocument": { "uri": uri }
                }),
                now,
            )
            .unwrap();
    }

    fn navigate(server: &mut Server, uri: &str, line: u32) -> Result<Value, ResponseError> {
        server.request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri }, "position": { "line": line, "character": 2 }
            }),
        )
    }

    fn complete_marked(
        server: &mut Server,
        uri: &str,
        marked: &str,
    ) -> Result<Value, ResponseError> {
        let cursor = marked.find('¦').unwrap();
        open(server, uri, &marked.replacen('¦', "", 1), Instant::now());
        let snapshot = server
            .documents
            .get_mut(uri)
            .unwrap()
            .snapshot(&server.parser, Position::default())?;
        let position = snapshot.lines.position(snapshot.text, cursor)?;
        server.complete(uri, position)
    }

    fn completion_labels(result: &Value) -> Vec<&str> {
        result["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["label"].as_str().unwrap())
            .collect()
    }

    fn assert_open_alias_selection(
        directory: &crate::files::tests::TestDir,
        alias_uri: &str,
        alias_destination: &str,
    ) {
        let uri = directory.uri("source.md");
        let target_uri = directory.uri("guide.md");
        let mut server = server();
        let now = Instant::now();
        open(&mut server, &target_uri, "# Current", now);
        open(&mut server, alias_uri, "\n# Alias", now);
        for (destination, expected_uri, heading, line) in [
            ("guide.md", target_uri.as_str(), "current", 0),
            (alias_destination, alias_uri, "alias", 1),
        ] {
            let result = complete_marked(
                &mut server,
                &uri,
                &format!("[go]({destination}#{heading}¦)"),
            )
            .unwrap();
            assert_eq!(completion_labels(&result), [heading], "{destination}");
            let definition = navigate(&mut server, &uri, 0).unwrap();
            assert_eq!(definition["uri"], expected_uri, "{destination}");
            assert_eq!(definition["range"]["start"]["line"], line);
            let links = server
                .request(
                    "textDocument/documentLink",
                    json!({ "textDocument": { "uri": uri } }),
                )
                .unwrap();
            assert_eq!(links[0]["target"], format!("{expected_uri}#{heading}"));
        }

        open(
            &mut server,
            &target_uri,
            &format!("# Current\n\n[go]({alias_destination}#alias)"),
            now,
        );
        assert_eq!(
            navigate(&mut server, &target_uri, 2).unwrap()["uri"],
            alias_uri
        );
        open(
            &mut server,
            alias_uri,
            "\n# Alias\n\n[go](guide.md#current)",
            now,
        );
        assert_eq!(
            navigate(&mut server, alias_uri, 3).unwrap()["uri"],
            target_uri
        );

        server
            .notification(
                "textDocument/didChange",
                json!({ "textDocument": { "uri": alias_uri, "version": 2 } }),
                now,
            )
            .unwrap_err();
        let result = complete_marked(&mut server, &uri, "[go](guide.md#cur¦rent)").unwrap();
        assert_eq!(completion_labels(&result), ["current"]);
        assert_eq!(navigate(&mut server, &uri, 0).unwrap()["uri"], target_uri);
        assert_eq!(
            complete_marked(
                &mut server,
                &uri,
                &format!("[go]({alias_destination}#al¦ias)")
            )
            .unwrap_err()
            .code,
            -32801
        );
        assert_eq!(navigate(&mut server, &uri, 0).unwrap_err().code, -32801);

        server
            .notification(
                "textDocument/didClose",
                json!({ "textDocument": { "uri": alias_uri } }),
                now,
            )
            .unwrap();
        let result = complete_marked(
            &mut server,
            &uri,
            &format!("[go]({alias_destination}#cur¦rent)"),
        )
        .unwrap();
        assert_eq!(completion_labels(&result), ["current"]);
        assert_eq!(navigate(&mut server, &uri, 0).unwrap()["uri"], target_uri);
    }

    #[test]
    fn navigation_and_completion_prefer_the_requested_open_uri_spelling() {
        let directory = crate::files::tests::TestDir::new();
        std::fs::write(directory.0.join("guide.md"), "# Disk").unwrap();
        let alias = directory
            .uri("guide.md")
            .replacen("file://", "FILE://localhost", 1)
            .replace("/guide.md", "/./%67uide.md");
        assert_open_alias_selection(&directory, &alias, &alias);
    }

    #[cfg(unix)]
    #[test]
    fn navigation_and_completion_prefer_the_requested_open_symlink_path() {
        let directory = crate::files::tests::TestDir::new();
        std::fs::write(directory.0.join("guide.md"), "# Disk").unwrap();
        std::os::unix::fs::symlink(directory.0.join("guide.md"), directory.0.join("alias.md"))
            .unwrap();
        let alias = directory
            .uri("alias.md")
            .replacen("file://", "file://localhost", 1);
        assert_open_alias_selection(&directory, &alias, "alias.md");
    }

    #[test]
    fn completion_does_not_reinterpret_uri_schemes_or_authorities_as_paths() {
        let directory = crate::files::tests::TestDir::new();
        for name in ["http", "file", "mailto", "command", "localhost"] {
            std::fs::create_dir(directory.0.join(name)).unwrap();
        }
        let uri = directory.uri("source.md");
        let mut server = server();
        for marked in [
            "[go](htt¦ps://example.test/guide.md)",
            "[go](¦https://example.test/guide.md)",
            "[go](htt¦ps&#58;//example.test/guide.md)",
            "![alt](ma¦ilto:reader@example.test)",
            "[ref]: co¦mmand:run",
            "[go](fi¦le:///guide.md)",
            "[go](file://local¦host/guide.md)",
            "[go](//local¦host/guide.md)",
        ] {
            let result = complete_marked(&mut server, &uri, marked).unwrap();
            assert!(completion_labels(&result).is_empty(), "{marked}: {result}");
        }
    }

    #[test]
    fn completion_inside_encoded_path_separators_does_not_replace_other_components() {
        let directory = crate::files::tests::TestDir::new();
        std::fs::create_dir(directory.0.join("part")).unwrap();
        std::fs::write(directory.0.join("part/file.md"), "# Intro").unwrap();
        std::fs::write(directory.0.join("part other.md"), "# Other").unwrap();
        let uri = directory.uri("source.md");
        let mut server = server();
        for marked in [
            "[go](part%¦2Ffile.md)",
            "[go](part%2¦Ffile.md)",
            "[go](part%2¦ffile.md)",
            "[go](part&#37;2¦Ffile.md)",
        ] {
            let result = complete_marked(&mut server, &uri, marked).unwrap();
            assert_eq!(
                navigate(&mut server, &uri, 0).unwrap()["uri"],
                directory.uri("part/file.md"),
                "{marked}"
            );
            assert!(completion_labels(&result).is_empty(), "{marked}: {result}");
        }
    }

    #[test]
    fn definition_selects_inner_direct_images_before_outer_reference_declarations() {
        let directory = crate::files::tests::TestDir::new();
        std::fs::write(directory.0.join("icon.svg"), "<svg/>").unwrap();
        let uri = directory.uri("source.md");
        let mut server = server();
        let result = complete_marked(
            &mut server,
            &uri,
            "[![alt](ic¦on.svg)][ref]\n\n[ref]: outer.md",
        )
        .unwrap();
        assert_eq!(completion_labels(&result), ["icon.svg"]);
        let inner = json!({
            "textDocument": { "uri": uri }, "position": { "line": 0, "character": 10 }
        });
        let hover = server.request("textDocument/hover", inner.clone()).unwrap();
        assert_eq!(hover["contents"]["value"], "icon.svg");
        assert_eq!(
            server
                .request("textDocument/definition", inner.clone())
                .unwrap(),
            json!({ "uri": directory.uri("icon.svg"), "range": document_start() })
        );
        let outer = server
            .request(
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": uri }, "position": { "line": 0, "character": 19 }
                }),
            )
            .unwrap();
        assert_eq!(outer["uri"], uri);
        assert_eq!(outer["range"]["start"]["line"], 2);
        std::fs::remove_file(directory.0.join("icon.svg")).unwrap();
        assert_eq!(
            server.request("textDocument/definition", inner).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn file_completion_edits_resolve_to_existing_or_unsaved_targets() {
        use crate::files::tests::TestDir;
        let directory = TestDir::new();
        std::fs::create_dir(directory.0.join("docs")).unwrap();
        std::fs::write(directory.0.join("docs/guide 中文.md"), "# Intro").unwrap();
        let uri = directory.uri("source.md");
        let mut server = server();
        open(
            &mut server,
            &directory.uri("docs/guide-new.md"),
            "# New",
            Instant::now(),
        );
        let result =
            complete_marked(&mut server, &uri, "😀 [go](docs/gu¦.md#intro \"Title\")").unwrap();
        assert_eq!(
            completion_labels(&result),
            ["guide 中文.md", "guide-new.md"]
        );
        let item = &result["items"][0];
        assert_eq!(item["kind"], 17);
        assert_eq!(item["textEdit"]["range"]["start"]["character"], 8);
        assert_eq!(
            item["textEdit"]["newText"],
            "docs/guide%20%E4%B8%AD%E6%96%87.md#intro"
        );
        server.notification("textDocument/didChange", json!({
            "textDocument": { "uri": uri, "version": 2 },
            "contentChanges": [{ "range": item["textEdit"]["range"], "text": item["textEdit"]["newText"] }]
        }), Instant::now()).unwrap();
        let definition = server
            .definition(
                &uri,
                Position {
                    line: 0,
                    character: 5,
                },
            )
            .unwrap();
        assert_eq!(definition["uri"], directory.uri("docs/guide 中文.md"));
        assert_eq!(definition["range"]["start"]["line"], 0);
        let folders = complete_marked(&mut server, &uri, "[go](do¦").unwrap();
        assert_eq!(completion_labels(&folders), ["docs/"]);
        assert_eq!(folders["items"][0]["textEdit"]["newText"], "docs/");
        assert_eq!(folders["items"][0]["kind"], 19);
        let alias = directory
            .uri("docs/")
            .replacen("file://", "file://localhost", 1);
        let result = complete_marked(&mut server, &uri, &format!("[go]({alias}gu¦)")).unwrap();
        assert_eq!(
            completion_labels(&result),
            ["guide 中文.md", "guide-new.md"]
        );
    }

    #[test]
    fn cross_file_anchor_completion_uses_latest_target_state_and_never_falls_back_when_out_of_sync()
    {
        use crate::files::tests::TestDir;
        let directory = TestDir::new();
        let uri = directory.uri("source.md");
        let target_uri = directory.uri("guide.md");
        let alias = target_uri.replace("/guide.md", "/./%67uide.md");
        std::fs::write(directory.0.join("guide.md"), "# Disk\n# Disk").unwrap();
        let mut server = server();
        let marked = "[go](guide.md#¦)";
        assert_eq!(
            completion_labels(&complete_marked(&mut server, &uri, marked).unwrap()),
            ["disk", "disk-2"]
        );
        open(
            &mut server,
            &alias,
            "# Unsaved\n# Unsaved\n> # Nested\n",
            Instant::now(),
        );
        assert_eq!(
            completion_labels(&complete_marked(&mut server, &uri, marked).unwrap()),
            ["unsaved", "unsaved-2"]
        );
        replace(&mut server, &alias, 2, "# Current", Instant::now()).unwrap();
        let result = complete_marked(&mut server, &uri, marked).unwrap();
        assert_eq!(completion_labels(&result), ["current"]);
        assert_eq!(
            result["items"][0]["textEdit"]["newText"],
            "guide.md#current"
        );
        server
            .notification(
                "textDocument/didChange",
                json!({ "textDocument": { "uri": alias, "version": 3 } }),
                Instant::now(),
            )
            .unwrap_err();
        assert_eq!(
            complete_marked(&mut server, &uri, marked).unwrap_err().code,
            -32801
        );
        replace(&mut server, &alias, 4, "# Restored", Instant::now()).unwrap();
        assert_eq!(
            completion_labels(&complete_marked(&mut server, &uri, marked).unwrap()),
            ["restored"]
        );
        server
            .notification(
                "textDocument/didClose",
                json!({ "textDocument": { "uri": alias } }),
                Instant::now(),
            )
            .unwrap();
        std::fs::write(directory.0.join("guide.md"), "# Changed on disk").unwrap();
        assert_eq!(
            completion_labels(&complete_marked(&mut server, &uri, marked).unwrap()),
            ["changed-on-disk"]
        );
    }

    #[test]
    fn anchor_completion_shares_heading_prefixes_and_does_not_replace_label_completion() {
        let mut server = Server::default();
        server
            .request(
                "initialize",
                json!({ "capabilities": {}, "initializationOptions": { "headingIdPrefix": "h-" } }),
            )
            .unwrap();
        let uri = "untitled:completion";
        let result =
            complete_marked(&mut server, uri, "# 中文😀\n# 中文😀\n\n[go](#h-%e4¦)").unwrap();
        assert_eq!(completion_labels(&result), ["h-中文😀", "h-中文😀-2"]);
        assert_eq!(
            result["items"][1]["textEdit"]["newText"],
            "#h-%E4%B8%AD%E6%96%87%F0%9F%98%80-2"
        );
        let result = complete_marked(&mut server, uri, "[go][Gu¦]\n\n[Guide]: /target").unwrap();
        assert_eq!(completion_labels(&result), ["Guide"]);
        assert_eq!(result["items"][0]["textEdit"]["newText"], "Guide");
        let result = complete_marked(&mut server, uri, "[Guide]: #h-%E4¦\n\n# 中文😀").unwrap();
        assert_eq!(completion_labels(&result), ["h-中文😀"]);
        for marked in [
            "[go](gu¦)",
            "# Intro\n\n[go](https://example.test/#¦)",
            "# Intro\n\n[go](command:run#¦)",
            "[go](file.md?query¦)",
        ] {
            assert!(
                completion_labels(&complete_marked(&mut server, uri, marked).unwrap()).is_empty(),
                "{marked}"
            );
        }
    }

    #[test]
    fn completion_respects_workspace_updates_missing_targets_and_read_limits() {
        use crate::files::tests::TestDir;
        let directory = TestDir::new();
        std::fs::create_dir(directory.0.join("root")).unwrap();
        std::fs::create_dir(directory.0.join("other")).unwrap();
        std::fs::write(directory.0.join("other/guide.md"), "# Outside").unwrap();
        std::fs::write(directory.0.join("root/invalid.md"), [0xFF]).unwrap();
        std::fs::write(directory.0.join("root/text.txt"), "# Text").unwrap();
        std::fs::File::create(directory.0.join("root/large.md"))
            .unwrap()
            .set_len((crate::document::MAX_DOCUMENT_BYTES + 1) as u64)
            .unwrap();
        let uri = directory.uri("root/source.md");
        let mut server = Server::default();
        server.request("initialize", json!({ "capabilities": {}, "workspaceFolders": [{ "uri": directory.uri("root"), "name": "root" }] })).unwrap();
        for marked in [
            "[go](../other/gu¦)",
            "[go](../other/guide.md#¦)",
            "[go](missing.md#¦)",
            "[go](invalid.md#¦)",
            "[go](large.md#¦)",
            "[go](text.txt#¦)",
            "[go](directory/#¦)",
        ] {
            assert!(
                completion_labels(&complete_marked(&mut server, &uri, marked).unwrap()).is_empty(),
                "{marked}"
            );
        }
        server.notification("workspace/didChangeWorkspaceFolders", json!({ "event": { "added": [{ "uri": directory.uri("other"), "name": "other" }], "removed": [] } }), Instant::now()).unwrap();
        assert_eq!(
            completion_labels(&complete_marked(&mut server, &uri, "[go](../other/gu¦)").unwrap()),
            ["guide.md"]
        );
        assert_eq!(
            completion_labels(
                &complete_marked(&mut server, &uri, "[go](../other/guide.md#¦)").unwrap()
            ),
            ["outside"]
        );
        server.notification("workspace/didChangeWorkspaceFolders", json!({ "event": { "added": [], "removed": [{ "uri": directory.uri("other"), "name": "other" }] } }), Instant::now()).unwrap();
        assert!(completion_labels(
            &complete_marked(&mut server, &uri, "[go](../other/gu¦)").unwrap()
        )
        .is_empty());
    }

    #[test]
    fn navigates_local_anchors_without_changing_reference_definition_or_rename_semantics() {
        let mut server = server();
        let uri = "untitled:anchors";
        let now = Instant::now();
        open(&mut server, uri, "# foo*bar*baz\r\n# 中文😀\r\n# 中文😀\r\n\r\n[one](#foo-bar-baz)\r\n[two](#%E4%B8%AD%E6%96%87%F0%9F%98%80-2)\r\n[case](#Foo-bar-baz)\r\n[root](#)\r\n[ref][label]\r\n\r\n[label]: #foo-bar-baz\r\n", now);
        let first = navigate(&mut server, uri, 4).unwrap();
        assert_eq!(first["uri"], uri);
        assert_eq!(
            first["range"],
            json!({ "start": { "line": 0, "character": 2 }, "end": { "line": 0, "character": 13 } })
        );
        let second = navigate(&mut server, uri, 5).unwrap();
        assert_eq!(
            second["range"],
            json!({ "start": { "line": 2, "character": 2 }, "end": { "line": 2, "character": 6 } })
        );
        assert_eq!(navigate(&mut server, uri, 6).unwrap(), Value::Null);
        assert_eq!(
            navigate(&mut server, uri, 7).unwrap()["range"],
            json!(document_start())
        );
        assert_eq!(
            navigate(&mut server, uri, 8).unwrap()["range"]["start"]["line"],
            10
        );
        assert_eq!(navigate(&mut server, uri, 10).unwrap(), first);
        let renamed = server.request("textDocument/rename", json!({
            "textDocument": { "uri": uri }, "position": { "line": 8, "character": 7 }, "newName": "next"
        })).unwrap();
        assert_eq!(renamed["changes"][uri].as_array().unwrap().len(), 2);
        let document = server.documents.get_mut(uri).unwrap();
        assert!(document
            .ast(&server.parser)
            .unwrap()
            .children
            .iter()
            .all(|node| {
                !matches!(node, yozora_ast::Node::Heading(heading) if heading.identifier.is_some())
            }));
    }

    #[test]
    fn validates_initialization_atomically_and_supports_heading_prefixes() {
        let mut server = Server::default();
        for options in [
            json!({ "headingIdPrefix": 1 }),
            json!({ "headingIdPrefix": "x".repeat(257) }),
        ] {
            assert_eq!(
                server
                    .request(
                        "initialize",
                        json!({ "capabilities": {}, "initializationOptions": options })
                    )
                    .unwrap_err()
                    .code,
                -32602
            );
            assert!(server.state == State::Uninitialized);
        }
        let initialized = server
            .request(
                "initialize",
                json!({
                    "capabilities": {}, "initializationOptions": { "headingIdPrefix": "h-" }
                }),
            )
            .unwrap();
        assert_eq!(
            initialized["capabilities"]["documentLinkProvider"],
            json!({ "resolveProvider": false })
        );
        assert_eq!(
            initialized["capabilities"]["workspace"]["workspaceFolders"],
            json!({ "supported": true, "changeNotifications": true })
        );
        open(
            &mut server,
            "untitled:prefix",
            "[go](#h-intro)\n[missing](#intro)\n\n# Intro\n",
            Instant::now(),
        );
        assert_eq!(
            navigate(&mut server, "untitled:prefix", 0).unwrap()["range"]["start"]["line"],
            3
        );
        assert_eq!(
            navigate(&mut server, "untitled:prefix", 1).unwrap(),
            Value::Null
        );
    }

    #[test]
    fn file_navigation_prefers_unsaved_uri_aliases_and_tracks_target_lifecycle() {
        use crate::files::tests::TestDir;
        let directory = TestDir::new();
        let path = directory.0.join("guide 中文%.md");
        std::fs::write(&path, "# Intro\n").unwrap();
        let source_uri = directory.uri("source.md");
        let target_uri = directory.uri("guide 中文%.md");
        let alias = target_uri
            .replacen("file://", "file://localhost", 1)
            .replace("/guide", "/./guide")
            .replace("%E4", "%e4");
        let mut server = server();
        let now = Instant::now();
        open(&mut server, &source_uri,
            "[go](guide%20%E4%B8%AD%E6%96%87%25.md#intro)\n[file](guide%20%E4%B8%AD%E6%96%87%25.md)\n", now);
        let disk = navigate(&mut server, &source_uri, 0).unwrap();
        assert_eq!(disk["uri"], target_uri);
        assert_eq!(disk["range"]["start"]["line"], 0);
        open(&mut server, &alias, "\n\n# Intro\n", now);
        let unsaved = navigate(&mut server, &source_uri, 0).unwrap();
        assert_eq!(unsaved["uri"], alias);
        assert_eq!(unsaved["range"]["start"]["line"], 2);
        let links = server
            .request(
                "textDocument/documentLink",
                json!({ "textDocument": { "uri": source_uri } }),
            )
            .unwrap();
        assert_eq!(links[0]["target"], format!("{alias}#intro"));
        assert_eq!(links[1]["target"], alias);

        server
            .queue_query(
                json!(42),
                "textDocument/definition",
                json!({
                    "textDocument": { "uri": source_uri }, "position": { "line": 0, "character": 2 }
                }),
                now,
            )
            .unwrap();
        replace(&mut server, &alias, 2, "\n# Intro\n", now).unwrap();
        let queued = server.take_due_work(now + QUERY_DELAY).unwrap().unwrap();
        assert_eq!(queued["id"], 42);
        assert_eq!(queued["result"]["range"]["start"]["line"], 1);

        assert!(server
            .notification(
                "textDocument/didChange",
                json!({
                    "textDocument": { "uri": alias, "version": 3 }
                }),
                now
            )
            .is_err());
        for line in [0, 1] {
            assert_eq!(
                navigate(&mut server, &source_uri, line).unwrap_err().code,
                -32801
            );
        }
        replace(&mut server, &alias, 4, "# Different\n", now).unwrap();
        assert_eq!(navigate(&mut server, &source_uri, 0).unwrap(), Value::Null);
        assert_eq!(navigate(&mut server, &source_uri, 1).unwrap()["uri"], alias);
        server
            .notification(
                "textDocument/didClose",
                json!({ "textDocument": { "uri": alias } }),
                now,
            )
            .unwrap();
        assert_eq!(navigate(&mut server, &source_uri, 0).unwrap(), disk);
        std::fs::write(&path, "\n\n\n# Intro\n").unwrap();
        assert_eq!(
            navigate(&mut server, &source_uri, 0).unwrap()["range"]["start"]["line"],
            3
        );
        open(&mut server, &alias, "# Intro\n", now);
        assert_eq!(navigate(&mut server, &source_uri, 0).unwrap()["uri"], alias);
    }

    #[test]
    fn handles_unsaved_targets_missing_paths_non_markdown_files_and_encoded_delimiters() {
        use crate::files::tests::TestDir;
        let directory = TestDir::new();
        let mut server = server();
        let uri = directory.uri("source.md");
        let now = Instant::now();
        std::fs::write(directory.0.join("asset.bin"), [0xFF]).unwrap();
        std::fs::write(directory.0.join("a#b?c%.md"), "# Intro").unwrap();
        open(&mut server, &uri,
            "[new](new.md#intro)\n[missing](missing.md)\n[file](asset.bin)\n[fragment](asset.bin#intro)\n[encoded](a%23b%3Fc%25.md?view=raw#intro)\n[self](./source.md#intro)\n\n# Intro\n", now);
        assert_eq!(navigate(&mut server, &uri, 0).unwrap(), Value::Null);
        open(&mut server, &directory.uri("new.md"), "\n# Intro", now);
        assert_eq!(
            navigate(&mut server, &uri, 0).unwrap()["range"]["start"]["line"],
            1
        );
        assert_eq!(navigate(&mut server, &uri, 1).unwrap(), Value::Null);
        assert_eq!(
            navigate(&mut server, &uri, 2).unwrap(),
            json!({ "uri": directory.uri("asset.bin"), "range": document_start() })
        );
        assert_eq!(navigate(&mut server, &uri, 3).unwrap(), Value::Null);
        assert_eq!(
            navigate(&mut server, &uri, 4).unwrap()["uri"],
            directory.uri("a#b?c%.md")
        );
        assert_eq!(
            navigate(&mut server, &uri, 5).unwrap()["range"]["start"]["line"],
            7
        );
        let links = server
            .request(
                "textDocument/documentLink",
                json!({ "textDocument": { "uri": uri } }),
            )
            .unwrap();
        assert_eq!(links.as_array().unwrap().len(), 5);
        assert_eq!(
            links[3]["target"],
            format!("{}?view=raw#intro", directory.uri("a#b?c%.md"))
        );
    }

    #[test]
    fn applies_workspace_folder_changes_atomically_and_honors_root_uri() {
        use crate::files::tests::TestDir;
        let directory = TestDir::new();
        std::fs::create_dir_all(directory.0.join("root/docs")).unwrap();
        std::fs::create_dir(directory.0.join("other")).unwrap();
        std::fs::write(directory.0.join("root/guide.md"), "# Intro").unwrap();
        std::fs::write(directory.0.join("other/guide.md"), "# Other").unwrap();
        let uri = directory.uri("root/docs/source.md");
        let root_uri = directory.uri("root");
        let other_uri = directory.uri("other");
        let text = "[parent](../guide.md#intro)\n[other](../../other/guide.md#other)\n";
        let now = Instant::now();
        let mut fallback = server();
        open(&mut fallback, &uri, text, now);
        assert_eq!(navigate(&mut fallback, &uri, 0).unwrap(), Value::Null);
        let mut server = Server::default();
        server
            .request(
                "initialize",
                json!({ "capabilities": {}, "rootUri": root_uri }),
            )
            .unwrap();
        open(&mut server, &uri, text, now);
        assert_eq!(
            navigate(&mut server, &uri, 0).unwrap()["uri"],
            directory.uri("root/guide.md")
        );
        assert_eq!(navigate(&mut server, &uri, 1).unwrap(), Value::Null);
        server
            .notification(
                "workspace/didChangeWorkspaceFolders",
                json!({
                    "event": { "removed": [], "added": [{ "uri": other_uri, "name": "other" }] }
                }),
                now,
            )
            .unwrap();
        assert_eq!(
            navigate(&mut server, &uri, 1).unwrap()["uri"],
            directory.uri("other/guide.md")
        );
        assert!(server.notification("workspace/didChangeWorkspaceFolders", json!({
            "event": { "removed": [{ "uri": root_uri }, { "uri": other_uri }], "added": [{ "uri": "invalid" }] }
        }), now).is_err());
        assert!(!navigate(&mut server, &uri, 0).unwrap().is_null());
        assert!(!navigate(&mut server, &uri, 1).unwrap().is_null());
        server
            .notification(
                "workspace/didChangeWorkspaceFolders",
                json!({
                    "event": { "removed": [{ "uri": other_uri }], "added": [] }
                }),
                now,
            )
            .unwrap();
        assert_eq!(navigate(&mut server, &uri, 1).unwrap(), Value::Null);
    }

    #[test]
    fn document_links_include_admonition_titles_and_safe_external_uris_and_share_query_cancellation(
    ) {
        let mut server = server();
        let uri = "untitled:links";
        let now = Instant::now();
        open(&mut server, uri, ":::note [title](#intro)\n[body](#intro)\n:::\n\n# Intro\n\n<https://example.test/docs>\n\n[mail](mailto:reader@example.test)\n\n[unsafe](javascript:alert)\n", now);
        let params = json!({ "textDocument": { "uri": uri } });
        let links = server
            .request("textDocument/documentLink", params.clone())
            .unwrap();
        assert_eq!(links.as_array().unwrap().len(), 4);
        assert_eq!(links[0]["range"]["start"]["line"], 0);
        assert_eq!(links[1]["range"]["start"]["line"], 1);
        assert_eq!(links[0]["target"], format!("{uri}#intro"));
        assert_eq!(links[2]["target"], "https://example.test/docs");
        assert_eq!(links[3]["target"], "mailto:reader@example.test");
        assert_eq!(
            server
                .definition(
                    uri,
                    Position {
                        line: 0,
                        character: 11
                    }
                )
                .unwrap()["range"]["start"]["line"],
            4
        );
        assert_eq!(navigate(&mut server, uri, 6).unwrap(), Value::Null);
        assert!(is_document_query("textDocument/documentLink"));
        server
            .queue_query(json!(7), "textDocument/documentLink", params, now)
            .unwrap();
        replace(&mut server, uri, 2, "# Changed", now).unwrap();
        assert_eq!(server.outgoing.last().unwrap()["error"]["code"], -32801);
        assert!(server.pending_queries.is_empty());
    }

    #[test]
    fn cancels_waiting_queries_by_exact_id_and_allows_id_reuse() {
        let mut server = server();
        let now = Instant::now();
        let uri = "untitled:cancel";
        open(&mut server, uri, "# Current", now);
        queue(&mut server, json!(1), uri, now);
        queue(&mut server, json!("1"), uri, now);
        assert!(server
            .take_due_work(now + Duration::from_millis(4))
            .unwrap()
            .is_none());
        server
            .notification("$/cancelRequest", json!({ "id": 1 }), now)
            .unwrap();
        assert_eq!(server.outgoing.len(), 1);
        let cancelled = server.outgoing.pop().unwrap();
        assert_eq!(cancelled["id"], 1);
        assert_eq!(cancelled["error"]["code"], -32800);
        let result = server.take_due_work(now + QUERY_DELAY).unwrap().unwrap();
        assert_eq!(result["id"], "1");
        assert_eq!(result["result"][0]["name"], "Current");
        server
            .notification("$/cancelRequest", json!({ "id": 1 }), now + QUERY_DELAY)
            .unwrap();
        assert!(server.outgoing.is_empty());
        queue(&mut server, json!(1), uri, now + QUERY_DELAY);
        let result = server
            .take_due_work(now + QUERY_DELAY * 2)
            .unwrap()
            .unwrap();
        assert_eq!(result["id"], 1);
        assert_eq!(result["result"][0]["name"], "Current");
    }

    #[test]
    fn newer_edits_close_and_reopen_invalidate_only_that_documents_queries() {
        for action in ["change", "malformed", "close", "reopen"] {
            let mut server = server();
            let now = Instant::now();
            let uri = "untitled:changing";
            open(&mut server, uri, "# Before", now);
            open(&mut server, "untitled:other", "# Other", now);
            queue(&mut server, json!(1), uri, now);
            queue(&mut server, json!(2), "untitled:other", now);
            assert!(replace(&mut server, uri, 1, "# Stale", now).is_err());
            assert!(server.outgoing.is_empty());
            assert_eq!(server.pending_queries.len(), 2);
            match action {
                "change" => replace(&mut server, uri, 2, "# After", now).unwrap(),
                "malformed" => assert!(server
                    .notification(
                        "textDocument/didChange",
                        json!({
                            "textDocument": { "uri": uri, "version": 2 }
                        }),
                        now
                    )
                    .is_err()),
                "close" => server
                    .notification(
                        "textDocument/didClose",
                        json!({
                            "textDocument": { "uri": uri }
                        }),
                        now,
                    )
                    .unwrap(),
                "reopen" => open(&mut server, uri, "# Reopened at the same version", now),
                _ => unreachable!(),
            }
            let responses: Vec<_> = server
                .outgoing
                .iter()
                .filter(|message| message.get("id").is_some())
                .collect();
            assert_eq!(responses.len(), 1, "{action}");
            assert_eq!(responses[0]["id"], 1);
            assert_eq!(responses[0]["error"]["code"], -32801);
            let remaining = server.take_due_work(now + QUERY_DELAY).unwrap().unwrap();
            assert_eq!(remaining["id"], 2);
            assert_eq!(remaining["result"][0]["name"], "Other");
            assert!(server.pending_queries.is_empty());
        }
    }

    #[test]
    fn query_traffic_preserves_older_diagnostic_deadlines() {
        let mut server = server();
        let now = Instant::now();
        open(&mut server, "untitled:diagnostics", DUPLICATES, now);
        open(
            &mut server,
            "untitled:queries",
            "# Query",
            now + Duration::from_millis(10),
        );
        queue(
            &mut server,
            json!(1),
            "untitled:queries",
            now + Duration::from_millis(149),
        );
        assert_eq!(server.next_deadline(), Some(now + DIAGNOSTICS_DELAY));
        let diagnostic = server
            .take_due_work(now + DIAGNOSTICS_DELAY)
            .unwrap()
            .unwrap();
        assert_eq!(diagnostic["params"]["uri"], "untitled:diagnostics");
        let query = server
            .take_due_work(now + Duration::from_millis(154))
            .unwrap()
            .unwrap();
        assert_eq!(query["id"], 1);
        assert_eq!(query["result"][0]["name"], "Query");
        assert_eq!(
            server.next_deadline(),
            Some(now + Duration::from_millis(160))
        );
    }

    #[test]
    fn shutdown_cancels_every_waiting_query_and_discards_deadlines() {
        let mut server = server();
        let now = Instant::now();
        open(&mut server, "untitled:shutdown", DUPLICATES, now);
        queue(&mut server, json!(1), "untitled:shutdown", now);
        queue(&mut server, json!(2), "untitled:shutdown", now);
        server.request("shutdown", Value::Null).unwrap();
        assert!(server.next_deadline().is_none());
        assert!(server
            .take_due_work(now + Duration::from_secs(1))
            .unwrap()
            .is_none());
        assert_eq!(server.outgoing.len(), 2);
        for (index, response) in server.outgoing.iter().enumerate() {
            assert_eq!(response["id"], index + 1);
            assert_eq!(response["error"]["code"], -32800);
        }
    }

    #[test]
    fn coalesces_versions_while_queries_use_the_latest_text_without_moving_the_deadline() {
        let mut server = server();
        let now = Instant::now();
        let at = |millis| now + Duration::from_millis(millis);
        let uri = "untitled:debounce";
        open(&mut server, uri, DUPLICATES, now);
        assert!(server.take_due_diagnostics(at(99)).unwrap().is_none());
        replace(
            &mut server,
            uri,
            2,
            "# Changed😀\n\n[ref]: /one\n[ref]: /two",
            at(100),
        )
        .unwrap();
        assert!(server.take_due_diagnostics(at(150)).unwrap().is_none());
        let params = json!({ "textDocument": { "uri": uri } });
        assert_eq!(
            server
                .request("textDocument/documentSymbol", params.clone())
                .unwrap()[0]["name"],
            "Changed😀"
        );
        replace(&mut server, uri, 3, "# Final\n", at(175)).unwrap();
        assert_eq!(
            server
                .request("textDocument/documentSymbol", params)
                .unwrap()[0]["name"],
            "Final"
        );
        assert!(server.take_due_diagnostics(at(250)).unwrap().is_none());
        assert!(server.take_due_diagnostics(at(324)).unwrap().is_none());
        assert_eq!(
            published(&mut server, at(325)),
            json!({
                "uri": uri, "version": 3, "diagnostics": []
            })
        );
        assert!(server.take_due_diagnostics(at(500)).unwrap().is_none());
    }

    #[test]
    fn edits_in_one_document_do_not_delay_another_document() {
        let mut server = server();
        let now = Instant::now();
        let at = |millis| now + Duration::from_millis(millis);
        open(&mut server, "untitled:a", DUPLICATES, now);
        open(&mut server, "untitled:b", DUPLICATES, at(10));
        replace(&mut server, "untitled:a", 2, DUPLICATES, at(100)).unwrap();
        assert!(server.take_due_diagnostics(at(159)).unwrap().is_none());
        let other = published(&mut server, at(160));
        assert_eq!(other["uri"], "untitled:b");
        assert_eq!(other["version"], 1);
        assert_eq!(other["diagnostics"].as_array().unwrap().len(), 1);
        assert!(server.take_due_diagnostics(at(249)).unwrap().is_none());
        let edited = published(&mut server, at(250));
        assert_eq!(edited["uri"], "untitled:a");
        assert_eq!(edited["version"], 2);
        assert!(server.take_due_diagnostics(at(500)).unwrap().is_none());
    }

    #[test]
    fn stale_edits_keep_the_deadline_and_failed_newer_edits_cancel_it() {
        let mut server = server();
        let now = Instant::now();
        let at = |millis| now + Duration::from_millis(millis);
        let uri = "untitled:sync";
        open(&mut server, uri, DUPLICATES, now);
        replace(&mut server, uri, 2, DUPLICATES, at(20)).unwrap();
        assert!(replace(&mut server, uri, 1, "# stale", at(100)).is_err());
        assert!(server.outgoing.is_empty());
        assert!(server.take_due_diagnostics(at(169)).unwrap().is_none());
        assert_eq!(published(&mut server, at(170))["version"], 2);
        replace(&mut server, uri, 3, DUPLICATES, at(200)).unwrap();
        let invalid = server.notification("textDocument/didChange", json!({
            "textDocument": { "uri": uri, "version": 4 },
            "contentChanges": [{
                "range": { "start": { "line": 0, "character": 3 }, "end": { "line": 0, "character": 4 } },
                "text": "broken"
            }]
        }), at(210));
        assert!(invalid.is_err());
        assert_eq!(
            server.outgoing.pop().unwrap()["params"],
            json!({
                "uri": uri, "version": 4, "diagnostics": []
            })
        );
        assert_eq!(
            server
                .request(
                    "textDocument/documentSymbol",
                    json!({ "textDocument": { "uri": uri } })
                )
                .unwrap_err()
                .code,
            -32801
        );
        assert!(server.take_due_diagnostics(at(1000)).unwrap().is_none());
        replace(&mut server, uri, 5, DUPLICATES, at(1100)).unwrap();
        assert!(replace(&mut server, uri, 3, "# stale", at(1150)).is_err());
        assert!(server.outgoing.is_empty());
        assert!(server.take_due_diagnostics(at(1249)).unwrap().is_none());
        let restored = published(&mut server, at(1250));
        assert_eq!(restored["version"], 5);
        assert_eq!(restored["diagnostics"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn close_reopen_and_shutdown_discard_scheduled_work() {
        let mut server = server();
        let now = Instant::now();
        let at = |millis| now + Duration::from_millis(millis);
        open(&mut server, "untitled:a", DUPLICATES, now);
        open(&mut server, "untitled:b", DUPLICATES, now);
        server
            .notification(
                "textDocument/didClose",
                json!({ "textDocument": { "uri": "untitled:a" } }),
                at(10),
            )
            .unwrap();
        assert_eq!(
            server.outgoing.pop().unwrap()["params"],
            json!({
                "uri": "untitled:a", "diagnostics": []
            })
        );
        open(&mut server, "untitled:a", "# Reopened", at(20));
        assert_eq!(published(&mut server, at(150))["uri"], "untitled:b");
        assert!(server.take_due_diagnostics(at(169)).unwrap().is_none());
        assert_eq!(
            published(&mut server, at(170)),
            json!({
                "uri": "untitled:a", "version": 1, "diagnostics": []
            })
        );
        replace(&mut server, "untitled:a", 2, DUPLICATES, at(180)).unwrap();
        server.request("shutdown", Value::Null).unwrap();
        assert!(server.take_due_diagnostics(at(500)).unwrap().is_none());
        assert!(server.outgoing.is_empty());
    }

    #[test]
    fn malformed_newer_batches_cancel_diagnostics_and_require_resynchronization() {
        for changes in [
            None,
            Some(Value::Null),
            Some(json!({ "text": "wrong container" })),
            Some(json!([{ "text": 42 }])),
            Some(json!([{ "text": "# Partial" }, {
                "range": { "start": { "line": 0, "character": 0 } }, "text": "missing end"
            }])),
        ] {
            let mut server = server();
            let now = Instant::now();
            let uri = "untitled:malformed";
            open(&mut server, uri, DUPLICATES, now);
            let mut params = json!({ "textDocument": { "uri": uri, "version": 2 } });
            if let Some(changes) = changes {
                params["contentChanges"] = changes;
            }
            assert!(server
                .notification("textDocument/didChange", params.clone(), now)
                .is_err());
            assert_eq!(
                server.outgoing.pop().unwrap()["params"],
                json!({
                    "uri": uri, "version": 2, "diagnostics": []
                })
            );
            assert!(server
                .take_due_diagnostics(now + Duration::from_secs(1))
                .unwrap()
                .is_none());
            assert_eq!(
                server
                    .request(
                        "textDocument/documentSymbol",
                        json!({ "textDocument": { "uri": uri } })
                    )
                    .unwrap_err()
                    .code,
                -32801
            );
            let recovered_at = now + Duration::from_secs(1);
            replace(&mut server, uri, 3, DUPLICATES, recovered_at).unwrap();
            assert!(server
                .notification("textDocument/didChange", params, recovered_at)
                .is_err());
            assert!(server.outgoing.is_empty());
            let diagnostics = published(&mut server, recovered_at + DIAGNOSTICS_DELAY);
            assert_eq!(diagnostics["version"], 3);
            assert_eq!(diagnostics["diagnostics"].as_array().unwrap().len(), 1);
        }
    }
}
