use yozora_ast::INLINE_CODE_TYPE;
use yozora_ast::{InlineCode, Node};
use yozora_character::{calc_string_from_node_points, is_space_like, AsciiCodePoint};
use yozora_core_tokenizer::engine::{
    DelimiterType, EngineInlineTokenizer, EngineTokenizer, InlineToken, MatchInlineHook,
    MatchInlinePhaseApi as EngineMatchInlinePhaseApi, ParseInlineHook,
    ParseInlinePhaseApi as EngineParseInlinePhaseApi, TokenDelimiter, TokenizerType,
};
use yozora_core_tokenizer::phase::NodeInterval;
use yozora_core_tokenizer::{
    InlineTokenizer, MatchInlinePhaseApi, ParseInlinePhaseApi, Tokenizer, TokenizerKind,
    TokenizerMeta,
};

use crate::{parse, r#match};

pub const INLINE_CODE_TOKENIZER_NAME: &str = "@yozora/tokenizer-inline-code";

#[derive(Debug, Clone)]
pub struct InlineCodeTokenizer {
    meta: TokenizerMeta,
}

impl Default for InlineCodeTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: INLINE_CODE_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                priority: 10,
            },
        }
    }
}

impl Tokenizer for InlineCodeTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for InlineCodeTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_inline_code_tokens(input)?;
        Some(parse::parse_inline_code_tokens(input, &tokens, None))
    }

    fn tokenize_inline_with_api(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
        _api: &dyn MatchInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_inline_code_tokens(input)?;
        Some(parse::parse_inline_code_tokens(input, &tokens, None))
    }

    fn tokenize_inline_with_apis(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
        _match_api: &dyn MatchInlinePhaseApi,
        parse_api: &dyn ParseInlinePhaseApi,
    ) -> Option<Vec<Node>> {
        let tokens = r#match::match_inline_code_tokens(input)?;
        Some(parse::parse_inline_code_tokens(
            input,
            &tokens,
            Some(parse_api),
        ))
    }
}

#[derive(Debug, Clone)]
struct InlineCodeTokenData {
    thickness: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PotentialDelimiterType {
    Opener,
    Closer,
    Both,
}

#[derive(Debug, Clone, Copy)]
struct PotentialDelimiter {
    delimiter_type: PotentialDelimiterType,
    start_index: usize,
    end_index: usize,
}

impl EngineTokenizer for InlineCodeTokenizer {
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

struct InlineCodeMatchHook {
    potential_delimiters: Vec<PotentialDelimiter>,
    cursor: usize,
    last_end_index: Option<usize>,
    last_delimiter: Option<TokenDelimiter>,
}

impl InlineCodeMatchHook {
    fn new(api: &dyn EngineMatchInlinePhaseApi) -> Self {
        let node_points = api.get_node_points();
        let block_start_index = api.get_block_start_index();
        let block_end_index = api.get_block_end_index();

        Self {
            potential_delimiters: collect_potential_delimiters(
                node_points,
                block_start_index,
                block_end_index,
            ),
            cursor: 0,
            last_end_index: None,
            last_delimiter: None,
        }
    }

    fn find_next_delimiter(&mut self, start_index: usize) -> Option<TokenDelimiter> {
        let len = self.potential_delimiters.len();
        while self.cursor < len {
            while self.cursor < len {
                let delimiter = self.potential_delimiters[self.cursor];
                if delimiter.start_index >= start_index
                    && delimiter.delimiter_type != PotentialDelimiterType::Closer
                {
                    break;
                }
                self.cursor += 1;
            }

            if self.cursor + 1 >= len {
                return None;
            }

            let opener = self.potential_delimiters[self.cursor];
            let thickness = opener.end_index - opener.start_index;

            let mut closer: Option<PotentialDelimiter> = None;
            for i in (self.cursor + 1)..len {
                let delimiter = self.potential_delimiters[i];
                if delimiter.delimiter_type != PotentialDelimiterType::Opener
                    && delimiter.end_index - delimiter.start_index == thickness
                {
                    closer = Some(delimiter);
                    break;
                }
            }

            if let Some(closer) = closer {
                return Some(TokenDelimiter {
                    delimiter_type: DelimiterType::Full,
                    start_index: opener.start_index,
                    end_index: closer.end_index,
                    thickness,
                    original_thickness: thickness,
                });
            }

            self.cursor += 1;
        }

        None
    }
}

impl MatchInlineHook for InlineCodeMatchHook {
    fn reset(&mut self) {
        self.cursor = 0;
        self.last_end_index = None;
        self.last_delimiter = None;
    }

