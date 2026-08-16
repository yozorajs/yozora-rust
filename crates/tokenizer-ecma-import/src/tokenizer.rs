use yozora_core_tokenizer::*;

use crate::types::ECMA_IMPORT_TOKENIZER_NAME;
use crate::{parse, r#match};

#[derive(Debug, Clone)]
pub struct EcmaImportTokenizer {
    meta: TokenizerMeta,
}

impl Default for EcmaImportTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl EcmaImportTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| ECMA_IMPORT_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Block,
                priority: options.priority.unwrap_or(TokenizerPriority::ATOMIC),
            },
        }
    }
}

impl Tokenizer for EcmaImportTokenizer {
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
pub struct EcmaImportMatchHook;

impl EcmaImportMatchHook {
    pub fn new() -> Self {
        Self
    }
}

impl MatchBlockHook for EcmaImportMatchHook {
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
}

pub struct EcmaImportParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl<'a> EcmaImportParseHook<'a> {
    pub fn new(api: &'a dyn ParseBlockPhaseApi) -> Self {
        Self { api }
    }
}

impl ParseBlockHook for EcmaImportParseHook<'_> {
    fn parse<'a>(&'a self, tokens: &'a [BlockToken]) -> ParseBlockResult<ParseBlockHookResult<'a>> {
        Ok(parse::parse_ecma_import_tokens(tokens, self.api).into())
    }
}

impl BlockTokenizer for EcmaImportTokenizer {
    fn r#match<'a>(&'a self, _api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(EcmaImportMatchHook::new())
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(EcmaImportParseHook::new(api))
    }
}
