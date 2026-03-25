use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const DEFINITION_TOKENIZER_NAME: &str = "@yozora/tokenizer-definition";

#[derive(Debug, Clone)]
pub struct DefinitionTokenizer {
    meta: TokenizerMeta,
}

impl Default for DefinitionTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl DefinitionTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| DEFINITION_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Block,
                priority: options.priority.unwrap_or(TokenizerPriority::ATOMIC),
            },
        }
    }
}

impl Tokenizer for DefinitionTokenizer {
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

struct DefinitionMatchHook<'a> {
    api: &'a dyn MatchBlockPhaseApi,
}

impl MatchBlockHook for DefinitionMatchHook<'_> {
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

    fn on_close(&mut self, token: &mut BlockToken) -> Option<OnCloseResult> {
        r#match::on_close(token, self.api)
    }
}

struct DefinitionParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl ParseBlockHook for DefinitionParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        parse::parse_definition_tokens(tokens, self.api)
    }
}

impl BlockTokenizer for DefinitionTokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(DefinitionMatchHook { api })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(DefinitionParseHook { api })
    }
}
