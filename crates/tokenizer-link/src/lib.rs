mod r#match;
mod parse;
mod tokenizer;
mod types;
mod util;

pub use tokenizer::{LinkDelimiterGenerator, LinkMatchHook, LinkParseHook, LinkTokenizer};
pub use types::{LinkDelimiter, LinkTokenData, LINK_TOKENIZER_NAME};
pub use util::{eat_link_destination, eat_link_title};
pub use yozora_core_tokenizer::check_balanced_brackets_status;

pub type LinkToken = yozora_core_tokenizer::TypedInlineToken<LinkTokenData>;
pub type LinkHookContext = LinkTokenizer;
pub type LinkTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn link_match<'a>(
    tokenizer: &'a LinkTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> LinkMatchHook<'a> {
    let _ = tokenizer;
    LinkMatchHook::new(api)
}

pub fn link_parse<'a>(
    tokenizer: &'a LinkTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> LinkParseHook<'a> {
    let _ = tokenizer;
    LinkParseHook::new(api)
}
