use std::collections::HashMap;

use yozora_ast::Association;
use yozora_core_tokenizer::{
    BlockTokenizer, InlineFallbackTokenizer, InlineTokenizer, TokenizerId,
};

use crate::types::FormatUrlFn;

pub struct ProcessorOptions<'a> {
    pub inline_tokenizers: &'a [Box<dyn InlineTokenizer>],
    pub inline_tokenizer_map: &'a HashMap<String, usize>,
    pub inline_tokenizer_uid_map: &'a HashMap<String, TokenizerId>,
    pub block_tokenizers: &'a [Box<dyn BlockTokenizer>],
    pub block_tokenizer_map: &'a HashMap<String, usize>,
    pub block_tokenizer_uid_map: &'a HashMap<String, TokenizerId>,
    pub block_fallback_tokenizer: Option<&'a dyn BlockTokenizer>,
    pub block_fallback_tokenizer_uid: Option<TokenizerId>,
    pub inline_fallback_tokenizer: Option<&'a dyn InlineFallbackTokenizer>,
    pub inline_fallback_tokenizer_uid: Option<TokenizerId>,
    pub should_reserve_position: bool,
    pub preset_definitions: &'a [Association],
    pub preset_footnote_definitions: &'a [Association],
    pub format_url: FormatUrlFn,
}

pub trait Processor {
    fn process<L>(&mut self, lines: L) -> yozora_ast::Root
    where
        L: IntoIterator<Item = Vec<yozora_core_tokenizer::PhrasingContentLine>>;
}
