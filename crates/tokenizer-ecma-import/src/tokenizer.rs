use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const ECMA_IMPORT_TOKENIZER_NAME: &str = "@yozora/tokenizer-ecma-import";

#[derive(Debug, Clone)]
pub struct EcmaImportTokenizer {
    meta: TokenizerMeta,
}

impl Default for EcmaImportTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: ECMA_IMPORT_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Block,
                priority: TokenizerPriority::ATOMIC,
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

struct EcmaImportMatchHook;

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

struct EcmaImportParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl ParseBlockHook for EcmaImportParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        parse::parse_ecma_import_tokens(tokens, self.api)
    }
}

impl BlockTokenizer for EcmaImportTokenizer {
    fn r#match<'a>(&'a self, _api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(EcmaImportMatchHook)
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(EcmaImportParseHook { api })
    }
}
