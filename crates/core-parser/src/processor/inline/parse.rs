use std::collections::{BTreeMap, HashMap};

use yozora_ast::{drop_nodes, Node};
use yozora_core_tokenizer::{
    InlineFallbackTokenizer, InlineToken, InlineTokenizer, ParseInlineGenerator,
    ParseInlineGeneratorResult, ParseInlineHook, ParseInlineHookResult, ParseInlinePhaseApi,
};

struct Frame<'a> {
    tokens: &'a [InlineToken],
    next: usize,
    nodes: Vec<Node>,
    generator: Option<Box<dyn ParseInlineGenerator<'a> + 'a>>,
    fallback_hook: Option<Box<dyn ParseInlineHook + 'a>>,
    hooks: Vec<Box<dyn ParseInlineHook + 'a>>,
}

impl Drop for Frame<'_> {
    fn drop(&mut self) {
        drop_nodes(std::mem::take(&mut self.nodes));
    }
}

impl<'a> Frame<'a> {
    fn new(
        tokens: &'a [InlineToken],
        tokenizers: &'a [Box<dyn InlineTokenizer>],
        fallback: Option<&'a dyn InlineFallbackTokenizer>,
        api: &'a dyn ParseInlinePhaseApi,
        active: &mut BTreeMap<usize, usize>,
    ) -> Self {
        if !tokens.is_empty() {
            // Slices are contiguous: one address interval detects the same
            // active-token overlap without hashing every token in wide lists.
            let range = tokens.as_ptr_range();
            let (start, end) = (range.start as usize, range.end as usize);
            assert!(
                active
                    .range(..end)
                    .next_back()
                    .is_none_or(|(_, previous_end)| *previous_end <= start),
                "[parseInline] cyclic token tree at tokenizer '{}'",
                tokens[0].tokenizer
            );
            active.insert(start, end);
        }
        Self {
            tokens,
            next: 0,
            nodes: Vec::new(),
            generator: None,
            // Preserve the synchronous API's hook lifetime: one instance per
            // tokenizer for each nonempty token list, shared by its batches.
            hooks: if tokens.is_empty() {
                Vec::new()
            } else {
                tokenizers
                    .iter()
                    .map(|tokenizer| tokenizer.parse(api))
                    .collect()
            },
            fallback_hook: if tokens.is_empty() {
                None
            } else {
                fallback.map(|tokenizer| tokenizer.parse(api))
            },
        }
    }
}

pub(crate) fn parse_inline_tokens<'a>(
    tokens: &'a [InlineToken],
    tokenizers: &'a [Box<dyn InlineTokenizer>],
    tokenizer_map: &HashMap<String, usize>,
    fallback: Option<&'a dyn InlineFallbackTokenizer>,
    api: &'a dyn ParseInlinePhaseApi,
) -> Vec<Node> {
    let mut active = BTreeMap::new();
    let mut frames = vec![Frame::new(tokens, tokenizers, fallback, api, &mut active)];
    let mut children = None;
    loop {
        let frame = frames.last_mut().expect("inline parse frame exists");
        if let Some(mut generator) = frame.generator.take() {
            match generator.resume(children.take()) {
                ParseInlineGeneratorResult::Yield(tokens) => {
                    frame.generator = Some(generator);
                    frames.push(Frame::new(tokens, tokenizers, fallback, api, &mut active));
                }
                ParseInlineGeneratorResult::Complete(nodes) => frame.nodes.extend(nodes),
            }
        } else if frame.next == frame.tokens.len() {
            let mut frame = frames.pop().expect("inline parse frame exists");
            if !frame.tokens.is_empty() {
                active.remove(&(frame.tokens.as_ptr() as usize));
            }
            if frames.is_empty() {
                return std::mem::take(&mut frame.nodes);
            }
            children = Some(std::mem::take(&mut frame.nodes));
        } else {
            let start = frame.next;
            let name = frame.tokens[start].tokenizer.as_ref();
            frame.next += 1;
            while frame.next < frame.tokens.len()
                && frame.tokens[frame.next].tokenizer.as_ref() == name
            {
                frame.next += 1;
            }
            let batch = &frame.tokens[start..frame.next];
            let result = if let Some(index) = tokenizer_map.get(name) {
                tokenizers[*index]
                    .parse_deferred(batch, api)
                    .unwrap_or_else(|| {
                        ParseInlineHookResult::Nodes(frame.hooks[*index].parse(batch))
                    })
            } else if let Some(tokenizer) = fallback.filter(|tokenizer| tokenizer.name() == name) {
                tokenizer.parse_deferred(batch, api).unwrap_or_else(|| {
                    ParseInlineHookResult::Nodes(
                        frame
                            .fallback_hook
                            .as_ref()
                            .expect("fallback hook exists")
                            .parse(batch),
                    )
                })
            } else {
                panic!("[parseInline] tokenizer '{name}' not found")
            };
            match result {
                ParseInlineHookResult::Nodes(nodes) => frame.nodes.extend(nodes),
                ParseInlineHookResult::Generator(generator) => {
                    frame.generator = Some(generator);
                }
            }
        }
    }
}
