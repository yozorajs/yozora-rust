use std::io;
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{mpsc, Arc};
use std::thread;

use yozora_parser::YozoraParser;

use crate::cancellation::Cancellation;
use crate::protocol::{Json, ResponseError};
use crate::query::{Dependencies, Request, State};
use crate::workspace_index::Index;

pub const WORKER_COUNT: usize = 2;

pub struct Task {
    pub state: State,
    pub work: Work,
    pub cancellation: Cancellation,
}

pub enum Work {
    Query(Request),
    Diagnostics(String),
}

impl Work {
    pub fn uses_workspace(&self) -> bool {
        matches!(self, Self::Query(request) if request.uses_workspace())
    }

    pub fn source_uri(&self) -> Option<&str> {
        match self {
            Self::Query(request) => request.source_uri(),
            Self::Diagnostics(uri) => Some(uri),
        }
    }
}

pub struct Finished {
    pub worker: usize,
    pub state: State,
    pub dependencies: Dependencies,
    pub result: Result<Json, ResponseError>,
    panicked: bool,
}

pub fn execute(
    worker: usize,
    mut task: Task,
    parser: &YozoraParser,
    index: &mut Index,
) -> Finished {
    let mut dependencies = Dependencies::default();
    let result = catch_unwind(AssertUnwindSafe(|| match task.work {
        Work::Query(request) => task.state.request(
            request,
            parser,
            index,
            &task.cancellation,
            &mut dependencies,
        ),
        Work::Diagnostics(uri) => task
            .state
            .diagnostics(&uri, parser, &task.cancellation, &mut dependencies)
            .map(Json::from),
    }));
    let panicked = result.is_err();
    Finished {
        worker,
        state: task.state,
        dependencies,
        result: result.unwrap_or_else(|_| Err(ResponseError::new(-32603, "analysis failed"))),
        panicked,
    }
}

pub struct Workers {
    senders: Vec<mpsc::SyncSender<Task>>,
}

impl Workers {
    pub fn new(publish: impl Fn(Finished) -> bool + Send + Sync + 'static) -> io::Result<Self> {
        Self::with_parser_factory(publish, YozoraParser::default)
    }

    pub(super) fn with_parser_factory(
        publish: impl Fn(Finished) -> bool + Send + Sync + 'static,
        make_parser: impl Fn() -> YozoraParser + Send + Sync + 'static,
    ) -> io::Result<Self> {
        let publish = Arc::new(publish);
        let make_parser = Arc::new(make_parser);
        let mut senders = Vec::new();
        for worker in 0..WORKER_COUNT {
            let (sender, receiver) = mpsc::sync_channel::<Task>(1);
            let publish = Arc::clone(&publish);
            let make_parser = Arc::clone(&make_parser);
            thread::Builder::new()
                .name(format!("yozora-lsp-analysis-{worker}"))
                .spawn(move || {
                    // Tokenizer trait objects stay on this thread; no new Send
                    // or Sync requirement is imposed on parser extensions.
                    let mut parser = make_parser();
                    let mut index = Index::default();
                    while let Ok(task) = receiver.recv() {
                        let finished = execute(worker, task, &parser, &mut index);
                        if finished.panicked {
                            parser = make_parser();
                            index = Index::default();
                        }
                        if !publish(finished) {
                            break;
                        }
                    }
                })?;
            senders.push(sender);
        }
        Ok(Self { senders })
    }

