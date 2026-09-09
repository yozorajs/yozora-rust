use super::tests::{open, replace, server};
use super::*;

fn queue_symbols(server: &mut Server, id: i32, now: Instant) {
    server
        .queue_query(json!(id), "workspace/symbol", json!({ "query": "" }), now)
        .unwrap();
}

#[test]
fn workspace_queries_share_one_scope_and_leave_a_worker_for_document_queries() {
    let mut server = server();
    let now = Instant::now();
    open(&mut server, "untitled:document", "# Document", now);
    queue_symbols(&mut server, 1, now);
    queue_symbols(&mut server, 2, now);
    server
        .queue_query(
            json!(3),
            "textDocument/documentSymbol",
            json!({
                "textDocument": { "uri": "untitled:document" }
            }),
            now,
        )
        .unwrap();
    let (workspace_worker, workspace) = server.start_work(now + QUERY_DELAY).unwrap();
    let (document_worker, document) = server.start_work(now + QUERY_DELAY).unwrap();
    assert!(workspace.work.source_uri().is_none());
    assert_eq!(document.work.source_uri(), Some("untitled:document"));
    assert_ne!(workspace_worker, document_worker);
    assert!(server.next_deadline().is_none());
    let finished = worker::execute(document_worker, document, &server.parser, &mut server.index);
    server.finish_work(finished);
    assert_eq!(server.outgoing[0]["id"], 3);
    assert_eq!(server.outgoing[0]["result"][0]["name"], "Document");
    assert!(server.start_work(now + QUERY_DELAY).is_none());
    let finished = worker::execute(
        workspace_worker,
        workspace,
        &server.parser,
        &mut server.index,
    );
    server.finish_work(finished);
    assert_eq!(server.outgoing[1]["id"], 1);
    assert_eq!(server.outgoing[1]["result"][0]["name"], "Document");
    let response = server.take_due_work(now + QUERY_DELAY).unwrap().unwrap();
    assert_eq!(response["id"], 2);
    assert_eq!(
        response["result"][0]["location"]["uri"],
        "untitled:document"
    );
}

#[test]
fn renames_share_workspace_scheduling_and_retire_when_another_buffer_changes() {
    let mut server = server();
    let now = Instant::now();
    open(&mut server, "untitled:heading", "# Old", now);
    open(&mut server, "untitled:other", "# Other", now);
    server.queue_query(json!(1), "textDocument/rename", json!({
        "textDocument": { "uri": "untitled:heading" }, "position": { "line": 0, "character": 3 }, "newName": "New"
    }), now).unwrap();
    let (worker, task) = server.start_work(now + QUERY_DELAY).unwrap();
    assert!(task.work.uses_workspace());
    assert_eq!(task.work.source_uri(), Some("untitled:heading"));
    queue_symbols(&mut server, 2, now);
    server
        .queue_query(
            json!(3),
            "workspace/willRenameFiles",
            json!({ "files": [] }),
            now,
        )
        .unwrap();
    assert!(server.start_work(now + QUERY_DELAY).is_none());
    replace(&mut server, "untitled:other", 2, "# Changed", now).unwrap();
    assert!(task.cancellation.is_cancelled());
    assert!(server.pending_queries.is_empty());
    let finished = worker::execute(worker, task, &server.parser, &mut server.index);
    server.finish_work(finished);
    assert_eq!(server.outgoing.len(), 3);
    assert!(server
        .outgoing
        .iter()
        .all(|response| response["error"]["code"] == -32801));
}

