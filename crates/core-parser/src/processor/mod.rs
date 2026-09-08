pub mod block;
pub mod inline;
pub mod types;

pub use types::{Processor, ProcessorApis, ProcessorOptions};

use std::cell::RefCell;
use std::collections::HashMap;
use std::collections::HashSet;
use std::rc::Rc;

use yozora_ast::{Node, Position, Root, ROOT_TYPE};
use yozora_character::NodePoint;
use yozora_core_tokenizer::NodeInterval;
use yozora_core_tokenizer::{
    calc_end_point, calc_start_point, BlockToken, InlineToken, MatchBlockPhaseApi,
    MatchInlineFallbackPhaseApi, MatchInlinePhaseApi, ParseBlockPhaseApi, ParseInlinePhaseApi,
    PhrasingContentLine,
};

use crate::processor::block::{create_block_content_processor, MatchBlockProcessorHook};
use crate::processor::inline::{
    create_phrasing_content_processor_from_hooks, create_processor_hook_groups_with_apis,
};

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
            token.tokenizer.as_ref(),
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
                original_token.tokenizer.as_ref(),
            ) {
                if let Some(mut token) = tokenizer.build_block_token(lines, original_token) {
                    token.tokenizer = std::sync::Arc::<str>::from(tokenizer.name());
                    return vec![token];
                }
            }
        }

        let group = vec![lines.to_vec()];
        let mut root = match_block_tokens_with_shared(self.shared.clone(), group);
        std::mem::take(&mut root.children).into_vec()
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
}

struct ParseInlineApiAdapter<'a, 'b> {
    context: &'b ParseContext<'a, 'b>,
    node_points: &'b [NodePoint],
}

impl ParseInlinePhaseApi for ParseInlineApiAdapter<'_, '_> {
    fn should_reserve_position(&self) -> bool {
        self.context.options.should_reserve_position
    }

