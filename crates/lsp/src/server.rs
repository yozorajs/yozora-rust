use std::collections::{HashMap, VecDeque};
use std::io::{self, BufRead, Write};
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
use crate::protocol::{
    DidChangeParams, DidOpenParams, DocumentSymbol, ReferencesParams, RenameParams, ResponseError,
    TextDocumentParams, TextDocumentPositionParams,
};
use crate::rename;
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
                    "hoverProvider": true,
                    "referencesProvider": true,
                    "completionProvider": { "triggerCharacters": ["[", "^"], "resolveProvider": false },
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
                let uri = params.text_document.uri;
                let document = open_document(&mut self.documents, &uri)?;
                document.validate_position(params.position)?;
                Ok(
                    analysis::definition(document.ast(&self.parser)?, params.position)
                        .map(|range| json!({ "uri": uri, "range": range }))
                        .unwrap_or(Value::Null),
                )
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
                let document = open_document(&mut self.documents, &params.text_document.uri)?;
                let snapshot = document.snapshot(&self.parser, params.position)?;
                Ok(json!(completion::complete(
                    &snapshot,
                    params.position,
                    &self.parser,
                )?))
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

    fn notification(
        &mut self,
        method: &str,
        params: Value,
        now: Instant,
    ) -> Result<(), ResponseError> {
        match method {
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
