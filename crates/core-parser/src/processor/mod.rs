pub mod block;
pub mod inline;
pub mod types;

pub use types::{Processor, ProcessorOptions};

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;

use yozora_ast::{Node, Point, Position, Root, ROOT_TYPE};
use yozora_character::NodePoint;
use yozora_core_tokenizer::NodeInterval;
use yozora_core_tokenizer::{
    BlockToken, InlineToken, MatchBlockPhaseApi, MatchInlinePhaseApi, ParseBlockPhaseApi,
    ParseInlinePhaseApi, PhrasingContentLine,
};

use crate::processor::block::{create_block_content_processor, MatchBlockProcessorHook};
use crate::processor::inline::{match_inline_tokens, MatchInlineProcessorHook};

pub fn create_processor<'a>(options: ProcessorOptions<'a>) -> impl Processor + 'a {
    ParserProcessor {
        options,
        definition_identifiers: HashSet::new(),
        footnote_definition_identifiers: HashSet::new(),
    }
}

struct ParserProcessor<'a> {
    options: ProcessorOptions<'a>,
    definition_identifiers: HashSet<String>,
    footnote_definition_identifiers: HashSet<String>,
}

#[derive(Debug, Default, Clone)]
struct IdentifierState {
    is_register_available: bool,
    definition_identifiers: HashSet<String>,
    footnote_definition_identifiers: HashSet<String>,
}

struct MatchBlockApiShared<'a> {
    block_tokenizers: &'a [Box<dyn yozora_core_tokenizer::BlockTokenizer>],
    block_tokenizer_map: &'a HashMap<String, usize>,
    block_fallback_tokenizer: Option<&'a dyn yozora_core_tokenizer::BlockTokenizer>,
    identifiers: Rc<RefCell<IdentifierState>>,
}

#[derive(Clone)]
struct MatchBlockApiAdapter<'a> {
    shared: Rc<MatchBlockApiShared<'a>>,
}

impl MatchBlockPhaseApi for MatchBlockApiAdapter<'_> {
    fn extract_phrasing_lines(&self, token: &BlockToken) -> Option<Vec<PhrasingContentLine>> {
        find_block_tokenizer_by_name(
            self.shared.block_tokenizers,
            self.shared.block_tokenizer_map,
            self.shared.block_fallback_tokenizer,
            &token.tokenizer,
        )
        .and_then(|tokenizer| tokenizer.extract_phrasing_content_lines(token))
    }

    fn rollback_phrasing_lines(
        &self,
        lines: &[PhrasingContentLine],
        original_token: Option<&BlockToken>,
    ) -> Vec<BlockToken> {
        if let Some(original_token) = original_token {
            if let Some(tokenizer) = find_block_tokenizer_by_name(
                self.shared.block_tokenizers,
                self.shared.block_tokenizer_map,
                self.shared.block_fallback_tokenizer,
                &original_token.tokenizer,
            ) {
                if let Some(mut token) = tokenizer.build_block_token(lines, original_token) {
                    token.tokenizer = tokenizer.name().to_string();
                    return vec![token];
                }
            }
        }

        let group = vec![lines.to_vec()];
        let root = match_block_tokens_with_shared(self.shared.clone(), &group);
        root.children
    }

    fn register_definition_identifier(&self, identifier: &str) {
        let mut state = self.shared.identifiers.borrow_mut();
        if state.is_register_available {
            state.definition_identifiers.insert(identifier.to_string());
        }
    }

    fn register_footnote_definition_identifier(&self, identifier: &str) {
        let mut state = self.shared.identifiers.borrow_mut();
        if state.is_register_available {
            state
                .footnote_definition_identifiers
                .insert(identifier.to_string());
        }
    }
}

struct ParseContext<'a, 'b> {
    options: &'b ProcessorOptions<'a>,
    definition_identifiers: &'b HashSet<String>,
    footnote_definition_identifiers: &'b HashSet<String>,
}

struct ParseBlockApiAdapter<'a, 'b> {
    context: &'b ParseContext<'a, 'b>,
}

