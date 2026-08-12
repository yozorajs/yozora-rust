use std::sync::Arc;

use yozora_ast::{Admonition, Node, Position};
use yozora_character::{calc_escaped_string_from_node_points, is_unicode_whitespace_character};
use yozora_core_tokenizer::{
    merge_and_strip_content_lines, BlockToken, ParseBlockPhaseApi, ParseBlockTask,
    ParseBlockTaskStep, PhrasingContentLine,
};
use yozora_tokenizer_fenced_block::FencedBlockTokenData;

pub(crate) fn parse_admonition_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<FencedBlockTokenData>() else {
            continue;
        };

        let info_string = &data.info_string;
        let mut i = 0usize;
        while i < info_string.len() && !is_unicode_whitespace_character(info_string[i].code_point) {
            i += 1;
        }

        let keyword = calc_escaped_string_from_node_points(info_string, 0, i, true);

        while i < info_string.len() && is_unicode_whitespace_character(info_string[i].code_point) {
            i += 1;
        }

        let title = if i >= info_string.len() {
            Vec::new()
        } else {
            let title_lines = vec![PhrasingContentLine {
                node_points: Arc::new(info_string.clone()),
                start_index: i,
                end_index: info_string.len(),
                first_non_whitespace_index: i,
                indent_width: 0,
                count_of_precede_spaces: 0,
            }];
            let contents = merge_and_strip_content_lines(&title_lines, 0, title_lines.len());
            parse_api.process_inlines(&contents)
        };

        let children = parse_api.parse_block_tokens(Some(&token.children));
        nodes.push(Node::Admonition(Admonition {
            position: if parse_api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            },
            keyword,
            title,
            children,
        }));
    }

    nodes
}

struct PendingAdmonition {
    position: Option<Position>,
    keyword: String,
    title: Vec<Node>,
    children: Vec<BlockToken>,
}

struct AdmonitionParseTask {
    pending: Vec<PendingAdmonition>,
    next_index: usize,
    waiting_for_children: bool,
    nodes: Vec<Node>,
}

impl ParseBlockTask for AdmonitionParseTask {
    fn resume(&mut self, children: Option<Vec<Node>>) -> ParseBlockTaskStep {
        if self.waiting_for_children {
            let pending = &self.pending[self.next_index - 1];
            self.nodes.push(Node::Admonition(Admonition {
                position: pending.position.clone(),
                keyword: pending.keyword.clone(),
                title: pending.title.clone(),
                children: children.expect("admonition task should resume with children"),
            }));
            self.waiting_for_children = false;
        }

        if self.next_index >= self.pending.len() {
            return ParseBlockTaskStep::Done(std::mem::take(&mut self.nodes));
        }

        let children = self.pending[self.next_index].children.clone();
        self.next_index += 1;
        self.waiting_for_children = true;
        ParseBlockTaskStep::Request(children)
    }
}

pub(crate) fn create_admonition_parse_task(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Box<dyn ParseBlockTask> {
    let mut pending = Vec::with_capacity(tokens.len());
    for token in tokens {
        let Some(data) = token.data_as::<FencedBlockTokenData>() else {
            continue;
        };

        let info_string = &data.info_string;
        let mut i = 0usize;
        while i < info_string.len() && !is_unicode_whitespace_character(info_string[i].code_point) {
            i += 1;
        }
        let keyword = calc_escaped_string_from_node_points(info_string, 0, i, true);
        while i < info_string.len() && is_unicode_whitespace_character(info_string[i].code_point) {
            i += 1;
        }
        let title = if i >= info_string.len() {
            Vec::new()
        } else {
            let title_lines = vec![PhrasingContentLine {
                node_points: Arc::new(info_string.clone()),
                start_index: i,
                end_index: info_string.len(),
                first_non_whitespace_index: i,
                indent_width: 0,
                count_of_precede_spaces: 0,
            }];
            let contents = merge_and_strip_content_lines(&title_lines, 0, title_lines.len());
            parse_api.process_inlines(&contents)
        };
        pending.push(PendingAdmonition {
            position: if parse_api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            },
            keyword,
            title,
            children: token.children.to_vec(),
        });
    }

    let capacity = pending.len();
    Box::new(AdmonitionParseTask {
        pending,
        next_index: 0,
        waiting_for_children: false,
        nodes: Vec::with_capacity(capacity),
    })
}
