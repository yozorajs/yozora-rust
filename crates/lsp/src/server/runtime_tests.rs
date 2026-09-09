use std::collections::HashSet;
use std::io::{BufReader, Cursor, Read};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

use yozora_core_parser::{DefaultParserProps, ParseOptions};

use super::*;
use crate::transport::read_message;

struct Input {
    receiver: mpsc::Receiver<Vec<u8>>,
    chunk: Cursor<Vec<u8>>,
}

impl Read for Input {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        if output.is_empty() {
            return Ok(0);
        }
        loop {
            let read = self.chunk.read(output)?;
            if read > 0 {
                return Ok(read);
            }
            match self.receiver.recv() {
                Ok(chunk) => self.chunk = Cursor::new(chunk),
                Err(_) => return Ok(0),
            }
        }
    }
}

struct Output {
    sender: mpsc::Sender<Vec<u8>>,
    frame: Vec<u8>,
}

impl Write for Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.frame.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.sender
            .send(std::mem::take(&mut self.frame))
            .map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))
    }
}

struct Session {
    input: Option<mpsc::Sender<Vec<u8>>>,
    output: mpsc::Receiver<Vec<u8>>,
    exited: mpsc::Receiver<io::Result<ExitCode>>,
    thread: Option<JoinHandle<()>>,
    responses: HashSet<String>,
}

impl Session {
    fn start(make_parser: impl Fn() -> YozoraParser + Send + Sync + 'static) -> Self {
        let (input, receiver) = mpsc::channel();
        let (sender, output) = mpsc::channel();
        let (status, exited) = mpsc::channel();
        let thread = thread::spawn(move || {
            let reader = BufReader::new(Input {
                receiver,
                chunk: Cursor::new(Vec::new()),
            });
            let writer = Output {
                sender,
                frame: Vec::new(),
            };
            let result = run_with_workers(reader, writer, |events| {
                Workers::with_parser_factory(
                    move |result| events.send(Event::Finished(Box::new(result))).is_ok(),
                    make_parser,
                )
            });
            let _ = status.send(result);
        });
        Self {
            input: Some(input),
            output,
            exited,
            thread: Some(thread),
            responses: HashSet::new(),
        }
    }

    fn send(&self, id: Option<&str>, method: &str, params: Value) {
        let mut message = json!({ "jsonrpc": "2.0", "method": method, "params": params });
        if let Some(id) = id {
            message["id"] = json!(id);
        }
        let mut frame = Vec::new();
        write_message(&mut frame, &message).unwrap();
        // Exercise the reader's incremental framing as well as the event loop.
        let body = frame.split_off(7);
        let input = self.input.as_ref().unwrap();
        input.send(frame).unwrap();
        input.send(body).unwrap();
    }

    #[track_caller]
    fn message(&self) -> Value {
        let frame = self.output.recv_timeout(Duration::from_secs(5)).unwrap();
        let body = read_message(&mut BufReader::new(Cursor::new(frame)))
            .unwrap()
            .unwrap();
        let message: Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(message["jsonrpc"], "2.0");
        message
    }

    #[track_caller]
    fn response(&mut self) -> Value {
        loop {
            let response = self.message();
            if let Some(id) = response.get("id") {
                assert!(
                    self.responses.insert(id.as_str().unwrap().to_string()),
                    "duplicate response: {response}"
                );
                return response;
            }
            assert_eq!(response["method"], "textDocument/publishDiagnostics");
        }
    }

    fn open(&self, uri: &str, text: &str) {
        self.send(
            None,
            "textDocument/didOpen",
            json!({
                "textDocument": { "uri": uri, "version": 1, "text": text }
            }),
        );
    }

    fn hover(&self, id: &str, uri: &str) {
        self.send(
            Some(id),
            "textDocument/hover",
            json!({
                "textDocument": { "uri": uri }, "position": { "line": 0, "character": 2 }
            }),
        );
    }

