use std::collections::HashMap;

use yozora_ast::Association;
use yozora_core_tokenizer::{
    BlockTokenizer, InlineFallbackTokenizer, InlineTokenizer, PhrasingContentLine,
};

use crate::types::FormatUrlFn;

pub struct ProcessorOptions<'a> {
    pub inline_tokenizers: &'a [Box<dyn InlineTokenizer>],
    pub inline_tokenizer_map: &'a HashMap<String, usize>,
    pub block_tokenizers: &'a [Box<dyn BlockTokenizer>],
    pub block_tokenizer_map: &'a HashMap<String, usize>,
    pub block_fallback_tokenizer: Option<&'a dyn BlockTokenizer>,
    pub inline_fallback_tokenizer: Option<&'a dyn InlineFallbackTokenizer>,
    pub should_reserve_position: bool,
    pub preset_definitions: &'a [Association],
    pub preset_footnote_definitions: &'a [Association],
    pub format_url: FormatUrlFn,
}

pub trait Processor {
    fn process(&mut self, lines: &[Vec<PhrasingContentLine>]) -> yozora_ast::Root;
}
