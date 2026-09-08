use yozora_ast::{Blockquote, Node, NodeBuffer};
use yozora_core_tokenizer::{
    BlockToken, ParseBlockError, ParseBlockGenerator, ParseBlockGeneratorResult,
    ParseBlockGeneratorResume, ParseBlockHookResult, ParseBlockPhaseApi, ParseBlockResult,
};

pub(crate) fn parse_blockquote_tokens<'a>(
    tokens: &'a [BlockToken],
    parse_api: &'a dyn ParseBlockPhaseApi,
) -> ParseBlockHookResult<'a> {
    ParseBlockHookResult::Generator(Box::new(BlockquoteParseGenerator {
        parse_api,
        tokens,
        next_token_index: 0,
        pending_token_index: None,
        nodes: NodeBuffer::with_capacity(tokens.len()),
        started: false,
    }))
}

struct BlockquoteParseGenerator<'a> {
    parse_api: &'a dyn ParseBlockPhaseApi,
    tokens: &'a [BlockToken],
    next_token_index: usize,
    pending_token_index: Option<usize>,
    nodes: NodeBuffer,
    started: bool,
}

impl<'a> ParseBlockGenerator<'a> for BlockquoteParseGenerator<'a> {
    fn resume(
        &mut self,
        state: ParseBlockGeneratorResume,
    ) -> ParseBlockResult<ParseBlockGeneratorResult<'a>> {
        if let Some(token_index) = self.pending_token_index.take() {
            let children = match state {
                ParseBlockGeneratorResume::Nodes(nodes) => nodes,
                ParseBlockGeneratorResume::Error(error) => return Err(error),
                ParseBlockGeneratorResume::Initial => {
                    return Err(ParseBlockError::new(
                        "[parseBlock] blockquote resumed without child nodes",
                    ));
                }
            };
            let token = &self.tokens[token_index];
            let position = if self.parse_api.should_reserve_position() {
                token.position.clone()
            } else {
                None
            };
            self.nodes
                .push(Node::Blockquote(Blockquote { position, children }));
            self.next_token_index = token_index + 1;
        } else if !self.started {
            if !matches!(state, ParseBlockGeneratorResume::Initial) {
                return Err(ParseBlockError::new(
                    "[parseBlock] blockquote generator did not start from initial state",
                ));
            }
            self.started = true;
        } else {
            return Err(ParseBlockError::new(
                "[parseBlock] blockquote generator resumed unexpectedly",
            ));
        }

        if self.next_token_index >= self.tokens.len() {
            return Ok(ParseBlockGeneratorResult::Complete(
                std::mem::take(&mut self.nodes).into_vec(),
            ));
        }

        let token_index = self.next_token_index;
        self.pending_token_index = Some(token_index);
        Ok(ParseBlockGeneratorResult::Yield(
            self.parse_api
                .request_block_tokens(Some(&self.tokens[token_index].children)),
        ))
    }
}
