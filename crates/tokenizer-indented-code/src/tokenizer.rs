use yozora_core_tokenizer::*;

use crate::types::INDENTED_CODE_TOKENIZER_NAME;
use crate::{parse, r#match};

#[derive(Debug, Clone)]
pub struct IndentedCodeTokenizer {
    meta: TokenizerMeta,
}

impl Default for IndentedCodeTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl IndentedCodeTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| INDENTED_CODE_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Block,
                priority: options.priority.unwrap_or(TokenizerPriority::ATOMIC),
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

#[derive(Debug, Clone, Copy, Default)]
pub struct IndentedCodeMatchHook;

impl IndentedCodeMatchHook {
    pub fn new() -> Self {
        Self
    }
}

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

pub struct IndentedCodeParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl<'a> IndentedCodeParseHook<'a> {
    pub fn new(api: &'a dyn ParseBlockPhaseApi) -> Self {
        Self { api }
    }
}

impl ParseBlockHook for IndentedCodeParseHook<'_> {
    fn parse<'a>(&'a self, tokens: &'a [BlockToken]) -> ParseBlockResult<ParseBlockHookResult<'a>> {
        Ok(parse::parse_indented_code_tokens(tokens, self.api).into())
    }
}

impl BlockTokenizer for IndentedCodeTokenizer {
    fn r#match<'a>(&'a self, _api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(IndentedCodeMatchHook::new())
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(IndentedCodeParseHook::new(api))
    }
}
