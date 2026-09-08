use std::any::Any;
use std::collections::{HashMap, HashSet};
use std::panic::{catch_unwind, AssertUnwindSafe};

use yozora_ast::{drop_nodes, Node};
use yozora_core_tokenizer::{
    BlockToken, ParseBlockError, ParseBlockGenerator, ParseBlockGeneratorResult,
    ParseBlockGeneratorResume, ParseBlockHook, ParseBlockHookResult, ParseBlockResult,
};

pub(crate) fn parse_block_tokens<'a>(
    tokens: Option<&'a [BlockToken]>,
    hook_map: &'a HashMap<&'a str, &'a dyn ParseBlockHook>,
) -> ParseBlockResult<Vec<Node>> {
    let Some(tokens) = tokens else {
        return Ok(Vec::new());
    };
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    ParseBlockScheduler::new(hook_map).parse(tokens)
}

enum ParseBlockGeneratorState {
    Initial,
    WaitingForChild,
    ResumeWithNodes(Vec<Node>),
    ResumeWithError(ParseBlockError),
}

struct ParseBlockHookExecution<'a> {
    generator: Box<dyn ParseBlockGenerator<'a> + 'a>,
    state: ParseBlockGeneratorState,
}

struct ParseBlockFrame<'a> {
    tokens: &'a [BlockToken],
    next_token_index: usize,
    parsed_nodes: Vec<Node>,
    hook_execution: Option<ParseBlockHookExecution<'a>>,
}

impl Drop for ParseBlockFrame<'_> {
    fn drop(&mut self) {
        drop_nodes(std::mem::take(&mut self.parsed_nodes));
    }
}

struct ParseBlockScheduler<'a> {
    frame_stack: Vec<ParseBlockFrame<'a>>,
    active_token_set: HashSet<usize>,
    hook_map: &'a HashMap<&'a str, &'a dyn ParseBlockHook>,
}

impl<'a> ParseBlockScheduler<'a> {
    fn new(hook_map: &'a HashMap<&'a str, &'a dyn ParseBlockHook>) -> Self {
        Self {
            frame_stack: Vec::new(),
            active_token_set: HashSet::new(),
            hook_map,
        }
    }

    fn parse(mut self, tokens: &'a [BlockToken]) -> ParseBlockResult<Vec<Node>> {
        let frame = self.create_frame(tokens)?;
        self.frame_stack.push(frame);

        while !self.frame_stack.is_empty() {
            let has_hook_execution = self
                .frame_stack
                .last()
                .is_some_and(|frame| frame.hook_execution.is_some());
            let root_nodes = if has_hook_execution {
                self.step_hook_execution()?
            } else {
                self.step_token_list_frame()?
            };
            if let Some(root_nodes) = root_nodes {
                return Ok(root_nodes);
            }
        }

        Err(ParseBlockError::new(
            "[parseBlock] scheduler stopped without a result",
        ))
    }

