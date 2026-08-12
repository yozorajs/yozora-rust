use yozora_ast::Node;
use yozora_core_tokenizer::*;
use yozora_tokenizer_fenced_block::FencedBlockTokenData;

use crate::{parse, r#match};

pub const ADMONITION_TOKENIZER_NAME: &str = "@yozora/tokenizer-admonition";

#[derive(Debug, Clone)]
pub struct AdmonitionTokenizer {
    meta: TokenizerMeta,
}

impl Default for AdmonitionTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl AdmonitionTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| ADMONITION_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Block,
                priority: options.priority.unwrap_or(TokenizerPriority::FENCED_BLOCK),
            },
        }
    }
}

impl Tokenizer for AdmonitionTokenizer {
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

struct AdmonitionMatchHook<'a> {
    api: &'a dyn MatchBlockPhaseApi,
}

impl MatchBlockHook for AdmonitionMatchHook<'_> {
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
        r#match::eat_continuation_text(line, token)
    }

    fn on_close(&mut self, token: &mut BlockToken) -> Option<OnCloseResult> {
        if let Some(data) = token.data_as::<FencedBlockTokenData>() {
            token.children = self.api.rollback_phrasing_lines(&data.lines, None).into();
        }
        None
    }
}

struct AdmonitionParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl ParseBlockHook for AdmonitionParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        parse::parse_admonition_tokens(tokens, self.api)
    }

    fn parse_task(&self, tokens: &[BlockToken]) -> Option<Box<dyn ParseBlockTask>> {
        Some(parse::create_admonition_parse_task(tokens, self.api))
    }
}

impl BlockTokenizer for AdmonitionTokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(AdmonitionMatchHook { api })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(AdmonitionParseHook { api })
    }
}
