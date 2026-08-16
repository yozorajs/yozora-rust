use std::collections::HashMap;
use std::sync::Arc;

use yozora_ast::Root;
use yozora_character::create_node_point_generator;
use yozora_core_tokenizer::encode_link_destination;
use yozora_core_tokenizer::{AnyFallbackTokenizer, AnyTokenizer};
use yozora_core_tokenizer::{BlockTokenizer, InlineFallbackTokenizer, InlineTokenizer};

use crate::processor::{create_processor, Processor, ProcessorOptions};
use crate::types::{DefaultParserProps, FormatUrlFn, ParseContents, ParseOptions, Parser};
use crate::util::phrasing_line::create_phrasing_line_generator;

#[derive(Clone)]
struct ResolvedParseOptions {
    should_reserve_position: bool,
    preset_definitions: Vec<yozora_ast::Association>,
    preset_footnote_definitions: Vec<yozora_ast::Association>,
    format_url: FormatUrlFn,
}
#[derive(Default)]
pub struct DefaultParser {
    block_tokenizers: Vec<Box<dyn BlockTokenizer>>,
    inline_tokenizers: Vec<Box<dyn InlineTokenizer>>,
    block_tokenizer_map: HashMap<String, usize>,
    inline_tokenizer_map: HashMap<String, usize>,
    block_fallback_tokenizer: Option<Box<dyn BlockTokenizer>>,
    inline_fallback_tokenizer: Option<Box<dyn InlineFallbackTokenizer>>,
    default_parse_options: ParseOptions,
}

impl DefaultParser {
    pub fn new(props: DefaultParserProps) -> Self {
        let mut parser = Self::default();
        parser.set_default_parse_options(props.default_parse_options);

        if let Some(tokenizer) = props.block_fallback_tokenizer {
            parser.use_fallback_tokenizer(AnyFallbackTokenizer::Block(tokenizer));
        }
        if let Some(tokenizer) = props.inline_fallback_tokenizer {
            parser.use_fallback_tokenizer(AnyFallbackTokenizer::Inline(tokenizer));
        }

        parser
    }

    pub fn use_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> &mut Self {
        match tokenizer {
            AnyTokenizer::Block(tokenizer) => {
                if let Err(err) =
                    self.register_block_tokenizer(tokenizer, register_before_tokenizer)
                {
                    panic!("{err}");
                }
            }
            AnyTokenizer::Inline(tokenizer) => {
                if let Err(err) =
                    self.register_inline_tokenizer(tokenizer, register_before_tokenizer)
                {
                    panic!("{err}");
                }
            }
        }
        self
    }

    pub fn replace_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> &mut Self {
        match tokenizer {
            AnyTokenizer::Block(tokenizer) => {
                let name = tokenizer.name().to_string();
                if let Some(index) = self.block_tokenizer_map.get(&name).copied() {
                    if register_before_tokenizer.is_none()
                        && self.block_tokenizers[index].priority() == tokenizer.priority()
                    {
                        self.block_tokenizers[index] = tokenizer;
                        return self;
                    }
                }
                self.unmount_block_tokenizer(&name);
                if let Err(err) =
                    self.register_block_tokenizer(tokenizer, register_before_tokenizer)
                {
                    panic!("{err}");
                }
            }
            AnyTokenizer::Inline(tokenizer) => {
                let name = tokenizer.name().to_string();
                if let Some(index) = self.inline_tokenizer_map.get(&name).copied() {
                    if register_before_tokenizer.is_none()
                        && self.inline_tokenizers[index].priority() == tokenizer.priority()
                    {
                        self.inline_tokenizers[index] = tokenizer;
                        return self;
                    }
                }
                self.unmount_inline_tokenizer(&name);
                if let Err(err) =
                    self.register_inline_tokenizer(tokenizer, register_before_tokenizer)
                {
                    panic!("{err}");
                }
            }
        }
        self
    }

