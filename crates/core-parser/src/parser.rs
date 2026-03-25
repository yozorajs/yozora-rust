use std::collections::HashMap;
use std::sync::Arc;

use yozora_ast::Root;
use yozora_character::create_node_point_generator;
use yozora_core_tokenizer::encode_link_destination;
use yozora_core_tokenizer::{AnyFallbackTokenizer, AnyTokenizer};
use yozora_core_tokenizer::{
    BlockTokenizer, InlineFallbackTokenizer, InlineTokenizer, TokenizerId,
};

use crate::processor::{create_processor, Processor, ProcessorOptions};
use crate::types::{DefaultParserProps, FormatUrlFn, ParseContents, ParseOptions, Parser};
use crate::util::phrasing_line::create_phrasing_line_generator;
use crate::util::tokenizer_uid::calc_tokenizer_uid;

#[derive(Clone)]
struct ResolvedParseOptions {
    shouldReservePosition: bool,
    presetDefinitions: Vec<yozora_ast::Association>,
    presetFootnoteDefinitions: Vec<yozora_ast::Association>,
    formatUrl: FormatUrlFn,
}

#[allow(non_snake_case)]
pub struct DefaultParser {
    blockTokenizers: Vec<Box<dyn BlockTokenizer>>,
    inlineTokenizers: Vec<Box<dyn InlineTokenizer>>,
    blockTokenizerMap: HashMap<String, usize>,
    inlineTokenizerMap: HashMap<String, usize>,
    blockTokenizerUidMap: HashMap<String, TokenizerId>,
    inlineTokenizerUidMap: HashMap<String, TokenizerId>,
    blockFallbackTokenizer: Option<Box<dyn BlockTokenizer>>,
    inlineFallbackTokenizer: Option<Box<dyn InlineFallbackTokenizer>>,
    blockFallbackTokenizerUid: Option<TokenizerId>,
    inlineFallbackTokenizerUid: Option<TokenizerId>,
    defaultParseOptions: ParseOptions,
}

impl Default for DefaultParser {
    fn default() -> Self {
        Self {
            blockTokenizers: Vec::new(),
            inlineTokenizers: Vec::new(),
            blockTokenizerMap: HashMap::new(),
            inlineTokenizerMap: HashMap::new(),
            blockTokenizerUidMap: HashMap::new(),
            inlineTokenizerUidMap: HashMap::new(),
            blockFallbackTokenizer: None,
            inlineFallbackTokenizer: None,
            blockFallbackTokenizerUid: None,
            inlineFallbackTokenizerUid: None,
            defaultParseOptions: ParseOptions::default(),
        }
    }
}

impl DefaultParser {
    pub fn new(props: DefaultParserProps) -> Self {
        let mut parser = Self::default();
        parser.setDefaultParseOptions(props.defaultParseOptions);

        if let Some(tokenizer) = props.blockFallbackTokenizer {
            parser.useFallbackTokenizer(AnyFallbackTokenizer::Block(tokenizer));
        }
        if let Some(tokenizer) = props.inlineFallbackTokenizer {
            parser.useFallbackTokenizer(AnyFallbackTokenizer::Inline(tokenizer));
        }

        parser
    }