    fn exit(&mut self) {
        self.send(None, "exit", Value::Null);
        assert_eq!(
            self.exited
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .unwrap(),
            ExitCode::SUCCESS
        );
        self.input.take();
        self.thread.take().unwrap().join().unwrap();
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        // EOF releases the reader even if an assertion fails. Do not join a
        // server whose parser might deliberately still be blocked by the test.
        self.input.take();
    }
}

#[test]
fn target_edits_during_initial_diagnostics_discard_stale_results_and_refresh_referrers() {
    let directory = crate::files::tests::TestDir::new();
    let source = directory.uri("source.md");
    let target = directory.uri("target.md");
    let (entered, started) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let gate = Arc::new(Mutex::new(gate));
    let blocked = Arc::new(AtomicBool::new(false));
    let mut session = Session::start(move || {
        let entered = entered.clone();
        let gate = Arc::clone(&gate);
        let blocked = Arc::clone(&blocked);
        YozoraParser::new(DefaultParserProps {
            default_parse_options: Some(ParseOptions {
                format_url: Some(Arc::new(move |url| {
                    if url == "target.md#intro" && !blocked.swap(true, Ordering::Relaxed) {
                        entered.send(()).unwrap();
                        let _ = gate.lock().unwrap().recv();
                    }
                    url.to_string()
                })),
                ..ParseOptions::default()
            }),
            ..DefaultParserProps::default()
        })
    });
    session.send(
        Some("initialize"),
        "initialize",
        json!({
            "rootUri": directory.uri(""),
            "capabilities": { "textDocument": { "publishDiagnostics": { "versionSupport": true } } }
        }),
    );
    assert_eq!(session.response()["id"], "initialize");
    session.open(&target, "# Other");
    session.open(&source, "[go](target.md#intro)");
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    session.send(None, "textDocument/didChange", json!({
        "textDocument": { "uri": target, "version": 2 }, "contentChanges": [{ "text": "# Intro" }]
    }));
    // The immediate response confirms the edit was processed while source
    // analysis was blocked, before its first dependency set was available.
    session.send(Some("edited"), "unknown/barrier", json!({}));
    assert_eq!(session.response()["error"]["code"], -32601);
    release.send(()).unwrap();
    let publication = || loop {
        let message = session.message();
        assert!(message.get("id").is_none(), "{message}");
        assert_eq!(message["method"], "textDocument/publishDiagnostics");
        if message["params"]["uri"] == source {
            assert_eq!(message["params"]["version"], 1);
            break message["params"]["diagnostics"].clone();
        }
    };
    // A stale warning must never be published, and dropping it must schedule
    // fresh diagnostics even though the source itself was not edited.
    assert_eq!(publication(), json!([]));
    for (version, text, missing) in [(3, "# Other", true), (4, "# Intro", false)] {
        session.send(None, "textDocument/didChange", json!({
            "textDocument": { "uri": target, "version": version }, "contentChanges": [{ "text": text }]
        }));
        let diagnostics = publication();
        if missing {
            assert_eq!(diagnostics.as_array().unwrap().len(), 1);
            assert_eq!(diagnostics[0]["code"], "missing-anchor");
        } else {
            assert_eq!(diagnostics, json!([]));
        }
    }
    session.send(Some("shutdown"), "shutdown", Value::Null);
    assert_eq!(session.response()["id"], "shutdown");
    session.exit();
}