    pub fn unmount_tokenizer(&mut self, tokenizer_name: &str) -> &mut Self {
        self.unmount_inline_tokenizer(tokenizer_name);
        self.unmount_block_tokenizer(tokenizer_name);

        if self
            .block_fallback_tokenizer
            .as_ref()
            .is_some_and(|tokenizer| tokenizer.name() == tokenizer_name)
        {
            self.block_fallback_tokenizer = None;
        }

        if self
            .inline_fallback_tokenizer
            .as_ref()
            .is_some_and(|tokenizer| tokenizer.name() == tokenizer_name)
        {
            self.inline_fallback_tokenizer = None;
        }

        self
    }

    pub fn use_fallback_tokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self {
        match tokenizer {
            AnyFallbackTokenizer::Block(tokenizer) => {
                let name = tokenizer.name();
                if self.block_tokenizer_map.contains_key(name) {
                    panic!("[useFallbackTokenizer] Name({name}) has been registered.");
                }

                self.block_fallback_tokenizer = Some(tokenizer);
            }
            AnyFallbackTokenizer::Inline(tokenizer) => {
                let name = tokenizer.name();
                if self.inline_tokenizer_map.contains_key(name) {
                    panic!("[useFallbackTokenizer] Name({name}) has been registered.");
                }

                self.inline_fallback_tokenizer = Some(tokenizer);
            }
        }
        self
    }

    pub fn set_default_parse_options(&mut self, options: Option<ParseOptions>) {
        self.default_parse_options = options.unwrap_or_default();
    }

    pub fn parse<'a, C>(&self, contents: C, options: Option<ParseOptions>) -> Root
    where
        C: Into<ParseContents<'a>>,
    {
        let parse_options = self.resolve_parse_options(options.unwrap_or_default());
        let chunks = normalize_contents_to_chunks(contents.into());
        let node_point_chunks = create_node_point_generator(chunks);
        let lines_iterator = create_phrasing_line_generator(node_point_chunks);

        let mut processor = create_processor(ProcessorOptions {
            inline_tokenizers: &self.inline_tokenizers,
            inline_tokenizer_map: &self.inline_tokenizer_map,
            block_tokenizers: &self.block_tokenizers,
            block_tokenizer_map: &self.block_tokenizer_map,
            block_fallback_tokenizer: self.block_fallback_tokenizer.as_deref(),
            inline_fallback_tokenizer: self.inline_fallback_tokenizer.as_deref(),
            should_reserve_position: parse_options.should_reserve_position,
            preset_definitions: &parse_options.preset_definitions,
            preset_footnote_definitions: &parse_options.preset_footnote_definitions,
            format_url: parse_options.format_url,
        });

        processor.process(lines_iterator)
    }

    fn resolve_parse_options(&self, options: ParseOptions) -> ResolvedParseOptions {
        ResolvedParseOptions {
            should_reserve_position: options
                .should_reserve_position
                .or(self.default_parse_options.should_reserve_position)
                .unwrap_or(false),
            preset_definitions: options
                .preset_definitions
                .or_else(|| self.default_parse_options.preset_definitions.clone())
                .unwrap_or_default(),
            preset_footnote_definitions: options
                .preset_footnote_definitions
                .or_else(|| {
                    self.default_parse_options
                        .preset_footnote_definitions
                        .clone()
                })
                .unwrap_or_default(),
            format_url: options
                .format_url
                .or_else(|| self.default_parse_options.format_url.clone())
                .unwrap_or_else(|| Arc::new(|url: &str| encode_link_destination(url))),
        }
    }

    fn register_block_tokenizer(
        &mut self,
        tokenizer: Box<dyn BlockTokenizer>,
        register_before_tokenizer: Option<&str>,
    ) -> Result<(), String> {
        let name = tokenizer.name().to_string();
        if self.block_tokenizer_map.contains_key(&name) {
            return Err(format!("[useTokenizer] Name({name}) has been registered."));
        }

        if self
            .block_fallback_tokenizer
            .as_ref()
            .is_some_and(|fallback| fallback.name() == name)
        {
            return Err(format!("[useTokenizer] Name({name}) has been registered."));
        }

        let insert_index = calc_insert_index_block(
            &self.block_tokenizers,
            tokenizer.priority(),
            register_before_tokenizer,
        );
        self.block_tokenizers.insert(insert_index, tokenizer);
        self.rebuild_block_index();
        Ok(())
    }