    pub fn useTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        registerBeforeTokenizer: Option<&str>,
    ) -> &mut Self {
        match tokenizer {
            AnyTokenizer::Block(tokenizer) => {
                if let Err(err) = self.registerBlockTokenizer(tokenizer, registerBeforeTokenizer) {
                    panic!("{err}");
                }
            }
            AnyTokenizer::Inline(tokenizer) => {
                if let Err(err) = self.registerInlineTokenizer(tokenizer, registerBeforeTokenizer) {
                    panic!("{err}");
                }
            }
        }
        self
    }

    pub fn replaceTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        registerBeforeTokenizer: Option<&str>,
    ) -> &mut Self {
        let name = tokenizer.name().to_string();
        self.unmountTokenizer(&name);
        self.useTokenizer(tokenizer, registerBeforeTokenizer)
    }

    pub fn unmountTokenizer(&mut self, tokenizerName: &str) -> &mut Self {
        self.unmountInlineTokenizer(tokenizerName);
        self.unmountBlockTokenizer(tokenizerName);

        if self
            .blockFallbackTokenizer
            .as_ref()
            .is_some_and(|tokenizer| tokenizer.name() == tokenizerName)
        {
            self.blockFallbackTokenizer = None;
            self.blockFallbackTokenizerUid = None;
        }

        if self
            .inlineFallbackTokenizer
            .as_ref()
            .is_some_and(|tokenizer| tokenizer.name() == tokenizerName)
        {
            self.inlineFallbackTokenizer = None;
            self.inlineFallbackTokenizerUid = None;
        }

        self
    }

    pub fn useFallbackTokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self {
        match tokenizer {
            AnyFallbackTokenizer::Block(tokenizer) => {
                if let Some(existing) = self.blockFallbackTokenizer.as_ref() {
                    let name = existing.name().to_string();
                    self.unmountTokenizer(&name);
                }

                let name = tokenizer.name().to_string();
                let uid = calc_tokenizer_uid(&name);
                if let Err(err) =
                    ensure_uid_no_collision(&self.blockTokenizerUidMap, &name, uid, "block")
                {
                    panic!("{err}");
                }
                self.blockFallbackTokenizer = Some(tokenizer);
                self.blockFallbackTokenizerUid = Some(uid);
            }
            AnyFallbackTokenizer::Inline(tokenizer) => {
                if let Some(existing) = self.inlineFallbackTokenizer.as_ref() {
                    let name = existing.name().to_string();
                    self.unmountTokenizer(&name);
                }

                let name = tokenizer.name().to_string();
                let uid = calc_tokenizer_uid(&name);
                if let Err(err) =
                    ensure_uid_no_collision(&self.inlineTokenizerUidMap, &name, uid, "inline")
                {
                    panic!("{err}");
                }
                self.inlineFallbackTokenizer = Some(tokenizer);
                self.inlineFallbackTokenizerUid = Some(uid);
            }
        }
        self
    }

    pub fn setDefaultParseOptions(&mut self, options: Option<ParseOptions>) {
        self.defaultParseOptions = options.unwrap_or_default();
    }

    pub fn parse<'a, C>(&self, contents: C, options: Option<ParseOptions>) -> Root
    where
        C: Into<ParseContents<'a>>,
    {
        let parse_options = self.resolveParseOptions(options.unwrap_or_default());
        let chunks = normalize_contents_to_chunks(contents.into());
        let node_point_chunks = create_node_point_generator(chunks);
        let lines_iterator = create_phrasing_line_generator(node_point_chunks);

        let mut processor = create_processor(ProcessorOptions {
            inline_tokenizers: &self.inlineTokenizers,
            inline_tokenizer_map: &self.inlineTokenizerMap,
            inline_tokenizer_uid_map: &self.inlineTokenizerUidMap,
            block_tokenizers: &self.blockTokenizers,
            block_tokenizer_map: &self.blockTokenizerMap,
            block_tokenizer_uid_map: &self.blockTokenizerUidMap,
            block_fallback_tokenizer: self.blockFallbackTokenizer.as_deref(),
            block_fallback_tokenizer_uid: self.blockFallbackTokenizerUid,
            inline_fallback_tokenizer: self.inlineFallbackTokenizer.as_deref(),
            inline_fallback_tokenizer_uid: self.inlineFallbackTokenizerUid,
            should_reserve_position: parse_options.shouldReservePosition,
            preset_definitions: &parse_options.presetDefinitions,
            preset_footnote_definitions: &parse_options.presetFootnoteDefinitions,
            format_url: parse_options.formatUrl,
        });

        processor.process(lines_iterator)
    }

    fn resolveParseOptions(&self, options: ParseOptions) -> ResolvedParseOptions {
        ResolvedParseOptions {
            shouldReservePosition: options
                .shouldReservePosition
                .or(self.defaultParseOptions.shouldReservePosition)
                .unwrap_or(false),
            presetDefinitions: options
                .presetDefinitions
                .or_else(|| self.defaultParseOptions.presetDefinitions.clone())
                .unwrap_or_default(),
            presetFootnoteDefinitions: options
                .presetFootnoteDefinitions
                .or_else(|| self.defaultParseOptions.presetFootnoteDefinitions.clone())
                .unwrap_or_default(),
            formatUrl: options
                .formatUrl
                .or_else(|| self.defaultParseOptions.formatUrl.clone())
                .unwrap_or_else(|| Arc::new(|url: &str| encode_link_destination(url))),
        }
    }

    fn registerBlockTokenizer(
        &mut self,
        tokenizer: Box<dyn BlockTokenizer>,
        registerBeforeTokenizer: Option<&str>,
    ) -> Result<(), String> {
        let name = tokenizer.name().to_string();
        if self.blockTokenizerMap.contains_key(&name) {
            return Err(format!("[useTokenizer] Name({name}) has been registered."));
        }

        let uid = calc_tokenizer_uid(&name);
        ensure_uid_no_collision(&self.blockTokenizerUidMap, &name, uid, "block")?;
        ensure_uid_no_collision_with_fallback(
            self.blockFallbackTokenizer
                .as_deref()
                .map(|tokenizer| tokenizer.name()),
            self.blockFallbackTokenizerUid,
            &name,
            uid,
            "block",
        )?;

        let insert_index = calc_insert_index_block(
            &self.blockTokenizers,
            tokenizer.priority(),
            registerBeforeTokenizer,
        );
        self.blockTokenizers.insert(insert_index, tokenizer);
        self.rebuildBlockIndex();
        Ok(())
    }

    fn registerInlineTokenizer(
        &mut self,
        tokenizer: Box<dyn InlineTokenizer>,
        registerBeforeTokenizer: Option<&str>,
    ) -> Result<(), String> {
        let name = tokenizer.name().to_string();
        if self.inlineTokenizerMap.contains_key(&name) {
            return Err(format!("[useTokenizer] Name({name}) has been registered."));
        }

        let uid = calc_tokenizer_uid(&name);
        ensure_uid_no_collision(&self.inlineTokenizerUidMap, &name, uid, "inline")?;
        ensure_uid_no_collision_with_fallback(
            self.inlineFallbackTokenizer
                .as_deref()
                .map(|tokenizer| tokenizer.name()),
            self.inlineFallbackTokenizerUid,
            &name,
            uid,
            "inline",
        )?;

        let insert_index = calc_insert_index_inline(
            &self.inlineTokenizers,
            tokenizer.priority(),
            registerBeforeTokenizer,
        );
        self.inlineTokenizers.insert(insert_index, tokenizer);
        self.rebuildInlineIndex();
        Ok(())
    }

    fn unmountBlockTokenizer(&mut self, tokenizer_name: &str) {
        if let Some(index) = self.blockTokenizerMap.remove(tokenizer_name) {
            self.blockTokenizers.remove(index);
            self.rebuildBlockIndex();
        }
    }

    fn unmountInlineTokenizer(&mut self, tokenizer_name: &str) {
        if let Some(index) = self.inlineTokenizerMap.remove(tokenizer_name) {
            self.inlineTokenizers.remove(index);
            self.rebuildInlineIndex();
        }
    }

    fn rebuildBlockIndex(&mut self) {
        self.blockTokenizerMap.clear();
        self.blockTokenizerUidMap.clear();
        let mut reverse_uid_map: HashMap<TokenizerId, String> = HashMap::new();

        for (idx, tokenizer) in self.blockTokenizers.iter().enumerate() {
            let name = tokenizer.name().to_string();
            let uid = calc_tokenizer_uid(&name);

            if let Some(existing_name) = reverse_uid_map.insert(uid, name.clone()) {
                panic!(
                    "[useTokenizer] block tokenizer uid collision: name({}) conflicts with existing name({}), uid={}",
                    name,
                    existing_name,
                    uid
                );
            }

            self.blockTokenizerMap.insert(name.clone(), idx);
            self.blockTokenizerUidMap.insert(name, uid);
        }

        validate_fallback_uid_collision(
            self.blockTokenizerUidMap.iter(),
            self.blockFallbackTokenizer
                .as_deref()
                .map(|tokenizer| tokenizer.name()),
            self.blockFallbackTokenizerUid,
            "block",
        );
    }

    fn rebuildInlineIndex(&mut self) {
        self.inlineTokenizerMap.clear();
        self.inlineTokenizerUidMap.clear();
        let mut reverse_uid_map: HashMap<TokenizerId, String> = HashMap::new();

        for (idx, tokenizer) in self.inlineTokenizers.iter().enumerate() {
            let name = tokenizer.name().to_string();
            let uid = calc_tokenizer_uid(&name);

            if let Some(existing_name) = reverse_uid_map.insert(uid, name.clone()) {
                panic!(
                    "[useTokenizer] inline tokenizer uid collision: name({}) conflicts with existing name({}), uid={}",
                    name,
                    existing_name,
                    uid
                );
            }

            self.inlineTokenizerMap.insert(name.clone(), idx);
            self.inlineTokenizerUidMap.insert(name, uid);
        }

        validate_fallback_uid_collision(
            self.inlineTokenizerUidMap.iter(),
            self.inlineFallbackTokenizer
                .as_deref()
                .map(|tokenizer| tokenizer.name()),
            self.inlineFallbackTokenizerUid,
            "inline",
        );
    }
}

