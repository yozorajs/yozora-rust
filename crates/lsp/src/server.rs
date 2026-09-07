use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::process::ExitCode;

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use yozora_parser::YozoraParser;

use crate::analysis;
use crate::document::Document;
use crate::protocol::{
    DefinitionParams, DidChangeParams, DidOpenParams, DocumentSymbol, ResponseError,
    TextDocumentParams,
};
use crate::transport::{read_message, write_message};

#[derive(Default, Eq, PartialEq)]
enum State {
    #[default]
    Uninitialized,
    Running,
    Shutdown,
}

#[derive(Default)]
struct Server {
    state: State,
    parser: YozoraParser,
    documents: HashMap<String, Document>,
    hierarchical_symbols: bool,
    folding_range_limit: Option<usize>,
}

pub fn run(mut reader: impl BufRead, mut writer: impl Write) -> io::Result<ExitCode> {
    let mut server = Server::default();
    while let Some(body) = read_message(&mut reader)? {
        let message: Value = match serde_json::from_slice(&body) {
            Ok(message) => message,
            Err(_) => {
                write_message(
                    &mut writer,
                    &ResponseError::new(-32700, "Parse error").response(Value::Null),
                )?;
                continue;
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
                &mut writer,
                &ResponseError::new(-32600, "Invalid Request")
                    .response(id.filter(|_| valid_id).unwrap_or(Value::Null)),
            )?;
            continue;
        }

        let method = method.expect("method was validated");
        let params = message.get("params").cloned().unwrap_or(Value::Null);
        if let Some(id) = id {
            let response = match server.request(method, params) {
                Ok(result) => json!({ "jsonrpc": "2.0", "id": id, "result": result }),
                Err(error) => error.response(id),
            };
            write_message(&mut writer, &response)?;
        } else if method == "exit" {
            return Ok(server.exit_status());
        } else if server.state == State::Running {
            if let Err(error) = server.notification(method, params) {
                eprintln!("yozora-lsp: {method}: {}", error.message);
            }
        }
    }
    Ok(server.exit_status())
}

impl Server {
    fn exit_status(&self) -> ExitCode {
        if self.state == State::Shutdown {
            ExitCode::SUCCESS
        } else {
            ExitCode::FAILURE
        }
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
            self.state = State::Running;
            return Ok(json!({
                "capabilities": {
                    "positionEncoding": "utf-16",
                    "textDocumentSync": { "openClose": true, "change": 2 },
                    "documentSymbolProvider": true,
                    "foldingRangeProvider": true,
                    "definitionProvider": true,
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
                let params: DefinitionParams = parse_params(params)?;
                let uri = params.text_document.uri;
                let document = open_document(&mut self.documents, &uri)?;
                document.validate_position(params.position)?;
                Ok(
                    analysis::definition(document.ast(&self.parser)?, params.position)
                        .map(|range| json!({ "uri": uri, "range": range }))
                        .unwrap_or(Value::Null),
                )
            }
            _ => Err(ResponseError::new(-32601, "Method not found")),
        }
    }

    fn notification(&mut self, method: &str, params: Value) -> Result<(), ResponseError> {
        match method {
            "textDocument/didOpen" => {
                let params: DidOpenParams = parse_params(params)?;
                let item = params.text_document;
                validate_uri(&item.uri)?;
                let document = Document::new(item.version, item.text)?;
                self.documents.insert(item.uri, document);
            }
            "textDocument/didChange" => {
                let params: DidChangeParams = parse_params(params)?;
                let document = open_document(&mut self.documents, &params.text_document.uri)?;
                document.change(params.text_document.version, params.content_changes)?;
            }
            "textDocument/didClose" => {
                let params: TextDocumentParams = parse_params(params)?;
                self.documents.remove(&params.text_document.uri);
            }
            // Unknown notifications and optional $/ messages have no response.
            _ => {}
        }
        Ok(())
    }
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
