use std::fs;
use std::future::Future;
use std::task::{Context, Poll, Waker};

use serde_json::json;
use yozora_markup_weaver::{DefaultMarkupWeaver, MarkupWeaverContract};
use yozora_parser::YozoraParser;
use yozora_test_util::{BaseTesterContract, MarkupTester, TokenizerTester};

#[test]
fn testers_scan_run_and_write_answers() {
    let root = std::env::temp_dir().join(format!("yozora-testers-{}", std::process::id()));
    fs::create_dir_all(&root).expect("create fixture root");

    let parser = YozoraParser::default();
    let input = "hello";
    let parse_answer = serde_json::to_value(parser.parse(input, None)).expect("serialize AST");
    let markup_answer =
        DefaultMarkupWeaver::default().weave(&YozoraParser::default().parse(input, None));
    let filepath = root.join("basic.json");
    fs::write(
        &filepath,
        serde_json::to_string_pretty(&json!({
            "title": "basic",
            "cases": [{
                "description": "plain text",
                "input": input,
                "parseAnswer": parse_answer,
                "markupAnswer": markup_answer,
            }],
        }))
        .expect("serialize fixture"),
    )
    .expect("write fixture");

    let mut tokenizer_tester = TokenizerTester::new(&root, parser);
    tokenizer_tester
        .scan(["**/*.json"], None, |_| true)
        .expect("scan tokenizer fixtures");
    tokenizer_tester.run_test().expect("run tokenizer tests");
    block_on(BaseTesterContract::run_answer(&mut tokenizer_tester))
        .expect("write tokenizer answers");

    let mut markup_tester = MarkupTester::new(
        &root,
        YozoraParser::default(),
        DefaultMarkupWeaver::default(),
    );
    markup_tester
        .scan(["**/*.json"], None, |_| true)
        .expect("scan markup fixtures");
    BaseTesterContract::run_test(&markup_tester).expect("run markup tests");

    let rewritten = fs::read_to_string(&filepath).expect("read rewritten fixture");
    assert!(rewritten.ends_with('\n'));
    assert!(rewritten.contains("parseAnswer"));
    fs::remove_dir_all(root).expect("remove fixture root");
}

fn block_on<F>(future: F) -> F::Output
where
    F: Future,
{
    let mut context = Context::from_waker(Waker::noop());
    let mut future = Box::pin(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}
