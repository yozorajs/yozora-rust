use std::sync::Arc;

use yozora_ast::{Association, Root};
use yozora_core_tokenizer::{
    AnyFallbackTokenizer, AnyTokenizer, BlockTokenizer, InlineFallbackTokenizer,
};

pub type FormatUrlFn = Arc<dyn Fn(&str) -> String + Send + Sync + 'static>;
#[derive(Clone, Default)]
pub struct ParseOptions {
    pub should_reserve_position: Option<bool>,
    pub preset_definitions: Option<Vec<Association>>,
    pub preset_footnote_definitions: Option<Vec<Association>>,
    pub format_url: Option<FormatUrlFn>,
}

#[derive(Debug, Clone)]
pub enum ParseContents<'a> {
    Text(&'a str),
    Chunks(&'a [&'a str]),
    OwnedText(String),
    OwnedChunks(Vec<String>),
}

impl<'a> From<&'a str> for ParseContents<'a> {
    fn from(value: &'a str) -> Self {
        Self::Text(value)
    }
}

impl<'a> From<&'a [&'a str]> for ParseContents<'a> {
    fn from(value: &'a [&'a str]) -> Self {
        Self::Chunks(value)
    }
}

impl<'a> From<&'a String> for ParseContents<'a> {
    fn from(value: &'a String) -> Self {
        Self::Text(value.as_str())
    }
}

impl<'a> From<String> for ParseContents<'a> {
    fn from(value: String) -> Self {
        Self::OwnedText(value)
    }
}

impl<'a> From<Vec<String>> for ParseContents<'a> {
    fn from(value: Vec<String>) -> Self {
        Self::OwnedChunks(value)
    }
}

impl<'a> From<&'a [String]> for ParseContents<'a> {
    fn from(value: &'a [String]) -> Self {
        Self::OwnedChunks(value.to_vec())
    }
}

impl<'a> From<Vec<&'a str>> for ParseContents<'a> {
    fn from(value: Vec<&'a str>) -> Self {
        Self::OwnedChunks(value.into_iter().map(str::to_string).collect())
    }
}
#[derive(Default)]
pub struct DefaultParserProps {
    pub block_fallback_tokenizer: Option<Box<dyn BlockTokenizer>>,
    pub inline_fallback_tokenizer: Option<Box<dyn InlineFallbackTokenizer>>,
    pub default_parse_options: Option<ParseOptions>,
}

pub trait Parser {
    fn use_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> &mut Self;

    fn replace_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> &mut Self;

    fn unmount_tokenizer(&mut self, tokenizer_name: &str) -> &mut Self;

    fn use_fallback_tokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self;

    fn set_default_parse_options(&mut self, options: Option<ParseOptions>);

    fn parse<'a, C>(&self, contents: C, options: Option<ParseOptions>) -> Root
    where
        C: Into<ParseContents<'a>>;
}
