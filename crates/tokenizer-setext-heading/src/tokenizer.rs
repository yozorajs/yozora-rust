use yozora_core_tokenizer::*;

use crate::types::SETEXT_HEADING_TOKENIZER_NAME;
use crate::{parse, r#match};

#[derive(Debug, Clone)]
pub struct SetextHeadingTokenizer {
    meta: TokenizerMeta,
}

impl Default for SetextHeadingTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl SetextHeadingTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| SETEXT_HEADING_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Block,
                priority: options.priority.unwrap_or(TokenizerPriority::ATOMIC),
            },
        }
    }
}

impl Tokenizer for SetextHeadingTokenizer {
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

pub struct SetextHeadingMatchHook<'a> {
    api: &'a dyn MatchBlockPhaseApi,
}

impl<'a> SetextHeadingMatchHook<'a> {
    pub fn new(api: &'a dyn MatchBlockPhaseApi) -> Self {
        Self { api }
    }
}

impl MatchBlockHook for SetextHeadingMatchHook<'_> {
    fn is_containing_block(&self) -> bool {
        false
    }

    fn eat_opener(
        &mut self,
        _line: &PhrasingContentLine,
        _parent_token: &BlockToken,
    ) -> Option<EatOpenerResult> {
        None
    }

    fn eat_and_interrupt_previous_sibling(
        &mut self,
        line: &PhrasingContentLine,
        prev_sibling_token: &BlockToken,
        _parent_token: &BlockToken,
    ) -> Option<EatAndInterruptPreviousSiblingResult> {
        r#match::eat_and_interrupt_previous_sibling(line, prev_sibling_token, self.api)
    }
}

pub struct SetextHeadingParseHook<'a> {
    api: &'a dyn ParseBlockPhaseApi,
}

impl<'a> SetextHeadingParseHook<'a> {
    pub fn new(api: &'a dyn ParseBlockPhaseApi) -> Self {
        Self { api }
    }
}

impl ParseBlockHook for SetextHeadingParseHook<'_> {
    fn parse<'a>(&'a self, tokens: &'a [BlockToken]) -> ParseBlockResult<ParseBlockHookResult<'a>> {
        Ok(parse::parse_setext_heading_tokens(tokens, self.api).into())
    }
}

impl BlockTokenizer for SetextHeadingTokenizer {
    fn r#match<'a>(&'a self, api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
        Box::new(SetextHeadingMatchHook::new(api))
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
        Box::new(SetextHeadingParseHook::new(api))
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;
    use std::sync::Arc;

    use super::*;
    use yozora_character::create_node_point_generator;

    struct CountingApi(Cell<usize>);

    impl MatchBlockPhaseApi for CountingApi {
        fn extract_phrasing_lines(&self, _token: &BlockToken) -> Option<Vec<PhrasingContentLine>> {
            self.0.set(self.0.get() + 1);
            Some(vec![line("heading")])
        }

        fn rollback_phrasing_lines(
            &self,
            _lines: &[PhrasingContentLine],
            _original_token: Option<&BlockToken>,
        ) -> Vec<BlockToken> {
            unreachable!()
        }

        fn register_definition_identifier(&self, _identifier: &str) {}
        fn register_footnote_definition_identifier(&self, _identifier: &str) {}
    }

    fn line(text: &str) -> PhrasingContentLine {
        PhrasingContentLine::whole(Arc::new(create_node_point_generator(text).pop().unwrap()))
    }

    #[test]
    fn only_extracts_previous_lines_after_matching_a_setext_marker() {
        let api = CountingApi(Cell::new(0));
        let mut hook = SetextHeadingMatchHook::new(&api);
        let paragraph = BlockToken::new("paragraph", "paragraph", None);
        let parent = BlockToken::new("root", "root", None);
        for text in ["ordinary text", "[t][old]", "- item", "== text", "- -", " "] {
            assert!(hook
                .eat_and_interrupt_previous_sibling(&line(text), &paragraph, &parent)
                .is_none());
        }
        assert_eq!(api.0.get(), 0);
        for text in ["---", "==="] {
            assert!(hook
                .eat_and_interrupt_previous_sibling(&line(text), &paragraph, &parent)
                .is_some());
        }
        assert_eq!(api.0.get(), 2);
    }
}
