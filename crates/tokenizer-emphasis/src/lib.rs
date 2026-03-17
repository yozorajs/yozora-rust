use yozora_ast::{Emphasis, Node, Strong, Text};
use yozora_core_tokenizer::{InlineTokenizer, Tokenizer, TokenizerKind, TokenizerMeta};

pub const EMPHASIS_TOKENIZER_NAME: &str = "@yozora/tokenizer-emphasis";

#[derive(Debug, Clone)]
pub struct EmphasisTokenizer {
    meta: TokenizerMeta,
}

#[derive(Debug, Clone)]
struct DelimiterRun {
    marker: char,
    start: usize,
    end: usize,
    original_len: usize,
    consumed_left: usize,
    consumed_right: usize,
    can_open: bool,
    can_close: bool,
}

#[derive(Debug, Clone)]
struct EmphasisSpan {
    open: usize,
    close: usize,
    thickness: usize,
}

impl EmphasisSpan {
    fn end_marker(&self) -> usize {
        self.close + self.thickness
    }
}

impl DelimiterRun {
    fn available_len(&self) -> usize {
        self.end
            .saturating_sub(self.start)
            .saturating_sub(self.consumed_left + self.consumed_right)
    }

    fn is_both(&self) -> bool {
        self.can_open && self.can_close
    }
}

impl Default for EmphasisTokenizer {
    fn default() -> Self {
        Self {
            meta: TokenizerMeta {
                name: EMPHASIS_TOKENIZER_NAME.to_string(),
                kind: TokenizerKind::Inline,
                // Run after link/image/reference grouping.
                priority: 4,
            },
        }
    }
}

impl Tokenizer for EmphasisTokenizer {
    fn meta(&self) -> &TokenizerMeta {
        &self.meta
    }
}

impl InlineTokenizer for EmphasisTokenizer {
    fn tokenize_inline(
        &self,
        input: &str,
        _position: Option<yozora_ast::Position>,
    ) -> Option<Vec<Node>> {
        if !input.contains('*') && !input.contains('_') {
            return None;
        }

        let chars: Vec<char> = input.chars().collect();
        let char_starts: Vec<usize> = input.char_indices().map(|(idx, _)| idx).collect();
        let mut runs = collect_delimiter_runs(&chars);
        if runs.is_empty() {
            return None;
        }

        let spans = pair_emphasis_spans(&mut runs);
        if spans.is_empty() {
            return None;
        }

        let tree = build_span_tree(&spans);
        let nodes = render_nodes(input, &char_starts, chars.len(), &tree, 0, chars.len());

        if nodes.len() == 1 {
            if let Node::Text(text) = &nodes[0] {
                if text.value == input {
                    return None;
                }
            }
        }

        Some(nodes)
    }
}

fn collect_delimiter_runs(chars: &[char]) -> Vec<DelimiterRun> {
    let mut runs = Vec::new();
    let mut i = 0usize;

    while i < chars.len() {
        let ch = chars[i];
        if ch == '\\' {
            i += 2;
            continue;
        }

        if ch != '*' && ch != '_' {
            i += 1;
            continue;
        }

        let start = i;
        while i < chars.len() && chars[i] == ch {
            i += 1;
        }
        let end = i;

        let prev = if start > 0 {
            Some(chars[start - 1])
        } else {
            None
        };
        let next = if end < chars.len() {
            Some(chars[end])
        } else {
            None
        };

        let left_flanking = is_left_flanking(prev, next);
        let right_flanking = is_right_flanking(prev, next);

        let (can_open, can_close) = if ch == '_' && left_flanking && right_flanking {
            (
                prev.is_some_and(is_punctuation_char),
                next.is_some_and(is_punctuation_char),
            )
        } else {
            (left_flanking, right_flanking)
        };

        if can_open || can_close {
            runs.push(DelimiterRun {
                marker: ch,
                start,
                end,
                original_len: end - start,
                consumed_left: 0,
                consumed_right: 0,
                can_open,
                can_close,
            });
        }
    }

    runs
}

