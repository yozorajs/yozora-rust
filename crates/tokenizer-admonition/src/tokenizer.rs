use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const ADMONITION_TOKENIZER_NAME: &str = "@yozora/tokenizer-admonition";

#[derive(Debug, Clone)]
pub struct AdmonitionTokenizer {
    meta: TokenizerMeta,
}

impl Default for AdmonitionTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: ADMONITION_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: TokenizerPriority::FENCED_BLOCK,
            },
        }
    }
}

impl Tokenizer for AdmonitionTokenizer {
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

struct AdmonitionMatchHook<'a> {
    api: &'a dyn MatchBlockPhaseApi,
}

impl MatchBlockHook for AdmonitionMatchHook<'_> {
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

    fn eat_continuation_text(
        &mut self,
        line: &PhrasingContentLine,
        token: &mut BlockToken,
        _parent_token: &BlockToken,
    ) -> EatContinuationTextResult {
        r#match::eat_continuation_text(line, token, self.api)
    }

    fn on_close(&mut self, _token: &BlockToken) -> Option<OnCloseResult> {
        None
    }
}

struct AdmonitionParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl ParseBlockHook for AdmonitionParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        parse::parse_admonition_tokens(tokens, self.api)
    }
}

impl BlockTokenizer for AdmonitionTokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(AdmonitionMatchHook { api })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(AdmonitionParseHook { api })
    }
}