fn ensure_uid_no_collision(
    uid_map: &HashMap<String, TokenizerId>,
    name: &str,
    uid: TokenizerId,
    kind: &str,
) -> Result<(), String> {
    if let Some((existing_name, _)) = uid_map.iter().find(|(existing_name, existing_uid)| {
        **existing_uid == uid && existing_name.as_str() != name
    }) {
        return Err(format!(
            "[useTokenizer] {kind} tokenizer uid collision: name({name}) conflicts with existing name({existing_name}), uid={uid}"
        ));
    }

    Ok(())
}

fn ensure_uid_no_collision_with_fallback(
    fallback_name: Option<&str>,
    fallback_uid: Option<TokenizerId>,
    name: &str,
    uid: TokenizerId,
    kind: &str,
) -> Result<(), String> {
    if fallback_uid == Some(uid) && fallback_name.is_some_and(|existing_name| existing_name != name)
    {
        return Err(format!(
            "[useTokenizer] {kind} tokenizer uid collision: name({name}) conflicts with fallback name({}), uid={uid}",
            fallback_name.unwrap_or("<unknown>")
        ));
    }

    Ok(())
}

fn validate_fallback_uid_collision<'a>(
    mut uid_map_iter: impl Iterator<Item = (&'a String, &'a TokenizerId)>,
    fallback_name: Option<&str>,
    fallback_uid: Option<TokenizerId>,
    kind: &str,
) {
    let Some(fallback_uid) = fallback_uid else {
        return;
    };
    let Some(fallback_name) = fallback_name else {
        return;
    };

    if let Some((existing_name, _)) =
        uid_map_iter.find(|(name, uid)| **uid == fallback_uid && name.as_str() != fallback_name)
    {
        panic!(
            "[useTokenizer] {kind} tokenizer uid collision: fallback name({fallback_name}) conflicts with existing name({existing_name}), uid={fallback_uid}"
        );
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
    fn useTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        registerBeforeTokenizer: Option<&str>,
    ) -> &mut Self {
        DefaultParser::useTokenizer(self, tokenizer, registerBeforeTokenizer)
    }

    fn replaceTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        registerBeforeTokenizer: Option<&str>,
    ) -> &mut Self {
        DefaultParser::replaceTokenizer(self, tokenizer, registerBeforeTokenizer)
    }

    fn unmountTokenizer(&mut self, tokenizerName: &str) -> &mut Self {
        DefaultParser::unmountTokenizer(self, tokenizerName)
    }

    fn useFallbackTokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self {
        DefaultParser::useFallbackTokenizer(self, tokenizer)
    }

    fn setDefaultParseOptions(&mut self, options: Option<ParseOptions>) {
        DefaultParser::setDefaultParseOptions(self, options)
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
    use std::collections::HashMap;

    use yozora_ast::Node;
    use yozora_core_tokenizer::{
        AnyFallbackTokenizer, AnyTokenizer, BlockToken, BlockTokenizer, FindDelimiterGenerator,
        InlineFallbackTokenizer, InlineToken, InlineTokenizer, IsDelimiterPairResult,
        MatchBlockHook, MatchBlockPhaseApi, MatchInlineFallbackPhaseApi, MatchInlineHook,
        MatchInlinePhaseApi, ParseBlockHook, ParseBlockPhaseApi, ParseInlineHook,
        ParseInlinePhaseApi, ProcessDelimiterPairResult, TokenDelimiter, Tokenizer, TokenizerType,
    };

    use super::{
        calc_tokenizer_uid, ensure_uid_no_collision, ensure_uid_no_collision_with_fallback,
        DefaultParser,
    };

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
        fn parse(&self, _tokens: &[BlockToken]) -> Vec<Node> {
            Vec::new()
        }
    }

    struct NoopMatchInlineHook;

    impl<'hook> MatchInlineHook<'hook> for NoopMatchInlineHook {
        fn findDelimiter(&self) -> Box<dyn FindDelimiterGenerator + 'hook> {
            Box::new(EmptyFindDelimiter)
        }

        fn isDelimiterPair(
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

        fn processDelimiterPair(
            &self,
            _opener_delimiter: &TokenDelimiter,
            _closer_delimiter: &TokenDelimiter,
            _internal_tokens: &[InlineToken],
        ) -> ProcessDelimiterPairResult {
            ProcessDelimiterPairResult {
                tokens: Vec::new(),
                remainOpenerDelimiter: None,
                remainCloserDelimiter: None,
            }
        }

        fn processSingleDelimiter(&self, _delimiter: &TokenDelimiter) -> Vec<InlineToken> {
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
        fn findAndHandleDelimiter(
            &self,
            start_index: usize,
            end_index: usize,
            _api: &dyn MatchInlineFallbackPhaseApi,
        ) -> InlineToken {
            InlineToken {
                tokenizer: self.name.clone().into(),
                tokenizer_id: calc_tokenizer_uid(&self.name),
                node_type: "text",
                start_index,
                end_index,
                data: std::sync::Arc::new(()),
            }
        }
    }

    #[test]
    fn tokenizer_uid_should_be_stable_across_registration_order() {
        let mut parser_a = DefaultParser::default();
        parser_a.useTokenizer(
            AnyTokenizer::Block(Box::new(DummyBlockTokenizer::new("@x/block-a", 10))),
            None,
        );
        parser_a.useTokenizer(
            AnyTokenizer::Block(Box::new(DummyBlockTokenizer::new("@x/block-b", 10))),
            None,
        );
        parser_a.useTokenizer(
            AnyTokenizer::Inline(Box::new(DummyInlineTokenizer::new("@x/inline-a", 10))),
            None,
        );
        parser_a.useTokenizer(
            AnyTokenizer::Inline(Box::new(DummyInlineTokenizer::new("@x/inline-b", 10))),
            None,
        );

        let mut parser_b = DefaultParser::default();
        parser_b.useTokenizer(
            AnyTokenizer::Block(Box::new(DummyBlockTokenizer::new("@x/block-b", 10))),
            None,
        );
        parser_b.useTokenizer(
            AnyTokenizer::Block(Box::new(DummyBlockTokenizer::new("@x/block-a", 10))),
            None,
        );
        parser_b.useTokenizer(
            AnyTokenizer::Inline(Box::new(DummyInlineTokenizer::new("@x/inline-b", 10))),
            None,
        );
        parser_b.useTokenizer(
            AnyTokenizer::Inline(Box::new(DummyInlineTokenizer::new("@x/inline-a", 10))),
            None,
        );

        assert_eq!(
            parser_a.blockTokenizerUidMap.get("@x/block-a"),
            parser_b.blockTokenizerUidMap.get("@x/block-a"),
        );
        assert_eq!(
            parser_a.blockTokenizerUidMap.get("@x/block-b"),
            parser_b.blockTokenizerUidMap.get("@x/block-b"),
        );
        assert_eq!(
            parser_a.inlineTokenizerUidMap.get("@x/inline-a"),
            parser_b.inlineTokenizerUidMap.get("@x/inline-a"),
        );
        assert_eq!(
            parser_a.inlineTokenizerUidMap.get("@x/inline-b"),
            parser_b.inlineTokenizerUidMap.get("@x/inline-b"),
        );

        assert_eq!(
            parser_a.blockTokenizerUidMap.get("@x/block-a").copied(),
            Some(calc_tokenizer_uid("@x/block-a")),
        );
    }

    #[test]
    fn fallback_tokenizer_should_get_stable_uid() {
        let mut parser = DefaultParser::default();

        parser.useFallbackTokenizer(AnyFallbackTokenizer::Block(Box::new(
            DummyBlockTokenizer::new("@x/fallback-block", -1),
        )));
        parser.useFallbackTokenizer(AnyFallbackTokenizer::Inline(Box::new(
            DummyInlineTokenizer::new("@x/fallback-inline", -1),
        )));

        assert_eq!(
            parser.blockFallbackTokenizerUid,
            Some(calc_tokenizer_uid("@x/fallback-block")),
        );
        assert_eq!(
            parser.inlineFallbackTokenizerUid,
            Some(calc_tokenizer_uid("@x/fallback-inline")),
        );
    }

    #[test]
    fn uid_collision_helpers_should_fail_fast() {
        let mut uid_map = HashMap::new();
        uid_map.insert("@x/a".to_string(), 42);

        assert!(ensure_uid_no_collision(&uid_map, "@x/b", 42, "block").is_err());
        assert!(ensure_uid_no_collision_with_fallback(
            Some("@x/fallback"),
            Some(7),
            "@x/b",
            7,
            "inline"
        )
        .is_err());
    }

    #[test]
    fn uid_hash_should_be_deterministic_for_unicode_name() {
        let left = calc_tokenizer_uid("@x/解析器-🧪-α");
        let right = calc_tokenizer_uid("@x/解析器-🧪-α");
        assert_eq!(left, right);
    }
}