impl ParseBlockPhaseApi for ParseBlockApiAdapter<'_, '_> {
    fn should_reserve_position(&self) -> bool {
        self.context.options.should_reserve_position
    }

    fn format_url(&self, url: &str) -> String {
        (self.context.options.format_url)(url)
    }

    fn process_inlines(&self, node_points: &[NodePoint]) -> Vec<Node> {
        process_inlines_with_context(self.context, node_points)
    }

    fn parse_block_tokens(&self, tokens: &[BlockToken]) -> Vec<Node> {
        parse_block_tokens_with_context(self.context, tokens)
    }
}

struct ParseInlineApiAdapter<'a, 'b> {
    context: &'b ParseContext<'a, 'b>,
    node_points: &'b [NodePoint],
}

impl ParseInlinePhaseApi for ParseInlineApiAdapter<'_, '_> {
    fn should_reserve_position(&self) -> bool {
        self.context.options.should_reserve_position
    }

    fn calc_position(&self, interval: NodeInterval) -> Option<Position> {
        if !self.context.options.should_reserve_position {
            return None;
        }
        calc_position_from_node_points(self.node_points, interval)
    }

    fn format_url(&self, url: &str) -> String {
        (self.context.options.format_url)(url)
    }

    fn get_node_points(&self) -> &[NodePoint] {
        self.node_points
    }

    fn has_definition(&self, identifier: &str) -> bool {
        self.context.definition_identifiers.contains(identifier)
    }

    fn has_footnote_definition(&self, identifier: &str) -> bool {
        self.context
            .footnote_definition_identifiers
            .contains(identifier)
    }

    fn parse_inline_tokens(&self, tokens: &[InlineToken]) -> Vec<Node> {
        parse_inline_tokens_with_context(self.context, self.node_points, tokens)
    }
}

struct MatchInlineApiAdapter<'a, 'b> {
    context: &'b ParseContext<'a, 'b>,
    node_points: &'b [NodePoint],
    block_start_index: usize,
    block_end_index: usize,
    tokenizer_start_index: usize,
}

impl MatchInlinePhaseApi for MatchInlineApiAdapter<'_, '_> {
    fn has_definition(&self, identifier: &str) -> bool {
        self.context.definition_identifiers.contains(identifier)
    }

    fn has_footnote_definition(&self, identifier: &str) -> bool {
        self.context
            .footnote_definition_identifiers
            .contains(identifier)
    }

    fn get_node_points(&self) -> &[NodePoint] {
        self.node_points
    }

    fn get_block_start_index(&self) -> usize {
        self.block_start_index
    }

    fn get_block_end_index(&self) -> usize {
        self.block_end_index
    }

    fn resolve_fallback_tokens(
        &self,
        tokens: &[InlineToken],
        token_start_index: usize,
        token_end_index: usize,
    ) -> Vec<InlineToken> {
        resolve_fallback_tokens_with_api(
            self.context.options.inline_fallback_tokenizer,
            self,
            tokens,
            token_start_index,
            token_end_index,
        )
    }

    fn resolve_internal_tokens(
        &self,
        higher_priority_tokens: &[InlineToken],
        start_index: usize,
        end_index: usize,
    ) -> Vec<InlineToken> {
        let tokens = match_inline_tokens_from_index(
            self.context,
            self.node_points,
            higher_priority_tokens,
            start_index,
            end_index,
            self.tokenizer_start_index,
        );

        resolve_fallback_tokens_with_api(
            self.context.options.inline_fallback_tokenizer,
            self,
            &tokens,
            start_index,
            end_index,
        )
    }
}

