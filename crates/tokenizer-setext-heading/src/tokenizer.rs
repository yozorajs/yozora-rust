use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const SETEXT_HEADING_TOKENIZER_NAME: &str = "@yozora/tokenizer-setext-heading";

#[derive(Debug, Clone)]
pub struct SetextHeadingTokenizer {
    meta: TokenizerMeta,
}

impl Default for SetextHeadingTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: SETEXT_HEADING_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: TokenizerPriority::ATOMIC,
            },
        }
    }
}

impl Tokenizer for SetextHeadingTokenizer {
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

struct SetextHeadingMatchHook<'a> {
    api: &'a dyn MatchBlockPhaseApi,
}

impl MatchBlockHook for SetextHeadingMatchHook<'_> {
    fn is_containing_block(&self) -> bool {
        false
    }

    fn eat_opener(
        &mut self,
        _line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        None
    }

    fn eat_and_interrupt_previous_sibling(
        &mut self,
        line: &PhrasingContentLine,
        prev_sibling_token: &BlockToken,
        _parent_token: &BlockToken,
    ) -> Option<EatAndInterruptPreviousSiblingResult> {
        r#match::eat_and_interrupt_previous_sibling(
            line,
            prev_sibling_token,
            self.api.extractPhrasingLines(prev_sibling_token),
        )
    }
}

struct SetextHeadingParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl ParseBlockHook for SetextHeadingParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        parse::parse_setext_heading_tokens(tokens, self.api)
    }
}

impl BlockTokenizer for SetextHeadingTokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(SetextHeadingMatchHook { api })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(SetextHeadingParseHook { api })
    }
}
