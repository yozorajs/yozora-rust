use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const FENCED_BLOCK_TOKENIZER_NAME: &str = "@yozora/tokenizer-fenced-block";

#[derive(Debug, Clone)]
pub struct FencedBlockTokenizer {
    meta: TokenizerMeta,
}

impl Default for FencedBlockTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: FENCED_BLOCK_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: TokenizerPriority::FENCED_BLOCK,
            },
        }
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

struct EmptyMatchHook;

impl MatchBlockHook for EmptyMatchHook {
    fn is_containing_block(&self) -> bool {
        false
    }

    fn eat_opener(
        &mut self,
        _line: &yozora_core_tokenizer::PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<yozora_core_tokenizer::EatOpenerResult> {
        None
    }
}

struct EmptyParseHook;

impl ParseBlockHook for EmptyParseHook {
    fn parse(&self, _tokens: &[BlockToken]) -> Vec<Node> {
        Vec::new()
    }
}

impl BlockTokenizer for FencedBlockTokenizer {
    fn r#match<'a>(
        &'a self,
        _api: &'a dyn MatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(EmptyMatchHook)
    }

    fn parse<'a>(
        &'a self,
        _api: &'a dyn ParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(EmptyParseHook)
    }
}
