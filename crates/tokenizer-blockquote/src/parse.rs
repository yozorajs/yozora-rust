use yozora_ast::{Blockquote, Node, Position};
use yozora_core_tokenizer::types::parse_block::{ParseBlockTask, ParseBlockTaskStep};
use yozora_core_tokenizer::{BlockToken, ParseBlockPhaseApi};

pub(crate) fn parse_blockquote_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let children = parse_api.parse_block_tokens(Some(&token.children));
        let position = if parse_api.should_reserve_position() {
            token.position.clone()
        } else {
            None
        };
        nodes.push(Node::Blockquote(Blockquote { position, children }));
    }

    nodes
}

struct PendingBlockquote {
    position: Option<Position>,
    children: Vec<BlockToken>,
}

struct BlockquoteParseTask {
    pending: Vec<PendingBlockquote>,
    next_index: usize,
    waiting_for_children: bool,
    nodes: Vec<Node>,
}

impl ParseBlockTask for BlockquoteParseTask {
    fn resume(&mut self, children: Option<Vec<Node>>) -> ParseBlockTaskStep {
        if self.waiting_for_children {
            let pending = &self.pending[self.next_index - 1];
            self.nodes.push(Node::Blockquote(Blockquote {
                position: pending.position.clone(),
                children: children.expect("blockquote task should resume with children"),
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

pub(crate) fn create_blockquote_parse_task(
    tokens: &[BlockToken],
    should_reserve_position: bool,
) -> Box<dyn ParseBlockTask> {
    Box::new(BlockquoteParseTask {
        pending: tokens
            .iter()
            .map(|token| PendingBlockquote {
                position: should_reserve_position
                    .then(|| token.position.clone())
                    .flatten(),
                children: token.children.to_vec(),
            })
            .collect(),
        next_index: 0,
        waiting_for_children: false,
        nodes: Vec::with_capacity(tokens.len()),
    })
}
