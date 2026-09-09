use std::collections::VecDeque;
use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, ExitStatus, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

struct Client {
    process: Child,
    input: Option<ChildStdin>,
    output: Receiver<Value>,
    reader: Option<JoinHandle<()>>,
    notifications: VecDeque<Value>,
    next_id: i32,
}

impl Client {
    fn start() -> Self {
        let mut process = Command::new(env!("CARGO_BIN_EXE_yozora-lsp"))
            .arg("--stdio")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let input = process.stdin.take();
        let stdout = process.stdout.take().unwrap();
        let (sender, output) = mpsc::channel();
        // Independent framing checks also catch accidental logging to stdout.
        let reader = thread::spawn(move || {
            let mut stdout = BufReader::new(stdout);
            loop {
                let mut header = String::new();
                if stdout.read_line(&mut header).unwrap() == 0 {
                    break;
                }
                let length: usize = header
                    .strip_prefix("Content-Length: ")
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap();
                let mut separator = String::new();
                stdout.read_line(&mut separator).unwrap();
                assert_eq!(separator, "\r\n");
                let mut body = vec![0; length];
                stdout.read_exact(&mut body).unwrap();
                if sender.send(serde_json::from_slice(&body).unwrap()).is_err() {
                    break;
                }
            }
        });
        Self {
            process,
            input,
            output,
            reader: Some(reader),
            notifications: VecDeque::new(),
            next_id: 1,
        }
    }

    fn send_raw(&mut self, body: &[u8]) {
        let input = self.input.as_mut().unwrap();
        write!(input, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        input.write_all(body).unwrap();
        input.flush().unwrap();
    }

    fn send(&mut self, message: Value) {
        self.send_raw(&serde_json::to_vec(&message).unwrap());
    }

    fn receive(&self) -> Value {
        let response = self.output.recv_timeout(Duration::from_secs(10)).unwrap();
        assert_eq!(response["jsonrpc"], "2.0");
        response
    }

    fn request(&mut self, method: &str, params: Value) -> Value {
        let id = self.next_id;
        self.next_id += 1;
        self.send(json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params }));
        loop {
            let response = self.receive();
            if response.get("method").is_some() && response.get("id").is_none() {
                self.notifications.push_back(response);
            } else {
                assert_eq!(response["id"], id, "unexpected response: {response}");
                return response;
            }
        }
    }

    fn diagnostics(&mut self) -> Value {
        let notification = self
            .notifications
            .pop_front()
            .unwrap_or_else(|| self.receive());
        assert!(notification.get("id").is_none(), "{notification}");
        assert_eq!(notification["method"], "textDocument/publishDiagnostics");
        notification["params"].clone()
    }

    fn diagnostics_for(&mut self, uri: &str) -> Value {
        loop {
            let publication = self.diagnostics();
            if publication["uri"] == uri {
                return publication;
            }
        }
    }

    fn notify(&mut self, method: &str, params: Value) {
        self.send(json!({ "jsonrpc": "2.0", "method": method, "params": params }));
    }

    fn initialize(&mut self, capabilities: Value) -> Value {
        let response = self.request(
            "initialize",
            json!({ "processId": null, "capabilities": capabilities }),
        );
        assert!(response.get("error").is_none(), "{response}");
        self.notify("initialized", json!({}));
        response["result"].clone()
    }

    fn open(&mut self, uri: &str, text: &str) {
        self.notify(
            "textDocument/didOpen",
            json!({
                "textDocument": { "uri": uri, "languageId": "markdown", "version": 1, "text": text }
            }),
        );
    }

    fn wait_for_exit(&mut self) -> ExitStatus {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = self.process.try_wait().unwrap() {
                return status;
            }
            assert!(Instant::now() < deadline, "language server did not exit");
            thread::sleep(Duration::from_millis(10));
        }
    }

    fn shutdown(&mut self) {
        let response = self.request("shutdown", Value::Null);
        assert_eq!(response["result"], Value::Null);
        assert!(response.get("error").is_none());
        self.notify("exit", Value::Null);
        assert!(self.wait_for_exit().success());
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        self.input.take();
        let _ = self.process.kill();
        let _ = self.process.wait();
        if let Some(reader) = self.reader.take() {
            let result = reader.join();
            if !thread::panicking() {
                result.unwrap();
            }
        }
    }
}

fn document(uri: &str) -> Value {
    json!({ "textDocument": { "uri": uri } })
}

fn edit_text(source: &str, edits: &[Value]) -> String {
    fn offset(text: &str, position: &Value) -> usize {
        let mut start = 0;
        for _ in 0..position["line"].as_u64().unwrap() {
            start += text[start..].find('\n').unwrap() + 1;
        }
        let expected = position["character"].as_u64().unwrap() as usize;
        let mut units = 0;
        for (index, character) in text[start..].char_indices() {
            if units == expected {
                return start + index;
            }
            assert!(!matches!(character, '\r' | '\n'));
            units += character.len_utf16();
            assert!(units <= expected, "edit split a surrogate pair");
        }
        assert_eq!(units, expected);
        text.len()
    }
    let mut edits: Vec<_> = edits.iter().collect();
    edits.sort_by_key(|edit| {
        (
            edit["range"]["start"]["line"].as_u64().unwrap(),
            edit["range"]["start"]["character"].as_u64().unwrap(),
        )
    });
    let mut text = source.to_string();
    for edit in edits.into_iter().rev() {
        let start = offset(&text, &edit["range"]["start"]);
        let end = offset(&text, &edit["range"]["end"]);
        text.replace_range(start..end, edit["newText"].as_str().unwrap());
    }
    text
}

fn workspace_changes(edit: &Value) -> Vec<(&str, Option<i32>, &[Value])> {
    if let Some(documents) = edit["documentChanges"].as_array() {
        documents
            .iter()
            .map(|document| {
                (
                    document["textDocument"]["uri"].as_str().unwrap(),
                    document["textDocument"]["version"]
                        .as_i64()
                        .map(|version| version as i32),
                    document["edits"].as_array().unwrap().as_slice(),
                )
            })
            .collect()
    } else {
        edit["changes"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(uri, edits)| (uri.as_str(), None, edits.as_array().unwrap().as_slice()))
            .collect()
    }
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "yozora-lsp-stdio-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path.canonicalize().unwrap())
    }

    fn uri(&self, relative: &str) -> String {
        let path = self.0.join(relative).to_string_lossy().replace('\\', "/");
        let encoded: String = path
            .bytes()
            .map(|byte| {
                if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
                    (byte as char).to_string()
                } else {
                    format!("%{byte:02X}")
                }
            })
            .collect();
        format!(
            "file://{}{encoded}",
            if encoded.starts_with('/') { "" } else { "/" }
        )
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn acknowledged_event_caches_refresh_references_and_varied_refactors_over_stdio() {
    for accept_watch in [true, false] {
        let directory = TestDirectory::new();
        let target = directory.uri("target.md");
        let referrer = directory.uri("referrer.md");
        fs::write(directory.0.join("target.md"), "# Old\n").unwrap();
        fs::write(directory.0.join("referrer.md"), "[go](target.md#old)\n").unwrap();
        let mut client = Client::start();
        let initialized = client.request("initialize", json!({
            "rootUri": directory.uri(""),
            "initializationOptions": {"refactorFileEventCache": true},
            "capabilities": {"workspace": {
                "workspaceEdit": {"documentChanges": true},
                "didChangeWatchedFiles": {"dynamicRegistration": true, "relativePatternSupport": true}
            }}
        }));
        assert!(initialized.get("error").is_none(), "{initialized}");
        client.notify("initialized", json!({}));
        let watch = client.receive();
        assert_eq!(watch["method"], "client/registerCapability");
        client.send(if accept_watch {
            json!({"jsonrpc": "2.0", "id": watch["id"], "result": null})
        } else {
            json!({"jsonrpc": "2.0", "id": watch["id"], "error": {"code": -32601, "message": "not supported"}})
        });
        client.open(&target, "# Old\n");
        for (index, name) in ["First", "Second", "Third"].into_iter().enumerate() {
            let text = format!("{}[go](target.md#old)\n", "\n".repeat(index));
            fs::write(directory.0.join("referrer.md"), &text).unwrap();
            if accept_watch {
                client.notify(
                    "workspace/didChangeWatchedFiles",
                    json!({"changes": [{"uri": referrer, "type": 2}]}),
                );
            }
            let references = client.request(
                "textDocument/references",
                json!({
                    "textDocument": {"uri": target}, "position": {"line": 0, "character": 3},
                    "context": {"includeDeclaration": false}
                }),
            );
            assert!(references.get("error").is_none(), "{references}");
            assert_eq!(references["result"].as_array().unwrap().len(), 1);
            assert_eq!(references["result"][0]["uri"], referrer);
            assert_eq!(references["result"][0]["range"]["start"]["line"], index);
            let renamed = client.request("textDocument/rename", json!({
                "textDocument": {"uri": target}, "position": {"line": 0, "character": 3}, "newName": name
            }));
            assert!(renamed.get("error").is_none(), "{renamed}");
            let changes = renamed["result"]["documentChanges"].as_array().unwrap();
            assert_eq!(changes.len(), 2);
            let changed = changes
                .iter()
                .find(|change| change["textDocument"]["uri"] == referrer)
                .unwrap();
            assert_eq!(
                edit_text(&text, changed["edits"].as_array().unwrap()),
                text.replace("#old", &format!("#{}", name.to_lowercase()))
            );
        }
        client.shutdown();
    }
}