impl ParserProcessor<'_> {
    fn match_block_tokens(
        &self,
        lines: &[Vec<PhrasingContentLine>],
        identifiers: Rc<RefCell<IdentifierState>>,
    ) -> BlockToken {
        let shared = Rc::new(MatchBlockApiShared {
            block_tokenizers: self.options.block_tokenizers,
            block_tokenizer_map: self.options.block_tokenizer_map,
            block_fallback_tokenizer: self.options.block_fallback_tokenizer,
            identifiers,
        });
        match_block_tokens_with_shared(shared, lines)
    }

    fn parse_block_tokens(&self, tokens: &[BlockToken]) -> Vec<Node> {
        let context = ParseContext {
            options: &self.options,
            definition_identifiers: &self.definition_identifiers,
            footnote_definition_identifiers: &self.footnote_definition_identifiers,
        };
        parse_block_tokens_with_context(&context, tokens)
    }

    fn match_inline_tokens(
        &self,
        higher_priority_tokens: &[InlineToken],
        start_index: usize,
        end_index: usize,
        node_points: &[NodePoint],
    ) -> Vec<InlineToken> {
        let context = ParseContext {
            options: &self.options,
            definition_identifiers: &self.definition_identifiers,
            footnote_definition_identifiers: &self.footnote_definition_identifiers,
        };
        match_inline_tokens_with_context(
            &context,
            node_points,
            higher_priority_tokens,
            start_index,
            end_index,
        )
    }

    fn parse_inline_tokens(&self, node_points: &[NodePoint], tokens: &[InlineToken]) -> Vec<Node> {
        let context = ParseContext {
            options: &self.options,
            definition_identifiers: &self.definition_identifiers,
            footnote_definition_identifiers: &self.footnote_definition_identifiers,
        };
        parse_inline_tokens_with_context(&context, node_points, tokens)
    }
}

impl Processor for ParserProcessor<'_> {
    fn process(&mut self, lines: &[Vec<PhrasingContentLine>]) -> Root {
        self.definition_identifiers.clear();
        self.footnote_definition_identifiers.clear();

        let identifiers = Rc::new(RefCell::new(IdentifierState {
            is_register_available: true,
            ..IdentifierState::default()
        }));

        let block_token_tree = self.match_block_tokens(lines, identifiers.clone());

        {
            let mut state = identifiers.borrow_mut();
            state.is_register_available = false;
            self.definition_identifiers = state.definition_identifiers.clone();
            self.footnote_definition_identifiers = state.footnote_definition_identifiers.clone();
        }

        for definition in self.options.preset_definitions {
            self.definition_identifiers
                .insert(definition.identifier.clone());
        }
        for footnote in self.options.preset_footnote_definitions {
            self.footnote_definition_identifiers
                .insert(footnote.identifier.clone());
        }

        let root_position = if self.options.should_reserve_position {
            block_token_tree.position.clone()
        } else {
            None
        };

        let children = self.parse_block_tokens(&block_token_tree.children);

        Root {
            node_type: ROOT_TYPE.to_string(),
            position: root_position,
            children,
        }
    }
}

fn find_block_tokenizer_by_name<'a>(
    tokenizers: &'a [Box<dyn yozora_core_tokenizer::BlockTokenizer>],
    tokenizer_map: &HashMap<String, usize>,
    fallback_tokenizer: Option<&'a dyn yozora_core_tokenizer::BlockTokenizer>,
    name: &str,
) -> Option<&'a dyn yozora_core_tokenizer::BlockTokenizer> {
    if let Some(index) = tokenizer_map.get(name) {
        if let Some(tokenizer) = tokenizers.get(*index) {
            return Some(tokenizer.as_ref());
        }
    }

    if let Some(fallback_tokenizer) = fallback_tokenizer {
        if fallback_tokenizer.name() == name {
            return Some(fallback_tokenizer);
        }
    }

    None
}

fn find_inline_tokenizer_by_name<'a>(
    tokenizers: &'a [Box<dyn yozora_core_tokenizer::InlineTokenizer>],
    tokenizer_map: &HashMap<String, usize>,
    fallback_tokenizer: Option<&'a dyn yozora_core_tokenizer::InlineFallbackTokenizer>,
    name: &str,
) -> Option<&'a dyn yozora_core_tokenizer::InlineTokenizer> {
    if let Some(index) = tokenizer_map.get(name) {
        if let Some(tokenizer) = tokenizers.get(*index) {
            return Some(tokenizer.as_ref());
        }
    }

    if let Some(fallback_tokenizer) = fallback_tokenizer {
        if fallback_tokenizer.name() == name {
            return Some(fallback_tokenizer);
        }
    }

    None
}

