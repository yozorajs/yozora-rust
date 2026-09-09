use std::collections::{HashMap, VecDeque};
use std::io::{self, BufRead, Write};
use std::process::ExitCode;
use std::sync::{
    mpsc::{self, RecvTimeoutError},
    Arc,
};
use std::time::{Duration, Instant};

use serde_json::{json, Value};
#[cfg(test)]
use yozora_parser::YozoraParser;

use crate::cancellation::Cancellation;
use crate::document::{open_document, Document};
use crate::files::Workspace;
use crate::protocol::{
    parse_params, DidChangeParams, DidChangeWatchedFilesParams, DidChangeWorkspaceFoldersParams,
    DidOpenParams, FileEvent, InitializeParams, Json, RenameFilesParams, ResponseError,
    TextDocumentParams,
};
use crate::query::{format_diagnostics, Dependencies, Request};
use crate::transport::{read_messages, write_encoded, write_message};
use crate::worker::{self, Work, Workers, WORKER_COUNT};
#[cfg(test)]
use crate::{protocol::Position, query::document_start};

const DIAGNOSTICS_DELAY: Duration = Duration::from_millis(150);
const QUERY_DELAY: Duration = Duration::ZERO;
const MAX_PENDING_QUERIES: usize = 128;
const MAX_PENDING_QUERY_BYTES: usize = 1024 * 1024;

#[derive(Default, Eq, PartialEq)]
enum State {
    #[default]
    Uninitialized,
    Running,
    Shutdown,
}

struct PendingQuery {
    id: Value,
    request: Request,
    deadline: Instant,
}

impl PendingQuery {
    fn retained_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            + self.request.owned_bytes()
            + match &self.id {
                Value::String(id) => id.capacity(),
                _ => 0,
            }
    }
}

enum Event {
    Input(io::Result<Option<Vec<u8>>>),
    Finished(Box<worker::Finished>),
}

enum Reply {
    Query(Value),
    Diagnostics,
}

struct RunningWork {
    // A workspace query has no source document and shares one scheduling slot.
    uri: Option<String>,
    workspace: bool,
    reply: Option<Reply>,
    scope_epoch: u64,
    cancellation: Cancellation,
}

struct WatchRequest {
    id: String,
    registration: String,
    generation: u64,
}

impl Drop for RunningWork {
    fn drop(&mut self) {
        // EOF, exit and I/O errors also stop the remaining stages of live work.
        self.cancellation.cancel();
    }
}

#[derive(Default)]
struct Server {
    state: State,
    #[cfg(test)]
    parser: YozoraParser,
    #[cfg(test)]
    index: crate::workspace_index::Index,
    query: crate::query::State,
    diagnostics_due: HashMap<String, Instant>,
    diagnostic_dependencies: HashMap<String, Dependencies>,
    pending_queries: VecDeque<PendingQuery>,
    pending_query_bytes: usize,
    running: [Option<RunningWork>; WORKER_COUNT],
    scope_epoch: u64,
    watches_supported: bool,
    watch_initialized: bool,
    watch_generation: u64,
    watch_serial: u64,
    pending_watch: Option<WatchRequest>,
    active_watch: Option<String>,
    // Notifications can invalidate queries and retract diagnostics together.
    // Drain these messages even when the notification returns an error.
    outgoing: Vec<Json>,
}

pub fn run(reader: impl BufRead + Send + 'static, writer: impl Write) -> io::Result<ExitCode> {
    run_with_workers(reader, writer, |events| {
        Workers::new(move |result| events.send(Event::Finished(Box::new(result))).is_ok())
    })
}

