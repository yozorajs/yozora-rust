use yozora_ast::Node;
use yozora_core_tokenizer::engine::{
    BlockToken, EngineBlockTokenizer, EngineTokenizer, MatchBlockHook,
    MatchBlockPhaseApi as EngineMatchBlockPhaseApi, ParseBlockHook,
    ParseBlockPhaseApi as EngineParseBlockPhaseApi, TokenizerType,
};
use yozora_core_tokenizer::{
    BlockTokenizeResult, BlockTokenizer, MatchBlockPhaseApi, Tokenizer, TokenizerKind,
    TokenizerMeta,
};

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
                priority: 0,
            },
        }
    }
}

impl Tokenizer for FencedBlockTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl BlockTokenizer for FencedBlockTokenizer {
    fn tokenize_block_lines(
        &self,
        lines: &[&str],
        position: Option<yozora_ast::Position>,
    ) -> Option<BlockTokenizeResult> {
        let first = *lines.first()?;
        let node = self.tokenize_block(first, position)?;
        Some(BlockTokenizeResult {
            node,
            consumed_lines: 1,
        })
    }

    fn tokenize_block_lines_with_api(
        &self,
        lines: &[&str],
        position: Option<yozora_ast::Position>,
        _api: &mut dyn MatchBlockPhaseApi,
    ) -> Option<BlockTokenizeResult> {
        self.tokenize_block_lines(lines, position)
    }

    fn tokenize_block(&self, input: &str, position: Option<yozora_ast::Position>) -> Option<Node> {
        let token = r#match::match_fenced_block_token(input)?;
        parse::parse_fenced_block_token(token, position)
    }
}

impl EngineTokenizer for FencedBlockTokenizer {
    fn tokenizer_type(&self) -> TokenizerType {
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
        _line: &yozora_core_tokenizer::engine::PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<yozora_core_tokenizer::engine::EatOpenerResult> {
        None
    }
}

struct EmptyParseHook;

impl ParseBlockHook for EmptyParseHook {
    fn parse(&self, _tokens: &[BlockToken]) -> Vec<Node> {
        Vec::new()
    }
}

impl EngineBlockTokenizer for FencedBlockTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        _api: &'a dyn EngineMatchBlockPhaseApi,
    ) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(EmptyMatchHook)
    }

    fn create_parse_hook<'a>(
        &'a self,
        _api: &'a dyn EngineParseBlockPhaseApi,
    ) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(EmptyParseHook)
    }
}