fn match_block_tokens_with_shared(
    shared: Rc<MatchBlockApiShared<'_>>,
    lines: &[Vec<PhrasingContentLine>],
) -> BlockToken {
    let api = MatchBlockApiAdapter {
        shared: shared.clone(),
    };

    let mut hooks = Vec::with_capacity(shared.block_tokenizers.len());
    for tokenizer in shared.block_tokenizers {
        let hook = tokenizer.r#match(&api);
        hooks.push(MatchBlockProcessorHook::new(
            tokenizer.name(),
            tokenizer.priority(),
            hook,
        ));
    }

    let fallback_hook = shared.block_fallback_tokenizer.map(|tokenizer| {
        let hook = tokenizer.r#match(&api);
        MatchBlockProcessorHook::new(tokenizer.name(), tokenizer.priority(), hook)
    });

    let mut processor = create_block_content_processor(hooks, fallback_hook);
    for group in lines {
        for line in group {
            processor.consume(line);
        }
    }
    processor.done()
}

fn parse_block_tokens_with_context(
    context: &ParseContext<'_, '_>,
    tokens: &[BlockToken],
) -> Vec<Node> {
    if tokens.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    let mut i0 = 0usize;
    while i0 < tokens.len() {
        let tokenizer_name = &tokens[i0].tokenizer;
        let mut i1 = i0 + 1;
        while i1 < tokens.len() && tokens[i1].tokenizer == *tokenizer_name {
            i1 += 1;
        }

        if let Some(tokenizer) = find_block_tokenizer_by_name(
            context.options.block_tokenizers,
            context.options.block_tokenizer_map,
            context.options.block_fallback_tokenizer,
            tokenizer_name,
        ) {
            let api = ParseBlockApiAdapter { context };
            let hook = tokenizer.parse(&api);
            results.extend(hook.parse(&tokens[i0..i1]));
        }

        i0 = i1;
    }

    results
}

fn process_inlines_with_context(
    context: &ParseContext<'_, '_>,
    node_points: &[NodePoint],
) -> Vec<Node> {
    if node_points.is_empty() {
        return Vec::new();
    }

    let inline_tokens =
        match_inline_tokens_with_context(context, node_points, &[], 0, node_points.len());
    parse_inline_tokens_with_context(context, node_points, &inline_tokens)
}

fn match_inline_tokens_with_context(
    context: &ParseContext<'_, '_>,
    node_points: &[NodePoint],
    higher_priority_tokens: &[InlineToken],
    start_index: usize,
    end_index: usize,
) -> Vec<InlineToken> {
    let api = MatchInlineApiAdapter {
        context,
        node_points,
        block_start_index: start_index,
        block_end_index: end_index,
        tokenizer_start_index: 0,
    };

    let tokens = match_inline_tokens_from_index(
        context,
        node_points,
        higher_priority_tokens,
        start_index,
        end_index,
        0,
    );

    resolve_fallback_tokens_with_api(
        context.options.inline_fallback_tokenizer,
        &api,
        &tokens,
        start_index,
        end_index,
    )
}