    fn register_inline_tokenizer(
        &mut self,
        tokenizer: Box<dyn InlineTokenizer>,
        register_before_tokenizer: Option<&str>,
    ) -> Result<(), String> {
        let name = tokenizer.name().to_string();
        if self.inline_tokenizer_map.contains_key(&name) {
            return Err(format!("[useTokenizer] Name({name}) has been registered."));
        }

        if self
            .inline_fallback_tokenizer
            .as_ref()
            .is_some_and(|fallback| fallback.name() == name)
        {
            return Err(format!("[useTokenizer] Name({name}) has been registered."));
        }

        let insert_index = calc_insert_index_inline(
            &self.inline_tokenizers,
            tokenizer.priority(),
            register_before_tokenizer,
        );
        self.inline_tokenizers.insert(insert_index, tokenizer);
        self.rebuild_inline_index();
        Ok(())
    }

    fn unmount_block_tokenizer(&mut self, tokenizer_name: &str) {
        if let Some(index) = self.block_tokenizer_map.remove(tokenizer_name) {
            self.block_tokenizers.remove(index);
            self.rebuild_block_index();
        }
    }

    fn unmount_inline_tokenizer(&mut self, tokenizer_name: &str) {
        if let Some(index) = self.inline_tokenizer_map.remove(tokenizer_name) {
            self.inline_tokenizers.remove(index);
            self.rebuild_inline_index();
        }
    }

    fn rebuild_block_index(&mut self) {
        self.block_tokenizer_map.clear();

        for (idx, tokenizer) in self.block_tokenizers.iter().enumerate() {
            let name = tokenizer.name().to_string();
            self.block_tokenizer_map.insert(name, idx);
        }
    }

    fn rebuild_inline_index(&mut self) {
        self.inline_tokenizer_map.clear();

        for (idx, tokenizer) in self.inline_tokenizers.iter().enumerate() {
            let name = tokenizer.name().to_string();
            self.inline_tokenizer_map.insert(name, idx);
        }
    }
}

fn normalize_contents_to_chunks(contents: ParseContents<'_>) -> Vec<String> {
    match contents {
        ParseContents::Text(input) => vec![input.to_string()],
        ParseContents::Chunks(chunks) => chunks.iter().map(|chunk| (*chunk).to_string()).collect(),
        ParseContents::OwnedText(input) => vec![input],
        ParseContents::OwnedChunks(chunks) => chunks,
    }
}

fn calc_insert_index_block(
    tokenizers: &[Box<dyn BlockTokenizer>],
    priority: i32,
    register_before_tokenizer: Option<&str>,
) -> usize {
    for (index, existing) in tokenizers.iter().enumerate() {
        if register_before_tokenizer.is_some_and(|target| target == existing.name()) {
            return index;
        }
        if priority > existing.priority() {
            return index;
        }
    }

    tokenizers.len()
}

fn calc_insert_index_inline(
    tokenizers: &[Box<dyn InlineTokenizer>],
    priority: i32,
    register_before_tokenizer: Option<&str>,
) -> usize {
    for (index, existing) in tokenizers.iter().enumerate() {
        if register_before_tokenizer.is_some_and(|target| target == existing.name()) {
            return index;
        }
        if priority > existing.priority() {
            return index;
        }
    }

    tokenizers.len()
}

impl Parser for DefaultParser {
    fn use_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> &mut Self {
        DefaultParser::use_tokenizer(self, tokenizer, register_before_tokenizer)
    }

    fn replace_tokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        register_before_tokenizer: Option<&str>,
    ) -> &mut Self {
        DefaultParser::replace_tokenizer(self, tokenizer, register_before_tokenizer)
    }

    fn unmount_tokenizer(&mut self, tokenizer_name: &str) -> &mut Self {
        DefaultParser::unmount_tokenizer(self, tokenizer_name)
    }

    fn use_fallback_tokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self {
        DefaultParser::use_fallback_tokenizer(self, tokenizer)
    }

    fn set_default_parse_options(&mut self, options: Option<ParseOptions>) {
        DefaultParser::set_default_parse_options(self, options)
    }

    fn parse<'a, C>(&self, contents: C, options: Option<ParseOptions>) -> Root
    where
        C: Into<ParseContents<'a>>,
    {
        DefaultParser::parse(self, contents, options)
    }
}

