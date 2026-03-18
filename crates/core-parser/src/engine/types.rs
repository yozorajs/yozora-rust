use std::sync::Arc;

use yozora_ast::Association;
use yozora_core_tokenizer::engine::{
    EngineBlockTokenizer, EngineInlineFallbackTokenizer, EngineInlineTokenizer, PhrasingContentLine,
};

pub type FormatUrlFn = Arc<dyn Fn(&str) -> String + Send + Sync + 'static>;

pub struct ProcessorOptions<'a> {
    pub inline_tokenizers: &'a [Box<dyn EngineInlineTokenizer>],
    pub block_tokenizers: &'a [Box<dyn EngineBlockTokenizer>],
    pub block_fallback_tokenizer: Option<&'a dyn EngineBlockTokenizer>,
    pub inline_fallback_tokenizer: Option<&'a dyn EngineInlineFallbackTokenizer>,
    pub should_reserve_position: bool,
    pub preset_definitions: &'a [Association],
    pub preset_footnote_definitions: &'a [Association],
    pub format_url: FormatUrlFn,
}

pub trait Processor {
    fn process(&mut self, lines: &[Vec<PhrasingContentLine>]) -> yozora_ast::Root;
}
