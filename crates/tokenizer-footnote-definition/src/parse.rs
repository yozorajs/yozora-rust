use yozora_ast::{FootnoteDefinition, Node, Position};
use yozora_core_tokenizer::{BlockToken, ParseBlockPhaseApi, ParseBlockTask, ParseBlockTaskStep};

#[derive(Debug, Clone)]
pub(crate) struct FootnoteDefinitionTokenData {
    pub label: String,
    pub identifier: String,
}

pub(crate) fn parse_footnote_definition_tokens(
    tokens: &[BlockToken],
    parse_api: &dyn ParseBlockPhaseApi,
) -> Vec<Node> {
    let mut nodes = Vec::with_capacity(tokens.len());

    for token in tokens {
        let Some(data) = token.data_as::<FootnoteDefinitionTokenData>() else {
            continue;
        };

        let children = parse_api.parse_block_tokens(Some(&token.children));
        nodes.push(Node::FootnoteDefinition(FootnoteDefinition {
            position: if parse_api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            },
            identifier: data.identifier.clone(),
            label: data.label.clone(),
            children,
        }));
    }

    nodes
}

struct PendingFootnoteDefinition {
    position: Option<Position>,
    identifier: String,
    label: String,
    children: Vec<BlockToken>,
}

struct FootnoteDefinitionParseTask {
    pending: Vec<PendingFootnoteDefinition>,
    next_index: usize,
    waiting_for_children: bool,
    nodes: Vec<Node>,
}

impl ParseBlockTask for FootnoteDefinitionParseTask {
    fn resume(&mut self, children: Option<Vec<Node>>) -> ParseBlockTaskStep {
        if self.waiting_for_children {
            let pending = &self.pending[self.next_index - 1];
            self.nodes
                .push(Node::FootnoteDefinition(FootnoteDefinition {
                    position: pending.position.clone(),
                    identifier: pending.identifier.clone(),
                    label: pending.label.clone(),
                    children: children
                        .expect("footnote definition task should resume with children"),
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

pub(crate) fn create_footnote_definition_parse_task(
    tokens: &[BlockToken],
    should_reserve_position: bool,
) -> Box<dyn ParseBlockTask> {
    let pending = tokens
        .iter()
        .filter_map(|token| {
            let data = token.data_as::<FootnoteDefinitionTokenData>()?;
            Some(PendingFootnoteDefinition {
                position: if should_reserve_position {
                    token.position.clone()
                } else {
                    None
                },
                identifier: data.identifier.clone(),
                label: data.label.clone(),
                children: token.children.to_vec(),
            })
        })
        .collect::<Vec<_>>();
    let capacity = pending.len();
    Box::new(FootnoteDefinitionParseTask {
        pending,
        next_index: 0,
        waiting_for_children: false,
        nodes: Vec::with_capacity(capacity),
    })
}
