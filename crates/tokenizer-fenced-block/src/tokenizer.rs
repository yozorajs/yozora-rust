use yozora_core_tokenizer::{
    BlockToken, BlockTokenizer, MatchBlockHook, MatchBlockPhaseApi, ParseBlockHook,
    ParseBlockHookResult, ParseBlockPhaseApi, ParseBlockResult, Tokenizer, TokenizerMeta,
    TokenizerPriority, TokenizerType,
};

use crate::r#match;
use crate::types::{
    FencedBlockHookContext, FencedBlockTokenizerOptions, FENCED_BLOCK_TOKENIZER_NAME,
};

#[derive(Clone)]
pub struct FencedBlockTokenizer {
    meta: TokenizerMeta,
    context: FencedBlockHookContext,
}

impl Default for FencedBlockTokenizer {
    fn default() -> Self {
        Self::new(FencedBlockTokenizerOptions::default())
    }
}

impl FencedBlockTokenizer {
    pub fn new(options: FencedBlockTokenizerOptions) -> Self {
        let context = FencedBlockHookContext {
            node_type: options.node_type,
            markers: options.markers,
            markers_required: options.markers_required,
            check_info_string: options.check_info_string,
        };
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| FENCED_BLOCK_TOKENIZER_NAME.to_string()),
                kind: yozora_core_tokenizer::TokenizerKind::Block,
                priority: options.priority.unwrap_or(TokenizerPriority::FENCED_BLOCK),
            },
            context,
        }
    }

    pub fn context(&self) -> &FencedBlockHookContext {
        &self.context
    }
}

impl Tokenizer for FencedBlockTokenizer {
    fn r#type(&self) -> TokenizerType {
        TokenizerType::Block
    }

    fn name(&self) -> &str {
        &self.meta.name
    }

    fn priority(&self) -> i32 {
        self.meta.priority
    }
}

struct FencedBlockParseHook;

impl ParseBlockHook for FencedBlockParseHook {
    fn parse<'a>(
        &'a self,
        _tokens: &'a [BlockToken],
    ) -> ParseBlockResult<ParseBlockHookResult<'a>> {
        panic!(
            "[{}] FencedBlockTokenizer is an abstract base tokenizer and does not implement parse().",
            FENCED_BLOCK_TOKENIZER_NAME
        );
    }
}

impl BlockTokenizer for FencedBlockTokenizer {
    fn r#match<'a>(&'a self, _api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(r#match::fenced_block_match(self.context.clone()))
    }

    fn parse<'a>(&'a self, _api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(FencedBlockParseHook)
    }
}
