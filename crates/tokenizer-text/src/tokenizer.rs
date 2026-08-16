use yozora_ast::Node;
use yozora_ast::TEXT_TYPE;
use yozora_character::calc_escaped_string_from_node_points;
use yozora_core_tokenizer::NodeInterval;
use yozora_core_tokenizer::*;

use crate::types::{TextDelimiter, TEXT_TOKENIZER_NAME};
use crate::{parse, r#match};

#[derive(Debug, Clone)]
pub struct TextTokenizer {
    meta: TokenizerMeta,
}

impl Default for TextTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl TextTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| TEXT_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Inline,
                priority: options.priority.unwrap_or(TokenizerPriority::FALLBACK),
            },
        }
    }
}

impl Tokenizer for TextTokenizer {
    fn r#type(&self) -> TokenizerType {
        TokenizerType::Inline
    }

    fn name(&self) -> &str {
        &self.meta.name
    }

    fn priority(&self) -> i32 {
        self.meta.priority
    }
}

pub struct TextDelimiterGenerator;

impl TextDelimiterGenerator {
    pub fn next(&mut self, range_index: (usize, usize)) -> Option<TextDelimiter> {
        Some(r#match::find_text_delimiter(range_index.0, range_index.1))
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct TextMatchHook;

impl TextMatchHook {
    pub fn new() -> Self {
        Self
    }

    pub fn find_delimiter(&self) -> TextDelimiterGenerator {
        TextDelimiterGenerator
    }

    pub fn process_single_delimiter(&self, delimiter: &TextDelimiter) -> Vec<InlineToken> {
        r#match::process_single_delimiter(delimiter)
    }
}

impl<'a> MatchInlineHook<'a> for TextMatchHook {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        let mut finder = TextMatchHook::find_delimiter(self);
        Box::new(gen_find_delimiter(move |start_index, end_index| {
            finder.next((start_index, end_index))
        }))
    }

    fn process_single_delimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        TextMatchHook::process_single_delimiter(self, delimiter)
    }
}

pub struct TextParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl<'a> TextParseHook<'a> {
    pub fn new(api: &'a dyn ParseInlinePhaseApi) -> Self {
        Self { api }
    }
}

impl ParseInlineHook for TextParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        let node_points = self.api.get_node_points();
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            if token.start_index >= token.end_index || token.end_index > node_points.len() {
                continue;
            }

            let position = if self.api.should_reserve_position() {
                Some(self.api.calc_position(NodeInterval {
                    start_index: token.start_index,
                    end_index: token.end_index,
                }))
            } else {
                None
            };

            let value = calc_escaped_string_from_node_points(
                node_points,
                token.start_index,
                token.end_index,
                false,
            );
            let value = normalize_soft_line_break_whitespace(value);
            nodes.push(parse::parse_text_node(&value, position));
        }

        nodes
    }
}

fn normalize_soft_line_break_whitespace(input: String) -> String {
    if !input.contains('\n') {
        return input;
    }

    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\n' {
            out.push(ch);
            continue;
        }

        while out.ends_with(' ') || out.ends_with('\t') {
            out.pop();
        }

        out.push('\n');

        while chars
            .peek()
            .is_some_and(|next| *next == ' ' || *next == '\t')
        {
            chars.next();
        }
    }

    out
}

impl InlineTokenizer for TextTokenizer {
    fn r#match<'a>(
        &'a self,
        _api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(TextMatchHook::new())
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(TextParseHook::new(api))
    }
}