    fn calc_position(&self, interval: NodeInterval) -> Position {
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

    fn parse_inline_tokens(&self, tokens: Option<&[InlineToken]>) -> Vec<Node> {
        let Some(tokens) = tokens else {
            return Vec::new();
        };
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

const DEFERRED_INLINE_TOKENIZER: &str = "@yozora/internal-deferred-inline";

#[derive(Clone)]
struct DeferredInlineRange {
    start_index: usize,
    end_index: usize,
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
        let mut covered_until = start_index;
        for token in higher_priority_tokens {
            if token.end_index <= start_index || token.start_index >= end_index {
                continue;
            }
            if token.start_index > covered_until {
                break;
            }
            covered_until = covered_until.max(token.end_index);
            if covered_until >= end_index {
                return higher_priority_tokens.to_vec();
            }
        }
        vec![InlineToken::new(
            DEFERRED_INLINE_TOKENIZER,
            "deferredInline",
            (start_index, end_index),
        )
        .with_children(higher_priority_tokens.to_vec())
        .with_data(DeferredInlineRange {
            start_index,
            end_index,
            tokenizer_start_index: self.tokenizer_start_index,
        })]
    }
}

impl MatchInlineFallbackPhaseApi for MatchInlineApiAdapter<'_, '_> {
    fn has_definition(&self, identifier: &str) -> bool {
        MatchInlinePhaseApi::has_definition(self, identifier)
    }

    fn has_footnote_definition(&self, identifier: &str) -> bool {
        MatchInlinePhaseApi::has_footnote_definition(self, identifier)
    }

    fn get_node_points(&self) -> &[NodePoint] {
        MatchInlinePhaseApi::get_node_points(self)
    }

    fn get_block_start_index(&self) -> usize {
        MatchInlinePhaseApi::get_block_start_index(self)
    }

    fn get_block_end_index(&self) -> usize {
        MatchInlinePhaseApi::get_block_end_index(self)
    }

    fn resolve_fallback_tokens(
        &self,
        tokens: &[InlineToken],
        token_start_index: usize,
        token_end_index: usize,
    ) -> Vec<InlineToken> {
        MatchInlinePhaseApi::resolve_fallback_tokens(
            self,
            tokens,
            token_start_index,
            token_end_index,
        )
    }
}

impl ParserProcessor<'_> {
    fn match_block_tokens<L>(
        &self,
        lines: L,
        identifiers: Rc<RefCell<IdentifierState>>,
    ) -> BlockToken
    where
        L: IntoIterator<Item = Vec<PhrasingContentLine>>,
    {
        let shared = Rc::new(MatchBlockApiShared {
            block_tokenizers: self.options.block_tokenizers,
            block_tokenizer_map: self.options.block_tokenizer_map,
            block_fallback_tokenizer: self.options.block_fallback_tokenizer,
            identifiers,
        });
        match_block_tokens_with_shared(shared, lines)
    }

    fn parse_block_tokens(&self, tokens: Option<&[BlockToken]>) -> Vec<Node> {
        let context = ParseContext {
            options: &self.options,
            definition_identifiers: &self.definition_identifiers,
            footnote_definition_identifiers: &self.footnote_definition_identifiers,
        };
        let Some(tokens) = tokens else {
            return Vec::new();
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

    fn parse_inline_tokens(
        &self,
        node_points: &[NodePoint],
        tokens: Option<&[InlineToken]>,
    ) -> Vec<Node> {
        let context = ParseContext {
            options: &self.options,
            definition_identifiers: &self.definition_identifiers,
            footnote_definition_identifiers: &self.footnote_definition_identifiers,
        };
        let Some(tokens) = tokens else {
            return Vec::new();
        };
        parse_inline_tokens_with_context(&context, node_points, tokens)
    }
}

impl Processor for ParserProcessor<'_> {
    fn process<L>(&mut self, lines: L) -> Root
    where
        L: IntoIterator<Item = Vec<PhrasingContentLine>>,
    {
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

        let children = self.parse_block_tokens(Some(&block_token_tree.children));

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
    if let Some(fallback_tokenizer) = fallback_tokenizer {
        if fallback_tokenizer.name() == name {
            return Some(fallback_tokenizer);
        }
    }

    if let Some(index) = tokenizer_map.get(name) {
        if let Some(tokenizer) = tokenizers.get(*index) {
            return Some(tokenizer.as_ref());
        }
    }

    None
}

fn match_block_tokens_with_shared(
    shared: Rc<MatchBlockApiShared<'_>>,
    lines: impl IntoIterator<Item = Vec<PhrasingContentLine>>,
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
        for line in &group {
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

    let api = ParseBlockApiAdapter { context };
    let mut parse_block_hooks: Vec<Box<dyn yozora_core_tokenizer::ParseBlockHook + '_>> =
        Vec::with_capacity(context.options.block_tokenizers.len());
    for tokenizer in context.options.block_tokenizers {
        parse_block_hooks.push(tokenizer.parse(&api));
    }

    let fallback_block_hook = context
        .options
        .block_fallback_tokenizer
        .map(|tokenizer| (tokenizer.name(), tokenizer.parse(&api)));

    let mut parse_block_hook_map: HashMap<&str, &dyn yozora_core_tokenizer::ParseBlockHook> =
        HashMap::with_capacity(
            parse_block_hooks.len() + usize::from(fallback_block_hook.is_some()),
        );
    for (tokenizer, hook) in context
        .options
        .block_tokenizers
        .iter()
        .zip(parse_block_hooks.iter())
    {
        parse_block_hook_map.insert(tokenizer.name(), hook.as_ref());
    }
    if let Some((name, hook)) = fallback_block_hook.as_ref() {
        parse_block_hook_map.insert(name, hook.as_ref());
    }

    block::parse::parse_block_tokens(Some(tokens), &parse_block_hook_map)
        .unwrap_or_else(|error| panic!("{error}"))
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

    let matched_tokens = match_inline_tokens_from_index(
        context,
        node_points,
        higher_priority_tokens,
        start_index,
        end_index,
        0,
    );

    let tokens = resolve_fallback_tokens_with_api(
        context.options.inline_fallback_tokenizer,
        &api,
        &matched_tokens,
        start_index,
        end_index,
    );
    drop(matched_tokens);
    resolve_deferred_inline_tokens(context, node_points, tokens)
}

fn resolve_deferred_inline_tokens(
    context: &ParseContext<'_, '_>,
    node_points: &[NodePoint],
    tokens: Vec<InlineToken>,
) -> Vec<InlineToken> {
    struct Frame {
        tokens: Vec<InlineToken>,
        index: usize,
        output: Vec<InlineToken>,
        parent: Option<InlineToken>,
    }

    let mut stack = vec![Frame {
        tokens,
        index: 0,
        output: Vec::new(),
        parent: None,
    }];
    loop {
        let frame = stack
            .last_mut()
            .expect("deferred inline stack should not be empty");
        if frame.index < frame.tokens.len() {
            let mut token = frame.tokens[frame.index].clone();
            frame.index += 1;
            if token.tokenizer.as_ref() == DEFERRED_INLINE_TOKENIZER {
                let higher_priority_tokens =
                    std::sync::Arc::unwrap_or_clone(std::mem::take(&mut token.children));
                let deferred = token
                    .data_as::<DeferredInlineRange>()
                    .expect("deferred inline token should contain range")
                    .clone();
                let api = MatchInlineApiAdapter {
                    context,
                    node_points,
                    block_start_index: deferred.start_index,
                    block_end_index: deferred.end_index,
                    tokenizer_start_index: deferred.tokenizer_start_index,
                };
                let matched = match_inline_tokens_from_index(
                    context,
                    node_points,
                    &higher_priority_tokens,
                    deferred.start_index,
                    deferred.end_index,
                    deferred.tokenizer_start_index,
                );
                let resolved = resolve_fallback_tokens_with_api(
                    context.options.inline_fallback_tokenizer,
                    &api,
                    &matched,
                    deferred.start_index,
                    deferred.end_index,
                );
                frame.tokens.splice(frame.index..frame.index, resolved);
                continue;
            }
            if token.children.is_empty() {
                frame.output.push(token);
            } else {
                let children = std::mem::take(&mut token.children);
                stack.push(Frame {
                    tokens: std::sync::Arc::unwrap_or_clone(children),
                    index: 0,
                    output: Vec::new(),
                    parent: Some(token),
                });
            }
            continue;
        }

        let frame = stack.pop().expect("deferred inline frame should exist");
        if let Some(mut parent) = frame.parent {
            parent.children = std::sync::Arc::new(frame.output);
            stack
                .last_mut()
                .expect("nested inline frame should have parent")
                .output
                .push(parent);
        } else {
            return frame.output;
        }
    }
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

    let mut groups = Vec::new();
    let mut i = tokenizer_start_index;
    while i < tokenizers.len() {
        let group_start = i;
        let priority = tokenizers[i].priority();
        i += 1;
        while i < tokenizers.len() && tokenizers[i].priority() == priority {
            i += 1;
        }
        let group_end = i;

        groups.push((group_start, group_end));
    }

    let mut apis = Vec::with_capacity(groups.len());
    for &(_, group_end) in &groups {
        apis.push(MatchInlineApiAdapter {
            context,
            node_points,
            block_start_index: start_index,
            block_end_index: end_index,
            tokenizer_start_index: group_end,
        });
    }

    let hook_groups =
        create_processor_hook_groups_with_apis(&tokenizers[tokenizer_start_index..], &apis);
    let mut processor = create_phrasing_content_processor_from_hooks(hook_groups, 0);
    processor.process(higher_priority_tokens, start_index, end_index)
}

fn resolve_fallback_tokens_with_api(
    fallback_tokenizer: Option<&dyn yozora_core_tokenizer::InlineFallbackTokenizer>,
    api: &dyn MatchInlineFallbackPhaseApi,
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
                fallback_tokenizer.find_and_handle_delimiter(i, token.start_index, api);
            fallback_token.tokenizer = std::sync::Arc::<str>::from(fallback_tokenizer.name());
            results.push(fallback_token);
        }

        results.push(token.clone());
        i = token.end_index;
    }

    if i < token_end_index {
        let mut fallback_token =
            fallback_tokenizer.find_and_handle_delimiter(i, token_end_index, api);
        fallback_token.tokenizer = std::sync::Arc::<str>::from(fallback_tokenizer.name());
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

    let api = ParseInlineApiAdapter {
        context,
        node_points,
    };
    inline::parse::parse_inline_tokens(
        tokens,
        context.options.inline_tokenizers,
        context.options.inline_tokenizer_map,
        context.options.inline_fallback_tokenizer,
        &api,
    )
}

fn calc_position_from_node_points(node_points: &[NodePoint], interval: NodeInterval) -> Position {
    if interval.start_index >= interval.end_index {
        panic!(
            "[parseInline.calcPosition] invalid interval: start_index({}) >= end_index({})",
            interval.start_index, interval.end_index
        );
    }

    node_points.get(interval.start_index).unwrap_or_else(|| {
        panic!(
            "[parseInline.calcPosition] start_index({}) out of range (len={})",
            interval.start_index,
            node_points.len()
        )
    });
    node_points
        .get(interval.end_index.saturating_sub(1))
        .unwrap_or_else(|| {
            panic!(
                "[parseInline.calcPosition] end_index({}) out of range (len={})",
                interval.end_index,
                node_points.len()
            )
        });

    Position {
        start: calc_start_point(node_points, interval.start_index),
        end: calc_end_point(node_points, interval.end_index - 1),
        indent: None,
    }
}

#[allow(dead_code)]
fn _sanity_check_inline_entry_points(
    processor: &ParserProcessor<'_>,
    node_points: &[NodePoint],
) -> (Vec<InlineToken>, Vec<Node>) {
    let tokens = processor.match_inline_tokens(&[], 0, node_points.len(), node_points);
    let nodes = processor.parse_inline_tokens(node_points, Some(&tokens));
    (tokens, nodes)
}
