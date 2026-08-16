mod r#match;
mod parse;
mod tokenizer;
mod types;
pub mod util;

pub use tokenizer::{
    AutolinkExtensionDelimiterGenerator, AutolinkExtensionMatchHook, AutolinkExtensionParseHook,
    AutolinkExtensionTokenizer,
};
pub use types::{
    AutolinkExtensionContentType, AutolinkExtensionDelimiter, AutolinkExtensionTokenData,
    AUTOLINK_EXTENSION_TOKENIZER_NAME,
};
pub use util::email::eat_extend_email_address;
pub use util::uri::{
    eat_domain_segment, eat_extended_url, eat_optional_domain_follows, eat_valid_domain,
    eat_www_domain, DomainSegmentEatResult,
};

pub type AutolinkExtensionToken =
    yozora_core_tokenizer::TypedInlineToken<AutolinkExtensionTokenData>;
pub type AutolinkExtensionHookContext = AutolinkExtensionTokenizer;
pub type AutolinkExtensionTokenizerProps = yozora_core_tokenizer::TokenizerOptions;

pub fn autolink_extension_match<'a>(
    tokenizer: &'a AutolinkExtensionTokenizer,
    api: &'a dyn yozora_core_tokenizer::MatchInlinePhaseApi,
) -> AutolinkExtensionMatchHook<'a> {
    let _ = tokenizer;
    AutolinkExtensionMatchHook::new(api)
}

pub fn autolink_extension_parse<'a>(
    tokenizer: &'a AutolinkExtensionTokenizer,
    api: &'a dyn yozora_core_tokenizer::ParseInlinePhaseApi,
) -> AutolinkExtensionParseHook<'a> {
    let _ = tokenizer;
    AutolinkExtensionParseHook::new(api)
}
