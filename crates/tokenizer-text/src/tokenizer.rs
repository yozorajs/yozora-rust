use yozora_ast::Node;
use yozora_ast::TEXT_TYPE;
use yozora_character::calc_escaped_string_from_node_points;
use yozora_core_tokenizer::engine::{
    EngineInlineFallbackTokenizer, EngineInlineTokenizer, EngineTokenizer, InlineToken,
    MatchInlineHook, MatchInlinePhaseApi as EngineMatchInlinePhaseApi, ParseInlineHook,
    ParseInlinePhaseApi as EngineParseInlinePhaseApi, TokenDelimiter, TokenizerType,
};
use yozora_core_tokenizer::phase::NodeInterval;
use yozora_core_tokenizer::{
    InlineFallbackTokenizer, InlineTokenizer, MatchInlinePhaseApi, ParseInlinePhaseApi, Tokenizer,
    TokenizerKind, TokenizerMeta,
};

use crate::{parse, r#match};

pub const TEXT_TOKENIZER_NAME: &str = "@yozora/tokenizer-text";

#[derive(Debug, Clone)]
pub struct TextTokenizer {
    meta: TokenizerMeta,
}

impl Default for TextTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: TEXT_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: -1,
            },
        }
    }
}

impl Tokenizer for TextTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for TextTokenizer {
    fn tokenize_inline_with_api(
        &self,
        input: &str,
        position: Option<yozora_ast::Position>,
        _api: &dyn MatchInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        self.tokenize_inline(input, position)
    }

    fn tokenize_inline_with_apis(
        &self,
        input: &str,
        position: Option<yozora_ast::Position>,
        _match_api: &dyn MatchInlinePhaseApi,
        _parse_api: &dyn ParseInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        self.tokenize_inline(input, position)
    }
}

impl InlineFallbackTokenizer for TextTokenizer {
    fn build_inline(&self, value: &str, position: Option<yozora_ast::Position>) -> Node {
        parse::parse_text_node(value, position)
    }

    fn find_and_handle_delimiter(
        &self,
        source: &str,
        start_index: usize,
        end_index: usize,
        position: Option<yozora_ast::Position>,
        _api: &dyn MatchInlinePhaseApi,
    ) -> Node {
        let value = r#match::match_text_slice(source, start_index, end_index);
        parse::parse_text_node(value, position)
    }
}

impl EngineTokenizer for TextTokenizer {
    fn tokenizer_type(&self) -> TokenizerType {
        TokenizerType::Inline
    }

    fn name(&self) -> &str {
        &self.meta.name
    }

    fn priority(&self) -> i32 {
        self.meta.priority
    }
}

struct TextMatchHook;

impl MatchInlineHook for TextMatchHook {
    fn find_delimiter(&mut self, _start_index: usize, _end_index: usize) -> Option<TokenDelimiter> {
        None
    }
}

struct TextParseHook<'a> {
    api: &'a dyn EngineParseInlinePhaseApi,
}

impl ParseInlineHook for TextParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        let node_points = self.api.get_node_points();
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            if token.start_index >= token.end_index || token.end_index > node_points.len() {
                continue;
            }

            let position = self.api.calc_position(NodeInterval {
                start_index: token.start_index,
                end_index: token.end_index,
            });

            let value = calc_escaped_string_from_node_points(
                node_points,
                token.start_index,
                token.end_index,
                false,
            );
            nodes.push(parse::parse_text_node(&value, position));
        }

        nodes
    }
}

impl EngineInlineTokenizer for TextTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        _api: &'a dyn EngineMatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(TextMatchHook)
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(TextParseHook { api })
    }
}

impl EngineInlineFallbackTokenizer for TextTokenizer {
    fn find_and_handle_delimiter(
        &self,
        start_index: usize,
        end_index: usize,
        _api: &dyn EngineMatchInlinePhaseApi,
    ) -> InlineToken {
        InlineToken::new(self.meta.name.clone(), TEXT_TYPE, (start_index, end_index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct DummyInlineApi;

    impl MatchInlinePhaseApi for DummyInlineApi {
        fn has_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn has_footnote_definition(&self, _identifier: &str) -> bool {
            false
        }
    }

    #[test]
    fn phase_fallback_should_build_text_from_slice() {
        let tokenizer = TextTokenizer::default();
        let api = DummyInlineApi;

        let node = yozora_core_tokenizer::InlineFallbackTokenizer::find_and_handle_delimiter(
            &tokenizer,
            "hello world",
            6,
            11,
            None,
            &api,
        );
        let Node::Text(text) = node else {
            panic!("expected text node");
        };
        assert_eq!(text.value, "world");
    }
}