fn run_with_workers(
    reader: impl BufRead + Send + 'static,
    mut writer: impl Write,
    make_workers: impl FnOnce(mpsc::SyncSender<Event>) -> io::Result<Workers>,
) -> io::Result<ExitCode> {
    let (events, incoming) = mpsc::sync_channel(1);
    let input_events = events.clone();
    read_messages(reader, move |message| {
        input_events.send(Event::Input(message)).is_ok()
    })?;
    let workers = make_workers(events)?;
    let mut server = Server::default();
    loop {
        let message = match server.next_deadline() {
            Some(deadline) => {
                incoming.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            }
            None => incoming.recv().map_err(|_| RecvTimeoutError::Disconnected),
        };
        match message {
            Ok(Event::Input(message)) => {
                let Some(body) = message? else { break };
                if let Some(status) = server.handle_message(&body, &mut writer)? {
                    return Ok(status);
                }
            }
            Ok(Event::Finished(result)) => server.finish_work(*result),
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => break,
        }
        // Check timers after every message as well as on idle timeouts, so
        // traffic for one document cannot starve diagnostics for another.
        server.complete_cached_queries(Instant::now());
        for message in server.outgoing.drain(..) {
            write_encoded(&mut writer, message.as_bytes())?;
        }
        while let Some((worker, task)) = server.start_work(Instant::now()) {
            workers.submit(worker, task)?;
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
        if message.get("method").is_none()
            && message.get("jsonrpc").and_then(Value::as_str) == Some("2.0")
            && id.is_some()
            && (message.get("result").is_some() || message.get("error").is_some())
        {
            self.watch_response(&message);
            return Ok(None);
        }
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
            if self.state == State::Running && is_query(method) {
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
            write_encoded(writer, message.as_bytes())?;
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

    fn advance_file_revision(&mut self, changes: Option<&[FileEvent]>) {
        self.scope_epoch = self.scope_epoch.wrapping_add(1);
        Arc::make_mut(&mut self.query.file_events).record(self.scope_epoch, changes);
        if self.query.file_revision.is_some() {
            self.query.file_revision = Some(self.scope_epoch);
        }
    }

    fn request_file_watches(&mut self) {
        if !self.watch_initialized || !self.watches_supported || self.pending_watch.is_some() {
            return;
        }
        self.query.file_revision = None;
        if let Some(registration) = self.active_watch.take() {
            self.watch_serial += 1;
            self.outgoing.push(json!({
                "jsonrpc": "2.0", "id": format!("yozora.unwatch.{}", self.watch_serial),
                "method": "client/unregisterCapability",
                "params": {"unregisterations": [{"id": registration, "method": "workspace/didChangeWatchedFiles"}]}
            }).into());
        }
        let roots = self.query.workspace.watch_roots();
        if roots.is_empty() || roots.len() > crate::files::MAX_WORKSPACE_ROOTS {
            return;
        }
        self.watch_serial += 1;
        let id = format!("yozora.watch.{}", self.watch_serial);
        let registration = id.clone();
        let watchers: Vec<_> = roots
            .into_iter()
            .map(|root| {
                json!({
                    "globPattern": {"baseUri": root, "pattern": "**/*"}, "kind": 7
                })
            })
            .collect();
        self.outgoing.push(json!({
            "jsonrpc": "2.0", "id": id,
            "method": "client/registerCapability",
            "params": {"registrations": [{"id": registration, "method": "workspace/didChangeWatchedFiles", "registerOptions": {"watchers": watchers}}]}
        }).into());
        self.pending_watch = Some(WatchRequest {
            id,
            registration,
            generation: self.watch_generation,
        });
    }

    fn watch_response(&mut self, response: &Value) {
        if self.state != State::Running
            || self
                .pending_watch
                .as_ref()
                .is_none_or(|pending| response["id"].as_str() != Some(&pending.id))
        {
            return;
        }
        let pending = self.pending_watch.take().expect("matched watch request");
        if response.get("error").is_some() || response.get("result") != Some(&Value::Null) {
            // A client may advertise registration but reject a pattern. Its
            // acknowledgment, not its capability bit, enables event caching.
            self.query.file_revision = None;
            self.watches_supported = false;
            return;
        }
        self.active_watch = Some(pending.registration);
        if pending.generation == self.watch_generation {
            self.query.file_revision = Some(self.scope_epoch);
        } else {
            self.request_file_watches();
        }
    }

    fn queue_query(
        &mut self,
        id: Value,
        method: &str,
        params: Value,
        now: Instant,
    ) -> Result<(), ResponseError> {
        let request = Request::parse(method, params)?;
        if let Some(uri) = request.source_uri() {
            open_document(&mut self.query.documents, uri)?;
        }
        let query = PendingQuery {
            id,
            request,
            deadline: now + QUERY_DELAY,
        };
        let bytes = query.retained_bytes();
        if self.pending_queries.len() >= MAX_PENDING_QUERIES
            || bytes > MAX_PENDING_QUERY_BYTES - self.pending_query_bytes
        {
            return Err(ResponseError::new(
                -32000,
                "server is busy; retry after outstanding queries complete",
            ));
        }
        self.pending_query_bytes += bytes;
        self.pending_queries.push_back(query);
        Ok(())
    }

    fn reject_queries(
        &mut self,
        reject: impl Fn(Option<&str>, bool, &Value) -> bool,
        error: ResponseError,
    ) {
        let outgoing = &mut self.outgoing;
        let pending_bytes = &mut self.pending_query_bytes;
        self.pending_queries.retain(|query| {
            if reject(
                query.request.source_uri(),
                query.request.uses_workspace(),
                &query.id,
            ) {
                *pending_bytes -= query.retained_bytes();
                outgoing.push(error.response(query.id.clone()).into());
                false
            } else {
                true
            }
        });
        for work in self.running.iter_mut().flatten() {
            if let Some(Reply::Query(id)) = &work.reply {
                if reject(work.uri.as_deref(), work.workspace, id) {
                    outgoing.push(error.response(id.clone()).into());
                    work.reply = None;
                    work.cancellation.cancel();
                }
            }
        }
    }

    fn invalidate_queries(&mut self, uri: &str) {
        self.reject_queries(
            |source, workspace, _| workspace || source == Some(uri),
            ResponseError::new(
                -32801,
                "document or workspace changed before the query completed",
            ),
        );
        for work in self.running.iter_mut().flatten() {
            if work.uri.as_deref() == Some(uri) {
                work.reply = None;
                work.cancellation.cancel();
            }
        }
    }

    fn invalidate_workspace_queries(&mut self) {
        self.reject_queries(
            |_, workspace, _| workspace,
            ResponseError::new(-32801, "workspace changed before the query completed"),
        );
    }

    /// Only completed, current diagnostics replace dependency information. A
    /// target edit refreshes its known referrers; file/scope events also refresh
    /// missing targets and alias selections that have no open-buffer dependency.
    fn refresh_link_diagnostics(&mut self, target: Option<&str>, now: Instant) {
        let sources: Vec<_> = self
            .diagnostic_dependencies
            .iter()
            .filter(|(uri, dependencies)| {
                target.map_or(dependencies.files, |target| {
                    uri.as_str() != target && dependencies.targets.contains(target)
                })
            })
            .map(|(uri, _)| uri.clone())
            .collect();
        for uri in sources {
            if !self
                .query
                .documents
                .get(&uri)
                .is_some_and(|document| document.text().is_ok())
            {
                continue;
            }
            for work in self.running.iter_mut().flatten() {
                if work.uri.as_deref() == Some(&uri)
                    && matches!(work.reply, Some(Reply::Diagnostics))
                {
                    work.reply = None;
                    work.cancellation.cancel();
                }
            }
            self.diagnostics_due.insert(uri, now + DIAGNOSTICS_DELAY);
        }
    }

    fn busy(&self, uri: Option<&str>, workspace: bool) -> bool {
        self.running.iter().flatten().any(|work| {
            (uri.is_some() && work.uri.as_deref() == uri) || (workspace && work.workspace)
        })
    }

    fn next_deadline(&self) -> Option<Instant> {
        if self.running.iter().all(Option::is_some) {
            return None;
        }
        self.pending_queries
            .iter()
            .filter(|query| !self.busy(query.request.source_uri(), query.request.uses_workspace()))
            .map(|query| query.deadline)
            .chain(
                self.diagnostics_due
                    .iter()
                    .filter(|(uri, _)| !self.busy(Some(uri), false))
                    .map(|(_, deadline)| *deadline),
            )
            .min()
    }

    fn start_work(&mut self, now: Instant) -> Option<(usize, worker::Task)> {
        if self.state != State::Running {
            return None;
        }
        let worker = self.running.iter().position(Option::is_none)?;
        // One task per source, and at most one workspace scan, prevent repeated
        // slow queries from occupying every worker. Older blocked work must not
        // prevent an eligible request in another scope from starting.
        let query = self
            .pending_queries
            .iter()
            .enumerate()
            .filter(|(_, query)| {
                query.deadline <= now
                    && !self.busy(query.request.source_uri(), query.request.uses_workspace())
            })
            .min_by_key(|(index, query)| (query.deadline, *index))
            .map(|(index, query)| (index, query.deadline));
        let diagnostic = self
            .diagnostics_due
            .iter()
            .filter(|(uri, deadline)| **deadline <= now && !self.busy(Some(uri), false))
            .min_by_key(|(uri, deadline)| (**deadline, *uri))
            .map(|(uri, deadline)| (uri.clone(), *deadline));
        let (reply, work) = if let Some((index, _)) =
            query.filter(|(_, deadline)| diagnostic.as_ref().is_none_or(|(_, due)| deadline <= due))
        {
            let query = self
                .pending_queries
                .remove(index)
                .expect("selected query exists");
            self.pending_query_bytes -= query.retained_bytes();
            (Reply::Query(query.id), Work::Query(query.request))
        } else {
            let (uri, _) = diagnostic?;
            self.diagnostics_due.remove(&uri);
            (Reply::Diagnostics, Work::Diagnostics(uri))
        };
        let cancellation = Cancellation::default();
        self.running[worker] = Some(RunningWork {
            uri: work.source_uri().map(str::to_string),
            workspace: work.uses_workspace(),
            reply: Some(reply),
            scope_epoch: self.scope_epoch,
            cancellation: cancellation.clone(),
        });
        Some((
            worker,
            worker::Task {
                state: self.query.clone(),
                work,
                cancellation,
            },
        ))
    }

    fn complete_cached_queries(&mut self, now: Instant) {
        let state = &self.query;
        let pending_bytes = &mut self.pending_query_bytes;
        let outgoing = &mut self.outgoing;
        let mut bytes = 0;
        let mut count = 0;
        // Bound work on the input thread even when a client pipelines many
        // large outlines. Cache misses retain normal worker/deadline ordering.
        self.pending_queries.retain(|query| {
            if query.deadline > now || count == 8 || bytes >= 1024 * 1024 {
                return true;
            }
            let Some(result) = state.cached_request(&query.request) else {
                return true;
            };
            *pending_bytes -= query.retained_bytes();
            bytes += result.as_bytes().len();
            count += 1;
            outgoing.push(Json::response(&query.id, &result));
            false
        });
    }

    fn finish_work(&mut self, mut finished: worker::Finished) {
        let Some(mut work) = self.running[finished.worker].take() else {
            return;
        };
        let matches = |uri: &str| {
            self.query
                .documents
                .get(uri)
                .zip(finished.state.documents.get(uri))
                .is_some_and(|(live, snapshot)| live.matches(snapshot))
        };
        let current = work.uri.as_deref().is_none_or(matches)
            && finished.dependencies.targets.iter().all(|uri| {
                self.query
                    .documents
                    .get(uri)
                    .zip(finished.state.documents.get(uri))
                    .is_some_and(|(live, snapshot)| live.same_revision(snapshot))
            })
            && (!finished.dependencies.files || work.scope_epoch == self.scope_epoch);
        for (uri, snapshot) in &mut finished.state.documents {
            if let Some(live) = self.query.documents.get_mut(uri) {
                live.adopt_ast(snapshot);
            }
        }
        match work.reply.take() {
            Some(Reply::Query(id)) => {
                let result = if current {
                    finished.result
                } else {
                    Err(ResponseError::new(
                        -32801,
                        "document or workspace changed during analysis",
                    ))
                };
                self.outgoing.push(match result {
                    Ok(result) => Json::response(&id, &result),
                    Err(error) => error.response(id).into(),
                });
            }
            Some(Reply::Diagnostics) if current => {
                let uri = work
                    .uri
                    .as_ref()
                    .expect("diagnostics have a source document");
                self.diagnostic_dependencies
                    .insert(uri.clone(), finished.dependencies);
                match finished.result {
                    Ok(publication) => self.outgoing.push(publication),
                    Err(error) => {
                        eprintln!("yozora-lsp: diagnostics: {}", error.message);
                        let uri = work
                            .uri
                            .as_ref()
                            .expect("diagnostics have a source document");
                        let version = self.query.documents[uri].version();
                        self.outgoing.push(
                            format_diagnostics(
                                uri,
                                self.query.diagnostic_version.then_some(version),
                                Vec::new(),
                                false,
                            )
                            .into(),
                        );
                    }
                }
            }
            Some(Reply::Diagnostics) => {
                if let Some(uri) = &work.uri {
                    if self
                        .query
                        .documents
                        .get(uri)
                        .is_some_and(|document| document.text().is_ok())
                    {
                        self.diagnostics_due
                            .entry(uri.clone())
                            .or_insert_with(|| Instant::now() + DIAGNOSTICS_DELAY);
                    }
                }
            }
            _ => {}
        }
    }

    #[cfg(test)]
    fn take_due_work(&mut self, now: Instant) -> Result<Option<Value>, ResponseError> {
        let Some((worker, task)) = self.start_work(now) else {
            return Ok(None);
        };
        let before = self.outgoing.len();
        let finished = worker::execute(worker, task, &self.parser, &mut self.index);
        self.finish_work(finished);
        Ok((self.outgoing.len() > before).then(|| self.outgoing.pop().unwrap().into_value()))
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
            let options = initialization.initialization_options.unwrap_or_default();
            let use_file_events = options.file_event_cache.unwrap_or(true);
            let prefix = options.heading_id_prefix;
            if prefix.len() > 256 || prefix.chars().any(char::is_control) {
                return Err(ResponseError::invalid_params(
                    "headingIdPrefix must be at most 256 UTF-8 bytes without control characters",
                ));
            }
            self.query.workspace = Workspace::new(folders);
            self.query.refactor_file_event_cache = options.refactor_file_event_cache;
            self.watches_supported = use_file_events
                && params
                    .pointer("/capabilities/workspace/didChangeWatchedFiles/dynamicRegistration")
                    .and_then(Value::as_bool)
                    == Some(true)
                && params
                    .pointer("/capabilities/workspace/didChangeWatchedFiles/relativePatternSupport")
                    .and_then(Value::as_bool)
                    == Some(true);
            self.query.heading_id_prefix = prefix;
            self.query.hierarchical_symbols = params
                .pointer(
                    "/capabilities/textDocument/documentSymbol/hierarchicalDocumentSymbolSupport",
                )
                .and_then(Value::as_bool)
                .unwrap_or(false);
            self.query.folding_range_limit = params
                .pointer("/capabilities/textDocument/foldingRange/rangeLimit")
                .and_then(Value::as_u64)
                .and_then(|limit| usize::try_from(limit).ok());
            self.query.hover_markdown = params
                .pointer("/capabilities/textDocument/hover/contentFormat")
                .and_then(Value::as_array)
                .and_then(|formats| {
                    formats
                        .iter()
                        .filter_map(Value::as_str)
                        .find(|format| matches!(*format, "markdown" | "plaintext"))
                })
                == Some("markdown");
            self.query.versioned_edits = params
                .pointer("/capabilities/workspace/workspaceEdit/documentChanges")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            self.query.diagnostic_related_information = params
                .pointer("/capabilities/textDocument/publishDiagnostics/relatedInformation")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            self.query.diagnostic_version = params
                .pointer("/capabilities/textDocument/publishDiagnostics/versionSupport")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            self.state = State::Running;
            return Ok(json!({
                "capabilities": {
                    "positionEncoding": "utf-16",
                    "textDocumentSync": { "openClose": true, "change": 2 },
                    "documentSymbolProvider": true,
                    "workspaceSymbolProvider": true,
                    "foldingRangeProvider": true,
                    "definitionProvider": true,
                    "documentLinkProvider": { "resolveProvider": false },
                    "workspace": {
                        "workspaceFolders": { "supported": true, "changeNotifications": true },
                        "fileOperations": {
                            "willRename": { "filters": [{ "scheme": "file", "pattern": { "glob": "**/*", "matches": "file" } }] },
                            "didRename": { "filters": [{ "scheme": "file", "pattern": { "glob": "**/*", "matches": "file" } }] },
                        },
                    },
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
                self.query.documents.clear();
                self.diagnostics_due.clear();
                self.diagnostic_dependencies.clear();
                self.outgoing.clear();
                self.reject_queries(
                    |_, _, _| true,
                    ResponseError::new(-32800, "server is shutting down"),
                );
                for work in self.running.iter_mut().flatten() {
                    work.reply = None;
                    work.cancellation.cancel();
                }
                Ok(Value::Null)
            }
            #[cfg(test)]
            method if is_query(method) => self
                .query
                .request(
                    Request::parse(method, params)?,
                    &self.parser,
                    &mut self.index,
                    &Cancellation::default(),
                    &mut crate::query::Dependencies::default(),
                )
                .map(Json::into_value),
            _ => Err(ResponseError::new(-32601, "Method not found")),
        }
    }

    #[cfg(test)]
    fn definition(&mut self, uri: &str, position: Position) -> Result<Value, ResponseError> {
        self.request(
            "textDocument/definition",
            json!({ "textDocument": { "uri": uri }, "position": position }),
        )
    }

    #[cfg(test)]
    fn complete(&mut self, uri: &str, position: Position) -> Result<Value, ResponseError> {
        self.request(
            "textDocument/completion",
            json!({ "textDocument": { "uri": uri }, "position": position }),
        )
    }

    fn notification(
        &mut self,
        method: &str,
        params: Value,
        now: Instant,
    ) -> Result<(), ResponseError> {
        match method {
            "initialized" => {
                if !self.watch_initialized {
                    self.watch_initialized = true;
                    self.request_file_watches();
                }
            }
            "workspace/didRenameFiles" => {
                let params: RenameFilesParams = parse_params(params)?;
                for file in &params.files {
                    validate_uri(&file.old_uri)?;
                    validate_uri(&file.new_uri)?;
                }
                if !params.files.is_empty() {
                    // File notifications invalidate snapshots; didClose/didOpen
                    // remain the authority for renamed buffer text and versions.
                    self.advance_file_revision(None);
                    self.invalidate_workspace_queries();
                    self.refresh_link_diagnostics(None, now);
                }
            }
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
                self.query.workspace.change(&removed, added);
                self.advance_file_revision(None);
                self.query.file_revision = None;
                self.watch_generation += 1;
                self.request_file_watches();
                self.invalidate_workspace_queries();
                self.refresh_link_diagnostics(None, now);
            }
            "workspace/didChangeWatchedFiles" => {
                let params: DidChangeWatchedFilesParams = parse_params(params)?;
                for event in &params.changes {
                    validate_uri(&event.uri)?;
                    if !(1..=3).contains(&event.kind) {
                        return Err(ResponseError::invalid_params("unknown file change type"));
                    }
                }
                if !params.changes.is_empty() {
                    // Queries revalidate disk without a watcher. Diagnostics
                    // also refresh when a client supplies file notifications.
                    self.advance_file_revision(Some(&params.changes));
                    self.invalidate_workspace_queries();
                    self.refresh_link_diagnostics(None, now);
                }
            }
            "textDocument/didOpen" => {
                let params: DidOpenParams = parse_params(params)?;
                let item = params.text_document;
                validate_uri(&item.uri)?;
                let document = Document::new(item.version, item.text)?;
                self.invalidate_queries(&item.uri);
                self.diagnostics_due
                    .insert(item.uri.clone(), now + DIAGNOSTICS_DELAY);
                self.query.documents.insert(item.uri, document);
                self.advance_file_revision(None);
                self.refresh_link_diagnostics(None, now);
            }
            "textDocument/didChange" => {
                let params: DidChangeParams = parse_params(params)?;
                let uri = params.text_document.uri;
                let document = open_document(&mut self.query.documents, &uri)?;
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
                        self.diagnostics_due
                            .insert(uri.clone(), now + DIAGNOSTICS_DELAY);
                    } else {
                        // Text is now out of sync; retract the previous snapshot.
                        self.diagnostics_due.remove(&uri);
                        self.outgoing.push(
                            format_diagnostics(
                                &uri,
                                self.query.diagnostic_version.then_some(version),
                                Vec::new(),
                                false,
                            )
                            .into(),
                        );
                    }
                    self.refresh_link_diagnostics(Some(&uri), now);
                }
                result?;
            }
            "textDocument/didClose" => {
                let params: TextDocumentParams = parse_params(params)?;
                let uri = params.text_document.uri;
                if self.query.documents.remove(&uri).is_some() {
                    self.advance_file_revision(None);
                    self.invalidate_queries(&uri);
                    self.diagnostics_due.remove(&uri);
                    self.diagnostic_dependencies.remove(&uri);
                    self.refresh_link_diagnostics(None, now);
                    self.outgoing
                        .push(format_diagnostics(&uri, None, Vec::new(), false).into());
                }
            }
            "$/cancelRequest" => {
                let id = params
                    .get("id")
                    .ok_or_else(|| ResponseError::invalid_params("cancelRequest requires an id"))?;
                self.reject_queries(
                    |_, _, query_id| query_id == id,
                    ResponseError::new(-32800, "request cancelled"),
                );
            }
            // Unknown notifications and optional $/ messages have no response.
            _ => {}
        }
        Ok(())
    }

    #[cfg(test)]
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
        let mut dependencies = Dependencies::default();
        let publication = self.query.diagnostics(
            &uri,
            &self.parser,
            &Cancellation::default(),
            &mut dependencies,
        )?;
        self.diagnostic_dependencies.insert(uri, dependencies);
        Ok(Some(publication))
    }
}

fn is_query(method: &str) -> bool {
    matches!(
        method,
        "workspace/symbol"
            | "workspace/willRenameFiles"
            | "textDocument/documentSymbol"
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

#[cfg(test)]
mod runtime_tests;

#[cfg(test)]
mod workspace_tests;

#[cfg(test)]
mod tests {
    use super::*;

    const DUPLICATES: &str = "# 😀\n\n[ref]: /first\n[REF]: /second\n";

    pub(super) fn server() -> Server {
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

    pub(super) fn open(server: &mut Server, uri: &str, text: &str, now: Instant) {
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

    pub(super) fn replace(
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
    fn file_event_caches_require_acknowledged_watches_and_allow_strict_validation() {
        let directory = crate::files::tests::TestDir::new();
        for (enabled, accepted) in [(true, true), (true, false), (false, true)] {
            let mut server = Server::default();
            server.request("initialize", json!({
                "rootUri": directory.uri(""),
                "initializationOptions": {"fileEventCache": enabled},
                "capabilities": {"workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true, "relativePatternSupport": true}}}
            })).unwrap();
            server
                .notification("initialized", json!({}), Instant::now())
                .unwrap();
            assert!(server.query.file_revision.is_none());
            if !enabled {
                assert!(server.outgoing.is_empty());
                continue;
            }
            let registration = server.outgoing.pop().unwrap();
            assert_eq!(registration["method"], "client/registerCapability");
            let response = if accepted {
                json!({"jsonrpc": "2.0", "id": registration["id"], "result": null})
            } else {
                json!({"jsonrpc": "2.0", "id": registration["id"], "error": {"code": -32601, "message": "unsupported"}})
            };
            server
                .handle_message(&serde_json::to_vec(&response).unwrap(), &mut Vec::new())
                .unwrap();
            assert_eq!(
                server.query.file_revision,
                accepted.then_some(server.scope_epoch)
            );
            server
                .notification(
                    "workspace/didChangeWatchedFiles",
                    json!({"changes": [{"uri": directory.uri("guide.md"), "type": 2}]}),
                    Instant::now(),
                )
                .unwrap();
            assert_eq!(server.query.file_revision, accepted.then_some(1));
        }
    }

    #[test]
    fn obsolete_watch_acknowledgments_cannot_enable_an_old_workspace_view() {
        let first = crate::files::tests::TestDir::new();
        let second = crate::files::tests::TestDir::new();
        let mut server = Server::default();
        server.request("initialize", json!({
            "rootUri": first.uri(""),
            "capabilities": {"workspace": {"didChangeWatchedFiles": {"dynamicRegistration": true, "relativePatternSupport": true}}}
        })).unwrap();
        let now = Instant::now();
        server.notification("initialized", json!({}), now).unwrap();
        let original = server.outgoing.pop().unwrap();
        server
            .notification(
                "workspace/didChangeWorkspaceFolders",
                json!({"event": {
                    "removed": [{"uri": first.uri(""), "name": "first"}],
                    "added": [{"uri": second.uri(""), "name": "second"}]
                }}),
                now,
            )
            .unwrap();
        server.watch_response(&json!({"id": original["id"], "result": null}));
        assert!(server.query.file_revision.is_none());
        assert_eq!(server.outgoing[0]["method"], "client/unregisterCapability");
        let current = server.outgoing.pop().unwrap();
        assert_eq!(
            current["params"]["registrations"][0]["registerOptions"]["watchers"][0]["globPattern"]
                ["baseUri"],
            second.uri("")
        );
        server.watch_response(&json!({"id": current["id"], "result": null}));
        assert_eq!(server.query.file_revision, Some(server.scope_epoch));
    }

    #[test]
    fn cached_outlines_release_queue_capacity_and_obey_cancellation_and_revisions() {
        let mut server = server();
        server.query.hierarchical_symbols = true;
        let now = Instant::now();
        let uri = "untitled:cached-outline";
        open(&mut server, uri, "# Before", now);
        queue(&mut server, json!(0), uri, now);
        assert_eq!(
            server.take_due_work(now).unwrap().unwrap()["result"][0]["name"],
            "Before"
        );

        for id in 1..=16 {
            queue(&mut server, json!(id), uri, now);
        }
        let queued_bytes = server.pending_query_bytes;
        server.complete_cached_queries(now);
        assert_eq!(server.outgoing.len(), 8);
        assert_eq!(server.pending_queries.len(), 8);
        assert!(server.pending_query_bytes < queued_bytes);
        assert!(server.running.iter().all(Option::is_none));
        server.complete_cached_queries(now);
        assert!(server.pending_queries.is_empty());
        assert_eq!(server.pending_query_bytes, 0);
        for (index, response) in server.outgoing.drain(..).enumerate() {
            assert_eq!(response["id"], index + 1);
            assert_eq!(response["result"][0]["name"], "Before");
        }

        queue(&mut server, json!(17), uri, now);
        server
            .notification("$/cancelRequest", json!({"id": 17}), now)
            .unwrap();
        server.complete_cached_queries(now);
        assert_eq!(server.outgoing.pop().unwrap()["error"]["code"], -32800);
        queue(&mut server, json!(18), uri, now);
        replace(&mut server, uri, 2, "# After", now).unwrap();
        server.complete_cached_queries(now);
        assert_eq!(server.outgoing.pop().unwrap()["error"]["code"], -32801);
        queue(&mut server, json!(19), uri, now);
        server.complete_cached_queries(now);
        assert!(server.outgoing.is_empty());
        assert_eq!(
            server.take_due_work(now).unwrap().unwrap()["result"][0]["name"],
            "After"
        );
    }

    #[test]
    fn queues_only_typed_parameters_and_rejects_malformed_requests_before_scheduling() {
        let mut server = server();
        let now = Instant::now();
        let uri = "untitled:typed-queue";
        open(&mut server, uri, "# Title", now);
        for method in [
            "textDocument/documentSymbol",
            "textDocument/foldingRange",
            "textDocument/definition",
            "textDocument/documentLink",
            "textDocument/hover",
            "textDocument/references",
            "textDocument/completion",
            "textDocument/prepareRename",
            "textDocument/rename",
        ] {
            server
                .queue_query(
                    json!(method),
                    method,
                    json!({
                        "textDocument": { "uri": uri, "extension": "x".repeat(512 * 1024) },
                        "position": { "line": 0, "character": 2 },
                        "context": { "includeDeclaration": false },
                        "newName": "renamed",
                        "ignoredExtension": "x".repeat(512 * 1024),
                    }),
                    now,
                )
                .unwrap();
        }
        assert_eq!(server.pending_queries.len(), 9);
        assert!(server.pending_query_bytes < 4_096);
        let before = server.pending_query_bytes;
        for (method, params) in [
            (
                "textDocument/hover",
                json!({ "textDocument": { "uri": uri } }),
            ),
            (
                "textDocument/rename",
                json!({ "textDocument": { "uri": uri }, "position": Position::default() }),
            ),
            (
                "textDocument/references",
                json!({ "textDocument": { "uri": uri }, "position": Position::default() }),
            ),
        ] {
            assert_eq!(
                server
                    .queue_query(json!("invalid"), method, params, now)
                    .unwrap_err()
                    .code,
                -32602
            );
        }
        assert_eq!(server.pending_query_bytes, before);
        assert_eq!(server.pending_queries.len(), 9);
    }

    #[test]
    fn bounds_pending_request_count_and_releases_capacity_on_dispatch_and_shutdown() {
        let mut server = server();
        let now = Instant::now();
        let uri = "untitled:queue-limit";
        open(&mut server, uri, "# Title", now);
        for id in 0..MAX_PENDING_QUERIES {
            queue(&mut server, json!(id), uri, now);
        }
        let before = server.pending_query_bytes;
        assert_eq!(
            server
                .queue_query(
                    json!("overflow"),
                    "textDocument/documentSymbol",
                    json!({ "textDocument": { "uri": uri } }),
                    now
                )
                .unwrap_err()
                .code,
            -32000
        );
        assert_eq!(server.pending_queries.len(), MAX_PENDING_QUERIES);
        assert_eq!(server.pending_query_bytes, before);
        let (_, task) = server.start_work(now + QUERY_DELAY).unwrap();
        assert!(server.pending_query_bytes < before);
        queue(&mut server, json!("replacement"), uri, now);
        assert_eq!(server.pending_queries.len(), MAX_PENDING_QUERIES);
        server.request("shutdown", Value::Null).unwrap();
        assert!(task.cancellation.is_cancelled());
        assert!(server.pending_queries.is_empty());
        assert_eq!(server.pending_query_bytes, 0);
        assert_eq!(server.outgoing.len(), MAX_PENDING_QUERIES + 1);
    }

    #[test]
    fn pending_byte_budget_counts_ids_uris_and_rename_strings() {
        for field in ["id", "uri", "newName"] {
            let mut server = server();
            let now = Instant::now();
            let padding = "x".repeat(MAX_PENDING_QUERY_BYTES / 2);
            let uri = if field == "uri" {
                format!("untitled:{padding}")
            } else {
                "untitled:byte-budget".into()
            };
            let id = if field == "id" {
                json!(padding)
            } else {
                json!(1)
            };
            let params = json!({
                "textDocument": { "uri": uri },
                "position": Position::default(),
                "newName": if field == "newName" { padding.as_str() } else { "new" },
            });
            open(&mut server, &uri, "# Title", now);
            server
                .queue_query(id.clone(), "textDocument/rename", params.clone(), now)
                .unwrap();
            let before = server.pending_query_bytes;
            assert!(before >= MAX_PENDING_QUERY_BYTES / 2);
            let overflow_id = if field == "id" {
                json!(format!("{padding}2"))
            } else {
                json!("overflow")
            };
            assert_eq!(
                server
                    .queue_query(overflow_id, "textDocument/rename", params.clone(), now)
                    .unwrap_err()
                    .code,
                -32000
            );
            assert_eq!(server.pending_query_bytes, before);
            assert_eq!(server.pending_queries.len(), 1);
            server
                .notification("$/cancelRequest", json!({ "id": id }), now)
                .unwrap();
            assert_eq!(server.pending_query_bytes, 0);
            server
                .queue_query(json!("reused"), "textDocument/rename", params, now)
                .unwrap();
        }
        let mut server = server();
        let now = Instant::now();
        let uri = "untitled:oversized-request";
        open(&mut server, uri, "# Title", now);
        assert_eq!(
            server
                .queue_query(
                    json!("x".repeat(MAX_PENDING_QUERY_BYTES)),
                    "textDocument/documentSymbol",
                    json!({ "textDocument": { "uri": uri } }),
                    now
                )
                .unwrap_err()
                .code,
            -32000
        );
        assert!(server.pending_queries.is_empty());
        assert_eq!(server.pending_query_bytes, 0);
    }

    #[test]
    fn running_work_does_not_block_eligible_queries_from_another_document() {
        let mut server = server();
        let now = Instant::now();
        open(&mut server, "untitled:slow", "# Slow", now);
        open(&mut server, "untitled:fast", "# Fast", now);
        queue(&mut server, json!(1), "untitled:slow", now);
        queue(&mut server, json!(2), "untitled:slow", now);
        queue(&mut server, json!(3), "untitled:fast", now);
        let (slow_worker, slow) = server.start_work(now + QUERY_DELAY).unwrap();
        let (fast_worker, fast) = server.start_work(now + QUERY_DELAY).unwrap();
        assert_ne!(slow_worker, fast_worker);
        assert_eq!(fast.work.source_uri(), Some("untitled:fast"));
        server.finish_work(worker::execute(
            fast_worker,
            fast,
            &server.parser,
            &mut Default::default(),
        ));
        assert_eq!(server.outgoing[0]["id"], 3);
        assert_eq!(server.outgoing[0]["result"][0]["name"], "Fast");
        assert!(server.start_work(now + QUERY_DELAY).is_none());
        server.finish_work(worker::execute(
            slow_worker,
            slow,
            &server.parser,
            &mut Default::default(),
        ));
        let (worker, task) = server.start_work(now + QUERY_DELAY).unwrap();
        assert_eq!(task.work.source_uri(), Some("untitled:slow"));
        server.finish_work(worker::execute(
            worker,
            task,
            &server.parser,
            &mut Default::default(),
        ));
        assert_eq!(server.outgoing[2]["id"], 2);
    }

    #[test]
    fn cancellation_retires_running_responses_before_the_worker_finishes_and_allows_id_reuse() {
        let mut server = server();
        let now = Instant::now();
        open(&mut server, "untitled:first", "# First", now);
        open(&mut server, "untitled:second", "# Second", now);
        queue(&mut server, json!(9), "untitled:first", now);
        let (old_worker, old) = server.start_work(now + QUERY_DELAY).unwrap();
        server
            .notification("$/cancelRequest", json!({ "id": 9 }), now)
            .unwrap();
        assert_eq!(server.outgoing[0]["error"]["code"], -32800);
        queue(&mut server, json!(9), "untitled:second", now);
        let (new_worker, new) = server.start_work(now + QUERY_DELAY).unwrap();
        server.finish_work(worker::execute(
            old_worker,
            old,
            &server.parser,
            &mut Default::default(),
        ));
        assert_eq!(server.outgoing.len(), 1);
        server.finish_work(worker::execute(
            new_worker,
            new,
            &server.parser,
            &mut Default::default(),
        ));
        assert_eq!(server.outgoing.len(), 2);
        assert_eq!(server.outgoing[1]["id"], 9);
        assert_eq!(server.outgoing[1]["result"][0]["name"], "Second");
    }

    #[test]
    fn close_and_reopen_with_reused_versions_cannot_adopt_a_running_tasks_old_ast() {
        let mut server = server();
        let now = Instant::now();
        let uri = "untitled:reopened";
        open(&mut server, uri, "# Old", now);
        queue(&mut server, json!(1), uri, now);
        let (worker, task) = server.start_work(now + QUERY_DELAY).unwrap();
        // Completion can be waiting in the event channel when close/reopen arrives.
        let finished = worker::execute(worker, task, &server.parser, &mut Default::default());
        server
            .notification(
                "textDocument/didClose",
                json!({ "textDocument": { "uri": uri } }),
                now,
            )
            .unwrap();
        assert_eq!(server.outgoing[0]["error"]["code"], -32801);
        open(&mut server, uri, "# Reopened", now);
        server.outgoing.clear();
        server.finish_work(finished);
        assert!(server.outgoing.is_empty());
        let symbols = server
            .request(
                "textDocument/documentSymbol",
                json!({ "textDocument": { "uri": uri } }),
            )
            .unwrap();
        assert_eq!(symbols[0]["name"], "Reopened");
    }

    #[test]
    fn only_changes_to_actual_target_buffers_invalidate_running_file_queries() {
        let directory = crate::files::tests::TestDir::new();
        let uri = directory.uri("source.md");
        let target = directory.uri("target.md");
        let unrelated = directory.uri("unrelated.md");
        let now = Instant::now();
        for (method, character) in [
            ("textDocument/definition", 2),
            ("textDocument/completion", 20),
        ] {
            for change_target in [false, true] {
                let mut server = server();
                open(&mut server, &uri, "[go](target.md#intro)", now);
                open(&mut server, &target, "# Intro", now);
                open(&mut server, &unrelated, "# Unrelated", now);
                server
                .queue_query(
                    json!(1),
                    method,
                    json!({
                        "textDocument": { "uri": uri }, "position": { "line": 0, "character": character }
                    }),
                    now,
                )
                .unwrap();
                let (worker, task) = server.start_work(now + QUERY_DELAY).unwrap();
                replace(
                    &mut server,
                    if change_target { &target } else { &unrelated },
                    2,
                    "# Changed",
                    now,
                )
                .unwrap();
                let finished =
                    worker::execute(worker, task, &server.parser, &mut Default::default());
                assert!(finished.dependencies.targets.contains(&target));
                server.finish_work(finished);
                if change_target {
                    assert_eq!(server.outgoing[0]["error"]["code"], -32801);
                } else if method == "textDocument/definition" {
                    assert_eq!(server.outgoing[0]["result"]["uri"], target);
                } else {
                    assert_eq!(completion_labels(&server.outgoing[0]["result"]), ["intro"]);
                }
            }
        }
    }

    #[test]
    fn workspace_changes_invalidate_running_file_results_without_discarding_local_symbols() {
        let directory = crate::files::tests::TestDir::new();
        let uri = directory.uri("source.md");
        let now = Instant::now();
        for method in ["textDocument/documentLink", "textDocument/documentSymbol"] {
            let mut server = server();
            open(&mut server, &uri, "# Intro\n\n[go](#intro)", now);
            server
                .queue_query(
                    json!(1),
                    method,
                    json!({ "textDocument": { "uri": uri } }),
                    now,
                )
                .unwrap();
            let (worker, task) = server.start_work(now + QUERY_DELAY).unwrap();
            server
                .notification(
                    "workspace/didChangeWorkspaceFolders",
                    json!({ "event": {
                "added": [{ "uri": directory.uri("") }], "removed": []
            } }),
                    now,
                )
                .unwrap();
            server.finish_work(worker::execute(
                worker,
                task,
                &server.parser,
                &mut Default::default(),
            ));
            if method == "textDocument/documentLink" {
                assert_eq!(server.outgoing[0]["error"]["code"], -32801);
            } else {
                assert_eq!(server.outgoing[0]["result"][0]["name"], "Intro");
            }
        }
    }

    #[test]
    fn edits_discard_running_diagnostics_and_publish_only_the_newer_version() {
        let mut server = server();
        let now = Instant::now();
        let uri = "untitled:diagnostic-race";
        open(&mut server, uri, DUPLICATES, now);
        let (worker, task) = server.start_work(now + DIAGNOSTICS_DELAY).unwrap();
        assert!(matches!(task.work, Work::Diagnostics(_)));
        let finished = worker::execute(worker, task, &server.parser, &mut Default::default());
        replace(&mut server, uri, 2, "# Clean", now + DIAGNOSTICS_DELAY).unwrap();
        server.finish_work(finished);
        assert!(server.outgoing.is_empty());
        let (worker, task) = server.start_work(now + DIAGNOSTICS_DELAY * 2).unwrap();
        server.finish_work(worker::execute(
            worker,
            task,
            &server.parser,
            &mut Default::default(),
        ));
        assert_eq!(server.outgoing[0]["params"]["version"], 2);
        assert_eq!(server.outgoing[0]["params"]["diagnostics"], json!([]));
    }

    #[test]
    fn shutdown_retires_running_work_without_waiting_for_analysis() {
        let mut server = server();
        let now = Instant::now();
        let uri = "untitled:shutdown-running";
        open(&mut server, uri, DUPLICATES, now);
        queue(&mut server, json!(1), uri, now);
        let (worker, task) = server.start_work(now + QUERY_DELAY).unwrap();
        assert_eq!(
            server.request("shutdown", Value::Null).unwrap(),
            Value::Null
        );
        assert_eq!(server.outgoing[0]["error"]["code"], -32800);
        server.finish_work(worker::execute(
            worker,
            task,
            &server.parser,
            &mut Default::default(),
        ));
        assert_eq!(server.outgoing.len(), 1);
        assert!(server.start_work(now + DIAGNOSTICS_DELAY).is_none());
        assert!(server.query.documents.is_empty());
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
            .query
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
        let document = server.query.documents.get_mut(uri).unwrap();
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
        assert!(is_query("textDocument/documentLink"));
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
        assert_eq!(server.next_deadline(), Some(now));
        assert_eq!(server.pending_queries.len(), 2);
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
            assert_eq!(server.pending_query_bytes, 0);
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
        let immediate = server
            .take_due_work(now + Duration::from_millis(149))
            .unwrap()
            .unwrap();
        assert_eq!(immediate["id"], 1);
        queue(
            &mut server,
            json!(2),
            "untitled:queries",
            now + Duration::from_millis(151),
        );
        assert_eq!(server.next_deadline(), Some(now + DIAGNOSTICS_DELAY));
        let diagnostic = server
            .take_due_work(now + Duration::from_millis(151))
            .unwrap()
            .unwrap();
        assert_eq!(diagnostic["params"]["uri"], "untitled:diagnostics");
        let query = server
            .take_due_work(now + Duration::from_millis(154))
            .unwrap()
            .unwrap();
        assert_eq!(query["id"], 2);
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
