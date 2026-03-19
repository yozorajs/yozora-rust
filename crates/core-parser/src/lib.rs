#![allow(non_snake_case)]

pub mod engine;

use std::collections::HashMap;
use std::sync::Arc;

use yozora_ast::{Association, Root};
use yozora_character::{
    create_node_point_generator, is_line_ending, is_space_character, NodePoint,
};
use yozora_core_tokenizer::engine::{
    EngineBlockTokenizer, EngineInlineFallbackTokenizer, EngineInlineTokenizer, PhrasingContentLine,
};
use yozora_core_tokenizer::{AnyFallbackTokenizer, AnyTokenizer};

use crate::engine::{create_processor, Processor, ProcessorOptions};

pub type FormatUrlFn = Arc<dyn Fn(&str) -> String + Send + Sync + 'static>;

#[allow(non_snake_case)]
#[derive(Clone, Default)]
pub struct ParseOptions {
    pub shouldReservePosition: Option<bool>,
    pub presetDefinitions: Option<Vec<Association>>,
    pub presetFootnoteDefinitions: Option<Vec<Association>>,
    pub formatUrl: Option<FormatUrlFn>,
}

#[derive(Clone)]
struct ResolvedParseOptions {
    shouldReservePosition: bool,
    presetDefinitions: Vec<Association>,
    presetFootnoteDefinitions: Vec<Association>,
    formatUrl: FormatUrlFn,
}

#[derive(Debug, Clone)]
pub enum ParseContents<'a> {
    Text(&'a str),
    Chunks(&'a [&'a str]),
    OwnedText(String),
    OwnedChunks(Vec<String>),
}

impl<'a> From<&'a str> for ParseContents<'a> {
    fn from(value: &'a str) -> Self {
        Self::Text(value)
    }
}

impl<'a> From<&'a [&'a str]> for ParseContents<'a> {
    fn from(value: &'a [&'a str]) -> Self {
        Self::Chunks(value)
    }
}

impl<'a> From<&'a String> for ParseContents<'a> {
    fn from(value: &'a String) -> Self {
        Self::Text(value.as_str())
    }
}

impl<'a> From<String> for ParseContents<'a> {
    fn from(value: String) -> Self {
        Self::OwnedText(value)
    }
}

impl<'a> From<Vec<String>> for ParseContents<'a> {
    fn from(value: Vec<String>) -> Self {
        Self::OwnedChunks(value)
    }
}

impl<'a> From<&'a [String]> for ParseContents<'a> {
    fn from(value: &'a [String]) -> Self {
        Self::OwnedChunks(value.to_vec())
    }
}

impl<'a> From<Vec<&'a str>> for ParseContents<'a> {
    fn from(value: Vec<&'a str>) -> Self {
        Self::OwnedChunks(value.into_iter().map(str::to_string).collect())
    }
}

#[allow(non_snake_case)]
pub struct DefaultParser {
    blockTokenizers: Vec<Box<dyn EngineBlockTokenizer>>,
    inlineTokenizers: Vec<Box<dyn EngineInlineTokenizer>>,
    blockTokenizerMap: HashMap<String, usize>,
    inlineTokenizerMap: HashMap<String, usize>,
    blockFallbackTokenizer: Option<Box<dyn EngineBlockTokenizer>>,
    inlineFallbackTokenizer: Option<Box<dyn EngineInlineFallbackTokenizer>>,
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
    pub fn new() -> Self {
        Self::default()
    }

