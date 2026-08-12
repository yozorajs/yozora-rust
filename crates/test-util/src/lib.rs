mod base_tester;
mod markup_tester;
mod tokenizer_tester;
mod types;

use std::path::PathBuf;

use yozora_core_parser::Parser;
use yozora_markup_weaver::MarkupWeaverContract;

pub use base_tester::{BaseTester, BaseTesterContract};
pub use markup_tester::MarkupTester;
pub use tokenizer_tester::TokenizerTester;
pub use types::{BaseTesterProps, TestFailure, YozoraUseCase, YozoraUseCaseGroup};

pub fn fixture_root_directory() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures")
}

pub fn create_tokenizer_tester<P>(parser: P) -> TokenizerTester<P>
where
    P: Parser,
{
    TokenizerTester::new(fixture_root_directory(), parser)
}

pub fn create_tokenizer_testers<P, I>(parsers: I) -> Vec<TokenizerTester<P>>
where
    P: Parser,
    I: IntoIterator<Item = P>,
{
    parsers.into_iter().map(create_tokenizer_tester).collect()
}

pub fn create_markup_tester<P, W>(parser: P, weaver: W) -> MarkupTester<P, W>
where
    P: Parser,
    W: MarkupWeaverContract,
{
    MarkupTester::new(fixture_root_directory(), parser, weaver)
}
