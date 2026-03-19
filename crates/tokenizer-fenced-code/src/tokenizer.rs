use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const FENCED_CODE_TOKENIZER_NAME: &str = "@yozora/tokenizer-fenced-code";

#[derive(Debug, Clone)]
pub struct FencedCodeTokenizer {
    meta: TokenizerMeta,
}

impl Default for FencedCodeTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: FENCED_CODE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 10,
            },
        }
    }
}

impl Tokenizer for FencedCodeTokenizer {
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

struct FencedCodeMatchHook;

impl MatchBlockHook for FencedCodeMatchHook {
    fn is_containing_block(&self) -> bool {
        false
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
        token: &mut BlockToken,
        _parent_token: &BlockToken,
    ) -> EatContinuationTextResult {
        r#match::eat_continuation_text(line, token)
    }
}

struct FencedCodeParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl ParseBlockHook for FencedCodeParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        parse::parse_fenced_code_tokens(tokens, self.api)
    }
}

impl BlockTokenizer for FencedCodeTokenizer {
    fn r#match<'a>(&'a self, _api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(FencedCodeMatchHook)
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(FencedCodeParseHook { api })
    }
}