#[test]
fn workspace_symbols_refresh_disk_and_buffer_lifecycles_over_stdio() {
    let directory = TestDirectory::new();
    fs::create_dir(directory.0.join("docs")).unwrap();
    let relative = "docs/指南 %.MD";
    let uri = directory.uri(relative);
    fs::write(directory.0.join(relative), "# Disk 😀\r\n").unwrap();
    let mut client = Client::start();
    let initialized = client.request("initialize", json!({
        "capabilities": {}, "workspaceFolders": [{ "uri": directory.uri(""), "name": "workspace" }]
    }));
    assert_eq!(
        initialized["result"]["capabilities"]["workspaceSymbolProvider"],
        true
    );
    client.notify("initialized", json!({}));
    assert_eq!(
        client.request("workspace/symbol", json!({}))["error"]["code"],
        -32602
    );
    let result = client.request("workspace/symbol", json!({ "query": "dISK" }));
    assert_eq!(result["result"].as_array().unwrap().len(), 1);
    assert_eq!(result["result"][0]["name"], "Disk 😀");
    assert_eq!(
        result["result"][0]["location"],
        json!({
            "uri": uri, "range": { "start": { "line": 0, "character": 2 }, "end": { "line": 0, "character": 9 } }
        })
    );
    client.open(&uri, "# Unsaved 😀");
    assert_eq!(
        client.request("workspace/symbol", json!({ "query": "Disk" }))["result"],
        json!([])
    );
    assert_eq!(
        client.request("workspace/symbol", json!({ "query": "" }))["result"][0]["name"],
        "Unsaved 😀"
    );
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 2 }, "contentChanges": [{ "text": "# New buffer" }]
    }));
    assert_eq!(
        client.request("workspace/symbol", json!({ "query": "new" }))["result"][0]["name"],
        "New buffer"
    );
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 3 }, "contentChanges": null
        }),
    );
    assert_eq!(
        client.request("workspace/symbol", json!({ "query": "" }))["error"]["code"],
        -32801
    );
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 4 }, "contentChanges": [{ "text": "# Recovered" }]
    }));
    assert_eq!(
        client.request("workspace/symbol", json!({ "query": "" }))["result"][0]["name"],
        "Recovered"
    );
    client.notify("textDocument/didClose", document(&uri));
    assert_eq!(
        client.request("workspace/symbol", json!({ "query": "" }))["result"][0]["name"],
        "Disk 😀"
    );

    // Disk refresh does not rely on watched-file notifications or mtime precision.
    fs::write(directory.0.join(relative), "# Note 😀\r\n").unwrap();
    assert_eq!(
        client.request("workspace/symbol", json!({ "query": "note" }))["result"][0]["name"],
        "Note 😀"
    );
    fs::write(directory.0.join("created.yozora"), "# Created").unwrap();
    assert_eq!(
        client.request("workspace/symbol", json!({ "query": "created" }))["result"][0]["location"]
            ["uri"],
        directory.uri("created.yozora")
    );
    fs::rename(
        directory.0.join("created.yozora"),
        directory.0.join("moved.markdown"),
    )
    .unwrap();
    let moved = client.request("workspace/symbol", json!({ "query": "created" }));
    assert_eq!(moved["result"].as_array().unwrap().len(), 1);
    assert_eq!(
        moved["result"][0]["location"]["uri"],
        directory.uri("moved.markdown")
    );
    fs::remove_file(directory.0.join("moved.markdown")).unwrap();
    assert_eq!(
        client.request("workspace/symbol", json!({ "query": "created" }))["result"],
        json!([])
    );
    client.notify(
        "workspace/didChangeWorkspaceFolders",
        json!({ "event": {
            "added": [], "removed": [{ "uri": directory.uri(""), "name": "workspace" }]
        }}),
    );
    assert_eq!(
        client.request("workspace/symbol", json!({ "query": "" }))["result"],
        json!([])
    );
    client.shutdown();
}

#[test]
fn heading_and_file_renames_round_trip_through_both_workspace_edit_formats() {
    for versioned in [false, true] {
        let directory = TestDirectory::new();
        let uri = directory.uri("guide 中文.md");
        let referrer_uri = directory.uri("source.md");
        let mut target_text =
            "# Intro 😀\r\n\r\n## Intro 😀\r\n\r\n[peer](peer.md#peer)\r\n".to_string();
        let mut referrer_text = "😀 [first](guide%20%E4%B8%AD%E6%96%87.md#intro-%F0%9F%98%80 \"Title\")\r\n[second](guide%20%E4%B8%AD%E6%96%87.md#intro-%F0%9F%98%80-2)\r\n".to_string();
        fs::write(directory.0.join("guide 中文.md"), "# Stale disk title").unwrap();
        fs::write(directory.0.join("source.md"), &referrer_text).unwrap();
        fs::write(directory.0.join("peer.md"), "# Peer").unwrap();
        let mut client = Client::start();
        let initialized = client.request("initialize", json!({
            "rootUri": directory.uri(""), "capabilities": { "workspace": { "workspaceEdit": { "documentChanges": versioned } } }
        }));
        assert_eq!(
            initialized["result"]["capabilities"]["workspace"]["fileOperations"]["willRename"]
                ["filters"][0]["pattern"]["matches"],
            "file"
        );
        client.notify("initialized", json!({}));
        client.open(&uri, &target_text);
        let params =
            json!({ "textDocument": { "uri": uri }, "position": { "line": 0, "character": 3 } });
        let prepared = client.request("textDocument/prepareRename", params.clone());
        assert_eq!(prepared["result"]["placeholder"], "Intro 😀");
        assert_eq!(prepared["result"]["range"]["end"]["character"], 10);
        let mut params = params;
        params["newName"] = json!("重命名 🦀");
        let response = client.request("textDocument/rename", params);
        assert!(response.get("error").is_none(), "{response}");
        let changes = workspace_changes(&response["result"]);
        assert_eq!(changes.len(), 2);
        for (changed_uri, version, edits) in changes {
            if changed_uri == uri {
                assert_eq!(version, versioned.then_some(1));
                target_text = edit_text(&target_text, edits);
                client.notify("textDocument/didChange", json!({
                    "textDocument": { "uri": uri, "version": 2 },
                    "contentChanges": edits.iter().rev().map(|edit| json!({ "range": edit["range"], "text": edit["newText"] })).collect::<Vec<_>>()
                }));
            } else {
                assert_eq!(changed_uri, referrer_uri);
                assert!(
                    version.is_none(),
                    "closed file edits must use a null version"
                );
                referrer_text = edit_text(&referrer_text, edits);
                fs::write(directory.0.join("source.md"), &referrer_text).unwrap();
            }
        }
        assert!(target_text.starts_with("# 重命名 🦀\r\n"));
        assert!(referrer_text.contains("#%E9%87%8D%E5%91%BD%E5%90%8D-%F0%9F%A6%80"));
        assert!(!referrer_text.contains("%F0%9F%98%80-2"));
        client.open(&referrer_uri, &referrer_text);
        for (line, character, target_line) in [(0, 5, 0), (1, 3, 2)] {
            let definition = client.request("textDocument/definition", json!({
                "textDocument": { "uri": referrer_uri }, "position": { "line": line, "character": character }
            }));
            assert_eq!(definition["result"]["uri"], uri);
            assert_eq!(definition["result"]["range"]["start"]["line"], target_line);
        }

        let moved_uri = directory.uri("docs/moved 中文.md");
        let files = json!({ "files": [{ "oldUri": uri, "newUri": moved_uri }] });
        let response = client.request("workspace/willRenameFiles", files.clone());
        assert!(response.get("error").is_none(), "{response}");
        for (changed_uri, version, edits) in workspace_changes(&response["result"]) {
            let next_version = if changed_uri == uri {
                assert_eq!(version, versioned.then_some(2));
                target_text = edit_text(&target_text, edits);
                3
            } else {
                assert_eq!(changed_uri, referrer_uri);
                assert_eq!(version, versioned.then_some(1));
                referrer_text = edit_text(&referrer_text, edits);
                2
            };
            client.notify("textDocument/didChange", json!({
                "textDocument": { "uri": changed_uri, "version": next_version },
                "contentChanges": edits.iter().rev().map(|edit| json!({ "range": edit["range"], "text": edit["newText"] })).collect::<Vec<_>>()
            }));
        }
        assert!(target_text.contains("[peer](../peer.md#peer)"));
        assert!(referrer_text.contains("docs/moved%20%E4%B8%AD%E6%96%87.md#"));
        client.notify("textDocument/didClose", document(&uri));
        fs::write(directory.0.join("guide 中文.md"), &target_text).unwrap();
        fs::create_dir(directory.0.join("docs")).unwrap();
        fs::rename(
            directory.0.join("guide 中文.md"),
            directory.0.join("docs/moved 中文.md"),
        )
        .unwrap();
        client.notify("workspace/didRenameFiles", files);
        client.open(&moved_uri, &target_text);
        let definition = client.request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": referrer_uri }, "position": { "line": 0, "character": 5 }
            }),
        );
        assert_eq!(definition["result"]["uri"], moved_uri);
        assert_eq!(definition["result"]["range"]["start"]["line"], 0);
        let symbols = client.request("workspace/symbol", json!({ "query": "重命名" }));
        assert_eq!(symbols["result"].as_array().unwrap().len(), 1);
        assert_eq!(symbols["result"][0]["location"]["uri"], moved_uri);
        client.shutdown();
    }
}