#[test]
fn workspace_refactors_stay_responsive_and_cancel_before_applying_stale_edits() {
    let directory = crate::files::tests::TestDir::new();
    std::fs::write(directory.0.join("source.md"), "# Old").unwrap();
    std::fs::write(directory.0.join("gate.md"), "[gate](blocked)").unwrap();
    let uri = directory.uri("source.md");
    let file_params = json!({ "files": [{ "oldUri": uri, "newUri": directory.uri("moved.md") }] });
    let (entered, started) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let gate = Arc::new(Mutex::new(gate));
    let mut session = Session::start(move || {
        let entered = entered.clone();
        let gate = Arc::clone(&gate);
        YozoraParser::new(DefaultParserProps {
            default_parse_options: Some(ParseOptions {
                format_url: Some(Arc::new(move |url| {
                    if url == "blocked" {
                        entered.send(()).unwrap();
                        let _ = gate.lock().unwrap().recv();
                    }
                    url.to_string()
                })),
                ..ParseOptions::default()
            }),
            ..DefaultParserProps::default()
        })
    });
    session.send(
        Some("initialize"),
        "initialize",
        json!({ "capabilities": {}, "rootUri": directory.uri("") }),
    );
    assert_eq!(session.response()["id"], "initialize");
    session.open(&uri, "# Old");
    session.open("untitled:fast", "# Fast");
    for (id, method, params) in [
        (
            "heading",
            "textDocument/rename",
            json!({ "textDocument": { "uri": uri }, "position": { "line": 0, "character": 3 }, "newName": "New" }),
        ),
        ("file", "workspace/willRenameFiles", file_params.clone()),
    ] {
        session.send(Some(id), method, params);
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        session.send(
            Some(&format!("responsive-{id}")),
            "textDocument/documentSymbol",
            json!({ "textDocument": { "uri": "untitled:fast" } }),
        );
        assert!(session.response().get("error").is_none());
        if id == "heading" {
            session.send(None, "textDocument/didChange", json!({
                "textDocument": { "uri": "untitled:fast", "version": 2 }, "contentChanges": [{ "text": "# Changed" }]
            }));
        } else {
            session.send(None, "$/cancelRequest", json!({ "id": id }));
        }
        let response = session.response();
        assert_eq!(response["id"], id);
        assert_eq!(
            response["error"]["code"],
            if id == "heading" { -32801 } else { -32800 }
        );
        release.send(()).unwrap();
        session.send(
            Some(&format!("recovered-{id}")),
            "textDocument/prepareRename",
            json!({ "textDocument": { "uri": uri }, "position": { "line": 0, "character": 3 } }),
        );
        assert_eq!(session.response()["result"]["placeholder"], "Old");
    }
    session.send(Some("stop-file"), "workspace/willRenameFiles", file_params);
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    session.send(Some("shutdown"), "shutdown", Value::Null);
    let cancelled = session.response();
    assert_eq!(cancelled["id"], "stop-file");
    assert_eq!(cancelled["error"]["code"], -32800);
    assert_eq!(session.response()["id"], "shutdown");
    session.exit();
    release.send(()).unwrap();
}

#[test]
fn framed_requests_remain_responsive_while_analysis_is_blocked() {
    let (entered, started) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let gate = Arc::new(Mutex::new(gate));
    let mut session = Session::start(move || {
        let entered = entered.clone();
        let gate = Arc::clone(&gate);
        YozoraParser::new(DefaultParserProps {
            default_parse_options: Some(ParseOptions {
                format_url: Some(Arc::new(move |url| {
                    if url == "blocked" {
                        entered.send(()).unwrap();
                        let _ = gate.lock().unwrap().recv();
                    }
                    url.to_string()
                })),
                ..ParseOptions::default()
            }),
            ..DefaultParserProps::default()
        })
    });
    session.send(
        Some("initialize"),
        "initialize",
        json!({ "capabilities": {} }),
    );
    assert_eq!(session.response()["id"], "initialize");
    session.open("untitled:fast", "[go](ready)");
    session.hover("warm", "untitled:fast");
    assert_eq!(session.response()["result"]["contents"]["value"], "ready");

    session.open("untitled:slow", "[go](blocked)");
    session.hover("cancelled", "untitled:slow");
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    session.send(None, "$/cancelRequest", json!({ "id": "cancelled" }));
    session.hover("responsive", "untitled:fast");
    for _ in 0..2 {
        let response = session.response();
        match response["id"].as_str().unwrap() {
            "cancelled" => assert_eq!(response["error"]["code"], -32800),
            "responsive" => assert_eq!(response["result"]["contents"]["value"], "ready"),
            _ => panic!("unexpected response: {response}"),
        }
    }
    release.send(()).unwrap();
    session.hover("fresh", "untitled:slow");
    let response = session.response();
    assert_eq!(response["id"], "fresh");
    assert_eq!(response["result"]["contents"]["value"], "blocked");

    for id in ["stop-first", "stop-second"] {
        let uri = format!("untitled:{id}");
        session.open(&uri, "[go](blocked)");
        session.hover(id, &uri);
        started.recv_timeout(Duration::from_secs(5)).unwrap();
    }
    // Both workers stay occupied while shutdown retires running and queued work.
    session.hover("queued", "untitled:fast");
    session.send(Some("shutdown"), "shutdown", Value::Null);
    for _ in 0..3 {
        let response = session.response();
        match response["id"].as_str().unwrap() {
            "stop-first" | "stop-second" | "queued" => {
                assert_eq!(response["error"]["code"], -32800)
            }
            _ => panic!("unexpected response: {response}"),
        }
    }
    let response = session.response();
    assert_eq!(response["id"], "shutdown");
    assert_eq!(response["result"], Value::Null);
    session.exit();
    release.send(()).unwrap();
    release.send(()).unwrap();
}

