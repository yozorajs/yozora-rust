mod r#match;
mod parse;
mod tokenizer;

pub use parse::{AutolinkContentType, AutolinkTokenData};
pub use r#match::{eat_absolute_uri, eat_autolink_schema, eat_email_address};
pub use tokenizer::{AutolinkTokenizer, AUTOLINK_TOKENIZER_NAME};

pub type AutolinkToken = yozora_core_tokenizer::TypedInlineToken<AutolinkTokenData>;
pub type AutolinkHookContext = AutolinkTokenizer;
pub type AutolinkTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn autolink_match<'a>(
    tokenizer: &'a AutolinkTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchInlineHook<'a> + 'a> {
    yozora_core_tokenizer::InlineTokenizer::r#match(tokenizer, api)
}

pub fn autolink_parse<'a>(
    tokenizer: &'a AutolinkTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseInlineHook + 'a> {
    yozora_core_tokenizer::InlineTokenizer::parse(tokenizer, api)
}