#[test]
fn deeply_nested_images_survive_automatic_worker_diagnostics_and_later_edits() {
    let mut client = Client::start();
    client.initialize(json!({}));
    let uri = "untitled:deep-images";
    let source = format!("{}x{}", "![".repeat(4_000), "](/url)".repeat(4_000));
    client.open(uri, &source);
    let diagnostics = client.diagnostics();
    assert_eq!(diagnostics["uri"], uri);
    assert_eq!(diagnostics["diagnostics"], json!([]));
    let response = client.request("textDocument/documentSymbol", document(uri));
    assert_eq!(response["result"], json!([]));
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 2 },
            "contentChanges": [{ "text": "# Recovered" }]
        }),
    );
    let response = client.request("textDocument/documentSymbol", document(uri));
    assert_eq!(response["result"][0]["name"], "Recovered");
    client.shutdown();
}

#[test]
fn rename_round_trips_through_a_large_incremental_edit_batch() {
    let mut client = Client::start();
    client.initialize(json!({}));
    let uri = "untitled:bulk-rename";
    let count = 2_000;
    client.open(
        uri,
        &format!("{}\n[old]: /url", "[😀][old]\r\n".repeat(count)),
    );
    let params = json!({
        "textDocument": { "uri": uri },
        "position": { "line": 0, "character": 6 },
        "newName": "renamed",
    });
    let response = client.request("textDocument/rename", params);
    let edits = response["result"]["changes"][uri].as_array().unwrap();
    assert_eq!(edits.len(), count + 1);
    let changes: Vec<_> = edits
        .iter()
        .rev()
        .map(|edit| {
            json!({
                "range": edit["range"], "text": edit["newText"]
            })
        })
        .collect();
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 2 }, "contentChanges": changes
        }),
    );
    let response = client.request(
        "textDocument/prepareRename",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 0, "character": 6 }
        }),
    );
    assert_eq!(response["result"]["placeholder"], "renamed");
    let response = client.request(
        "textDocument/references",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 0, "character": 6 },
            "context": { "includeDeclaration": true }
        }),
    );
    let locations = response["result"].as_array().unwrap();
    assert_eq!(locations.len(), count + 1);
    assert_eq!(
        locations.last().unwrap()["range"]["start"]["line"],
        count + 1
    );
    client.shutdown();
}

#[test]
fn file_and_anchor_completion_follow_unsaved_target_edits_over_stdio() {
    let directory = std::env::temp_dir().join(format!(
        "yozora-lsp-stdio-completion-{}",
        std::process::id()
    ));
    let path = directory.to_string_lossy().replace('\\', "/");
    let encoded: String = path
        .bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b"/-._~:".contains(&byte) {
                (byte as char).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect();
    let root = format!(
        "file://{}{encoded}",
        if encoded.starts_with('/') { "" } else { "/" }
    );
    let uri = format!("{root}/source.md");
    let filename = "guide%20%E4%B8%AD%E6%96%87.md";
    let target = format!("{root}/{filename}");
    let mut client = Client::start();
    let initialized = client.request("initialize", json!({ "capabilities": {}, "rootUri": root }));
    assert!(initialized.get("error").is_none(), "{initialized}");
    client.notify("initialized", json!({}));
    client.open(&target, "# Intro\r\n\r\n# 中文😀\r\n");
    client.open(&uri, "[go](gu)");
    let at = |character| json!({ "textDocument": { "uri": uri }, "position": { "line": 0, "character": character } });
    let completion = client.request("textDocument/completion", at(7));
    let items = completion["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["label"], "guide 中文.md");
    assert_eq!(items[0]["textEdit"]["newText"], filename);
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [{ "range": items[0]["textEdit"]["range"], "text": items[0]["textEdit"]["newText"] }]
    }));
    assert_eq!(
        client.request("textDocument/definition", at(2))["result"]["uri"],
        target
    );

    let text = format!("[go]({filename}#%E4)");
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 3 }, "contentChanges": [{ "text": text }]
        }),
    );
    let completion = client.request("textDocument/completion", at(text.len() - 1));
    let items = completion["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["label"], "中文😀");
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 4 },
        "contentChanges": [{ "range": items[0]["textEdit"]["range"], "text": items[0]["textEdit"]["newText"] }]
    }));
    let definition = client.request("textDocument/definition", at(2));
    assert_eq!(definition["result"]["uri"], target);
    assert_eq!(
        definition["result"]["range"],
        json!({ "start": { "line": 2, "character": 2 }, "end": { "line": 2, "character": 6 } })
    );

    let text = format!("[go]({filename}#)");
    client.notify("textDocument/didChange", json!({ "textDocument": { "uri": uri, "version": 5 }, "contentChanges": [{ "text": text }] }));
    client.notify("textDocument/didChange", json!({ "textDocument": { "uri": target, "version": 2 }, "contentChanges": [{ "text": "# Modified" }] }));
    let completion = client.request("textDocument/completion", at(text.len() - 1));
    assert_eq!(completion["result"]["items"][0]["label"], "modified");
    client.notify(
        "textDocument/didChange",
        json!({ "textDocument": { "uri": target, "version": 3 } }),
    );
    assert_eq!(
        client.request("textDocument/completion", at(text.len() - 1))["error"]["code"],
        -32801
    );
    client.notify("textDocument/didChange", json!({ "textDocument": { "uri": target, "version": 4 }, "contentChanges": [{ "text": "# Restored" }] }));
    assert_eq!(
        client.request("textDocument/completion", at(text.len() - 1))["result"]["items"][0]
            ["label"],
        "restored"
    );
    client.notify("textDocument/didClose", document(&target));
    assert_eq!(
        client.request("textDocument/completion", at(text.len() - 1))["result"]["items"],
        json!([])
    );
    client.shutdown();
}

#[test]
fn anchor_completion_uses_cr_line_positions_and_preserves_the_closing_delimiter() {
    let mut client = Client::start();
    client.initialize(json!({}));
    let uri = "untitled:cr-anchors";
    client.open(uri, "# Intro\r# Intro\r\r[go](#in)\r");
    let result = client.request(
        "textDocument/completion",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 3, "character": 8 }
        }),
    );
    let item = &result["result"]["items"][1];
    assert_eq!(item["label"], "intro-2");
    assert_eq!(
        item["textEdit"],
        json!({
            "range": { "start": { "line": 3, "character": 5 }, "end": { "line": 3, "character": 8 } }, "newText": "#intro-2"
        })
    );
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [{ "range": item["textEdit"]["range"], "text": item["textEdit"]["newText"] }]
    }));
    let definition = client.request(
        "textDocument/definition",
        json!({ "textDocument": { "uri": uri }, "position": { "line": 3, "character": 2 } }),
    );
    assert_eq!(definition["result"]["range"]["start"]["line"], 1);
    client.shutdown();
}

#[test]
fn heading_navigation_and_document_links_follow_utf16_edits_over_stdio() {
    let mut client = Client::start();
    let initialized = client.request(
        "initialize",
        json!({
            "capabilities": {}, "initializationOptions": { "headingIdPrefix": "h-" }
        }),
    );
    assert!(initialized.get("error").is_none(), "{initialized}");
    assert_eq!(
        initialized["result"]["capabilities"]["documentLinkProvider"],
        json!({ "resolveProvider": false })
    );
    client.notify("initialized", json!({}));
    let uri = "untitled:heading-links";
    client.open(uri, "# 中文😀\r\n# 中文😀\r\n\r\n[go](#h-%E4%B8%AD%E6%96%87%F0%9F%98%80-2)\r\n[ref][label]\r\n\r\n[label]: #h-中文😀\r\n\r\n<https://example.test/docs>\r\n");
    let at = |line| json!({ "textDocument": { "uri": uri }, "position": { "line": line, "character": 2 } });
    let definition = client.request("textDocument/definition", at(3));
    assert_eq!(
        definition["result"],
        json!({
            "uri": uri, "range": { "start": { "line": 1, "character": 2 }, "end": { "line": 1, "character": 6 } }
        })
    );
    let reference = client.request("textDocument/definition", at(4));
    assert_eq!(reference["result"]["range"]["start"]["line"], 6);
    let declaration = client.request("textDocument/definition", at(6));
    assert_eq!(declaration["result"]["range"]["start"]["line"], 0);
    let links = client.request("textDocument/documentLink", document(uri));
    let links = links["result"].as_array().unwrap();
    assert_eq!(links.len(), 4);
    assert_eq!(
        links[0]["target"],
        format!("{uri}#h-%E4%B8%AD%E6%96%87%F0%9F%98%80-2")
    );
    assert_eq!(links[1]["target"], links[2]["target"]);
    assert_eq!(links[3]["target"], "https://example.test/docs");

    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [{ "range": { "start": { "line": 0, "character": 2 }, "end": { "line": 0, "character": 6 } }, "text": "Changed" }]
    }));
    assert_eq!(
        client.request("textDocument/definition", at(3))["result"],
        Value::Null
    );
    assert_eq!(
        client.request("textDocument/definition", at(6))["result"]["range"]["start"]["line"],
        1
    );
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 3 },
        "contentChanges": [{ "range": { "start": { "line": 1, "character": 5 }, "end": { "line": 1, "character": 6 } }, "text": "x" }]
    }));
    assert_eq!(
        client.request("textDocument/documentLink", document(uri))["error"]["code"],
        -32801
    );
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 4 }, "contentChanges": [{ "text": "# Restored\n" }]
    }));
    assert_eq!(
        client.request("textDocument/documentLink", document(uri))["result"],
        json!([])
    );
    client.shutdown();
}