#[test]
fn file_rename_params_are_bounded_and_did_rename_notifications_validate_atomically() {
    let mut server = server();
    let now = Instant::now();
    let oversized = json!({ "files": (0..129).map(|index| json!({ "oldUri": format!("file:///old{index}.md"), "newUri": format!("file:///new{index}.md") })).collect::<Vec<_>>() });
    for params in [
        json!({}),
        json!({ "files": [{ "oldUri": "file:///old.md" }] }),
        oversized,
    ] {
        assert_eq!(
            server
                .queue_query(json!(0), "workspace/willRenameFiles", params, now)
                .unwrap_err()
                .code,
            -32602
        );
    }
    let params = json!({ "files": [{ "oldUri": format!("file:///{}", "x".repeat(MAX_PENDING_QUERY_BYTES / 2)), "newUri": "file:///new.md" }] });
    server
        .queue_query(json!(1), "workspace/willRenameFiles", params.clone(), now)
        .unwrap();
    assert_eq!(
        server
            .queue_query(json!(2), "workspace/willRenameFiles", params, now)
            .unwrap_err()
            .code,
        -32000
    );
    let epoch = server.scope_epoch;
    assert!(server
        .notification(
            "workspace/didRenameFiles",
            json!({ "files": [
        { "oldUri": "file:///one.md", "newUri": "file:///two.md" },
        { "oldUri": "relative.md", "newUri": "file:///three.md" }
    ] }),
            now
        )
        .is_err());
    assert_eq!(server.scope_epoch, epoch);
    assert_eq!(server.pending_queries.len(), 1);
    server
        .notification(
            "workspace/didRenameFiles",
            json!({ "files": [{ "oldUri": "file:///one.md", "newUri": "file:///two.md" }] }),
            now,
        )
        .unwrap();
    assert!(server.pending_queries.is_empty());
    assert_eq!(server.outgoing[0]["error"]["code"], -32801);
}

#[test]
fn reference_label_rename_keeps_priority_inside_a_heading() {
    let mut server = server();
    let uri = "untitled:label-in-heading";
    open(
        &mut server,
        uri,
        "# [shown][old]\n\n[old]: /target",
        Instant::now(),
    );
    let params =
        json!({ "textDocument": { "uri": uri }, "position": { "line": 0, "character": 11 } });
    assert_eq!(
        server
            .request("textDocument/prepareRename", params.clone())
            .unwrap()["placeholder"],
        "old"
    );
    let mut params = params;
    params["newName"] = json!("new");
    let edits = server.request("textDocument/rename", params).unwrap();
    assert_eq!(edits["changes"][uri].as_array().unwrap().len(), 2);
    assert_eq!(edits["changes"][uri][0]["newText"], "new");
}