    fn create_frame(&mut self, tokens: &'a [BlockToken]) -> ParseBlockResult<ParseBlockFrame<'a>> {
        let mut registered_tokens = Vec::with_capacity(tokens.len());
        for token in tokens {
            let token_key = std::ptr::from_ref(token) as usize;
            if !self.active_token_set.insert(token_key) {
                for registered_token_key in registered_tokens {
                    self.active_token_set.remove(&registered_token_key);
                }
                return Err(ParseBlockError::new(format!(
                    "[parseBlock] cyclic or shared token tree at tokenizer '{}'",
                    token.tokenizer
                )));
            }
            registered_tokens.push(token_key);
        }

        Ok(ParseBlockFrame {
            tokens,
            next_token_index: 0,
            parsed_nodes: Vec::new(),
            hook_execution: None,
        })
    }

    fn settle_frame(
        &mut self,
        state: ParseBlockGeneratorState,
    ) -> ParseBlockResult<Option<Vec<Node>>> {
        let frame = self
            .frame_stack
            .pop()
            .ok_or_else(|| ParseBlockError::new("[parseBlock] unexpected token-list frame"))?;
        for token in frame.tokens {
            self.active_token_set
                .remove(&(std::ptr::from_ref(token) as usize));
        }

        let Some(parent_frame) = self.frame_stack.last_mut() else {
            return match state {
                ParseBlockGeneratorState::ResumeWithNodes(nodes) => Ok(Some(nodes)),
                ParseBlockGeneratorState::ResumeWithError(error) => Err(error),
                _ => Err(ParseBlockError::new(
                    "[parseBlock] unexpected root frame state",
                )),
            };
        };

        let parent_hook_execution = parent_frame
            .hook_execution
            .as_mut()
            .ok_or_else(|| ParseBlockError::new("[parseBlock] expected active hook execution"))?;
        if !matches!(
            parent_hook_execution.state,
            ParseBlockGeneratorState::WaitingForChild
        ) {
            return Err(ParseBlockError::new(
                "[parseBlock] generator is not waiting for a child",
            ));
        }
        parent_hook_execution.state = state;
        Ok(None)
    }

    fn step_token_list_frame(&mut self) -> ParseBlockResult<Option<Vec<Node>>> {
        let frame = self
            .frame_stack
            .last_mut()
            .ok_or_else(|| ParseBlockError::new("[parseBlock] expected token-list frame"))?;
        if frame.next_token_index >= frame.tokens.len() {
            let nodes = std::mem::take(&mut frame.parsed_nodes);
            return self.settle_frame(ParseBlockGeneratorState::ResumeWithNodes(nodes));
        }

        let batch_start_index = frame.next_token_index;
        let tokenizer_name = frame.tokens[batch_start_index].tokenizer.as_ref();
        let mut batch_end_index = batch_start_index + 1;
        while batch_end_index < frame.tokens.len()
            && frame.tokens[batch_end_index].tokenizer.as_ref() == tokenizer_name
        {
            batch_end_index += 1;
        }
        frame.next_token_index = batch_end_index;

        let tokens: &'a [BlockToken] = frame.tokens;
        let token_batch: &'a [BlockToken] = &tokens[batch_start_index..batch_end_index];
        let Some(hook) = self.hook_map.get(tokenizer_name).copied() else {
            return self.settle_frame(ParseBlockGeneratorState::ResumeWithError(
                ParseBlockError::new(format!(
                    "[parseBlock] tokenizer '{tokenizer_name}' not found"
                )),
            ));
        };

        let hook_result = catch_parse_block(|| hook.parse(token_batch));
        match hook_result {
            Ok(ParseBlockHookResult::Nodes(nodes)) => {
                self.frame_stack
                    .last_mut()
                    .expect("block parse frame should exist")
                    .parsed_nodes
                    .extend(nodes);
                Ok(None)
            }
            Ok(ParseBlockHookResult::Generator(generator)) => {
                self.frame_stack
                    .last_mut()
                    .expect("block parse frame should exist")
                    .hook_execution = Some(ParseBlockHookExecution {
                    generator,
                    state: ParseBlockGeneratorState::Initial,
                });
                Ok(None)
            }
            Err(error) => self.settle_frame(ParseBlockGeneratorState::ResumeWithError(error)),
        }
    }

    fn step_hook_execution(&mut self) -> ParseBlockResult<Option<Vec<Node>>> {
        let mut hook_execution = self
            .frame_stack
            .last_mut()
            .and_then(|frame| frame.hook_execution.take())
            .ok_or_else(|| ParseBlockError::new("[parseBlock] expected active hook execution"))?;

        let resume = match std::mem::replace(
            &mut hook_execution.state,
            ParseBlockGeneratorState::WaitingForChild,
        ) {
            ParseBlockGeneratorState::Initial => ParseBlockGeneratorResume::Initial,
            ParseBlockGeneratorState::ResumeWithNodes(nodes) => {
                ParseBlockGeneratorResume::Nodes(nodes)
            }
            ParseBlockGeneratorState::ResumeWithError(error) => {
                ParseBlockGeneratorResume::Error(error)
            }
            ParseBlockGeneratorState::WaitingForChild => {
                return Err(ParseBlockError::new(
                    "[parseBlock] generator resumed without a child result",
                ));
            }
        };

        let generator_result = catch_parse_block(|| hook_execution.generator.resume(resume));
        match generator_result {
            Ok(ParseBlockGeneratorResult::Complete(nodes)) => {
                self.frame_stack
                    .last_mut()
                    .expect("block parse frame should exist")
                    .parsed_nodes
                    .extend(nodes);
                Ok(None)
            }
            Ok(ParseBlockGeneratorResult::Yield(request)) => {
                let Some(tokens) = request.tokens else {
                    hook_execution.state = ParseBlockGeneratorState::ResumeWithNodes(Vec::new());
                    self.frame_stack
                        .last_mut()
                        .expect("block parse frame should exist")
                        .hook_execution = Some(hook_execution);
                    return Ok(None);
                };
                if tokens.is_empty() {
                    hook_execution.state = ParseBlockGeneratorState::ResumeWithNodes(Vec::new());
                    self.frame_stack
                        .last_mut()
                        .expect("block parse frame should exist")
                        .hook_execution = Some(hook_execution);
                    return Ok(None);
                }

                hook_execution.state = ParseBlockGeneratorState::WaitingForChild;
                self.frame_stack
                    .last_mut()
                    .expect("block parse frame should exist")
                    .hook_execution = Some(hook_execution);
                match self.create_frame(tokens) {
                    Ok(frame) => self.frame_stack.push(frame),
                    Err(error) => {
                        self.frame_stack
                            .last_mut()
                            .and_then(|frame| frame.hook_execution.as_mut())
                            .expect("block parse hook execution should exist")
                            .state = ParseBlockGeneratorState::ResumeWithError(error);
                    }
                }
                Ok(None)
            }
            Err(error) => self.settle_frame(ParseBlockGeneratorState::ResumeWithError(error)),
        }
    }
}

fn catch_parse_block<T>(operation: impl FnOnce() -> ParseBlockResult<T>) -> ParseBlockResult<T> {
    match catch_unwind(AssertUnwindSafe(operation)) {
        Ok(result) => result,
        Err(payload) => Err(parse_block_error_from_panic(payload)),
    }
}

fn parse_block_error_from_panic(payload: Box<dyn Any + Send>) -> ParseBlockError {
    match payload.downcast::<String>() {
        Ok(message) => ParseBlockError::new(*message),
        Err(payload) => match payload.downcast::<&'static str>() {
            Ok(message) => ParseBlockError::new(*message),
            Err(_) => ParseBlockError::new("[parseBlock] hook panicked"),
        },
    }
}
