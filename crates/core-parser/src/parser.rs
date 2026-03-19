use std::collections::HashMap;
use std::sync::Arc;

use yozora_ast::Root;
use yozora_character::create_node_point_generator;
use yozora_core_tokenizer::encode_link_destination;
use yozora_core_tokenizer::{AnyFallbackTokenizer, AnyTokenizer};
use yozora_core_tokenizer::{BlockTokenizer, InlineFallbackTokenizer, InlineTokenizer};

use crate::processor::{create_processor, Processor, ProcessorOptions};
use crate::types::{DefaultParserProps, FormatUrlFn, ParseContents, ParseOptions, Parser};
use crate::util::phrasing_line::create_phrasing_line_groups;

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
    blockFallbackTokenizer: Option<Box<dyn BlockTokenizer>>,
    inlineFallbackTokenizer: Option<Box<dyn InlineFallbackTokenizer>>,
    defaultParseOptions: ParseOptions,
}

impl Default for DefaultParser {
    fn default() -> Self {
        Self {
            blockTokenizers: Vec::new(),
            inlineTokenizers: Vec::new(),
            blockTokenizerMap: HashMap::new(),
            inlineTokenizerMap: HashMap::new(),
            blockFallbackTokenizer: None,
            inlineFallbackTokenizer: None,
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
        }

        if self
            .inlineFallbackTokenizer
            .as_ref()
            .is_some_and(|tokenizer| tokenizer.name() == tokenizerName)
        {
            self.inlineFallbackTokenizer = None;
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
                self.blockFallbackTokenizer = Some(tokenizer);
            }
            AnyFallbackTokenizer::Inline(tokenizer) => {
                if let Some(existing) = self.inlineFallbackTokenizer.as_ref() {
                    let name = existing.name().to_string();
                    self.unmountTokenizer(&name);
                }
                self.inlineFallbackTokenizer = Some(tokenizer);
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
        let line_groups = create_phrasing_line_groups(node_point_chunks);

        let mut processor = create_processor(ProcessorOptions {
            inline_tokenizers: &self.inlineTokenizers,
            inline_tokenizer_map: &self.inlineTokenizerMap,
            block_tokenizers: &self.blockTokenizers,
            block_tokenizer_map: &self.blockTokenizerMap,
            block_fallback_tokenizer: self.blockFallbackTokenizer.as_deref(),
            inline_fallback_tokenizer: self.inlineFallbackTokenizer.as_deref(),
            should_reserve_position: parse_options.shouldReservePosition,
            preset_definitions: &parse_options.presetDefinitions,
            preset_footnote_definitions: &parse_options.presetFootnoteDefinitions,
            format_url: parse_options.formatUrl,
        });

        processor.process(&line_groups)
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
        for (idx, tokenizer) in self.blockTokenizers.iter().enumerate() {
            self.blockTokenizerMap
                .insert(tokenizer.name().to_string(), idx);
        }
    }

    fn rebuildInlineIndex(&mut self) {
        self.inlineTokenizerMap.clear();
        for (idx, tokenizer) in self.inlineTokenizers.iter().enumerate() {
            self.inlineTokenizerMap
                .insert(tokenizer.name().to_string(), idx);
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
