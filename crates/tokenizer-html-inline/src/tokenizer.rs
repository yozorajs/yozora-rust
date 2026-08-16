use std::cell::RefCell;
use std::rc::Rc;

use yozora_ast::Node;
use yozora_core_tokenizer::*;

#[cfg(test)]
use yozora_ast::HTML_TYPE;
#[cfg(test)]
use yozora_character::NodePoint;
#[cfg(test)]
use yozora_core_tokenizer::NodeInterval;

use crate::types::{HtmlInlineDelimiter, HTML_INLINE_TOKENIZER_NAME};
use crate::{parse, r#match};

#[derive(Debug, Clone)]
pub struct HtmlInlineTokenizer {
    meta: TokenizerMeta,
}

impl Default for HtmlInlineTokenizer {
    fn default() -> Self {
        Self::new(TokenizerOptions::default())
    }
}

impl HtmlInlineTokenizer {
    pub fn new(options: TokenizerOptions) -> Self {
        Self {
            meta: TokenizerMeta {
                name: options
                    .name
                    .unwrap_or_else(|| HTML_INLINE_TOKENIZER_NAME.to_string()),
                kind: TokenizerKind::Inline,
                priority: options.priority.unwrap_or(TokenizerPriority::ATOMIC),
            },
        }
    }
}

impl Tokenizer for HtmlInlineTokenizer {
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

pub struct HtmlInlineDelimiterGenerator<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    closer_cache: r#match::HtmlInlineCloserCache,
    last_end_index: Option<usize>,
    last_delimiter: Option<HtmlInlineDelimiter>,
}

impl<'a> HtmlInlineDelimiterGenerator<'a> {
    fn new(api: &'a dyn MatchInlinePhaseApi) -> Self {
        Self {
            api,
            closer_cache: r#match::HtmlInlineCloserCache::default(),
            last_end_index: None,
            last_delimiter: None,
        }
    }

    pub fn next(&mut self, range_index: (usize, usize)) -> Option<HtmlInlineDelimiter> {
        let (start_index, end_index) = range_index;

        if self.last_end_index == Some(end_index) {
            match self.last_delimiter.as_ref() {
                Some(delimiter) if delimiter.delimiter().start_index >= start_index => {
                    return Some(delimiter.clone());
                }
                None => return None,
                _ => {}
            }
        }

        self.last_end_index = Some(end_index);
        self.last_delimiter = r#match::find_html_inline_delimiter(
            self.api.get_node_points(),
            start_index,
            end_index,
            &mut self.closer_cache,
        );
        self.last_delimiter.clone()
    }
}

pub struct HtmlInlineMatchHook<'a> {
    api: &'a dyn MatchInlinePhaseApi,
    delimiters: Rc<RefCell<Vec<HtmlInlineDelimiter>>>,
}

impl<'a> HtmlInlineMatchHook<'a> {
    pub fn new(api: &'a dyn MatchInlinePhaseApi) -> Self {
        Self {
            api,
            delimiters: Rc::new(RefCell::new(Vec::new())),
        }
    }

    pub fn find_delimiter(&self) -> HtmlInlineDelimiterGenerator<'a> {
        HtmlInlineDelimiterGenerator::new(self.api)
    }

    pub fn process_single_delimiter(&self, delimiter: &HtmlInlineDelimiter) -> Vec<InlineToken> {
        r#match::process_single_delimiter(delimiter)
    }

    fn lookup_delimiter(&self, delimiter: &TokenDelimiter) -> Option<HtmlInlineDelimiter> {
        self.delimiters
            .borrow()
            .iter()
            .rev()
            .find(|candidate| candidate.delimiter() == delimiter)
            .cloned()
    }
}

impl<'a> MatchInlineHook<'a> for HtmlInlineMatchHook<'a> {
    fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
        self.delimiters.borrow_mut().clear();
        let mut finder = HtmlInlineMatchHook::find_delimiter(self);
        let delimiters = Rc::clone(&self.delimiters);

        Box::new(gen_find_delimiter(move |start_index, end_index| {
            let delimiter = finder.next((start_index, end_index))?;
            let core_delimiter = delimiter.to_core();
            delimiters.borrow_mut().push(delimiter);
            Some(core_delimiter)
        }))
    }

    fn process_single_delimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
        let Some(delimiter) = self.lookup_delimiter(delimiter) else {
            return Vec::new();
        };

        HtmlInlineMatchHook::process_single_delimiter(self, &delimiter)
    }
}

