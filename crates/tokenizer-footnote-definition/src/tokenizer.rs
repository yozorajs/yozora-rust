use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const FOOTNOTE_DEFINITION_TOKENIZER_NAME: &str = "@yozora/tokenizer-footnote-definition";

#[derive(Debug, Clone)]
pub struct FootnoteDefinitionTokenizer {
    meta: TokenizerMeta,
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

struct FootnoteDefinitionMatchHook<'a> {
    api: &'a dyn MatchBlockPhaseApi,
    indent: usize,
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
        r#match::eat_opener(line, self.api)
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

struct FootnoteDefinitionParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl ParseBlockHook for FootnoteDefinitionParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        parse::parse_footnote_definition_tokens(tokens, self.api)
    }
}

impl BlockTokenizer for FootnoteDefinitionTokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(FootnoteDefinitionMatchHook { api, indent: 4 })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(FootnoteDefinitionParseHook { api })
    }
}
