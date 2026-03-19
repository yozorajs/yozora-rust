use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const MATH_TOKENIZER_NAME: &str = "@yozora/tokenizer-math";

#[derive(Debug, Clone)]
pub struct MathTokenizer {
    meta: TokenizerMeta,
}

impl Default for MathTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: MATH_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 11,
            },
        }
    }
}

impl Tokenizer for MathTokenizer {
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

struct MathMatchHook;

impl MatchBlockHook for MathMatchHook {
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

struct MathParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl ParseBlockHook for MathParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        parse::parse_math_tokens(tokens, self.api)
    }
}

impl BlockTokenizer for MathTokenizer {
    fn r#match<'a>(&'a self, _api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(MathMatchHook)
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(MathParseHook { api })
    }
}
