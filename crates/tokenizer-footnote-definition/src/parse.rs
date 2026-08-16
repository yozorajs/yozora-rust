use yozora_ast::{FootnoteDefinition, Node};
use yozora_core_tokenizer::{
    BlockToken, ParseBlockError, ParseBlockGenerator, ParseBlockGeneratorResult,
    ParseBlockGeneratorResume, ParseBlockHookResult, ParseBlockPhaseApi, ParseBlockResult,
};

use crate::types::FootnoteDefinitionTokenData;

pub(crate) fn parse_footnote_definition_tokens<'a>(
    tokens: &'a [BlockToken],
    parse_api: &'a dyn ParseBlockPhaseApi,
) -> ParseBlockHookResult<'a> {
    ParseBlockHookResult::Generator(Box::new(FootnoteDefinitionParseGenerator {
        parse_api,
        tokens,
        next_token_index: 0,
        pending: None,
        nodes: Vec::with_capacity(tokens.len()),
        started: false,
    }))
}

struct PendingFootnoteDefinition {
    token_index: usize,
    identifier: String,
    label: String,
}

struct FootnoteDefinitionParseGenerator<'a> {
    parse_api: &'a dyn ParseBlockPhaseApi,
    tokens: &'a [BlockToken],
    next_token_index: usize,
    pending: Option<PendingFootnoteDefinition>,
    nodes: Vec<Node>,
    started: bool,
}

impl<'a> ParseBlockGenerator<'a> for FootnoteDefinitionParseGenerator<'a> {
    fn resume(
        &mut self,
        state: ParseBlockGeneratorResume,
    ) -> ParseBlockResult<ParseBlockGeneratorResult<'a>> {
        if let Some(pending) = self.pending.take() {
            let children = match state {
                ParseBlockGeneratorResume::Nodes(nodes) => nodes,
                ParseBlockGeneratorResume::Error(error) => return Err(error),
                ParseBlockGeneratorResume::Initial => {
                    return Err(ParseBlockError::new(
                        "[parseBlock] footnote definition resumed without child nodes",
                    ));
                }
            };
            let token = &self.tokens[pending.token_index];
            self.nodes
                .push(Node::FootnoteDefinition(FootnoteDefinition {
                    position: if self.parse_api.should_reserve_position() {
                        token.position.clone()
                    } else {
                        None
                    },
                    identifier: pending.identifier,
                    label: pending.label,
                    children,
                }));
            self.next_token_index = pending.token_index + 1;
        } else if !self.started {
            if !matches!(state, ParseBlockGeneratorResume::Initial) {
                return Err(ParseBlockError::new(
                    "[parseBlock] footnote definition generator did not start from initial state",
                ));
            }
            self.started = true;
        } else {
            return Err(ParseBlockError::new(
                "[parseBlock] footnote definition generator resumed unexpectedly",
            ));
        }

        while self.next_token_index < self.tokens.len() {
            let token_index = self.next_token_index;
            let token = &self.tokens[token_index];
            let Some(data) = token.data_as::<FootnoteDefinitionTokenData>() else {
                self.next_token_index += 1;
                continue;
            };

            self.pending = Some(PendingFootnoteDefinition {
                token_index,
                identifier: data._identifier.clone().unwrap_or_default(),
                label: data._label.clone().unwrap_or_default(),
            });
            return Ok(ParseBlockGeneratorResult::Yield(
                self.parse_api.request_block_tokens(Some(&token.children)),
            ));
        }

        Ok(ParseBlockGeneratorResult::Complete(std::mem::take(
            &mut self.nodes,
        )))
    }
}
