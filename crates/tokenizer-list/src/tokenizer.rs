use yozora_ast::NodeType;
use yozora_core_tokenizer::*;

use crate::types::{ListTokenizerOptions, LIST_TOKENIZER_NAME};
use crate::{parse, r#match};

#[derive(Debug, Clone)]
pub struct ListTokenizer {
    meta: TokenizerMeta,
    pub enable_task_list_item: bool,
    pub empty_item_could_not_interrupted_types: Vec<NodeType>,
}

impl Default for ListTokenizer {
    fn default() -> Self {
        Self::new(ListTokenizerOptions::default())
    }
}

impl ListTokenizer {
    pub fn new(options: ListTokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| LIST_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Block,
                priority: options
                    .priority
                    .unwrap_or(TokenizerPriority::CONTAINING_BLOCK),
            },
            enable_task_list_item: options.enable_task_list_item,
            empty_item_could_not_interrupted_types: options.empty_item_could_not_interrupted_types,
        }
    }
}

impl Tokenizer for ListTokenizer {
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

struct ListMatchHook {
    enable_task_list_item: bool,
    empty_item_could_not_interrupted_types: Vec<NodeType>,
}

impl MatchBlockHook for ListMatchHook {
    fn is_containing_block(&self) -> bool {
        true
    }

    fn eat_opener(
        &mut self,
        line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        r#match::eat_opener(line, self.enable_task_list_item)
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
            &self.empty_item_could_not_interrupted_types,
            self.enable_task_list_item,
        )
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

struct ListParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl ParseBlockHook for ListParseHook<'_> {
    fn parse<'a>(&'a self, tokens: &'a [BlockToken]) -> ParseBlockResult<ParseBlockHookResult<'a>> {
        Ok(parse::parse_list_tokens(tokens, self.api))
    }
}

impl BlockTokenizer for ListTokenizer {
    fn r#match<'a>(&'a self, _api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(ListMatchHook {
            enable_task_list_item: self.enable_task_list_item,
            empty_item_could_not_interrupted_types: self
                .empty_item_could_not_interrupted_types
                .clone(),
        })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(ListParseHook { api })
    }
}