impl InlineFallbackTokenizer for TextTokenizer {
    fn find_and_handle_delimiter(
        &self,
        start_index: usize,
        end_index: usize,
        _api: &dyn MatchInlineFallbackPhaseApi,
    ) -> InlineToken {
        InlineToken::new(self.meta.name.clone(), TEXT_TYPE, (start_index, end_index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_character::{create_node_point_generator, NodePoint};

    struct DummyInlineApi;

    impl MatchInlinePhaseApi for DummyInlineApi {
        fn has_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn has_footnote_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn get_node_points(&self) -> &[NodePoint] {
            &[]
        }

        fn get_block_start_index(&self) -> usize {
            0
        }

        fn get_block_end_index(&self) -> usize {
            0
        }

        fn resolve_fallback_tokens(
            &self,
            tokens: &[InlineToken],
            _token_start_index: usize,
            _token_end_index: usize,
        ) -> Vec<InlineToken> {
            tokens.to_vec()
        }

        fn resolve_internal_tokens(
            &self,
            higher_priority_tokens: &[InlineToken],
            _start_index: usize,
            _end_index: usize,
        ) -> Vec<InlineToken> {
            higher_priority_tokens.to_vec()
        }
    }

    impl MatchInlineFallbackPhaseApi for DummyInlineApi {
        fn has_definition(&self, identifier: &str) -> bool {
            MatchInlinePhaseApi::has_definition(self, identifier)
        }

        fn has_footnote_definition(&self, identifier: &str) -> bool {
            MatchInlinePhaseApi::has_footnote_definition(self, identifier)
        }

        fn get_node_points(&self) -> &[NodePoint] {
            MatchInlinePhaseApi::get_node_points(self)
        }

        fn get_block_start_index(&self) -> usize {
            MatchInlinePhaseApi::get_block_start_index(self)
        }

        fn get_block_end_index(&self) -> usize {
            MatchInlinePhaseApi::get_block_end_index(self)
        }

        fn resolve_fallback_tokens(
            &self,
            tokens: &[InlineToken],
            token_start_index: usize,
            token_end_index: usize,
        ) -> Vec<InlineToken> {
            MatchInlinePhaseApi::resolve_fallback_tokens(
                self,
                tokens,
                token_start_index,
                token_end_index,
            )
        }
    }

    struct DummyParseApi {
        node_points: Vec<NodePoint>,
    }

    impl ParseInlinePhaseApi for DummyParseApi {
        fn should_reserve_position(&self) -> bool {
            false
        }

        fn calc_position(&self, _interval: NodeInterval) -> yozora_ast::Position {
            panic!("calc_position should not be called in this test")
        }

        fn format_url(&self, url: &str) -> String {
            url.to_string()
        }

        fn get_node_points(&self) -> &[NodePoint] {
            &self.node_points
        }

        fn has_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn has_footnote_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn parse_inline_tokens(&self, _tokens: Option<&[InlineToken]>) -> Vec<Node> {
            Vec::new()
        }
    }

    #[test]
    fn phase_fallback_should_build_text_token_and_parse_node() {
        let tokenizer = TextTokenizer::default();
        let match_api = DummyInlineApi;
        let token = tokenizer.find_and_handle_delimiter(6, 11, &match_api);

        let parse_api = DummyParseApi {
            node_points: create_node_point_generator("hello world")
                .pop()
                .expect("expected node points"),
        };
        let parse_hook = tokenizer.parse(&parse_api);
        let nodes = parse_hook.parse(&[token]);

        let Some(Node::Text(text)) = nodes.first() else {
            panic!("expected text node");
        };
        assert_eq!(text.value, "world");
    }

    #[test]
    fn parse_should_trim_spaces_around_soft_line_break() {
        let tokenizer = TextTokenizer::default();
        let input = "foo \n baz";
        let node_points = create_node_point_generator(input)
            .pop()
            .expect("expected node points");

        let parse_api = DummyParseApi {
            node_points: node_points.clone(),
        };
        let token = InlineToken::new(
            tokenizer.name().to_string(),
            TEXT_TYPE,
            (0, node_points.len()),
        );

        let parse_hook = tokenizer.parse(&parse_api);
        let nodes = parse_hook.parse(&[token]);

        let Some(Node::Text(text)) = nodes.first() else {
            panic!("expected text node");
        };
        assert_eq!(text.value, "foo\nbaz");
    }
}