    pub fn useTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        registerBeforeTokenizer: Option<&str>,
    ) -> Result<&mut Self, String> {
        match tokenizer {
            AnyTokenizer::Block(tokenizer) => {
                self.registerBlockTokenizer(tokenizer, registerBeforeTokenizer)?;
            }
            AnyTokenizer::Inline(tokenizer) => {
                self.registerInlineTokenizer(tokenizer, registerBeforeTokenizer)?;
            }
        }
        Ok(self)
    }

    pub fn replaceTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        registerBeforeTokenizer: Option<&str>,
    ) -> Result<&mut Self, String> {
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
                self.blockFallbackTokenizer = Some(tokenizer);
            }
            AnyFallbackTokenizer::Inline(tokenizer) => {
                self.inlineFallbackTokenizer = Some(tokenizer);
            }
        }
        self
    }

    pub fn setDefaultParseOptions(&mut self, options: ParseOptions) {
        self.defaultParseOptions = options;
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
            block_tokenizers: &self.blockTokenizers,
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
                .unwrap_or_else(|| Arc::new(|url: &str| url.to_string())),
        }
    }

    fn registerBlockTokenizer(
        &mut self,
        tokenizer: Box<dyn EngineBlockTokenizer>,
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
        tokenizer: Box<dyn EngineInlineTokenizer>,
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
    tokenizers: &[Box<dyn EngineBlockTokenizer>],
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
    tokenizers: &[Box<dyn EngineInlineTokenizer>],
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

#[derive(Debug, Clone, Copy)]
struct TempLine {
    start_index: usize,
    end_index: usize,
    first_non_whitespace_index: usize,
    count_of_precede_spaces: usize,
}

fn create_phrasing_line_groups(
    node_point_chunks: Vec<Vec<NodePoint>>,
) -> Vec<Vec<PhrasingContentLine>> {
    let mut all_node_points: Vec<NodePoint> = Vec::new();
    let mut line_groups: Vec<Vec<TempLine>> = Vec::new();

    let mut start_index = 0usize;
    let mut first_non_whitespace_index = 0usize;
    let mut count_of_precede_spaces = 0usize;

    for chunk in node_point_chunks {
        let mut lines: Vec<TempLine> = Vec::new();

        for point in chunk {
            if first_non_whitespace_index == all_node_points.len()
                && is_space_character(point.code_point)
            {
                count_of_precede_spaces += 1;
                first_non_whitespace_index += 1;
            }

            all_node_points.push(point);
            if is_line_ending(point.code_point) {
                if first_non_whitespace_index + 1 == all_node_points.len() {
                    first_non_whitespace_index += 1;
                }

                lines.push(TempLine {
                    start_index,
                    end_index: all_node_points.len(),
                    first_non_whitespace_index,
                    count_of_precede_spaces,
                });

                start_index = all_node_points.len();
                first_non_whitespace_index = all_node_points.len();
                count_of_precede_spaces = 0;
            }
        }

        line_groups.push(lines);
    }

    if start_index < all_node_points.len() {
        line_groups.push(vec![TempLine {
            start_index,
            end_index: all_node_points.len(),
            first_non_whitespace_index,
            count_of_precede_spaces,
        }]);
    }

    let shared_points = Arc::new(all_node_points);
    line_groups
        .into_iter()
        .map(|group| {
            group
                .into_iter()
                .map(|line| PhrasingContentLine {
                    node_points: shared_points.clone(),
                    start_index: line.start_index,
                    end_index: line.end_index,
                    first_non_whitespace_index: line.first_non_whitespace_index,
                    count_of_precede_spaces: line.count_of_precede_spaces,
                })
                .collect()
        })
        .collect()
}

pub trait Parser {
    fn useTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        registerBeforeTokenizer: Option<&str>,
    ) -> Result<&mut Self, String>;

    fn replaceTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        registerBeforeTokenizer: Option<&str>,
    ) -> Result<&mut Self, String>;

    fn unmountTokenizer(&mut self, tokenizerName: &str) -> &mut Self;

    fn useFallbackTokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self;

    fn setDefaultParseOptions(&mut self, options: ParseOptions);

    fn parse<'a, C>(&self, contents: C, options: Option<ParseOptions>) -> Root
    where
        C: Into<ParseContents<'a>>;
}

impl Parser for DefaultParser {
    fn useTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        registerBeforeTokenizer: Option<&str>,
    ) -> Result<&mut Self, String> {
        DefaultParser::useTokenizer(self, tokenizer, registerBeforeTokenizer)
    }

    fn replaceTokenizer(
        &mut self,
        tokenizer: AnyTokenizer,
        registerBeforeTokenizer: Option<&str>,
    ) -> Result<&mut Self, String> {
        DefaultParser::replaceTokenizer(self, tokenizer, registerBeforeTokenizer)
    }

    fn unmountTokenizer(&mut self, tokenizerName: &str) -> &mut Self {
        DefaultParser::unmountTokenizer(self, tokenizerName)
    }

    fn useFallbackTokenizer(&mut self, tokenizer: AnyFallbackTokenizer) -> &mut Self {
        DefaultParser::useFallbackTokenizer(self, tokenizer)
    }

    fn setDefaultParseOptions(&mut self, options: ParseOptions) {
        DefaultParser::setDefaultParseOptions(self, options)
    }

    fn parse<'a, C>(&self, contents: C, options: Option<ParseOptions>) -> Root
    where
        C: Into<ParseContents<'a>>,
    {
        DefaultParser::parse(self, contents, options)
    }
}
