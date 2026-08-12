use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const BLOCKQUOTE_TOKENIZER_NAME: &str = "@yozora/tokenizer-blockquote";

#[derive(Debug, Clone)]
pub struct BlockquoteTokenizer {
    meta: TokenizerMeta,
}

impl Default for BlockquoteTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl BlockquoteTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| BLOCKQUOTE_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Block,
                priority: options
                    .priority
                    .unwrap_or(TokenizerPriority::CONTAINING_BLOCK),
            },
        }
    }
}

impl Tokenizer for BlockquoteTokenizer {
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

struct BlockquoteMatchHook;

impl MatchBlockHook for BlockquoteMatchHook {
    fn is_containing_block(&self) -> bool {
        true
    }

    fn eat_opener(
        &mut self,
        line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        r#match::eat_opener(line)
    }

    fn eat_and_interrupt_previous_sibling(
        &mut self,
        line: &PhrasingContentLine,
        prev_sibling_token: &BlockToken,
        _parent_token: &BlockToken,
    ) -> Option<EatAndInterruptPreviousSiblingResult> {
        r#match::eat_and_interrupt_previous_sibling(line, prev_sibling_token)
    }

    fn eat_continuation_text(
        &mut self,
        line: &PhrasingContentLine,
        _token: &mut BlockToken,
        parent_token: &BlockToken,
    ) -> EatContinuationTextResult {
        r#match::eat_continuation_text(line, parent_token)
    }
}

struct BlockquoteParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl ParseBlockHook for BlockquoteParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        parse::parse_blockquote_tokens(tokens, self.api)
    }
}

impl BlockTokenizer for BlockquoteTokenizer {
    fn r#match<'a>(&'a self, _api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(BlockquoteMatchHook)
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(BlockquoteParseHook { api })
    }
}