#[test]
fn link_diagnostics_follow_target_lifecycle_without_source_edits() {
    let directory = TestDirectory::new();
    let source = directory.uri("source.md");
    let target = directory.uri("target.md");
    let mut client = Client::start();
    let initialized = client.request(
        "initialize",
        json!({
            "rootUri": directory.uri(""),
            "capabilities": { "textDocument": { "publishDiagnostics": { "versionSupport": true } } }
        }),
    );
    assert!(initialized.get("error").is_none(), "{initialized}");
    client.notify("initialized", json!({}));
    client.open(
        &source,
        "😀 [go](target.md#intro)\n\n[ref]: https://example.test/one\n[REF]: https://example.test/two\n",
    );
    let expect = |client: &mut Client, link_code: Option<&str>| {
        let publication = client.diagnostics_for(&source);
        assert_eq!(publication["version"], 1, "source version must not change");
        let diagnostics = publication["diagnostics"].as_array().unwrap();
        let codes: Vec<_> = diagnostics
            .iter()
            .map(|diagnostic| diagnostic["code"].as_str().unwrap())
            .collect();
        let expected: Vec<_> = link_code
            .into_iter()
            .chain(["duplicate-link-definition"])
            .collect();
        assert_eq!(codes, expected, "{publication}");
        if link_code.is_some() {
            assert_eq!(
                diagnostics[0]["range"]["start"],
                json!({ "line": 0, "character": 3 })
            );
        }
    };
    expect(&mut client, Some("missing-file"));
    client.open(&target, "# Other");
    expect(&mut client, Some("missing-anchor"));
    for (version, text, issue) in [(2, "# Intro", None), (3, "# Other", Some("missing-anchor"))] {
        client.notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": target, "version": version },
                "contentChanges": [{ "text": text }]
            }),
        );
        expect(&mut client, issue);
    }
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": target, "version": 4 },
        "contentChanges": [{ "range": { "start": { "line": 0, "character": 0 } }, "text": "broken" }]
    }));
    expect(&mut client, None);
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": target, "version": 5 },
            "contentChanges": [{ "text": "# Intro" }]
        }),
    );
    expect(&mut client, None);
    client.notify("textDocument/didClose", document(&target));
    expect(&mut client, Some("missing-file"));
    for (kind, text, issue) in [
        (1, Some("# Other"), Some("missing-anchor")),
        (2, Some("# Intro"), None),
        (3, None, Some("missing-file")),
    ] {
        if let Some(text) = text {
            fs::write(directory.0.join("target.md"), text).unwrap();
        } else {
            fs::remove_file(directory.0.join("target.md")).unwrap();
        }
        client.notify(
            "workspace/didChangeWatchedFiles",
            json!({
                "changes": [{ "uri": target, "type": kind }]
            }),
        );
        expect(&mut client, issue);
    }
    client.shutdown();
}

#[test]
fn publishes_duplicate_diagnostics_on_open_edit_close_and_reopen() {
    let mut client = Client::start();
    client.initialize(json!({ "textDocument": { "publishDiagnostics": {
        "versionSupport": true, "relatedInformation": true
    } } }));
    let uri = "untitled:diagnostics";
    let text = "[ref]: /first\r\n[REF]: /second\r\n[ref]: /third\r\n\r\n[^ref]: first note\r\n\r\n[^REF]: second note\r\n";
    client.open(uri, text);
    // No request or save is needed to trigger diagnostics.
    let published = client.diagnostics();
    assert_eq!(published["uri"], uri);
    assert_eq!(published["version"], 1);
    let diagnostics = published["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 3);
    for (diagnostic, line, code, first_line) in [
        (&diagnostics[0], 1, "duplicate-link-definition", 0),
        (&diagnostics[1], 2, "duplicate-link-definition", 0),
        (&diagnostics[2], 6, "duplicate-footnote-definition", 4),
    ] {
        assert_eq!(diagnostic["range"]["start"]["line"], line);
        assert_eq!(diagnostic["severity"], 2);
        assert_eq!(diagnostic["code"], code);
        assert_eq!(diagnostic["source"], "yozora-lsp");
        assert!(diagnostic["message"]
            .as_str()
            .unwrap()
            .contains("first definition"));
        let related = diagnostic["relatedInformation"].as_array().unwrap();
        assert_eq!(related.len(), 1);
        assert_eq!(related[0]["location"]["uri"], uri);
        assert_eq!(related[0]["location"]["range"]["start"]["line"], first_line);
    }

    client.open("untitled:other-diagnostics", "[ref]: /other");
    // Opening a buffer also refreshes sources whose local target selection
    // could change. The two workers may publish these documents in either order.
    let mut publications = [client.diagnostics(), client.diagnostics()];
    publications.sort_by(|left, right| left["uri"].as_str().cmp(&right["uri"].as_str()));
    assert_eq!(publications[0], published);
    assert_eq!(publications[1]["uri"], "untitled:other-diagnostics");
    assert_eq!(publications[1]["diagnostics"], json!([]));

    // Removing the active definition promotes the next declaration.
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [{
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } },
            "text": ""
        }]
    }));
    let changed = client.diagnostics();
    assert_eq!(changed["uri"], uri);
    assert_eq!(changed["version"], 2);
    let diagnostics = changed["diagnostics"].as_array().unwrap();
    assert_eq!(diagnostics.len(), 2);
    assert_eq!(diagnostics[0]["range"]["start"]["line"], 1);
    assert_eq!(
        diagnostics[0]["relatedInformation"][0]["location"]["range"],
        json!({
            "start": { "line": 0, "character": 0 },
            "end": { "line": 1, "character": 0 }
        })
    );
    assert_eq!(diagnostics[1]["range"]["start"]["line"], 5);
    assert_eq!(
        diagnostics[1]["relatedInformation"][0]["location"]["range"]["start"]["line"],
        3
    );

    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 3 },
        "contentChanges": [
            { "range": { "start": { "line": 1, "character": 1 }, "end": { "line": 1, "character": 4 } }, "text": "new😀" },
            { "range": { "start": { "line": 5, "character": 2 }, "end": { "line": 5, "character": 5 } }, "text": "note" }
        ]
    }));
    let fixed = client.diagnostics();
    assert_eq!(fixed["version"], 3);
    assert_eq!(fixed["diagnostics"], json!([]));

    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 4 },
            "contentChanges": [{ "text": text }]
        }),
    );
    let restored = client.diagnostics();
    assert_eq!(restored["version"], 4);
    assert_eq!(restored["diagnostics"], published["diagnostics"]);

    client.notify("textDocument/didClose", document(uri));
    let closed = client.diagnostics_for(uri);
    assert_eq!(closed, json!({ "uri": uri, "diagnostics": [] }));
    client.open(uri, text);
    assert_eq!(client.diagnostics_for(uri), published);
    client.shutdown();
}

#[test]
fn retracts_diagnostics_for_failed_newer_edits_and_ignores_stale_versions() {
    let mut client = Client::start();
    client.initialize(
        json!({ "textDocument": { "publishDiagnostics": { "versionSupport": true } } }),
    );
    let uri = "untitled:diagnostic-sync";
    let text = "[中😀]: /one\n[中😀]: /two\n";
    client.open(uri, text);
    let initial = client.diagnostics();
    assert_eq!(initial["diagnostics"].as_array().unwrap().len(), 1);
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [
            { "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } }, "text": "#" },
            { "range": { "start": { "line": 0, "character": 4 }, "end": { "line": 0, "character": 5 } }, "text": "broken" }
        ]
    }));
    assert_eq!(
        client.diagnostics(),
        json!({ "uri": uri, "version": 2, "diagnostics": [] })
    );
    assert_eq!(
        client.request("textDocument/documentSymbol", document(uri))["error"]["code"],
        -32801
    );
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 3 },
        "contentChanges": [{
            "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } },
            "text": "#"
        }]
    }));
    assert_eq!(
        client.diagnostics(),
        json!({ "uri": uri, "version": 3, "diagnostics": [] })
    );

    // A stale full replacement cannot restore sync or publish another snapshot.
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 2 },
            "contentChanges": [{ "text": text }]
        }),
    );
    assert_eq!(
        client.request("textDocument/documentSymbol", document(uri))["error"]["code"],
        -32801
    );
    assert!(client.notifications.is_empty());

    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 4 },
            "contentChanges": [{ "text": text }]
        }),
    );
    let restored = client.diagnostics();
    assert_eq!(restored["version"], 4);
    assert_eq!(restored["diagnostics"], initial["diagnostics"]);
    for version in [3, 4] {
        client.notify(
            "textDocument/didChange",
            json!({
                "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": "# stale" }]
            }),
        );
    }
    assert_eq!(
        client.request("textDocument/documentSymbol", document(uri))["result"],
        json!([])
    );
    assert!(client.notifications.is_empty());
    client.shutdown();
}

