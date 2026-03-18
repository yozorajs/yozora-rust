use yozora_ast::{Emphasis, Node, Strong, Text};
use yozora_core_tokenizer::{NodeInterval, ParseInlinePhaseApi};

use crate::r#match::{EmphasisMatchResult, SpanTree};

pub(crate) fn parse_emphasis(
    input: &str,
    matched: &EmphasisMatchResult,
    parse_api: Option<&dyn ParseInlinePhaseApi>,
) -> Vec<Node> {
    render_nodes(
        input,
        &matched.char_starts,
        matched.total_chars,
        &matched.tree,
        parse_api,
        0,
        matched.total_chars,
    )
}

fn render_nodes(
    input: &str,
    char_starts: &[usize],
    total_chars: usize,
    tree: &SpanTree,
    parse_api: Option<&dyn ParseInlinePhaseApi>,
    start: usize,
    end: usize,
) -> Vec<Node> {
    let mut nodes = Vec::new();
    let mut cursor = start;

    let span_ids: Vec<usize> = if start == 0 && end == total_chars {
        tree.roots.clone()
    } else {
        tree.roots
            .iter()
            .copied()
            .filter(|id| {
                let span = &tree.spans[*id];
                span.open >= start && span.end_marker() <= end
            })
            .collect()
    };

    render_span_list(
        input,
        char_starts,
        tree,
        parse_api,
        &span_ids,
        &mut nodes,
        &mut cursor,
        end,
    );

    nodes
}

fn render_span_list(
    input: &str,
    char_starts: &[usize],
    tree: &SpanTree,
    parse_api: Option<&dyn ParseInlinePhaseApi>,
    span_ids: &[usize],
    out: &mut Vec<Node>,
    cursor: &mut usize,
    end: usize,
) {
    for span_id in span_ids {
        let span = &tree.spans[*span_id];

        if span.open > *cursor {
            push_text(input, char_starts, parse_api, *cursor, span.open, out);
        }

        let inner_start = span.open + span.thickness;
        let inner_end = span.close;
        let mut children = Vec::new();
        let mut inner_cursor = inner_start;
        render_span_list(
            input,
            char_starts,
            tree,
            parse_api,
            &tree.children[*span_id],
            &mut children,
            &mut inner_cursor,
            inner_end,
        );

        let start_byte = char_to_byte_index(char_starts, input, span.open);
        let end_byte = char_to_byte_index(char_starts, input, span.end_marker());
        let position = parse_api.and_then(|api| {
            api.calc_position(NodeInterval {
                start_index: start_byte,
                end_index: end_byte,
            })
        });

        if span.thickness == 2 {
            out.push(Node::Strong(Strong { position, children }));
        } else {
            out.push(Node::Emphasis(Emphasis { position, children }));
        }

        *cursor = span.end_marker();
    }

    if *cursor < end {
        push_text(input, char_starts, parse_api, *cursor, end, out);
    }
}

fn push_text(
    input: &str,
    char_starts: &[usize],
    parse_api: Option<&dyn ParseInlinePhaseApi>,
    start: usize,
    end: usize,
    out: &mut Vec<Node>,
) {
    if start >= end {
        return;
    }

    let start_byte = char_to_byte_index(char_starts, input, start);
    let end_byte = char_to_byte_index(char_starts, input, end);
    let position = parse_api.and_then(|api| {
        api.calc_position(NodeInterval {
            start_index: start_byte,
            end_index: end_byte,
        })
    });

    out.push(Node::Text(Text {
        position,
        value: input[start_byte..end_byte].to_string(),
    }));
}

fn char_to_byte_index(char_starts: &[usize], input: &str, char_idx: usize) -> usize {
    if char_idx >= char_starts.len() {
        input.len()
    } else {
        char_starts[char_idx]
    }
}