#[test]
fn link_diagnostics_refresh_for_target_edits_close_and_watched_file_changes() {
    let directory = crate::files::tests::TestDir::new();
    let mut server = server();
    let start = Instant::now();
    server
        .notification(
            "workspace/didChangeWorkspaceFolders",
            json!({ "event": {
        "added": [{ "uri": directory.uri("") }], "removed": []
    } }),
            start,
        )
        .unwrap();
    let source = directory.uri("source.md");
    let target = directory.uri("target.md");
    open(&mut server, &source, "[go](target.md#intro)", start);
    let codes = |publication: Value| {
        publication["params"]["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["code"].as_str().unwrap().to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(
        codes(
            server
                .take_due_diagnostics(start + DIAGNOSTICS_DELAY)
                .unwrap()
                .unwrap()
        ),
        ["missing-file"]
    );
    let now = start + DIAGNOSTICS_DELAY * 2;
    open(&mut server, &target, "# Other", now);
    assert_eq!(
        codes(
            server
                .take_due_diagnostics(now + DIAGNOSTICS_DELAY)
                .unwrap()
                .unwrap()
        ),
        ["missing-anchor"]
    );
    let now = start + DIAGNOSTICS_DELAY * 4;
    replace(&mut server, &target, 2, "# Intro", now).unwrap();
    assert!(codes(
        server
            .take_due_diagnostics(now + DIAGNOSTICS_DELAY)
            .unwrap()
            .unwrap()
    )
    .is_empty());
    let now = start + DIAGNOSTICS_DELAY * 6;
    server
        .notification(
            "textDocument/didClose",
            json!({ "textDocument": { "uri": target } }),
            now,
        )
        .unwrap();
    assert_eq!(
        codes(
            server
                .take_due_diagnostics(now + DIAGNOSTICS_DELAY)
                .unwrap()
                .unwrap()
        ),
        ["missing-file"]
    );
    for (index, text, expected) in [
        (8, Some("# Intro"), None),
        (10, Some("# Other"), Some("missing-anchor")),
        (12, None, Some("missing-file")),
    ] {
        let path = directory.0.join("target.md");
        if let Some(text) = text {
            std::fs::write(path, text).unwrap();
        } else {
            std::fs::remove_file(path).unwrap();
        }
        let now = start + DIAGNOSTICS_DELAY * index;
        server.notification("workspace/didChangeWatchedFiles", json!({ "changes": [{ "uri": target, "type": if text.is_some() { 2 } else { 3 } }] }), now).unwrap();
        assert_eq!(
            codes(
                server
                    .take_due_diagnostics(now + DIAGNOSTICS_DELAY)
                    .unwrap()
                    .unwrap()
            ),
            expected.into_iter().map(str::to_string).collect::<Vec<_>>()
        );
    }
}

#[test]
fn unavailable_targets_keep_local_diagnostics_and_resynchronize_without_a_source_edit() {
    let directory = crate::files::tests::TestDir::new();
    let mut server = server();
    let now = Instant::now();
    let source = directory.uri("source.md");
    let target = directory.uri("target.md");
    open(
        &mut server,
        &source,
        "[go](target.md#intro)\n\n[r]: /one\n[R]: /two",
        now,
    );
    open(&mut server, &target, "# Other", now);
    server
        .query
        .documents
        .get_mut(&target)
        .unwrap()
        .invalidate(2)
        .unwrap();
    let (worker, task) = server.start_work(now + DIAGNOSTICS_DELAY).unwrap();
    assert_eq!(task.work.source_uri(), Some(source.as_str()));
    let finished = worker::execute(worker, task, &server.parser, &mut server.index);
    server.finish_work(finished);
    let issues = server.outgoing.last().unwrap()["params"]["diagnostics"]
        .as_array()
        .unwrap();
    assert_eq!(issues.len(), 1);
    assert_eq!(issues[0]["code"], "duplicate-link-definition");
    replace(
        &mut server,
        &target,
        3,
        "# Still absent",
        now + DIAGNOSTICS_DELAY * 2,
    )
    .unwrap();
    let publication = server
        .take_due_diagnostics(now + DIAGNOSTICS_DELAY * 3)
        .unwrap()
        .unwrap();
    assert!(publication["params"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|item| item["code"] == "missing-anchor"));
}

#[test]
fn buffer_root_and_watcher_changes_retire_pending_and_finished_workspace_responses_once() {
    let directory = crate::files::tests::TestDir::new();
    let now = Instant::now();
    for change in [
        "edit",
        "invalid edit",
        "open",
        "close",
        "reopen",
        "roots",
        "watcher",
    ] {
        let mut server = server();
        let uri = "untitled:original";
        open(&mut server, uri, "# Original", now);
        queue_symbols(&mut server, 1, now);
        let (worker, task) = server.start_work(now + QUERY_DELAY).unwrap();
        let cancellation = task.cancellation.clone();
        let finished = worker::execute(worker, task, &server.parser, &mut server.index);
        assert!(finished.dependencies.targets.contains(uri));
        queue_symbols(&mut server, 2, now);
        match change {
            "edit" => replace(&mut server, uri, 2, "# Changed", now).unwrap(),
            "invalid edit" => {
                assert!(server.notification("textDocument/didChange", json!({
                    "textDocument": { "uri": uri, "version": 2 },
                    "contentChanges": [{ "range": { "start": { "line": 9, "character": 0 }, "end": { "line": 9, "character": 1 } }, "text": "x" }]
                }), now).is_err());
            }
            "open" => open(&mut server, "untitled:new", "# New", now),
            "close" | "reopen" => {
                server
                    .notification(
                        "textDocument/didClose",
                        json!({ "textDocument": { "uri": uri } }),
                        now,
                    )
                    .unwrap();
                if change == "reopen" {
                    open(&mut server, uri, "# Reopened", now);
                }
            }
            "roots" => server
                .notification(
                    "workspace/didChangeWorkspaceFolders",
                    json!({ "event": {
                        "added": [{ "uri": directory.uri("") }], "removed": []
                    }}),
                    now,
                )
                .unwrap(),
            "watcher" => server
                .notification(
                    "workspace/didChangeWatchedFiles",
                    json!({ "changes": [
                        { "uri": directory.uri("changed.md"), "type": 2 }
                    ]}),
                    now,
                )
                .unwrap(),
            _ => unreachable!(),
        }
        assert!(cancellation.is_cancelled(), "{change}");
        assert!(server.pending_queries.is_empty());
        assert_eq!(server.pending_query_bytes, 0);
        server.finish_work(finished);
        let responses: Vec<_> = server
            .outgoing
            .iter()
            .filter(|message| message.get("id").is_some())
            .collect();
        assert_eq!(responses.len(), 2, "{change}");
        assert!(
            responses
                .iter()
                .all(|response| response["error"]["code"] == -32801),
            "{change}"
        );
        if change == "invalid edit" {
            replace(&mut server, uri, 3, "# Recovered", now).unwrap();
        }
        let result = server
            .request("workspace/symbol", json!({ "query": "" }))
            .unwrap();
        if matches!(change, "edit" | "reopen" | "invalid edit") {
            assert!(
                result
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|symbol| symbol["name"] != "Original"),
                "{change}"
            );
        }
    }
}