#[test]
fn workspace_scans_allow_document_queries_and_retire_on_cancel_edits_and_shutdown() {
    let directory = crate::files::tests::TestDir::new();
    let path = directory.0.join("guide.md");
    std::fs::write(&path, "# Disk [link](blocked)").unwrap();
    let (entered, started) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let gate = Arc::new(Mutex::new(gate));
    let mut session = Session::start(move || {
        let entered = entered.clone();
        let gate = Arc::clone(&gate);
        YozoraParser::new(DefaultParserProps {
            default_parse_options: Some(ParseOptions {
                format_url: Some(Arc::new(move |url| {
                    if url == "blocked" {
                        entered.send(()).unwrap();
                        let _ = gate.lock().unwrap().recv();
                    }
                    url.to_string()
                })),
                ..ParseOptions::default()
            }),
            ..DefaultParserProps::default()
        })
    });
    session.send(
        Some("initialize"),
        "initialize",
        json!({ "capabilities": {}, "rootUri": directory.uri("") }),
    );
    assert_eq!(
        session.response()["result"]["capabilities"]["workspaceSymbolProvider"],
        true
    );
    session.open("untitled:fast", "[go](ready)");
    session.hover("warm", "untitled:fast");
    assert_eq!(session.response()["result"]["contents"]["value"], "ready");

    session.send(
        Some("cancel-scan"),
        "workspace/symbol",
        json!({ "query": "" }),
    );
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    session.send(
        Some("queued-scan"),
        "workspace/symbol",
        json!({ "query": "" }),
    );
    session.hover("responsive", "untitled:fast");
    let response = session.response();
    assert_eq!(response["id"], "responsive");
    assert_eq!(response["result"]["contents"]["value"], "ready");
    for id in ["cancel-scan", "queued-scan"] {
        session.send(None, "$/cancelRequest", json!({ "id": id }));
        let response = session.response();
        assert_eq!(response["id"], id);
        assert_eq!(response["error"]["code"], -32800);
    }
    std::fs::write(&path, "# Fresh disk").unwrap();
    release.send(()).unwrap();
    session.send(
        Some("fresh-scan"),
        "workspace/symbol",
        json!({ "query": "Fresh" }),
    );
    let response = session.response();
    assert_eq!(response["id"], "fresh-scan");
    assert_eq!(response["result"][0]["name"], "Fresh disk");

    std::fs::write(&path, "# Again [link](blocked)").unwrap();
    session.send(
        Some("stale-scan"),
        "workspace/symbol",
        json!({ "query": "" }),
    );
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    session.send(
        None,
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": "untitled:fast", "version": 2 },
            "contentChanges": [{ "text": "# New buffer" }]
        }),
    );
    let response = session.response();
    assert_eq!(response["id"], "stale-scan");
    assert_eq!(response["error"]["code"], -32801);
    session.send(
        Some("changed-buffer"),
        "textDocument/documentSymbol",
        json!({
            "textDocument": { "uri": "untitled:fast" }
        }),
    );
    assert_eq!(session.response()["result"][0]["name"], "New buffer");
    std::fs::write(&path, "# Stable disk").unwrap();
    release.send(()).unwrap();
    session.send(
        Some("changed-workspace"),
        "workspace/symbol",
        json!({ "query": "New buffer" }),
    );
    assert_eq!(session.response()["result"][0]["name"], "New buffer");

    std::fs::write(&path, "# Shutdown [link](blocked)").unwrap();
    session.send(
        Some("stop-scan"),
        "workspace/symbol",
        json!({ "query": "" }),
    );
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    session.send(
        Some("stop-queued"),
        "workspace/symbol",
        json!({ "query": "" }),
    );
    session.send(Some("shutdown"), "shutdown", Value::Null);
    for _ in 0..2 {
        let response = session.response();
        assert!(matches!(
            response["id"].as_str(),
            Some("stop-scan" | "stop-queued")
        ));
        assert_eq!(response["error"]["code"], -32800);
    }
    assert_eq!(session.response()["id"], "shutdown");
    session.exit();
    release.send(()).unwrap();
}

