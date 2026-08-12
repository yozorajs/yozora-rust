use yozora_character::NodePoint;
use yozora_core_parser::{
    create_phrasing_content_processor, create_processor_hook_groups, ProcessorApis,
};
use yozora_core_tokenizer::{
    FindDelimiterGenerator, InlineToken, InlineTokenizer, MatchInlineFallbackPhaseApi,
    MatchInlineHook, MatchInlinePhaseApi, ParseInlineHook, ParseInlinePhaseApi, TokenDelimiter,
    Tokenizer, TokenizerType,
};

struct EmptyDelimiterGenerator;

impl FindDelimiterGenerator for EmptyDelimiterGenerator {
    fn next(&mut self, _: (usize, usize)) -> Option<TokenDelimiter> {
        None
    }
}

struct EmptyMatchHook;

impl<'a> MatchInlineHook<'a> for EmptyMatchHook {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        Box::new(EmptyDelimiterGenerator)
    }
}

struct EmptyParseHook;

impl ParseInlineHook for EmptyParseHook {
    fn parse(&self, _: &[InlineToken]) -> Vec<yozora_ast::Node> {
        Vec::new()
    }
}

struct DummyTokenizer;

impl Tokenizer for DummyTokenizer {
    fn r#type(&self) -> TokenizerType {
        TokenizerType::Inline
    }

    fn name(&self) -> &str {
        "dummy"
    }

    fn priority(&self) -> i32 {
        1
    }
}

impl InlineTokenizer for DummyTokenizer {
    fn r#match<'a>(&'a self, _: &'a dyn MatchInlinePhaseApi) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(EmptyMatchHook)
    }

    fn parse<'a>(&'a self, _: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(EmptyParseHook)
    }
}

struct BaseApi {
    node_points: Vec<NodePoint>,
}

impl MatchInlineFallbackPhaseApi for BaseApi {
    fn has_definition(&self, _: &str) -> bool {
        false
    }

    fn has_footnote_definition(&self, _: &str) -> bool {
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
        tokens: &[InlineToken],
        _: usize,
        _: usize,
    ) -> Vec<InlineToken> {
        tokens.to_vec()
    }
}

#[test]
fn public_inline_factories_accept_base_api_and_fallback_resolver() {
    let api = BaseApi {
        node_points: Vec::new(),
    };
    let tokenizers: Vec<Box<dyn yozora_core_tokenizer::InlineTokenizer>> =
        vec![Box::new(DummyTokenizer)];
    let resolve_fallback = |tokens: &[InlineToken], _: usize, _: usize| tokens.to_vec();
    let groups = create_processor_hook_groups(&tokenizers, &api, &resolve_fallback);
    let mut processor = create_phrasing_content_processor(groups, 0);
    let input = vec![InlineToken::new("text", "text", (0, 1))];

    assert_eq!(processor.process(&input, 0, 1).len(), 1);
}

#[test]
fn processor_apis_exposes_base_match_inline_api() {
    fn assert_match_inline_api(_: &dyn MatchInlineFallbackPhaseApi) {}

    fn inspect(apis: &ProcessorApis<'_>) {
        assert_match_inline_api(apis.match_inline_api);
    }

    let _ = inspect;
    let _ = std::mem::size_of::<Option<&dyn MatchInlinePhaseApi>>();
    let _ = std::mem::size_of::<Option<&dyn ParseInlinePhaseApi>>();
    let _ = std::mem::size_of::<Option<&dyn ParseInlineHook>>();
    let _ = std::mem::size_of::<Option<&dyn Tokenizer>>();
    let _ = TokenizerType::Inline;
}
