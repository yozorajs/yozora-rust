use yozora_core_tokenizer::*;

use crate::types::FOOTNOTE_DEFINITION_TOKENIZER_NAME;
use crate::{parse, r#match};

#[derive(Debug, Clone)]
pub struct FootnoteDefinitionTokenizer {
    meta: TokenizerMeta,
    pub indent: usize,
}

impl Default for FootnoteDefinitionTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl FootnoteDefinitionTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| FOOTNOTE_DEFINITION_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Block,
                priority: options
                    .priority
                    .unwrap_or(TokenizerPriority::CONTAINING_BLOCK),
            },
            indent: 4,
        }
    }
}

impl Tokenizer for FootnoteDefinitionTokenizer {
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

pub struct FootnoteDefinitionMatchHook<'a> {
    api: &'a dyn MatchBlockPhaseApi,
    indent: usize,
}

impl<'a> FootnoteDefinitionMatchHook<'a> {
    pub fn new(tokenizer: &FootnoteDefinitionTokenizer, api: &'a dyn MatchBlockPhaseApi) -> Self {
        Self {
            api,
            indent: tokenizer.indent,
        }
    }
}

impl MatchBlockHook for FootnoteDefinitionMatchHook<'_> {
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
        r#match::eat_continuation_text(line, token, self.indent)
    }

    fn on_close(&mut self, token: &mut BlockToken) -> Option<OnCloseResult> {
        r#match::on_close(token, self.api);
        None
    }
}

pub struct FootnoteDefinitionParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl<'a> FootnoteDefinitionParseHook<'a> {
    pub fn new(api: &'a dyn ParseBlockPhaseApi) -> Self {
        Self { api }
    }
}

impl ParseBlockHook for FootnoteDefinitionParseHook<'_> {
    fn parse<'a>(&'a self, tokens: &'a [BlockToken]) -> ParseBlockResult<ParseBlockHookResult<'a>> {
        Ok(parse::parse_footnote_definition_tokens(tokens, self.api))
    }
}

impl BlockTokenizer for FootnoteDefinitionTokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(FootnoteDefinitionMatchHook::new(self, api))
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(FootnoteDefinitionParseHook::new(api))
    }
}