#[test]
fn publishes_only_supported_diagnostic_metadata() {
    for (capabilities, versioned, related) in [
        (json!({}), false, false),
        (
            json!({ "textDocument": { "publishDiagnostics": { "versionSupport": true } } }),
            true,
            false,
        ),
        (
            json!({ "textDocument": { "publishDiagnostics": { "relatedInformation": true } } }),
            false,
            true,
        ),
    ] {
        let mut client = Client::start();
        client.initialize(capabilities);
        let uri = "untitled:diagnostic-capabilities";
        client.open(uri, "[label]: /first\n[LABEL]: /second\n");
        client.request("textDocument/documentSymbol", document(uri));
        let published = client.diagnostics();
        assert_eq!(published.get("version").is_some(), versioned);
        let diagnostics = published["diagnostics"].as_array().unwrap();
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].get("relatedInformation").is_some(), related);
        client.notify("textDocument/didClose", document(uri));
        // Immediate clears must still be retained when they precede a response.
        assert_eq!(
            client.request("textDocument/documentSymbol", document(uri))["error"]["code"],
            -32602
        );
        assert_eq!(client.notifications.len(), 1);
        assert_eq!(
            client.diagnostics(),
            json!({ "uri": uri, "diagnostics": [] })
        );
        client.shutdown();
    }
}

#[test]
fn malformed_newer_edits_retract_diagnostics_and_block_stale_queries() {
    let mut client = Client::start();
    client.initialize(
        json!({ "textDocument": { "publishDiagnostics": { "versionSupport": true } } }),
    );
    let uri = "untitled:malformed-edits";
    let source = "# Before\n\n[ref]: /first\n[REF]: /second\n";
    client.open(uri, source);
    let initial = client.diagnostics();
    assert_eq!(initial["diagnostics"].as_array().unwrap().len(), 1);
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 2 },
            "contentChanges": [
                { "text": "# Partial" },
                { "range": { "start": { "line": 0, "character": 0 } }, "text": "missing end" }
            ]
        }),
    );
    assert_eq!(
        client.diagnostics(),
        json!({ "uri": uri, "version": 2, "diagnostics": [] })
    );
    for method in [
        "textDocument/documentSymbol",
        "textDocument/hover",
        "textDocument/completion",
        "textDocument/rename",
    ] {
        let response = client.request(method, json!({
            "textDocument": { "uri": uri }, "position": { "line": 2, "character": 2 }, "newName": "new"
        }));
        assert_eq!(response["error"]["code"], -32801);
    }
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 3 }, "contentChanges": [{ "text": source }]
        }),
    );
    let restored = client.diagnostics();
    assert_eq!(restored["version"], 3);
    assert_eq!(restored["diagnostics"], initial["diagnostics"]);
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 2 }, "contentChanges": [{ "text": 42 }]
        }),
    );
    assert_eq!(
        client.request("textDocument/documentSymbol", document(uri))["result"][0]["name"],
        "Before"
    );
    assert!(client.notifications.is_empty());
    client.shutdown();
}

#[test]
fn table_completion_preserves_neighboring_cells_and_resolves_escaped_labels() {
    let mut client = Client::start();
    client.initialize(json!({}));
    let uri = "untitled:table-completion";
    client.open(
        uri,
        "| A | B |\n| --- | --- |\n| 😀 [shown][a | later ] |\n\n[a|b]: /url\n",
    );
    let response = client.request(
        "textDocument/completion",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 2, "character": 14 }
        }),
    );
    let items = response["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["label"], "a|b");
    assert_eq!(
        items[0]["textEdit"],
        json!({
            "range": { "start": { "line": 2, "character": 13 }, "end": { "line": 2, "character": 14 } },
            "newText": "a\\|b]"
        })
    );
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [{ "range": items[0]["textEdit"]["range"], "text": items[0]["textEdit"]["newText"] }]
    }));
    let definition = client.request(
        "textDocument/definition",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 2, "character": 7 }
        }),
    );
    assert_eq!(definition["result"]["range"]["start"]["line"], 4);
    client.shutdown();
}

#[test]
fn completion_rechecks_multiline_context_after_incremental_edits() {
    let mut client = Client::start();
    client.initialize(json!({}));
    let uri = "untitled:completion-context";
    client.open(
        uri,
        "# Intro\r\n\r\n> 😀 [shown][a]\r\n> later `\r\n\r\n[a`b]: /url\r\n[another]: /safe",
    );
    let params = json!({
        "textDocument": { "uri": uri },
        "position": { "line": 2, "character": 14 }
    });
    let initial = client.request("textDocument/completion", params.clone());
    assert_eq!(initial["result"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(initial["result"]["items"][0]["label"], "another");

    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [{
            "range": { "start": { "line": 3, "character": 8 }, "end": { "line": 3, "character": 9 } },
            "text": ""
        }]
    }));
    let refreshed = client.request("textDocument/completion", params);
    let item = &refreshed["result"]["items"][0];
    assert_eq!(item["label"], "a`b");
    assert_eq!(
        item["textEdit"],
        json!({
            "range": { "start": { "line": 2, "character": 13 }, "end": { "line": 2, "character": 14 } },
            "newText": "a`b"
        })
    );
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 3 },
        "contentChanges": [{ "range": item["textEdit"]["range"], "text": item["textEdit"]["newText"] }]
    }));
    let target = client.request(
        "textDocument/definition",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 2, "character": 6 }
        }),
    );
    assert_eq!(
        target["result"]["range"]["start"],
        json!({ "line": 5, "character": 0 })
    );
    client.shutdown();
}

#[test]
fn coalesces_queued_edits_and_serves_a_query_against_the_latest_version() {
    let mut client = Client::start();
    client.initialize(
        json!({ "textDocument": { "publishDiagnostics": { "versionSupport": true } } }),
    );
    let uri = "untitled:queued-edits";
    let mut messages = vec![json!({
        "jsonrpc": "2.0", "method": "textDocument/didOpen",
        "params": { "textDocument": { "uri": uri, "version": 1,
            "text": "# Initial\n\n[ref]: /one\n[REF]: /two\n" } }
    })];
    for version in 2..=10 {
        messages.push(json!({
            "jsonrpc": "2.0", "method": "textDocument/didChange",
            "params": { "textDocument": { "uri": uri, "version": version },
                "contentChanges": [{ "text": format!("# Version {version}😀\n") }] }
        }));
    }
    let mut frames = Vec::new();
    for message in messages {
        let body = serde_json::to_vec(&message).unwrap();
        write!(frames, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
        frames.extend_from_slice(&body);
    }
    let input = client.input.as_mut().unwrap();
    input.write_all(&frames).unwrap();
    input.flush().unwrap();
    let symbols = client.request("textDocument/documentSymbol", document(uri));
    assert_eq!(symbols["result"][0]["name"], "Version 10😀");
    assert_eq!(
        client.diagnostics(),
        json!({ "uri": uri, "version": 10, "diagnostics": [] })
    );
    client.notify("textDocument/didClose", document(uri));
    assert_eq!(
        client.diagnostics(),
        json!({ "uri": uri, "diagnostics": [] })
    );
    client.shutdown();
}

#[test]
fn publishes_diagnostics_while_waiting_for_the_rest_of_an_input_frame() {
    let mut client = Client::start();
    client.initialize(json!({}));
    let uri = "untitled:partial-frame";
    client.open(uri, "[ref]: /one\n[REF]: /two\n");
    let body = serde_json::to_vec(&json!({
        "jsonrpc": "2.0", "id": "partial", "method": "unknown"
    }))
    .unwrap();
    let input = client.input.as_mut().unwrap();
    write!(input, "Content-Length: {}\r\n\r\n", body.len()).unwrap();
    input.write_all(&body[..5]).unwrap();
    input.flush().unwrap();
    let published = client.diagnostics();
    assert_eq!(published["uri"], uri);
    assert_eq!(published["diagnostics"].as_array().unwrap().len(), 1);
    let input = client.input.as_mut().unwrap();
    input.write_all(&body[5..]).unwrap();
    input.flush().unwrap();
    let response = client.receive();
    assert_eq!(response["id"], "partial");
    assert_eq!(response["error"]["code"], -32601);
    client.shutdown();
}

#[test]
fn exits_on_eof_and_propagates_truncated_frames_from_the_reader() {
    for trailing_bytes in [b"".as_slice(), b"Content-Length: 10\r\n\r\n{"] {
        let mut client = Client::start();
        client.initialize(json!({}));
        assert!(client
            .request("shutdown", Value::Null)
            .get("error")
            .is_none());
        let input = client.input.as_mut().unwrap();
        input.write_all(trailing_bytes).unwrap();
        input.flush().unwrap();
        client.input.take();
        assert_eq!(client.wait_for_exit().success(), trailing_bytes.is_empty());
    }
}

#[test]
fn edits_open_documents_and_updates_symbols_folds_and_definitions() {
    let mut client = Client::start();
    let result = client.initialize(json!({
        "general": { "positionEncodings": ["utf-8", "utf-16"] },
        "textDocument": { "documentSymbol": { "hierarchicalDocumentSymbolSupport": true } }
    }));
    assert_eq!(result["capabilities"]["positionEncoding"], "utf-16");
    assert_eq!(
        result["capabilities"]["textDocumentSync"],
        json!({ "openClose": true, "change": 2 })
    );
    for capability in [
        "documentSymbolProvider",
        "foldingRangeProvider",
        "definitionProvider",
        "hoverProvider",
        "referencesProvider",
    ] {
        assert_eq!(result["capabilities"][capability], true);
    }
    assert_eq!(
        result["capabilities"]["completionProvider"],
        json!({
            "triggerCharacters": ["[", "^", "(", "/", "#"], "resolveProvider": false
        })
    );
    assert_eq!(
        result["capabilities"]["renameProvider"],
        json!({ "prepareProvider": true })
    );
    let uri = "file:///workspace/note.md";
    client.open(uri, "# 中文😀\r\nintro\r\n## Child\r\n[guide][REF] and [^n]\r\n\r\n[ref]: /docs\r\n[^n]: note\r\n");
    client.open("untitled:other", "# Other");
    let symbols = client.request("textDocument/documentSymbol", document(uri));
    assert_eq!(symbols["result"][0]["name"], "中文😀");
    assert_eq!(
        symbols["result"][0]["selectionRange"]["end"],
        json!({ "line": 0, "character": 6 })
    );
    assert_eq!(symbols["result"][0]["children"][0]["name"], "Child");
    let folds = client.request("textDocument/foldingRange", document(uri));
    assert_eq!(
        folds["result"],
        json!([
            { "startLine": 0, "endLine": 6 },
            { "startLine": 2, "endLine": 6 }
        ])
    );
    for (character, target_line) in [(3, 5), (19, 6)] {
        let target = client.request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri }, "position": { "line": 3, "character": character }
            }),
        );
        assert_eq!(target["result"]["uri"], uri);
        assert_eq!(target["result"]["range"]["start"]["line"], target_line);
    }

    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [
            { "range": { "start": { "line": 0, "character": 2 }, "end": { "line": 0, "character": 6 } }, "text": "Changed" },
            { "range": { "start": { "line": 5, "character": 0 }, "end": { "line": 5, "character": 0 } }, "text": "\r\n" }
        ]
    }));
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 3 },
        "contentChanges": [
            { "range": { "start": { "line": 0, "character": 2 }, "end": { "line": 0, "character": 9 } }, "text": "Latest" }
        ]
    }));
    let symbols = client.request("textDocument/documentSymbol", document(uri));
    assert_eq!(symbols["result"][0]["name"], "Latest");
    let target = client.request(
        "textDocument/definition",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 3, "character": 3 }
        }),
    );
    assert_eq!(target["result"]["range"]["start"]["line"], 6);
    let other = client.request("textDocument/documentSymbol", document("untitled:other"));
    assert_eq!(other["result"][0]["name"], "Other");

    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 4 },
            "contentChanges": [{ "text": "# Reset\n" }]
        }),
    );
    assert_eq!(
        client.request("textDocument/documentSymbol", document(uri))["result"][0]["name"],
        "Reset"
    );
    assert_eq!(
        client.request("textDocument/foldingRange", document(uri))["result"],
        json!([])
    );
    client.notify("textDocument/didClose", document(uri));
    assert_eq!(
        client.request("textDocument/documentSymbol", document(uri))["error"]["code"],
        -32602
    );
    client.open(uri, "# Reopened");
    assert_eq!(
        client.request("textDocument/documentSymbol", document(uri))["result"][0]["name"],
        "Reopened"
    );
    client.shutdown();
}

