mod r#match;
mod parse;
mod tokenizer;
mod types;
mod util;

pub use tokenizer::{EcmaImportMatchHook, EcmaImportParseHook, EcmaImportTokenizer};
pub use types::{EcmaImportTokenData, ECMA_IMPORT_TOKENIZER_NAME};

pub type EcmaImportToken = yozora_core_tokenizer::TypedBlockToken<EcmaImportTokenData>;
pub type EcmaImportHookContext = EcmaImportTokenizer;
pub type EcmaImportProps = yozora_core_tokenizer::TokenizerOptions;
pub type EcmaImportTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn ecma_import_match<'a>(
    tokenizer: &'a EcmaImportTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> EcmaImportMatchHook {
    let _ = (tokenizer, api);
    EcmaImportMatchHook::new()
}

pub fn ecma_import_parse<'a>(
    tokenizer: &'a EcmaImportTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> EcmaImportParseHook<'a> {
    let _ = tokenizer;
    EcmaImportParseHook::new(api)
}