#[cfg(test)]
mod tests {
    use yozora_ast::Node;
    use yozora_core_tokenizer::{
        AnyFallbackTokenizer, AnyTokenizer, BlockToken, BlockTokenizer, FindDelimiterGenerator,
        InlineFallbackTokenizer, InlineToken, InlineTokenizer, IsDelimiterPairResult,
        MatchBlockHook, MatchBlockPhaseApi, MatchInlineFallbackPhaseApi, MatchInlineHook,
        MatchInlinePhaseApi, ParseBlockHook, ParseBlockHookResult, ParseBlockPhaseApi,
        ParseBlockResult, ParseInlineHook, ParseInlinePhaseApi, ProcessDelimiterPairResult,
        TokenDelimiter, Tokenizer, TokenizerType,
    };

    use super::DefaultParser;

    struct EmptyFindDelimiter;

    impl FindDelimiterGenerator for EmptyFindDelimiter {
        fn next(&mut self, _range_index: (usize, usize)) -> Option<TokenDelimiter> {
            None
        }
    }

    struct NoopMatchBlockHook;

    impl MatchBlockHook for NoopMatchBlockHook {
        fn is_containing_block(&self) -> bool {
            true
        }

        fn eat_opener(
            &mut self,
            _line: &yozora_core_tokenizer::PhrasingContentLine,
            _parent_token: &BlockToken,
        ) -> Option<yozora_core_tokenizer::EatOpenerResult> {
            None
        }
    }

    struct NoopParseBlockHook;