    fn find_delimiter(&mut self, start_index: usize, end_index: usize) -> Option<TokenDelimiter> {
        if self.last_end_index == Some(end_index) {
            match &self.last_delimiter {
                Some(delimiter) if delimiter.start_index >= start_index => {
                    return Some(delimiter.clone());
                }
                None => return None,
                _ => {}
            }
        }

        self.last_end_index = Some(end_index);
        self.last_delimiter = self.find_next_delimiter(start_index);
        self.last_delimiter.clone()
    }

    fn process_single_delimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        vec![InlineToken::new(
            "",
            INLINE_CODE_TYPE,
            (delimiter.start_index, delimiter.end_index),
        )
        .with_data(InlineCodeTokenData {
            thickness: delimiter.thickness,
        })]
    }
}

fn collect_potential_delimiters(
    node_points: &[yozora_character::NodePoint],
    block_start_index: usize,
    block_end_index: usize,
) -> Vec<PotentialDelimiter> {
    let mut delimiters = Vec::new();
    let mut i = block_start_index;

    while i < block_end_index {
        let c = node_points[i].code_point;
        if c == AsciiCodePoint::BACKSLASH as i32 {
            i += 1;
            if i < block_end_index && node_points[i].code_point == AsciiCodePoint::BACKTICK as i32 {
                let j = eat_optional_characters(
                    node_points,
                    i + 1,
                    block_end_index,
                    AsciiCodePoint::BACKTICK as i32,
                );

                delimiters.push(PotentialDelimiter {
                    delimiter_type: PotentialDelimiterType::Closer,
                    start_index: i,
                    end_index: j,
                });

                if j > i + 1 {
                    delimiters.push(PotentialDelimiter {
                        delimiter_type: PotentialDelimiterType::Opener,
                        start_index: i + 1,
                        end_index: j,
                    });
                }

                i = j;
                continue;
            }

            continue;
        }

        if c == AsciiCodePoint::BACKTICK as i32 {
            let start_index = i;
            let end_index = eat_optional_characters(
                node_points,
                i + 1,
                block_end_index,
                AsciiCodePoint::BACKTICK as i32,
            );

            delimiters.push(PotentialDelimiter {
                delimiter_type: PotentialDelimiterType::Both,
                start_index,
                end_index,
            });

            i = end_index;
            continue;
        }

        i += 1;
    }

    delimiters
}

fn eat_optional_characters(
    node_points: &[yozora_character::NodePoint],
    mut start_index: usize,
    end_index: usize,
    code_point: i32,
) -> usize {
    while start_index < end_index && node_points[start_index].code_point == code_point {
        start_index += 1;
    }
    start_index
}

struct InlineCodeParseHook<'a> {
    api: &'a dyn EngineParseInlinePhaseApi,
}

