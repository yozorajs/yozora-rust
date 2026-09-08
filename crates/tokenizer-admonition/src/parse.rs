use std::sync::Arc;

use yozora_ast::{Admonition, Node, NodeBuffer};
use yozora_character::{calc_escaped_string_from_node_points, is_unicode_whitespace_character};
use yozora_core_tokenizer::{
    merge_and_strip_content_lines, BlockToken, ParseBlockError, ParseBlockGenerator,
    ParseBlockGeneratorResult, ParseBlockGeneratorResume, ParseBlockHookResult, ParseBlockPhaseApi,
    ParseBlockResult, PhrasingContentLine,
};
use yozora_tokenizer_fenced_block::FencedBlockTokenData;

pub(crate) fn parse_admonition_tokens<'a>(
    tokens: &'a [BlockToken],
    parse_api: &'a dyn ParseBlockPhaseApi,
) -> ParseBlockHookResult<'a> {
    ParseBlockHookResult::Generator(Box::new(AdmonitionParseGenerator {
        parse_api,
        tokens,
        next_token_index: 0,
        pending: None,
        nodes: NodeBuffer::with_capacity(tokens.len()),
        started: false,
    }))
}

struct PendingAdmonition {
    token_index: usize,
    keyword: String,
    title: NodeBuffer,
}

struct AdmonitionParseGenerator<'a> {
    parse_api: &'a dyn ParseBlockPhaseApi,
    tokens: &'a [BlockToken],
    next_token_index: usize,
    pending: Option<PendingAdmonition>,
    nodes: NodeBuffer,
    started: bool,
}

impl<'a> ParseBlockGenerator<'a> for AdmonitionParseGenerator<'a> {
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
                        "[parseBlock] admonition resumed without child nodes",
                    ));
                }
            };
            let token = &self.tokens[pending.token_index];
            self.nodes.push(Node::Admonition(Admonition {
                position: if self.parse_api.should_reserve_position() {
                    token.position.clone()
                } else {
                    None
                },
                keyword: pending.keyword,
                title: pending.title.into_vec(),
                children,
            }));
            self.next_token_index = pending.token_index + 1;
        } else if !self.started {
            if !matches!(state, ParseBlockGeneratorResume::Initial) {
                return Err(ParseBlockError::new(
                    "[parseBlock] admonition generator did not start from initial state",
                ));
            }
            self.started = true;
        } else {
            return Err(ParseBlockError::new(
                "[parseBlock] admonition generator resumed unexpectedly",
            ));
        }

        while self.next_token_index < self.tokens.len() {
            let token_index = self.next_token_index;
            let token = &self.tokens[token_index];
            let Some(data) = token.data_as::<FencedBlockTokenData>() else {
                self.next_token_index += 1;
                continue;
            };

            let info_string = &data.info_string;
            let mut i = 0usize;
            while i < info_string.len()
                && !is_unicode_whitespace_character(info_string[i].code_point)
            {
                i += 1;
            }

            let keyword = calc_escaped_string_from_node_points(info_string, 0, i, true);

            while i < info_string.len()
                && is_unicode_whitespace_character(info_string[i].code_point)
            {
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
                self.parse_api.process_inlines(&contents)
            };

            self.pending = Some(PendingAdmonition {
                token_index,
                keyword,
                title: title.into(),
            });
            return Ok(ParseBlockGeneratorResult::Yield(
                self.parse_api.request_block_tokens(Some(&token.children)),
            ));
        }

        Ok(ParseBlockGeneratorResult::Complete(
            std::mem::take(&mut self.nodes).into_vec(),
        ))
    }
}
