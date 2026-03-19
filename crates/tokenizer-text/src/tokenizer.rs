use yozora_ast::Node;
use yozora_ast::TEXT_TYPE;
use yozora_character::calc_escaped_string_from_node_points;
use yozora_core_tokenizer::NodeInterval;
use yozora_core_tokenizer::*;

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
                priority: TokenizerPriority::FALLBACK,
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

struct TextMatchHook {
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl MatchInlineHook for TextMatchHook {
    fn reset(&mut self) {
        self.last_end_index = None;
        self.last_delimiter = None;
    }

    fn findDelimiter(&mut self, range_index: (usize, usize)) -> Option<TokenDelimiter> {
        let mut last_end_index = self.last_end_index;
        let mut last_delimiter = self.last_delimiter.clone();
        let delimiter = genFindDelimiter(
            range_index,
            &mut last_end_index,
            &mut last_delimiter,
            |start_index, end_index| Some(r#match::find_text_delimiter(start_index, end_index)),
        );
        self.last_end_index = last_end_index;
        self.last_delimiter = last_delimiter;
        delimiter
    }

    fn processSingleDelimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        r#match::process_single_delimiter(delimiter)
    }
}

struct TextParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl ParseInlineHook for TextParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        let node_points = self.api.getNodePoints();
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            if token.start_index >= token.end_index || token.end_index > node_points.len() {
                continue;
            }

            let position = if self.api.shouldReservePosition() {
                Some(self.api.calcPosition(NodeInterval {
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
    fn r#match<'a>(&'a self, _api: &'a dyn MatchInlinePhaseApi) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(TextMatchHook {
            last_end_index: None,
            last_delimiter: None,
        })
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(TextParseHook { api })
    }
}

impl InlineFallbackTokenizer for TextTokenizer {
    fn findAndHandleDelimiter(
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
        fn hasDefinition(&self, _identifier: &str) -> bool {
            false
        }

        fn hasFootnoteDefinition(&self, _identifier: &str) -> bool {
            false
        }

        fn getNodePoints(&self) -> &[NodePoint] {
            &[]
        }

        fn getBlockStartIndex(&self) -> usize {
            0
        }

        fn getBlockEndIndex(&self) -> usize {
            0
        }

        fn resolveFallbackTokens(
            &self,
            tokens: &[InlineToken],
            _token_start_index: usize,
            _token_end_index: usize,
        ) -> Vec<InlineToken> {
            tokens.to_vec()
        }

        fn resolveInternalTokens(
            &self,
            higher_priority_tokens: &[InlineToken],
            _start_index: usize,
            _end_index: usize,
        ) -> Vec<InlineToken> {
            higher_priority_tokens.to_vec()
        }
    }

    impl MatchInlineFallbackPhaseApi for DummyInlineApi {
        fn hasDefinition(&self, identifier: &str) -> bool {
            MatchInlinePhaseApi::hasDefinition(self, identifier)
        }

        fn hasFootnoteDefinition(&self, identifier: &str) -> bool {
            MatchInlinePhaseApi::hasFootnoteDefinition(self, identifier)
        }

        fn getNodePoints(&self) -> &[NodePoint] {
            MatchInlinePhaseApi::getNodePoints(self)
        }

        fn getBlockStartIndex(&self) -> usize {
            MatchInlinePhaseApi::getBlockStartIndex(self)
        }

        fn getBlockEndIndex(&self) -> usize {
            MatchInlinePhaseApi::getBlockEndIndex(self)
        }

        fn resolveFallbackTokens(
            &self,
            tokens: &[InlineToken],
            token_start_index: usize,
            token_end_index: usize,
        ) -> Vec<InlineToken> {
            MatchInlinePhaseApi::resolveFallbackTokens(
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
        fn shouldReservePosition(&self) -> bool {
            false
        }

        fn calcPosition(&self, _interval: NodeInterval) -> yozora_ast::Position {
            panic!("calcPosition should not be called in this test")
        }

        fn formatUrl(&self, url: &str) -> String {
            url.to_string()
        }

        fn getNodePoints(&self) -> &[NodePoint] {
            &self.node_points
        }

        fn hasDefinition(&self, _identifier: &str) -> bool {
            false
        }

        fn hasFootnoteDefinition(&self, _identifier: &str) -> bool {
            false
        }

        fn parseInlineTokens(&self, _tokens: Option<&[InlineToken]>) -> Vec<Node> {
            Vec::new()
        }
    }

    #[test]
    fn phase_fallback_should_build_text_token_and_parse_node() {
        let tokenizer = TextTokenizer::default();
        let match_api = DummyInlineApi;
        let token = tokenizer.findAndHandleDelimiter(6, 11, &match_api);

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
