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
                priority: -1,
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
        _api: &dyn MatchInlinePhaseApi,
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
    }

    struct DummyParseApi {
        node_points: Vec<NodePoint>,
    }

    impl ParseInlinePhaseApi for DummyParseApi {
        fn should_reserve_position(&self) -> bool {
            false
        }

        fn calc_position(&self, _interval: NodeInterval) -> Option<yozora_ast::Position> {
            None
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

        fn parse_inline_tokens(&self, _tokens: &[InlineToken]) -> Vec<Node> {
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