impl ParseInlineHook for InlineCodeParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        let node_points = self.api.get_node_points();
        let mut nodes = Vec::with_capacity(tokens.len());

        for token in tokens {
            if token.start_index >= token.end_index || token.end_index > node_points.len() {
                continue;
            }

            let thickness = token
                .data_as::<InlineCodeTokenData>()
                .map(|x| x.thickness)
                .unwrap_or(0);

            let mut start_index = token.start_index.saturating_add(thickness);
            let mut end_index = token.end_index.saturating_sub(thickness);
            if start_index > end_index || end_index > node_points.len() {
                continue;
            }

            let mut is_all_space = true;
            for point in &node_points[start_index..end_index] {
                if is_space_like(point.code_point) {
                    continue;
                }
                is_all_space = false;
                break;
            }

            if !is_all_space && start_index + 2 < end_index {
                let first_character = node_points[start_index].code_point;
                let last_character = node_points[end_index - 1].code_point;
                if is_space_like(first_character) && is_space_like(last_character) {
                    start_index += 1;
                    end_index -= 1;
                }
            }

            let value = calc_string_from_node_points(node_points, start_index, end_index, false)
                .replace('\n', " ");

            let position = if self.api.should_reserve_position() {
                self.api.calc_position(NodeInterval {
                    start_index: token.start_index,
                    end_index: token.end_index,
                })
            } else {
                None
            };

            nodes.push(Node::InlineCode(InlineCode { position, value }));
        }

        nodes
    }
}

impl EngineInlineTokenizer for InlineCodeTokenizer {
    fn create_match_hook<'a>(
        &'a self,
        api: &'a dyn EngineMatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook + 'a> {
        Box::new(InlineCodeMatchHook::new(api))
    }

    fn create_parse_hook<'a>(
        &'a self,
        api: &'a dyn EngineParseInlinePhaseApi,
    ) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(InlineCodeParseHook { api })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_character::{create_node_point_generator, NodePoint};

    struct DummyMatchApi {
        node_points: Vec<NodePoint>,
    }

    impl EngineMatchInlinePhaseApi for DummyMatchApi {
        fn has_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn has_footnote_definition(&self, _identifier: &str) -> bool {
            false
        }

        fn get_node_points(&self) -> &[NodePoint] {
            &self.node_points
        }

        fn get_block_start_index(&self) -> usize {
            0
        }

        fn get_block_end_index(&self) -> usize {
            self.node_points.len()
        }

        fn resolve_fallback_tokens(
            &self,
            _tokens: &[InlineToken],
            _token_start_index: usize,
            _token_end_index: usize,
        ) -> Vec<InlineToken> {
            Vec::new()
        }

        fn resolve_internal_tokens(
            &self,
            _higher_priority_tokens: &[InlineToken],
            _start_index: usize,
            _end_index: usize,
        ) -> Vec<InlineToken> {
            Vec::new()
        }
    }

    struct DummyParseApi {
        node_points: Vec<NodePoint>,
    }

    impl EngineParseInlinePhaseApi for DummyParseApi {
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
    fn engine_match_should_emit_full_backtick_delimiter() {
        let tokenizer = InlineCodeTokenizer::default();
        let node_points = create_node_point_generator("`foo`")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };

        let mut hook = tokenizer.create_match_hook(&api);
        let delimiter = hook
            .find_delimiter(0, api.get_block_end_index())
            .expect("expected delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Full);
        assert_eq!(delimiter.start_index, 0);
        assert_eq!(delimiter.end_index, 5);
        assert_eq!(delimiter.thickness, 1);
    }

    #[test]
    fn engine_parse_should_trim_boundary_space_like() {
        let tokenizer = InlineCodeTokenizer::default();
        let node_points = create_node_point_generator("` foo `")
            .pop()
            .expect("expected node points");

        let parse_api = DummyParseApi { node_points };
        let parse_hook = tokenizer.create_parse_hook(&parse_api);

        let token = InlineToken::new(INLINE_CODE_TOKENIZER_NAME, INLINE_CODE_TYPE, (0, 7))
            .with_data(InlineCodeTokenData { thickness: 1 });
        let nodes = parse_hook.parse(&[token]);

        assert_eq!(nodes.len(), 1);
        let Node::InlineCode(inline_code) = &nodes[0] else {
            panic!("expected inline code node");
        };
        assert_eq!(inline_code.value, "foo");
    }
}