fn pair_emphasis_spans(runs: &mut [DelimiterRun]) -> Vec<EmphasisSpan> {
    let mut spans = Vec::new();

    for closer_idx in 0..runs.len() {
        if !runs[closer_idx].can_close {
            continue;
        }

        while runs[closer_idx].available_len() > 0 {
            let mut opener_idx = closer_idx;
            let mut matched = false;

            while opener_idx > 0 {
                opener_idx -= 1;

                if runs[opener_idx].marker != runs[closer_idx].marker {
                    continue;
                }
                if !runs[opener_idx].can_open || runs[opener_idx].available_len() == 0 {
                    continue;
                }

                if (runs[opener_idx].is_both() || runs[closer_idx].is_both())
                    && (runs[opener_idx].original_len + runs[closer_idx].original_len) % 3 == 0
                    && (runs[opener_idx].original_len % 3 != 0
                        || runs[closer_idx].original_len % 3 != 0)
                {
                    continue;
                }

                let thickness = if runs[opener_idx].available_len() > 1
                    && runs[closer_idx].available_len() > 1
                {
                    2
                } else {
                    1
                };

                let open = runs[opener_idx].end - runs[opener_idx].consumed_right - thickness;
                let close = runs[closer_idx].start + runs[closer_idx].consumed_left;

                if would_cross_existing_span(&spans, open, close) {
                    continue;
                }

                spans.push(EmphasisSpan {
                    open,
                    close,
                    thickness,
                });

                runs[opener_idx].consumed_right += thickness;
                runs[closer_idx].consumed_left += thickness;
                matched = true;
                break;
            }

            if !matched {
                break;
            }
        }
    }

    spans.sort_by(|a, b| {
        if a.open == b.open {
            b.close.cmp(&a.close)
        } else {
            a.open.cmp(&b.open)
        }
    });
    spans
}

fn would_cross_existing_span(spans: &[EmphasisSpan], open: usize, close: usize) -> bool {
    spans.iter().any(|span| {
        let inner_start = span.open + span.thickness;
        let inner_end = span.close;

        (open >= inner_start && open < inner_end && close >= span.end_marker())
            || (close >= inner_start && close < inner_end && open < span.open)
    })
}

#[derive(Debug, Clone)]
struct SpanTree {
    spans: Vec<EmphasisSpan>,
    children: Vec<Vec<usize>>,
    roots: Vec<usize>,
}

fn build_span_tree(spans: &[EmphasisSpan]) -> SpanTree {
    let mut children = vec![Vec::new(); spans.len()];
    let mut roots = Vec::new();
    let mut stack: Vec<usize> = Vec::new();

    for idx in 0..spans.len() {
        while let Some(last) = stack.last().copied() {
            if spans[last].end_marker() <= spans[idx].open {
                stack.pop();
            } else {
                break;
            }
        }

        if let Some(parent) = stack.last().copied() {
            children[parent].push(idx);
        } else {
            roots.push(idx);
        }
        stack.push(idx);
    }

    SpanTree {
        spans: spans.to_vec(),
        children,
        roots,
    }
}

fn render_nodes(
    input: &str,
    char_starts: &[usize],
    total_chars: usize,
    tree: &SpanTree,
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
    span_ids: &[usize],
    out: &mut Vec<Node>,
    cursor: &mut usize,
    end: usize,
) {
    for span_id in span_ids {
        let span = &tree.spans[*span_id];

        if span.open > *cursor {
            push_text(input, char_starts, *cursor, span.open, out);
        }

        let inner_start = span.open + span.thickness;
        let inner_end = span.close;
        let mut children = Vec::new();
        let mut inner_cursor = inner_start;
        render_span_list(
            input,
            char_starts,
            tree,
            &tree.children[*span_id],
            &mut children,
            &mut inner_cursor,
            inner_end,
        );

        if span.thickness == 2 {
            out.push(Node::Strong(Strong {
                position: None,
                children,
            }));
        } else {
            out.push(Node::Emphasis(Emphasis {
                position: None,
                children,
            }));
        }

        *cursor = span.end_marker();
    }

    if *cursor < end {
        push_text(input, char_starts, *cursor, end, out);
    }
}

fn push_text(input: &str, char_starts: &[usize], start: usize, end: usize, out: &mut Vec<Node>) {
    if start >= end {
        return;
    }

    let start_byte = char_to_byte_index(char_starts, input, start);
    let end_byte = char_to_byte_index(char_starts, input, end);
    out.push(Node::Text(Text {
        position: None,
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

fn is_left_flanking(prev: Option<char>, next: Option<char>) -> bool {
    let Some(next_char) = next else {
        return false;
    };
    if next_char.is_whitespace() {
        return false;
    }
    if !is_punctuation_char(next_char) {
        return true;
    }

    match prev {
        None => true,
        Some(ch) => ch.is_whitespace() || is_punctuation_char(ch),
    }
}

fn is_right_flanking(prev: Option<char>, next: Option<char>) -> bool {
    let Some(prev_char) = prev else {
        return false;
    };
    if prev_char.is_whitespace() {
        return false;
    }
    if !is_punctuation_char(prev_char) {
        return true;
    }

    match next {
        None => true,
        Some(ch) => ch.is_whitespace() || is_punctuation_char(ch),
    }
}

fn is_punctuation_char(ch: char) -> bool {
    ch.is_ascii_punctuation() || (!ch.is_alphanumeric() && !ch.is_whitespace())
}