#[test]
fn respects_flat_symbol_clients_and_folding_range_limits() {
    let mut client = Client::start();
    client.initialize(
        json!({ "textDocument": { "foldingRange": { "rangeLimit": 1, "lineFoldingOnly": true } } }),
    );
    let uri = "untitled:flat";
    client.open(uri, "# One\ntext\n## Child\n```\nx\n```\n# Two\n");
    let symbols = client.request("textDocument/documentSymbol", document(uri));
    assert_eq!(symbols["result"].as_array().unwrap().len(), 3);
    assert_eq!(symbols["result"][0]["location"]["uri"], uri);
    assert_eq!(symbols["result"][1]["containerName"], "One");
    assert_eq!(
        symbols["result"][1]["location"]["range"]["start"]["line"],
        2
    );
    assert_eq!(symbols["result"][2]["name"], "Two");
    let folds = client.request("textDocument/foldingRange", document(uri));
    assert_eq!(folds["result"], json!([{ "startLine": 0, "endLine": 5 }]));
    client.shutdown();
}

#[test]
fn handles_protocol_errors_and_lifecycle_without_losing_the_stream() {
    let mut client = Client::start();
    assert_eq!(
        client.request("unknown", Value::Null)["error"]["code"],
        -32002
    );
    client.send_raw(b"{");
    let parse_error = client.receive();
    assert_eq!(parse_error["error"]["code"], -32700);
    assert_eq!(parse_error["id"], Value::Null);
    client.send(json!([]));
    assert_eq!(client.receive()["error"]["code"], -32600);
    assert_eq!(
        client.request("initialize", json!({ "capabilities": [] }))["error"]["code"],
        -32602
    );
    client.initialize(json!({}));
    assert_eq!(
        client.request("initialize", json!({ "capabilities": {} }))["error"]["code"],
        -32600
    );
    client.notify("$/cancelRequest", json!({ "id": 999 }));
    client.notify("unknown/notification", json!({}));
    client.notify("textDocument/didOpen", json!({}));
    client.send(json!({ "jsonrpc": "2.0", "id": "unknown-method", "method": "unknown" }));
    let error = client.receive();
    assert_eq!(error["id"], "unknown-method");
    assert_eq!(error["error"]["code"], -32601);
    assert_eq!(
        client.request("textDocument/definition", json!({ "position": "invalid" }))["error"]
            ["code"],
        -32602
    );
    assert!(client
        .request("shutdown", Value::Null)
        .get("error")
        .is_none());
    assert_eq!(
        client.request("unknown", Value::Null)["error"]["code"],
        -32600
    );
    client.notify("exit", Value::Null);
    assert!(client.wait_for_exit().success());
}

#[test]
fn rejects_stale_versions_and_recovers_after_invalid_utf16_edits() {
    let mut client = Client::start();
    client.initialize(json!({}));
    let uri = "untitled:sync";
    client.open(uri, "# 😀");
    assert_eq!(
        client.request("textDocument/documentSymbol", document(uri))["result"][0]["name"],
        "😀"
    );
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [
            { "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } }, "text": "#" },
            { "range": { "start": { "line": 0, "character": 4 }, "end": { "line": 0, "character": 5 } }, "text": "broken" }
        ]
    }));
    assert_eq!(
        client.request("textDocument/documentSymbol", document(uri))["error"]["code"],
        -32801
    );
    for method in [
        "textDocument/hover",
        "textDocument/references",
        "textDocument/completion",
        "textDocument/prepareRename",
        "textDocument/rename",
    ] {
        let response = client.request(
            method,
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": 0, "character": 0 },
                "newName": "new",
                "context": { "includeDeclaration": true }
            }),
        );
        assert_eq!(response["error"]["code"], -32801);
    }
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 3 },
            "contentChanges": [{ "text": "# Recovered\n" }]
        }),
    );
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 2 },
            "contentChanges": [{ "text": "# Stale\n" }]
        }),
    );
    assert_eq!(
        client.request("textDocument/documentSymbol", document(uri))["result"][0]["name"],
        "Recovered"
    );
    client.shutdown();
}

#[test]
fn supports_yozora_admonitions_math_and_references_in_titles() {
    let mut client = Client::start();
    client.initialize(json!({ "textDocument": { "documentSymbol": { "hierarchicalDocumentSymbolSupport": true } } }));
    let uri = "untitled:extensions";
    client.open(uri, ":::note [guide][ref] [^n]\n## Inside\nbody\n:::\n\n$$\nx^2\n$$\n\n[ref]: /docs\n[^n]: note\n");
    let symbols = client.request("textDocument/documentSymbol", document(uri));
    assert_eq!(symbols["result"][0]["name"], "Inside");
    assert_eq!(
        symbols["result"][0]["range"]["end"],
        json!({ "line": 3, "character": 0 })
    );
    assert_eq!(
        client.request("textDocument/foldingRange", document(uri))["result"],
        json!([
            { "startLine": 0, "endLine": 3 },
            { "startLine": 1, "endLine": 2 },
            { "startLine": 5, "endLine": 7 }
        ])
    );
    for (character, target_line) in [(11, 9), (22, 10)] {
        let result = client.request(
            "textDocument/definition",
            json!({
                "textDocument": { "uri": uri }, "position": { "line": 0, "character": character }
            }),
        );
        assert_eq!(result["result"]["range"]["start"]["line"], target_line);
    }
    client.shutdown();
}

#[test]
fn exits_with_failure_without_a_shutdown_request() {
    let mut client = Client::start();
    client.notify("exit", Value::Null);
    assert_eq!(client.wait_for_exit().code(), Some(1));
}