#[test]
fn a_full_queue_rejects_new_queries_without_blocking_cancellation_or_shutdown() {
    let (entered, started) = mpsc::channel();
    let (release, gate) = mpsc::channel();
    let gate = Arc::new(Mutex::new(gate));
    let mut session = Session::start(move || {
        let entered = entered.clone();
        let gate = Arc::clone(&gate);
        YozoraParser::new(DefaultParserProps {
            default_parse_options: Some(ParseOptions {
                format_url: Some(Arc::new(move |url| {
                    if url == "blocked" {
                        entered.send(()).unwrap();
                        let _ = gate.lock().unwrap().recv();
                    }
                    url.to_string()
                })),
                ..ParseOptions::default()
            }),
            ..DefaultParserProps::default()
        })
    });
    session.send(
        Some("initialize"),
        "initialize",
        json!({ "capabilities": {} }),
    );
    assert_eq!(session.response()["id"], "initialize");
    session.open("untitled:fast", "[go](ready)");
    session.hover("warm", "untitled:fast");
    assert_eq!(session.response()["result"]["contents"]["value"], "ready");
    session.open("untitled:slow", "[go](blocked)");
    session.hover("blocked", "untitled:slow");
    started.recv_timeout(Duration::from_secs(5)).unwrap();
    for id in 0..MAX_PENDING_QUERIES {
        session.hover(&format!("queued-{id}"), "untitled:slow");
    }
    session.hover("overflow", "untitled:slow");
    let response = session.response();
    assert_eq!(response["id"], "overflow");
    assert_eq!(response["error"]["code"], -32000);
    session.send(None, "$/cancelRequest", json!({ "id": "queued-0" }));
    let response = session.response();
    assert_eq!(response["id"], "queued-0");
    assert_eq!(response["error"]["code"], -32800);
    session.hover("after-capacity-released", "untitled:fast");
    let response = session.response();
    assert_eq!(response["id"], "after-capacity-released");
    assert_eq!(response["result"]["contents"]["value"], "ready");
    session.send(Some("shutdown"), "shutdown", Value::Null);
    for _ in 0..MAX_PENDING_QUERIES {
        assert_eq!(session.response()["error"]["code"], -32800);
    }
    let response = session.response();
    assert_eq!(response["id"], "shutdown");
    assert_eq!(response["result"], Value::Null);
    session.exit();
    release.send(()).unwrap();
}

