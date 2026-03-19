use std::sync::Arc;

use yozora_ast::{Association, Root};
use yozora_core_tokenizer::{
    AnyFallbackTokenizer, AnyTokenizer, BlockTokenizer, InlineFallbackTokenizer,
};

pub type FormatUrlFn = Arc<dyn Fn(&str) -> String + Send + Sync + 'static>;

#[allow(non_snake_case)]
#[derive(Clone, Default)]
pub struct ParseOptions {
    pub shouldReservePosition: Option<bool>,
    pub presetDefinitions: Option<Vec<Association>>,
    pub presetFootnoteDefinitions: Option<Vec<Association>>,
    pub formatUrl: Option<FormatUrlFn>,
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

#[allow(non_snake_case)]
#[derive(Default)]
pub struct DefaultParserProps {
    pub blockFallbackTokenizer: Option<Box<dyn BlockTokenizer>>,
    pub inlineFallbackTokenizer: Option<Box<dyn InlineFallbackTokenizer>>,
    pub defaultParseOptions: Option<ParseOptions>,
}

pub trait Parser {
    fn useTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        registerBeforeTokenizer: Option<&str>,
    ) -> &mut Self;

    fn replaceTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        registerBeforeTokenizer: Option<&str>,
    ) -> &mut Self;

    fn unmountTokenizer(&mut self, tokenizerName: &str) -> &mut Self;

    fn useFallbackTokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self;

    fn setDefaultParseOptions(&mut self, options: Option<ParseOptions>);

    fn parse<'a, C>(&self, contents: C, options: Option<ParseOptions>) -> Root
    where
        C: Into<ParseContents<'a>>;
}