#[test]
fn heading_references_follow_workspace_lifecycles_and_keep_label_queries_local() {
    let directory = TestDirectory::new();
    let target = directory.uri("target.md");
    let referrer = directory.uri("referrer.md");
    let labels = directory.uri("labels.md");
    let text = "# Intro 😀\n[self](#intro-%F0%9F%98%80)\n## Intro 😀";
    fs::write(directory.0.join("target.md"), "# Disk").unwrap();
    fs::write(
        directory.0.join("referrer.md"),
        "😀 [first](target.md#intro-%F0%9F%98%80)\n[second](target.md#intro-%F0%9F%98%80-2)",
    )
    .unwrap();
    let mut client = Client::start();
    assert!(client
        .request(
            "initialize",
            json!({ "capabilities": {}, "rootUri": directory.uri("") })
        )
        .get("error")
        .is_none());
    client.notify("initialized", json!({}));
    client.open(&target, text);
    client.open(
        &labels,
        "# [intro][ref]\n\n[ref]: target.md#intro-%F0%9F%98%80\n[REF]: target.md#missing",
    );
    let references = |client: &mut Client, uri: &str, line: u32, character: u32, include: bool| {
        let response = client.request("textDocument/references", json!({
            "textDocument": { "uri": uri }, "position": { "line": line, "character": character },
            "context": { "includeDeclaration": include }
        }));
        assert!(response.get("error").is_none(), "{response}");
        response["result"].clone()
    };
    let initial = references(&mut client, &target, 0, 3, false);
    assert_eq!(initial.as_array().unwrap().len(), 4);
    assert_eq!(initial[2]["uri"], referrer);
    assert_eq!(initial[2]["range"]["start"]["character"], 3);
    assert_eq!(
        references(&mut client, &target, 0, 3, true)
            .as_array()
            .unwrap()
            .len(),
        5
    );
    let local = references(&mut client, &labels, 0, 11, true);
    assert_eq!(local.as_array().unwrap().len(), 2);
    assert!(local
        .as_array()
        .unwrap()
        .iter()
        .all(|location| location["uri"] == labels));
    assert_eq!(references(&mut client, &labels, 3, 2, true), json!([]));

    client.open(
        &referrer,
        &fs::read_to_string(directory.0.join("referrer.md")).unwrap(),
    );
    assert_eq!(references(&mut client, &referrer, 0, 5, false), initial);
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": referrer, "version": 2 },
            "contentChanges": [{ "text": "[second](target.md#intro-%F0%9F%98%80-2)" }]
        }),
    );
    assert_eq!(
        references(&mut client, &target, 0, 3, false)
            .as_array()
            .unwrap()
            .len(),
        3
    );
    let second = references(&mut client, &target, 2, 4, false);
    assert_eq!(second.as_array().unwrap().len(), 1);
    assert_eq!(second[0]["uri"], referrer);
    assert_eq!(second[0]["range"]["start"]["line"], 0);
    fs::write(
        directory.0.join("created.md"),
        "[new](target.md#intro-%F0%9F%98%80)",
    )
    .unwrap();
    assert_eq!(
        references(&mut client, &target, 0, 3, false)
            .as_array()
            .unwrap()
            .len(),
        4
    );
    fs::remove_file(directory.0.join("created.md")).unwrap();
    assert_eq!(
        references(&mut client, &target, 0, 3, false)
            .as_array()
            .unwrap()
            .len(),
        3
    );

    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": target, "version": 2 }, "contentChanges": null
        }),
    );
    let unavailable = client.request(
        "textDocument/references",
        json!({
            "textDocument": { "uri": referrer }, "position": { "line": 0, "character": 2 },
            "context": { "includeDeclaration": true }
        }),
    );
    assert_eq!(unavailable["error"]["code"], -32801);
    assert_eq!(references(&mut client, &labels, 0, 11, true), local);
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": target, "version": 3 }, "contentChanges": [{ "text": text }]
        }),
    );
    assert_eq!(references(&mut client, &referrer, 0, 2, false), second);
    client.notify("textDocument/didClose", document(&target));
    assert_eq!(references(&mut client, &referrer, 0, 2, false), json!([]));
    client.open(&target, text);
    assert_eq!(references(&mut client, &referrer, 0, 2, false), second);
    client.shutdown();
}

#[test]
fn hover_and_references_follow_edits_and_stay_within_the_requested_document() {
    let mut client = Client::start();
    client.initialize(json!({}));
    let uri = "untitled:references";
    client.open(uri, "😀 [one][ID] ![two][id]\n\n[id]: /old \"Old\"\n");
    client.open("untitled:other", "[other][id]\n\n[id]: /other\n");
    let at_reference = json!({
        "textDocument": { "uri": uri },
        "position": { "line": 0, "character": 4 }
    });
    let info = client.request("textDocument/hover", at_reference.clone());
    assert_eq!(
        info["result"]["contents"],
        json!({ "kind": "plaintext", "value": "/old\n\nOld" })
    );
    assert_eq!(
        info["result"]["range"],
        json!({
            "start": { "line": 0, "character": 3 }, "end": { "line": 0, "character": 12 }
        })
    );
    for (position, include_declaration, expected_count) in [
        (json!({ "line": 0, "character": 4 }), false, 2),
        (json!({ "line": 2, "character": 2 }), true, 3),
    ] {
        let response = client.request(
            "textDocument/references",
            json!({
                "textDocument": { "uri": uri }, "position": position,
                "context": { "includeDeclaration": include_declaration }
            }),
        );
        let locations = response["result"].as_array().unwrap();
        assert_eq!(locations.len(), expected_count);
        assert!(locations.iter().all(|location| location["uri"] == uri));
        assert_eq!(
            locations[0]["range"]["start"],
            json!({ "line": 0, "character": 3 })
        );
        assert_eq!(
            locations[1]["range"]["start"],
            json!({ "line": 0, "character": 13 })
        );
    }

    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [
            { "range": { "start": { "line": 2, "character": 6 }, "end": { "line": 2, "character": 10 } }, "text": "/new" },
            { "range": { "start": { "line": 2, "character": 12 }, "end": { "line": 2, "character": 15 } }, "text": "New" },
            { "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 0, "character": 0 } }, "text": "[more][id]\n" }
        ]
    }));
    let info = client.request(
        "textDocument/hover",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 1, "character": 4 }
        }),
    );
    assert_eq!(info["result"]["contents"]["value"], "/new\n\nNew");
    let response = client.request(
        "textDocument/references",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 3, "character": 2 },
            "context": { "includeDeclaration": false }
        }),
    );
    let starts: Vec<_> = response["result"]
        .as_array()
        .unwrap()
        .iter()
        .map(|location| location["range"]["start"].clone())
        .collect();
    assert_eq!(
        starts,
        [
            json!({ "line": 0, "character": 0 }),
            json!({ "line": 1, "character": 3 }),
            json!({ "line": 1, "character": 13 }),
        ]
    );
    let other = client.request(
        "textDocument/hover",
        json!({
            "textDocument": { "uri": "untitled:other" }, "position": { "line": 0, "character": 2 }
        }),
    );
    assert_eq!(other["result"]["contents"]["value"], "/other");

    assert_eq!(
        client.request("textDocument/references", at_reference.clone())["error"]["code"],
        -32602
    );
    client.notify("textDocument/didClose", document(uri));
    for method in [
        "textDocument/hover",
        "textDocument/references",
        "textDocument/completion",
    ] {
        let mut params = at_reference.clone();
        params["context"] = json!({ "includeDeclaration": true });
        assert_eq!(client.request(method, params)["error"]["code"], -32602);
    }
    client.shutdown();
}

#[test]
fn hover_respects_content_format_preference_and_preserves_literal_markdown() {
    for (formats, kind, expected) in [
        (
            json!(["markdown", "plaintext"]),
            "markdown",
            "\\/target\n\n\\*literal\\* \\[text\\]",
        ),
        (
            json!(["plaintext", "markdown"]),
            "plaintext",
            "/target\n\n*literal* [text]",
        ),
    ] {
        let mut client = Client::start();
        client.initialize(json!({ "textDocument": { "hover": { "contentFormat": formats } } }));
        let uri = "untitled:hover-format";
        client.open(uri, "[link](/target \"*literal* [text]\")\n");
        let response = client.request(
            "textDocument/hover",
            json!({
                "textDocument": { "uri": uri }, "position": { "line": 0, "character": 3 }
            }),
        );
        assert_eq!(
            response["result"]["contents"],
            json!({ "kind": kind, "value": expected })
        );
        client.shutdown();
    }
}

#[test]
fn completion_edits_resolve_references_and_refresh_with_document_changes() {
    let mut client = Client::start();
    client.initialize(json!({}));
    let uri = "untitled:completion";
    client.open(
        uri,
        "😀 [text][Gu\n\n[Guide]: /first\n[guide]: /ignored\n[^Guide]: note\n",
    );
    let response = client.request(
        "textDocument/completion",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 0, "character": 12 },
            "context": { "triggerKind": 1 }
        }),
    );
    assert_eq!(response["result"]["isIncomplete"], true);
    let items = response["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["label"], "Guide");
    assert_eq!(items[0]["detail"], "/first");
    assert_eq!(items[0]["filterText"], "Gu");
    assert_eq!(
        items[0]["textEdit"],
        json!({
            "range": { "start": { "line": 0, "character": 10 }, "end": { "line": 0, "character": 12 } },
            "newText": "Guide]"
        })
    );
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 2 },
        "contentChanges": [{ "range": items[0]["textEdit"]["range"], "text": items[0]["textEdit"]["newText"] }]
    }));
    let target = client.request(
        "textDocument/definition",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 0, "character": 11 }
        }),
    );
    assert_eq!(target["result"]["range"]["start"]["line"], 2);

    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 3 },
            "contentChanges": [{ "text": "😀 [text][N\n\n[New]: /new\n[^Note]: body\n" }]
        }),
    );
    let response = client.request(
        "textDocument/completion",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 0, "character": 11 }
        }),
    );
    assert_eq!(response["result"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(response["result"]["items"][0]["label"], "New");
    assert_eq!(response["result"]["items"][0]["detail"], "/new");
    let invalid = client.request(
        "textDocument/completion",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 0, "character": 1 }
        }),
    );
    assert_eq!(invalid["error"]["code"], -32602);

    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 4 },
            "contentChanges": [{ "text": "[^N]\n\n[New]: /new\n[^Note]: body\n" }]
        }),
    );
    let response = client.request(
        "textDocument/completion",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 0, "character": 3 },
            "context": { "triggerKind": 2, "triggerCharacter": "^" }
        }),
    );
    let items = response["result"]["items"].as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["label"], "Note");
    assert_eq!(items[0]["textEdit"]["newText"], "Note");
    client.notify("textDocument/didChange", json!({
        "textDocument": { "uri": uri, "version": 5 },
        "contentChanges": [{ "range": items[0]["textEdit"]["range"], "text": items[0]["textEdit"]["newText"] }]
    }));
    let target = client.request(
        "textDocument/definition",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 0, "character": 3 }
        }),
    );
    assert_eq!(target["result"]["range"]["start"]["line"], 3);
    client.shutdown();
}