    pub fn submit(&self, worker: usize, task: Task) -> io::Result<()> {
        self.senders[worker]
            .send(task)
            .map_err(|_| io::Error::other("analysis worker stopped"))
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;
    use std::time::Duration;

    use serde_json::json;
    use yozora_core_parser::{DefaultParserProps, ParseOptions};

    use super::*;
    use crate::document::Document;

    fn task(uri: &str, destination: &str) -> Task {
        let mut state = State::default();
        state.documents.insert(
            uri.to_string(),
            Document::new(1, format!("[go]({destination})")).unwrap(),
        );
        Task {
            state,
            work: Work::Query(
                Request::parse(
                    "textDocument/hover",
                    json!({
                        "textDocument": { "uri": uri }, "position": { "line": 0, "character": 2 }
                    }),
                )
                .unwrap(),
            ),
            cancellation: Cancellation::default(),
        }
    }

    #[test]
    fn cancelled_queries_and_diagnostics_do_not_enter_the_parser() {
        let parser = YozoraParser::new(DefaultParserProps {
            default_parse_options: Some(ParseOptions {
                format_url: Some(Arc::new(|_| panic!("cancelled task entered the parser"))),
                ..ParseOptions::default()
            }),
            ..DefaultParserProps::default()
        });
        for diagnostics in [false, true] {
            let mut task = task("untitled:cancelled", "blocked");
            if diagnostics {
                task.work = Work::Diagnostics("untitled:cancelled".to_string());
            }
            task.cancellation.cancel();
            let finished = execute(0, task, &parser, &mut Index::default());
            assert!(!finished.panicked);
            assert_eq!(finished.result.unwrap_err().code, -32800);
        }
    }

    #[test]
    fn analysis_panics_release_deep_partial_asts_without_aborting_the_worker() {
        const CHILD_PROCESS: &str = "YOZORA_LSP_UNWIND_CHILD";
        if std::env::var_os(CHILD_PROCESS).is_none() {
            // A recursive destructor aborts the process instead of unwinding.
            // Isolate that failure while exercising the real worker lifecycle.
            let output = std::process::Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "worker::tests::analysis_panics_release_deep_partial_asts_without_aborting_the_worker",
                    "--nocapture",
                ])
                .env(CHILD_PROCESS, "1")
                .env("RUST_MIN_STACK", "2097152")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "worker subprocess failed: {}\n{}\n{}",
                output.status,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr),
            );
            return;
        }

        let (finished, results) = mpsc::channel();
        let workers = Workers::with_parser_factory(
            move |result| finished.send(result).is_ok(),
            || {
                YozoraParser::new(DefaultParserProps {
                    default_parse_options: Some(ParseOptions {
                        format_url: Some(Arc::new(|url| {
                            assert_ne!(url, "panic", "test query failure after a deep AST");
                            url.to_string()
                        })),
                        ..ParseOptions::default()
                    }),
                    ..DefaultParserProps::default()
                })
            },
        )
        .unwrap();
        let deep = format!("{}x{}", "**".repeat(64_000), "**".repeat(64_000));
        for (case, source) in [
            ("inline", format!("{deep} [go](panic)")),
            ("paragraphs", format!("{deep}\n\n[go](panic)")),
            ("headings", format!("# {deep}\n# [go](panic)")),
            ("setext headings", format!("{deep}\n===\n[go](panic)\n===")),
            ("list items", format!("- {deep}\n- [go](panic)")),
            ("list groups", format!("- {deep}\n+ [go](panic)")),
            (
                "table cells",
                format!("| {deep} | [go](panic) |\n| --- | --- |"),
            ),
            (
                "table rows",
                format!("| {deep} |\n| --- |\n| [go](panic) |"),
            ),
            (
                "tables",
                format!("| {deep} |\n| --- |\n\n| [go](panic) |\n| --- |"),
            ),
            ("blockquotes", format!("> {deep}\n\n> [go](panic)")),
            (
                "footnote definitions",
                format!("[^one]: {deep}\n\n[^two]: [go](panic)"),
            ),
            (
                "admonition title",
                format!(":::note {deep}\n[go](panic)\n:::"),
            ),
            (
                "admonitions",
                format!(":::note\n{deep}\n:::\n\n:::note\n[go](panic)\n:::"),
            ),
        ] {
            eprintln!("checking partial AST cleanup: {case}");
            let mut task = task("untitled:deep-partial", "panic");
            task.state.documents.insert(
                "untitled:deep-partial".to_string(),
                Document::new(1, source).unwrap(),
            );
            workers.submit(0, task).unwrap();
            let result = results.recv_timeout(Duration::from_secs(15)).unwrap();
            assert!(result.panicked, "{case}");
            assert_eq!(result.result.unwrap_err().code, -32603, "{case}");
            workers
                .submit(0, self::task("untitled:recovered", "ready"))
                .unwrap();
            let result = results.recv_timeout(Duration::from_secs(5)).unwrap();
            assert_eq!(
                result.result.unwrap()["contents"]["value"],
                "ready",
                "{case}"
            );
        }
    }

    #[test]
    fn cancelling_source_parsing_skips_rename_validation_and_preserves_the_ast() {
        let (finished, results) = mpsc::channel();
        let (entered, started) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let gate = Arc::new(Mutex::new(gate));
        let workers = Workers::with_parser_factory(
            move |result| finished.send(result).is_ok(),
            move || {
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
            },
        )
        .unwrap();
        let uri = "untitled:rename-cancelled";
        let mut state = State::default();
        state.documents.insert(
            uri.to_string(),
            Document::new(1, "[go](blocked)\n\n[text][old]\n\n[old]: /old".to_string()).unwrap(),
        );
        let cancellation = Cancellation::default();
        workers.submit(0, Task {
            state,
            work: Work::Query(Request::parse("textDocument/rename", json!({
                "textDocument": { "uri": uri }, "position": { "line": 2, "character": 8 }, "newName": "new"
            })).unwrap()),
            cancellation: cancellation.clone(),
        }).unwrap();
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        cancellation.cancel();
        release.send(()).unwrap();
        let finished = results.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(finished.result.unwrap_err().code, -32800);
        // A fresh task reuses the parsed source without re-entering the gate.
        workers.submit(0, Task {
            state: finished.state,
            work: Work::Query(Request::parse("textDocument/hover", json!({
                "textDocument": { "uri": uri }, "position": { "line": 0, "character": 2 }
            })).unwrap()),
            cancellation: Cancellation::default(),
        }).unwrap();
        let finished = results.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(finished.result.unwrap()["contents"]["value"], "blocked");
    }

    #[test]
    fn workers_progress_independently_and_recover_from_a_query_panic() {
        let (finished, results) = mpsc::channel();
        let (entered, started) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let gate = Arc::new(Mutex::new(gate));
        let workers = Workers::with_parser_factory(
            move |result| finished.send(result).is_ok(),
            move || {
                let entered = entered.clone();
                let gate = Arc::clone(&gate);
                YozoraParser::new(DefaultParserProps {
                    default_parse_options: Some(ParseOptions {
                        format_url: Some(Arc::new(move |url| {
                            if url == "blocked" {
                                entered.send(()).unwrap();
                                // Dropping the test sender also releases this wait.
                                let _ = gate.lock().unwrap().recv();
                            }
                            assert_ne!(url, "panic", "test query failure");
                            url.to_string()
                        })),
                        ..ParseOptions::default()
                    }),
                    ..DefaultParserProps::default()
                })
            },
        )
        .unwrap();
        workers
            .submit(0, task("untitled:blocked", "blocked"))
            .unwrap();
        started.recv_timeout(Duration::from_secs(5)).unwrap();
        workers.submit(1, task("untitled:ready", "ready")).unwrap();
        let fast = results.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(fast.worker, 1);
        assert_eq!(fast.result.unwrap()["contents"]["value"], "ready");
        release.send(()).unwrap();
        let slow = results.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(slow.worker, 0);
        assert_eq!(slow.result.unwrap()["contents"]["value"], "blocked");
        workers.submit(0, task("untitled:panic", "panic")).unwrap();
        assert_eq!(
            results
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .result
                .unwrap_err()
                .code,
            -32603
        );
        workers
            .submit(0, task("untitled:recovered", "ready"))
            .unwrap();
        assert_eq!(
            results
                .recv_timeout(Duration::from_secs(5))
                .unwrap()
                .result
                .unwrap()["contents"]["value"],
            "ready"
        );
    }
}
