mod r#match;
mod parse;
mod tokenizer;
mod types;

pub use tokenizer::{TableMatchHook, TableParseHook, TableTokenizer};
pub use types::{TableCellTokenData, TableRowTokenData, TableTokenData, TABLE_TOKENIZER_NAME};

pub type TableToken = yozora_core_tokenizer::TypedBlockToken<TableTokenData>;
pub type TableRowToken = TableRowTokenData;
pub type TableCellToken = TableCellTokenData;
pub type TableHookContext = TableTokenizer;
pub type TableTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn table_match<'a>(
    tokenizer: &'a TableTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchBlockPhaseApi,
) -> TableMatchHook<'a> {
    let _ = tokenizer;
    TableMatchHook::new(api)
}

pub fn table_parse<'a>(
    tokenizer: &'a TableTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseBlockPhaseApi,
) -> TableParseHook<'a> {
    let _ = tokenizer;
    TableParseHook::new(api)
}