pub struct HtmlInlineParseHook<'a> {
    api: &'a dyn ParseInlinePhaseApi,
}

impl<'a> HtmlInlineParseHook<'a> {
    pub fn new(api: &'a dyn ParseInlinePhaseApi) -> Self {
        Self { api }
    }
}

impl ParseInlineHook for HtmlInlineParseHook<'_> {
    fn parse(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse::parse_html_inline_tokens(tokens, self.api)
    }
}

impl InlineTokenizer for HtmlInlineTokenizer {
    fn r#match<'a>(
        &'a self,
        api: &'a dyn MatchInlinePhaseApi,
    ) -> Box<dyn MatchInlineHook<'a> + 'a> {
        Box::new(HtmlInlineMatchHook::new(api))
    }

    fn parse<'a>(&'a self, api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
        Box::new(HtmlInlineParseHook::new(api))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_ast::Node;
    use yozora_character::create_node_point_generator;

    struct DummyMatchApi {
        node_points: Vec<NodePoint>,
    }

    impl MatchInlinePhaseApi for DummyMatchApi {
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
    fn engine_match_should_find_html_delimiter() {
        let tokenizer = HtmlInlineTokenizer::default();
        let node_points = create_node_point_generator("<kbd>x</kbd>")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };

        let hook = tokenizer.r#match(&api);
        let mut find_delimiter = hook.find_delimiter();
        let delimiter = find_delimiter
            .next((0, api.get_block_end_index()))
            .expect("expected html delimiter");

        assert_eq!(delimiter.delimiter_type, DelimiterType::Full);
        assert_eq!(delimiter.start_index, 0);
        assert_eq!(delimiter.end_index, 5);
    }

    #[test]
    fn typed_hook_preserves_html_variant() {
        let node_points = create_node_point_generator("<kbd>x</kbd>")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };
        let hook = HtmlInlineMatchHook::new(&api);
        let mut finder = hook.find_delimiter();
        let delimiter = finder
            .next((0, api.get_block_end_index()))
            .expect("expected html delimiter");

        let HtmlInlineDelimiter::Open(delimiter) = delimiter else {
            panic!("expected open delimiter");
        };
        assert_eq!(delimiter.tag_name.start_index, 1);
        assert_eq!(delimiter.tag_name.end_index, 4);
    }

    #[test]
    fn typed_finder_rechecks_closer_when_range_expands() {
        let node_points = create_node_point_generator("<?x?>")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };
        let hook = HtmlInlineMatchHook::new(&api);
        let mut finder = hook.find_delimiter();

        assert!(finder.next((0, api.get_block_end_index() - 1)).is_none());
        assert!(matches!(
            finder.next((0, api.get_block_end_index())),
            Some(HtmlInlineDelimiter::Instruction(_))
        ));
    }

    #[test]
    fn typed_finders_keep_closer_caches_isolated() {
        let node_points = create_node_point_generator("<?ok?> <?")
            .pop()
            .expect("expected node points");
        let api = DummyMatchApi { node_points };
        let hook = HtmlInlineMatchHook::new(&api);
        let mut first_finder = hook.find_delimiter();
        let mut second_finder = hook.find_delimiter();

        assert!(second_finder.next((7, api.get_block_end_index())).is_none());
        let delimiter = first_finder
            .next((0, api.get_block_end_index()))
            .expect("expected processing instruction");
        assert_eq!(delimiter.delimiter().end_index, 6);
    }

    #[test]
    fn engine_parse_should_build_html_node() {
        let tokenizer = HtmlInlineTokenizer::default();
        let node_points = create_node_point_generator("<kbd>x</kbd>")
            .pop()
            .expect("expected node points");
        let api = DummyParseApi { node_points };
        let parse_hook = tokenizer.parse(&api);

        let token = InlineToken::new(HTML_INLINE_TOKENIZER_NAME, HTML_TYPE, (0, 12));
        let nodes = parse_hook.parse(&[token]);

        assert_eq!(nodes.len(), 1);
        let Node::Html(html) = &nodes[0] else {
            panic!("expected html node");
        };
        assert_eq!(html.value, "<kbd>x</kbd>");
    }
}
