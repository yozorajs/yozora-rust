use yozora_core_tokenizer::*;

use crate::types::THEMATIC_BREAK_TOKENIZER_NAME;
use crate::{parse, r#match};

#[derive(Debug, Clone)]
pub struct ThematicBreakTokenizer {
    meta: TokenizerMeta,
}

impl Default for ThematicBreakTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl ThematicBreakTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| THEMATIC_BREAK_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Block,
                priority: options.priority.unwrap_or(TokenizerPriority::ATOMIC),
            },
        }
    }
}

impl Tokenizer for ThematicBreakTokenizer {
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

#[derive(Debug, Clone, Copy, Default)]
pub struct ThematicBreakMatchHook;

impl ThematicBreakMatchHook {
    pub fn new() -> Self {
        Self
    }
}

impl MatchBlockHook for ThematicBreakMatchHook {
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
}

pub struct ThematicBreakParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl<'a> ThematicBreakParseHook<'a> {
    pub fn new(api: &'a dyn ParseBlockPhaseApi) -> Self {
        Self { api }
    }
}

impl ParseBlockHook for ThematicBreakParseHook<'_> {
    fn parse<'a>(&'a self, tokens: &'a [BlockToken]) -> ParseBlockResult<ParseBlockHookResult<'a>> {
        Ok(parse::parse_thematic_break_tokens(tokens, self.api).into())
    }
}

impl BlockTokenizer for ThematicBreakTokenizer {
    fn r#match<'a>(&'a self, _api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(ThematicBreakMatchHook::new())
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(ThematicBreakParseHook::new(api))
    }
}