#[test]
fn completion_uses_cr_line_positions_and_skips_unclosed_code_blocks() {
    let mut client = Client::start();
    client.initialize(json!({}));
    let uri = "untitled:completion-context";
    client.open(uri, "[Guide]: /first\r\r😀 [x][gu");
    let response = client.request(
        "textDocument/completion",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 2, "character": 9 }
        }),
    );
    assert_eq!(
        response["result"]["items"][0]["textEdit"],
        json!({
            "range": { "start": { "line": 2, "character": 7 }, "end": { "line": 2, "character": 9 } },
            "newText": "Guide]"
        })
    );
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 2 },
            "contentChanges": [{ "text": "[Guide]: /first\n\n```md\n[text][gu" }]
        }),
    );
    let response = client.request(
        "textDocument/completion",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 3, "character": 9 }
        }),
    );
    assert_eq!(
        response["result"],
        json!({ "isIncomplete": false, "items": [] })
    );
    client.shutdown();
}

#[test]
fn rename_returns_versioned_edits_and_waits_for_client_document_changes() {
    let mut client = Client::start();
    client.initialize(json!({ "workspace": { "workspaceEdit": { "documentChanges": true } } }));
    let uri = "untitled:rename";
    client.open(
        uri,
        "😀 [Shown][old] [old][] ![old]\n\n[old]: /target \"Title\"\n",
    );
    let params =
        json!({ "textDocument": { "uri": uri }, "position": { "line": 0, "character": 12 } });
    let prepared = client.request("textDocument/prepareRename", params.clone());
    assert_eq!(
        prepared["result"],
        json!({
            "range": { "start": { "line": 0, "character": 11 }, "end": { "line": 0, "character": 14 } },
            "placeholder": "old"
        })
    );
    let mut rename_params = params.clone();
    rename_params["newName"] = json!("new");
    let response = client.request("textDocument/rename", rename_params.clone());
    let changes = response["result"]["documentChanges"].as_array().unwrap();
    assert_eq!(changes.len(), 1);
    assert_eq!(
        changes[0]["textDocument"],
        json!({ "uri": uri, "version": 1 })
    );
    let edits = changes[0]["edits"].as_array().unwrap();
    assert_eq!(edits.len(), 4);
    assert_eq!(edits[0]["newText"], "new");
    assert_eq!(
        edits[1]["range"],
        json!({
            "start": { "line": 0, "character": 22 }, "end": { "line": 0, "character": 22 }
        })
    );
    assert_eq!(edits[2]["newText"], "[new]");
    assert_eq!(
        client.request("textDocument/prepareRename", params.clone())["result"]["placeholder"],
        "old"
    );

    let content_changes: Vec<_> = edits
        .iter()
        .rev()
        .map(|edit| json!({ "range": edit["range"], "text": edit["newText"] }))
        .collect();
    client.notify(
        "textDocument/didChange",
        json!({
            "textDocument": { "uri": uri, "version": 2 }, "contentChanges": content_changes
        }),
    );
    assert_eq!(
        client.request("textDocument/prepareRename", params.clone())["result"]["placeholder"],
        "new"
    );
    let mut references_params = params.clone();
    references_params["context"] = json!({ "includeDeclaration": true });
    let references = client.request("textDocument/references", references_params);
    assert_eq!(references["result"].as_array().unwrap().len(), 4);
    assert_eq!(
        client.request("textDocument/hover", params)["result"]["contents"]["value"],
        "/target\n\nTitle"
    );
    rename_params["newName"] = json!("next");
    let next = client.request("textDocument/rename", rename_params);
    assert_eq!(
        next["result"]["documentChanges"][0]["textDocument"]["version"],
        2
    );
    client.shutdown();
}

#[test]
fn rename_supports_legacy_workspace_edits_and_separate_footnote_namespaces() {
    let mut client = Client::start();
    client.initialize(json!({}));
    let uri = "untitled:rename-legacy";
    client.open(
        uri,
        "[^old] [text][old]\n\n[^old]: note\n\n[old]: /url\n[new]: /other\n",
    );
    let response = client.request("textDocument/rename", json!({
        "textDocument": { "uri": uri }, "position": { "line": 0, "character": 3 }, "newName": "new"
    }));
    assert!(response["result"].get("documentChanges").is_none());
    let edits = response["result"]["changes"][uri].as_array().unwrap();
    assert_eq!(edits.len(), 2);
    assert_eq!(
        edits[0]["range"],
        json!({
            "start": { "line": 0, "character": 2 }, "end": { "line": 0, "character": 5 }
        })
    );
    let conflict = client.request("textDocument/rename", json!({
        "textDocument": { "uri": uri }, "position": { "line": 0, "character": 15 }, "newName": "NEW"
    }));
    assert_eq!(conflict["error"]["code"], -32803);
    let invalid = client.request(
        "textDocument/rename",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 0, "character": 3 }, "newName": ""
        }),
    );
    assert_eq!(invalid["error"]["code"], -32602);
    let body = client.request(
        "textDocument/prepareRename",
        json!({
            "textDocument": { "uri": uri }, "position": { "line": 2, "character": 9 }
        }),
    );
    assert_eq!(body["result"], Value::Null);
    client.shutdown();
}

#[test]
fn rename_rejects_unintended_bindings_without_mutating_the_document() {
    let mut client = Client::start();
    client.initialize(json!({}));
    let uri = "untitled:rename-conflict";
    client.open(uri, "[old] [new]\n\n[old]: /url\n");
    let params =
        json!({ "textDocument": { "uri": uri }, "position": { "line": 0, "character": 2 } });
    let mut rename_params = params.clone();
    rename_params["newName"] = json!("new");
    let rejected = client.request("textDocument/rename", rename_params.clone());
    assert_eq!(rejected["error"]["code"], -32803);
    assert_eq!(
        client.request("textDocument/prepareRename", params)["result"]["placeholder"],
        "old"
    );
    rename_params["newName"] = json!("old");
    let noop = client.request("textDocument/rename", rename_params);
    assert_eq!(noop["result"]["changes"][uri], json!([]));
    client.shutdown();
}

#[test]
fn queued_queries_receive_one_response_across_cancellation_close_and_shutdown() {
    let mut client = Client::start();
    client.initialize(json!({}));
    client.open("untitled:a", "# Before");
    client.open("untitled:b", "# Other");
    client.send(json!({ "jsonrpc": "2.0", "id": "cancel", "method": "textDocument/documentSymbol", "params": document("untitled:a") }));
    client.notify("$/cancelRequest", json!({ "id": "cancel" }));
    client.send(json!({ "jsonrpc": "2.0", "id": "close", "method": "textDocument/documentSymbol", "params": document("untitled:a") }));
    client.notify("textDocument/didClose", document("untitled:a"));
    client.notify("$/cancelRequest", json!({ "id": "unknown" }));
    for id in ["unknown", "keep"] {
        client.send(json!({ "jsonrpc": "2.0", "id": id, "method": "textDocument/documentSymbol", "params": document("untitled:b") }));
    }
    let mut responses = std::collections::BTreeMap::new();
    while responses.len() < 4 {
        let response = client.receive();
        if let Some(id) = response["id"].as_str() {
            assert!(responses.insert(id.to_string(), response).is_none());
        } else {
            client.notifications.push_back(response);
        }
    }
    // A query may finish before a cancellation arrives. Both outcomes require
    // exactly one response; deterministic cancellation timing is tested in Server.
    for (id, code) in [("cancel", -32800), ("close", -32801)] {
        if responses[id].get("error").is_some() {
            assert_eq!(responses[id]["error"]["code"], code);
        } else {
            assert_eq!(responses[id]["result"][0]["name"], "Before");
        }
    }
    for id in ["unknown", "keep"] {
        assert_eq!(responses[id]["result"][0]["name"], "Other");
    }
    for id in ["stop1", "stop2"] {
        client.send(json!({ "jsonrpc": "2.0", "id": id, "method": "textDocument/documentSymbol", "params": document("untitled:b") }));
    }
    client.send(json!({ "jsonrpc": "2.0", "id": "shutdown", "method": "shutdown" }));
    let mut stopped = std::collections::BTreeSet::new();
    loop {
        let response = client.receive();
        let Some(id) = response["id"].as_str() else {
            client.notifications.push_back(response);
            continue;
        };
        if id == "shutdown" {
            assert!(response.get("error").is_none());
            assert_eq!(stopped.len(), 2);
            break;
        }
        assert!(matches!(id, "stop1" | "stop2"));
        assert!(stopped.insert(id.to_string()));
        if response.get("error").is_some() {
            assert_eq!(response["error"]["code"], -32800);
        } else {
            assert_eq!(response["result"][0]["name"], "Other");
        }
    }
    client.notify("exit", Value::Null);
    assert!(client.wait_for_exit().success());
}
