mod r#match;
mod parse;
mod tokenizer;

pub use tokenizer::{TableTokenizer, TABLE_TOKENIZER_NAME};

pub type TableToken = yozora_core_tokenizer::BlockToken;
pub type TableRowToken = yozora_core_tokenizer::BlockToken;
pub type TableCellToken = yozora_core_tokenizer::BlockToken;
pub type TableHookContext = TableTokenizer;
pub type TableTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn table_match<'a>(
    tokenizer: &'a TableTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::r#match(tokenizer, api)
}

pub fn table_parse<'a>(
    tokenizer: &'a TableTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseBlockHook + 'a> {
    yozora_core_tokenizer::BlockTokenizer::parse(tokenizer, api)
}
