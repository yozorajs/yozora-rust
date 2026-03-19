use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const INDENTED_CODE_TOKENIZER_NAME: &str = "@yozora/tokenizer-indented-code";

#[derive(Debug, Clone)]
pub struct IndentedCodeTokenizer {
    meta: TokenizerMeta,
}

impl Default for IndentedCodeTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: INDENTED_CODE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 10,
            },
        }
    }
}

impl Tokenizer for IndentedCodeTokenizer {
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

struct IndentedCodeMatchHook;

impl MatchBlockHook for IndentedCodeMatchHook {
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

    fn eat_continuation_text(
        &mut self,
        line: &PhrasingContentLine,
        token: &mut BlockToken,
        _parent_token: &BlockToken,
    ) -> EatContinuationTextResult {
        r#match::eat_continuation_text(line, token)
    }
}

struct IndentedCodeParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl ParseBlockHook for IndentedCodeParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        parse::parse_indented_code_tokens(tokens, self.api)
    }
}

impl BlockTokenizer for IndentedCodeTokenizer {
    fn r#match<'a>(&'a self, _api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(IndentedCodeMatchHook)
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(IndentedCodeParseHook { api })
    }
}
