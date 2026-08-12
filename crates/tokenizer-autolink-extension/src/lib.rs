mod r#match;
mod parse;
mod tokenizer;

pub use parse::{AutolinkExtensionContentType, AutolinkExtensionTokenData};
pub use r#match::{
    eat_domain_segment, eat_extend_email_address, eat_extended_url, eat_optional_domain_follows,
    eat_valid_domain, eat_www_domain, DomainSegmentEatResult,
};
pub use tokenizer::{AutolinkExtensionTokenizer, AUTOLINK_EXTENSION_TOKENIZER_NAME};

pub type AutolinkExtensionToken =
    yozora_core_tokenizer::TypedInlineToken<AutolinkExtensionTokenData>;
pub type AutolinkExtensionHookContext = AutolinkExtensionTokenizer;
pub type AutolinkExtensionTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn autolink_extension_match<'a>(
    tokenizer: &'a AutolinkExtensionTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::MatchInlineHook<'a> + 'a> {
    yozora_core_tokenizer::InlineTokenizer::r#match(tokenizer, api)
}

pub fn autolink_extension_parse<'a>(
    tokenizer: &'a AutolinkExtensionTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> Box<dyn yozora_core_tokenizer::ParseInlineHook + 'a> {
    yozora_core_tokenizer::InlineTokenizer::parse(tokenizer, api)
}
