use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const TABLE_TOKENIZER_NAME: &str = "@yozora/tokenizer-table";

#[derive(Debug, Clone)]
pub struct TableTokenizer {
    meta: TokenizerMeta,
}

impl Default for TableTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: TABLE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: 5,
            },
        }
    }
}

impl Tokenizer for TableTokenizer {
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

struct TableMatchHook<'a> {
    api: &'a dyn MatchBlockPhaseApi,
}

impl MatchBlockHook for TableMatchHook<'_> {
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
        r#match::eat_and_interrupt_previous_sibling(line, prev_sibling_token, self.api)
    }

    fn eat_lazy_continuation_text(
        &mut self,
        line: &PhrasingContentLine,
        token: &mut BlockToken,
        _parent_token: &BlockToken,
    ) -> EatLazyContinuationTextResult {
        r#match::eat_lazy_continuation_text(line, token)
    }
}

struct TableParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl ParseBlockHook for TableParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        parse::parse_table_tokens(tokens, self.api)
    }
}

impl BlockTokenizer for TableTokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(TableMatchHook { api })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(TableParseHook { api })
    }
}