    impl ParseBlockHook for NoopParseBlockHook {
        fn parse<'a>(
            &'a self,
            _tokens: &'a [BlockToken],
        ) -> ParseBlockResult<ParseBlockHookResult<'a>> {
            Ok(Vec::new().into())
        }
    }

    struct NoopMatchInlineHook;

    impl<'hook> MatchInlineHook<'hook> for NoopMatchInlineHook {
        fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'hook> {
            Box::new(EmptyFindDelimiter)
        }

        fn is_delimiter_pair(
            &self,
            _opener_delimiter: &TokenDelimiter,
            _closer_delimiter: &TokenDelimiter,
            _internal_tokens: &[InlineToken],
        ) -> IsDelimiterPairResult {
            IsDelimiterPairResult::NotPaired {
                opener: false,
                closer: false,
            }
        }

        fn process_delimiter_pair(
            &self,
            _opener_delimiter: &TokenDelimiter,
            _closer_delimiter: &TokenDelimiter,
            _internal_tokens: &[InlineToken],
        ) -> ProcessDelimiterPairResult {
            ProcessDelimiterPairResult {
                tokens: Vec::new(),
                remain_opener_delimiter: None,
                remain_closer_delimiter: None,
            }
        }

        fn process_single_delimiter(&self, _delimiter: &TokenDelimiter) -> Vec<InlineToken> {
            Vec::new()
        }
    }

    struct NoopParseInlineHook;

    impl ParseInlineHook for NoopParseInlineHook {
        fn parse(&self, _tokens: &[InlineToken]) -> Vec<Node> {
            Vec::new()
        }
    }

    #[derive(Clone)]
    struct DummyBlockTokenizer {
        name: String,
        priority: i32,
    }

    impl DummyBlockTokenizer {
        fn new(name: &str, priority: i32) -> Self {
            Self {
                name: name.to_string(),
                priority,
            }
        }
    }

    impl Tokenizer for DummyBlockTokenizer {
        fn r#type(&self) -> TokenizerType {
            TokenizerType::Block
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn priority(&self) -> i32 {
            self.priority
        }
    }

    impl BlockTokenizer for DummyBlockTokenizer {
        fn r#match<'a>(&'a self, _api: &'a dyn MatchBlockPhaseApi) -> Box<dyn MatchBlockHook + 'a> {
            Box::new(NoopMatchBlockHook)
        }

        fn parse<'a>(&'a self, _api: &'a dyn ParseBlockPhaseApi) -> Box<dyn ParseBlockHook + 'a> {
            Box::new(NoopParseBlockHook)
        }
    }

    #[derive(Clone)]
    struct DummyInlineTokenizer {
        name: String,
        priority: i32,
    }

    impl DummyInlineTokenizer {
        fn new(name: &str, priority: i32) -> Self {
            Self {
                name: name.to_string(),
                priority,
            }
        }
    }

    impl Tokenizer for DummyInlineTokenizer {
        fn r#type(&self) -> TokenizerType {
            TokenizerType::Inline
        }

        fn name(&self) -> &str {
            &self.name
        }

        fn priority(&self) -> i32 {
            self.priority
        }
    }

    impl InlineTokenizer for DummyInlineTokenizer {
        fn r#match<'a>(
            &'a self,
            _api: &'a dyn MatchInlinePhaseApi,
        ) -> Box<dyn MatchInlineHook<'a> + 'a> {
            Box::new(NoopMatchInlineHook)
        }

        fn parse<'a>(&'a self, _api: &'a dyn ParseInlinePhaseApi) -> Box<dyn ParseInlineHook + 'a> {
            Box::new(NoopParseInlineHook)
        }
    }

    impl InlineFallbackTokenizer for DummyInlineTokenizer {
        fn find_and_handle_delimiter(
            &self,
            start_index: usize,
            end_index: usize,
            _api: &dyn MatchInlineFallbackPhaseApi,
        ) -> InlineToken {
            InlineToken::new(self.name.clone(), "text", (start_index, end_index))
        }
    }

    #[test]
    fn fallback_tokenizers_can_share_a_name_across_types() {
        let mut parser = DefaultParser::default();

        parser.use_fallback_tokenizer(AnyFallbackTokenizer::Block(Box::new(
            DummyBlockTokenizer::new("@x/fallback", -1),
        )));
        parser.use_fallback_tokenizer(AnyFallbackTokenizer::Inline(Box::new(
            DummyInlineTokenizer::new("@x/fallback", -1),
        )));

        assert_eq!(
            parser
                .block_fallback_tokenizer
                .as_ref()
                .map(|tokenizer| tokenizer.name()),
            Some("@x/fallback")
        );
        assert_eq!(
            parser
                .inline_fallback_tokenizer
                .as_ref()
                .map(|tokenizer| tokenizer.name()),
            Some("@x/fallback")
        );
    }

    #[test]
    #[should_panic(expected = "[useFallbackTokenizer] Name(@x/block) has been registered.")]
    fn fallback_name_collision_should_fail() {
        let mut parser = DefaultParser::default();
        parser.use_tokenizer(
            AnyTokenizer::Block(Box::new(DummyBlockTokenizer::new("@x/block", 10))),
            None,
        );
        parser.use_fallback_tokenizer(AnyFallbackTokenizer::Block(Box::new(
            DummyBlockTokenizer::new("@x/block", -1),
        )));
    }

    #[test]
    fn same_priority_replacement_preserves_order() {
        let mut parser = DefaultParser::default();
        parser
            .use_tokenizer(
                AnyTokenizer::Inline(Box::new(DummyInlineTokenizer::new("first", 10))),
                None,
            )
            .use_tokenizer(
                AnyTokenizer::Inline(Box::new(DummyInlineTokenizer::new("second", 10))),
                None,
            )
            .replace_tokenizer(
                AnyTokenizer::Inline(Box::new(DummyInlineTokenizer::new("first", 10))),
                None,
            );

        let names = parser
            .inline_tokenizers
            .iter()
            .map(|tokenizer| tokenizer.name())
            .collect::<Vec<_>>();
        assert_eq!(names, ["first", "second"]);
    }

    #[test]
    fn replacing_fallback_keeps_other_type() {
        let mut parser = DefaultParser::default();
        parser
            .use_fallback_tokenizer(AnyFallbackTokenizer::Block(Box::new(
                DummyBlockTokenizer::new("shared", -1),
            )))
            .use_fallback_tokenizer(AnyFallbackTokenizer::Inline(Box::new(
                DummyInlineTokenizer::new("shared", -1),
            )))
            .use_fallback_tokenizer(AnyFallbackTokenizer::Block(Box::new(
                DummyBlockTokenizer::new("shared", -1),
            )));

        assert!(parser.block_fallback_tokenizer.is_some());
        assert!(parser.inline_fallback_tokenizer.is_some());
    }
}
