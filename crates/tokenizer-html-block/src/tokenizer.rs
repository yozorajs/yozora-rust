use yozora_ast::Node;
use yozora_core_tokenizer::*;

use crate::{parse, r#match};

pub const HTML_BLOCK_TOKENIZER_NAME: &str = "@yozora/tokenizer-html-block";

#[derive(Debug, Clone)]
pub struct HtmlBlockTokenizer {
    meta: TokenizerMeta,
}

impl Default for HtmlBlockTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl HtmlBlockTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| HTML_BLOCK_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Block,
                priority: options.priority.unwrap_or(TokenizerPriority::ATOMIC),
            },
        }
    }
}

impl Tokenizer for HtmlBlockTokenizer {
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

struct HtmlBlockMatchHook;

impl MatchBlockHook for HtmlBlockMatchHook {
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

    fn eat_continuation_text(
        &mut self,
        line: &PhrasingContentLine,
        token: &mut BlockToken,
        _parent_token: &BlockToken,
    ) -> EatContinuationTextResult {
        r#match::eat_continuation_text(line, token)
    }
}

struct HtmlBlockParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl ParseBlockHook for HtmlBlockParseHook<'_> {
    fn parse(&self, tokens: &[BlockToken]) -> Vec<Node> {
        parse::parse_html_block_tokens(tokens, self.api)
    }
}

impl BlockTokenizer for HtmlBlockTokenizer {
    fn r#match<'a>(&'a self, _api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(HtmlBlockMatchHook)
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(HtmlBlockParseHook { api })
    }
}