#[test]
fn rejected_notifications_do_not_invalidate_workspace_queries_or_change_scope() {
    let mut server = server();
    let now = Instant::now();
    let uri = "untitled:original";
    open(&mut server, uri, "# Original", now);
    queue_symbols(&mut server, 1, now);
    let (worker, task) = server.start_work(now + QUERY_DELAY).unwrap();
    let cancellation = task.cancellation.clone();
    let epoch = server.scope_epoch;
    assert!(replace(&mut server, uri, 1, "# Stale", now).is_err());
    for changes in [
        json!([{ "uri": "file:///changed.md", "type": 2 }, { "uri": "file:///invalid.md", "type": 4 }]),
        json!([{ "uri": "file:///changed.md", "type": 2 }, { "uri": "relative.md", "type": 1 }]),
    ] {
        assert!(server
            .notification(
                "workspace/didChangeWatchedFiles",
                json!({ "changes": changes }),
                now
            )
            .is_err());
    }
    assert_eq!(server.scope_epoch, epoch);
    assert!(!cancellation.is_cancelled());
    assert!(server.outgoing.is_empty());
    let finished = worker::execute(worker, task, &server.parser, &mut server.index);
    server.finish_work(finished);
    assert_eq!(server.outgoing[0]["result"][0]["name"], "Original");
}

#[test]
fn workspace_queries_decode_typed_params_and_charge_query_text_to_the_queue_budget() {
    let mut server = server();
    let now = Instant::now();
    assert!(is_query("workspace/symbol"));
    for params in [json!({}), json!({ "query": null }), json!({ "query": 1 })] {
        assert_eq!(
            server
                .queue_query(json!(0), "workspace/symbol", params, now)
                .unwrap_err()
                .code,
            -32602
        );
    }
    let params = json!({ "query": "x".repeat(MAX_PENDING_QUERY_BYTES / 2), "ignored": "x".repeat(MAX_PENDING_QUERY_BYTES) });
    server
        .queue_query(json!(1), "workspace/symbol", params.clone(), now)
        .unwrap();
    assert!(server.pending_query_bytes >= MAX_PENDING_QUERY_BYTES / 2);
    assert!(server.pending_query_bytes < MAX_PENDING_QUERY_BYTES / 2 + 1_024);
    assert_eq!(
        server
            .queue_query(json!(2), "workspace/symbol", params.clone(), now)
            .unwrap_err()
            .code,
        -32000
    );
    server
        .notification("$/cancelRequest", json!({ "id": 1 }), now)
        .unwrap();
    assert_eq!(server.pending_query_bytes, 0);
    server
        .queue_query(json!(3), "workspace/symbol", params, now)
        .unwrap();
    assert_eq!(server.pending_queries.len(), 1);
}

#[test]
fn shutdown_cancels_running_and_queued_workspace_queries_without_duplicate_responses() {
    let mut server = server();
    let now = Instant::now();
    queue_symbols(&mut server, 1, now);
    let (worker, task) = server.start_work(now + QUERY_DELAY).unwrap();
    queue_symbols(&mut server, 2, now);
    server.request("shutdown", Value::Null).unwrap();
    assert!(task.cancellation.is_cancelled());
    let finished = worker::execute(worker, task, &server.parser, &mut server.index);
    server.finish_work(finished);
    assert_eq!(server.outgoing.len(), 2);
    assert!(server
        .outgoing
        .iter()
        .all(|response| response["error"]["code"] == -32800));
}
