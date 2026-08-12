mod r#match;
mod parse;
mod tokenizer;

pub use tokenizer::{EcmaImportTokenizer, ECMA_IMPORT_TOKENIZER_NAME};

pub type EcmaImportToken = yozora_core_tokenizer::BlockToken;
pub type EcmaImportHookContext = EcmaImportTokenizer;
pub type EcmaImportProps = yozora_core_tokenizer::TokenizerOptions;
pub type EcmaImportTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn ecma_import_match<'a>(
    tokenizer: &'a EcmaImportTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::r#match(tokenizer, api)
}

pub fn ecma_import_parse<'a>(
    tokenizer: &'a EcmaImportTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::parse(tokenizer, api)
}