#[track_caller]
fn assert_completion_stops_after_retirement(source: &str, skip_probes: usize) {
    for retire in ["cancel", "change", "reopen"] {
        let (entered, started) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let gate = Arc::new(Mutex::new(gate));
        let armed = Arc::new(AtomicBool::new(false));
        let block = Arc::clone(&armed);
        let probes = Arc::new(AtomicUsize::new(0));
        let mut session = Session::start(move || {
            let entered = entered.clone();
            let gate = Arc::clone(&gate);
            let block = Arc::clone(&block);
            let probes = Arc::clone(&probes);
            YozoraParser::new(DefaultParserProps {
                default_parse_options: Some(ParseOptions {
                    format_url: Some(Arc::new(move |url| {
                        if url == "blocked"
                            && block.load(Ordering::Relaxed)
                            && probes.fetch_add(1, Ordering::Relaxed) >= skip_probes
                        {
                            entered.send(()).unwrap();
                            let _ = gate.lock().unwrap().recv();
                        }
                        url.to_string()
                    })),
                    ..ParseOptions::default()
                }),
                ..DefaultParserProps::default()
            })
        });
        session.send(
            Some("initialize"),
            "initialize",
            json!({ "capabilities": {} }),
        );
        assert_eq!(session.response()["id"], "initialize");
        let uri = "untitled:completion-retired";
        session.open(uri, source);
        session.hover("warm", uri);
        assert_eq!(session.response()["result"]["contents"]["value"], "blocked");
        let params = json!({
            "textDocument": { "uri": uri }, "position": { "line": 0, "character": 23 }
        });
        session.send(
            Some("candidates"),
            "textDocument/completion",
            params.clone(),
        );
        assert_eq!(
            session.response()["result"]["items"]
                .as_array()
                .unwrap()
                .len(),
            2
        );

        // Source parsing and diagnostics now reuse the cached AST. Each probe
        // contains exactly one blocked URL, so skip_probes selects its stage.
        armed.store(true, Ordering::Relaxed);
        session.send(Some("retired"), "textDocument/completion", params);
        started
            .recv_timeout(Duration::from_secs(5))
            .unwrap_or_else(|error| panic!("{retire}: waiting for a probe: {error}"));
        match retire {
            "cancel" => session.send(None, "$/cancelRequest", json!({ "id": "retired" })),
            "change" => session.send(
                None,
                "textDocument/didChange",
                json!({
                    "textDocument": { "uri": uri, "version": 2 },
                    "contentChanges": [{ "text": "[go](ready)" }]
                }),
            ),
            "reopen" => {
                session.send(
                    None,
                    "textDocument/didClose",
                    json!({ "textDocument": { "uri": uri } }),
                );
                session.open(uri, "[go](ready)");
            }
            _ => unreachable!(),
        }
        let response = session.response();
        assert_eq!(response["id"], "retired", "{retire}");
        assert_eq!(
            response["error"]["code"],
            if retire == "cancel" { -32800 } else { -32801 },
            "{retire}"
        );
        release.send(()).unwrap();
        session.hover("fresh", uri);
        let response = session.response();
        assert_eq!(response["id"], "fresh", "{retire}");
        assert_eq!(
            response["result"]["contents"]["value"],
            if retire == "cancel" {
                "blocked"
            } else {
                "ready"
            },
            "{retire}"
        );

        session.send(Some("shutdown"), "shutdown", Value::Null);
        assert_eq!(session.response()["id"], "shutdown");
        session.exit();
    }
}

#[test]
fn retired_label_completion_skips_remaining_candidates() {
    assert_completion_stops_after_retirement(
        "[go](blocked) [text][ol]\n\n[old-one]: /one\n[old-two]: /two",
        0,
    );
}

#[test]
fn retired_destination_completion_skips_candidates_after_context_probe() {
    assert_completion_stops_after_retirement(
        "[go](blocked) [text](#hea)\n\n# Heading one\n\n# Heading two",
        0,
    );
}

#[test]
fn retired_destination_completion_skips_remaining_candidates() {
    assert_completion_stops_after_retirement(
        "[go](blocked) [text](#hea)\n\n# Heading one\n\n# Heading two",
        1,
    );
}
