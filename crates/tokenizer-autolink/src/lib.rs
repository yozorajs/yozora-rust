mod r#match;
mod parse;
mod tokenizer;
mod types;
pub mod util;

pub use tokenizer::{
    AutolinkDelimiterGenerator, AutolinkMatchHook, AutolinkParseHook, AutolinkTokenizer,
};
pub use types::{
    AutolinkContentEater, AutolinkContentHelper, AutolinkContentType, AutolinkDelimiter,
    AutolinkTokenData, AUTOLINK_TOKENIZER_NAME,
};
pub use util::{eat_absolute_uri, eat_autolink_schema, eat_email_address};

pub type AutolinkToken = yozora_core_tokenizer::TypedInlineToken<AutolinkTokenData>;
pub type AutolinkHookContext = AutolinkTokenizer;
pub type AutolinkTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn autolink_match<'a>(
    tokenizer: &'a AutolinkTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> AutolinkMatchHook<'a> {
    let _ = tokenizer;
    AutolinkMatchHook::new(api)
}

pub fn autolink_parse<'a>(
    tokenizer: &'a AutolinkTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> AutolinkParseHook<'a> {
    let _ = tokenizer;
    AutolinkParseHook::new(api)
}