fn match_inline_tokens_from_index(
    context: &ParseContext<'_, '_>,
    node_points: &[NodePoint],
    higher_priority_tokens: &[InlineToken],
    start_index: usize,
    end_index: usize,
    tokenizer_start_index: usize,
) -> Vec<InlineToken> {
    let tokenizers = context.options.inline_tokenizers;
    if tokenizer_start_index >= tokenizers.len() {
        return higher_priority_tokens.to_vec();
    }

    let mut tokens = higher_priority_tokens.to_vec();

    let mut i = tokenizer_start_index;
    while i < tokenizers.len() {
        let group_start = i;
        let priority = tokenizers[i].priority();
        i += 1;
        while i < tokenizers.len() && tokenizers[i].priority() == priority {
            i += 1;
        }
        let group_end = i;

        let api = MatchInlineApiAdapter {
            context,
            node_points,
            block_start_index: start_index,
            block_end_index: end_index,
            tokenizer_start_index: group_end,
        };

        let mut hooks = Vec::with_capacity(group_end - group_start);
        for tokenizer in &tokenizers[group_start..group_end] {
            let hook = tokenizer.r#match(&api);
            hooks.push(MatchInlineProcessorHook::new(
                tokenizer.name(),
                tokenizer.priority(),
                hook,
            ));
        }

        tokens = match_inline_tokens(&mut hooks, &tokens, start_index, end_index);
    }

    tokens
}

fn resolve_fallback_tokens_with_api(
    fallback_tokenizer: Option<&dyn yozora_core_tokenizer::InlineFallbackTokenizer>,
    api: &dyn MatchInlinePhaseApi,
    tokens: &[InlineToken],
    token_start_index: usize,
    token_end_index: usize,
) -> Vec<InlineToken> {
    let Some(fallback_tokenizer) = fallback_tokenizer else {
        return tokens.to_vec();
    };

    let mut i = token_start_index;
    let mut results = Vec::with_capacity(tokens.len() * 2 + 1);

    for token in tokens {
        if i < token.start_index {
            let mut fallback_token =
                fallback_tokenizer.findAndHandleDelimiter(i, token.start_index, api);
            fallback_token.tokenizer = fallback_tokenizer.name().to_string();
            results.push(fallback_token);
        }

        results.push(token.clone());
        i = token.end_index;
    }

    if i < token_end_index {
        let mut fallback_token =
            fallback_tokenizer.findAndHandleDelimiter(i, token_end_index, api);
        fallback_token.tokenizer = fallback_tokenizer.name().to_string();
        results.push(fallback_token);
    }

    results
}

fn parse_inline_tokens_with_context(
    context: &ParseContext<'_, '_>,
    node_points: &[NodePoint],
    tokens: &[InlineToken],
) -> Vec<Node> {
    if tokens.is_empty() {
        return Vec::new();
    }

    let mut results = Vec::new();
    let mut i0 = 0usize;
    while i0 < tokens.len() {
        let tokenizer_name = &tokens[i0].tokenizer;
        let mut i1 = i0 + 1;
        while i1 < tokens.len() && tokens[i1].tokenizer == *tokenizer_name {
            i1 += 1;
        }

        if let Some(tokenizer) = find_inline_tokenizer_by_name(
            context.options.inline_tokenizers,
            context.options.inline_tokenizer_map,
            context.options.inline_fallback_tokenizer,
            tokenizer_name,
        ) {
            let api = ParseInlineApiAdapter {
                context,
                node_points,
            };
            let hook = tokenizer.parse(&api);
            results.extend(hook.parse(&tokens[i0..i1]));
        }

        i0 = i1;
    }

    results
}

fn calc_position_from_node_points(
    node_points: &[NodePoint],
    interval: NodeInterval,
) -> Option<Position> {
    if interval.start_index >= interval.end_index {
        return None;
    }

    let start = node_points.get(interval.start_index)?;
    let end = node_points.get(interval.end_index.saturating_sub(1))?;

    Some(Position {
        start: Point {
            line: start.line,
            column: start.column,
            offset: Some(start.offset),
        },
        end: Point {
            line: end.line,
            column: end.column + 1,
            offset: Some(end.offset + 1),
        },
        indent: None,
    })
}

#[allow(dead_code)]
fn _sanity_check_inline_entry_points(
    processor: &ParserProcessor<'_>,
    node_points: &[NodePoint],
) -> (Vec<InlineToken>, Vec<Node>) {
    let tokens = processor.match_inline_tokens(&[], 0, node_points.len(), node_points);
    let nodes = processor.parse_inline_tokens(node_points, &tokens);
    (tokens, nodes)
}
