pub(crate) mod parse;
mod types;

pub use types::{MatchBlockProcessorHook, SharedMatchBlockHook};

use yozora_ast::{Point, Position, PARAGRAPH_TYPE, ROOT_TYPE};
use yozora_character::{is_space_character, is_whitespace_character};
use yozora_core_tokenizer::{
    calc_end_point, BlockToken, EatContinuationTextResult, EatLazyContinuationTextResult,
    OnCloseResult, PhrasingContentLine, RemainingSibling,
};

#[derive(Debug, Clone)]
struct MatchBlockState {
    hook_index: Option<usize>,
    path: Vec<usize>,
    is_containing_block: bool,
    priority: i32,
}

pub struct BlockContentProcessor<'a> {
    hooks: Vec<MatchBlockProcessorHook<'a>>,
    hook_index_mapping: Vec<usize>,
    normal_hook_len: usize,
    fallback_hook_index: Option<usize>,
    root: BlockToken,
    state_stack: Vec<MatchBlockState>,
    current_stack_index: usize,
}

pub fn create_block_content_processor<'a>(
    hooks: Vec<MatchBlockProcessorHook<'a>>,
    fallback_hook: Option<MatchBlockProcessorHook<'a>>,
) -> BlockContentProcessor<'a> {
    let hook_index_mapping = (0..hooks.len()).collect::<Vec<_>>();
    create_block_content_processor_with_mapping(hooks, fallback_hook, hook_index_mapping, None)
}

fn create_block_content_processor_with_mapping<'a>(
    hooks: Vec<MatchBlockProcessorHook<'a>>,
    fallback_hook: Option<MatchBlockProcessorHook<'a>>,
    hook_index_mapping: Vec<usize>,
    fallback_hook_mapping: Option<usize>,
) -> BlockContentProcessor<'a> {
    let normal_hook_len = hooks.len();

    let mut hooks = hooks;
    let mut hook_index_mapping = hook_index_mapping;
    let fallback_hook_index = fallback_hook.map(|hook| {
        let index = hooks.len();
        hooks.push(hook);
        hook_index_mapping.push(fallback_hook_mapping.unwrap_or(index));
        index
    });

    let root = BlockToken::new(
        "root",
        ROOT_TYPE,
        Some(Position {
            start: Point {
                line: 1,
                column: 1,
                offset: Some(0),
            },
            end: Point {
                line: 1,
                column: 1,
                offset: Some(0),
            },
            indent: None,
        }),
    );

    let state_stack = vec![MatchBlockState {
        hook_index: None,
        path: Vec::new(),
        is_containing_block: true,
        priority: i32::MAX,
    }];

    BlockContentProcessor {
        hooks,
        hook_index_mapping,
        normal_hook_len,
        fallback_hook_index,
        root,
        state_stack,
        current_stack_index: 0,
    }
}

impl<'a> BlockContentProcessor<'a> {
    pub fn consume(&mut self, line: &PhrasingContentLine) {
        let mut i = line.start_index;
        let end_index_of_line = line.end_index;
        let mut first_non_whitespace_index = line.first_non_whitespace_index;
        let mut count_of_precede_spaces = line.count_of_precede_spaces;

        let move_forward = |this: &mut Self,
                            i_ref: &mut usize,
                            first_ref: &mut usize,
                            spaces_ref: &mut usize,
                            next_index: usize,
                            should_refresh_position: bool| {
            debug_assert!(*i_ref <= next_index);

            if should_refresh_position && next_index > 0 {
                let end_point = calc_end_point(line.node_points.as_ref(), next_index - 1);
                this.refresh_position(end_point);
            }
            if *i_ref == next_index {
                return;
            }

            *i_ref = next_index;
            *spaces_ref = 0;
            *first_ref = next_index;
            while *first_ref < end_index_of_line {
                let code_point = line.node_points[*first_ref].code_point;
                if is_space_character(code_point) {
                    *spaces_ref += 1;
                    *first_ref += 1;
                    continue;
                }
                if !is_whitespace_character(code_point) {
                    break;
                }
                *first_ref += 1;
            }
        };

        // Step 1
        self.current_stack_index = 1;
        if self.state_stack.len() >= 2 {
            while i < end_index_of_line && self.current_stack_index < self.state_stack.len() {
                let current_state = self.state_stack[self.current_stack_index].clone();
                let Some(current_hook_idx) = current_state.hook_index else {
                    break;
                };

                let eating_info = self.eating_info(
                    line,
                    i,
                    end_index_of_line,
                    first_non_whitespace_index,
                    count_of_precede_spaces,
                );

                let mut interrupted = false;
                for hook_idx in 0..self.normal_hook_len {
                    if hook_idx == current_hook_idx {
                        continue;
                    }
                    if self.interrupt_sibling(
                        hook_idx,
                        &eating_info,
                        &mut i,
                        &mut first_non_whitespace_index,
                        &mut count_of_precede_spaces,
                        &move_forward,
                    ) {
                        interrupted = true;
                        break;
                    }
                }
                if interrupted {
                    break;
                }

                let parent_path = self.state_stack[self.current_stack_index - 1].path.clone();
                let current_path = current_state.path.clone();
                let hook = self.hooks[current_hook_idx].hook.clone();
                let result = hook.borrow_mut().eat_continuation_text_without_parent(
                    &eating_info,
                    token_mut(&mut self.root, &current_path),
                );
                let result = result.unwrap_or_else(|| {
                    let (child_index, mut token) = detach_child_for_parent_snapshot(
                        &mut self.root,
                        &parent_path,
                        &current_path,
                    );
                    let parent_token = token_ref(&self.root, &parent_path);
                    let result = hook.borrow_mut().eat_continuation_text(
                        &eating_info,
                        &mut token,
                        parent_token,
                    );
                    restore_detached_child(&mut self.root, &parent_path, child_index, token);
                    result
                });

                let mut finished = false;
                let mut rolled_back = false;
                match result {
                    EatContinuationTextResult::FailedAndRollback { lines } => {
                        if let Some(parent) = token_mut(&mut self.root, &parent_path).children.pop()
                        {
                            let _ = parent;
                        }

                        self.state_stack.truncate(self.current_stack_index);
                        self.current_stack_index = self.current_stack_index.saturating_sub(1);

                        if !lines.is_empty() {
                            rolled_back =
                                self.rollback(current_hook_idx, &lines, self.current_stack_index);
                        }
                        if !rolled_back {
                            finished = true;
                        }
                    }
                    EatContinuationTextResult::ClosingAndRollback { lines } => {
                        self.cut_stale_branch(self.current_stack_index);

                        if !lines.is_empty() {
                            rolled_back =
                                self.rollback(current_hook_idx, &lines, self.current_stack_index);
                        }
                        if !rolled_back {
                            finished = true;
                        }
                    }
                    EatContinuationTextResult::NotMatched => {
                        self.current_stack_index = self.current_stack_index.saturating_sub(1);
                        finished = true;
                    }
                    EatContinuationTextResult::Closing { next_index } => {
                        move_forward(
                            self,
                            &mut i,
                            &mut first_non_whitespace_index,
                            &mut count_of_precede_spaces,
                            next_index,
                            true,
                        );
                        self.current_stack_index = self.current_stack_index.saturating_sub(1);
                        finished = true;
                    }
                    EatContinuationTextResult::Opening { next_index } => {
                        move_forward(
                            self,
                            &mut i,
                            &mut first_non_whitespace_index,
                            &mut count_of_precede_spaces,
                            next_index,
                            true,
                        );
                    }
                }

                if finished {
                    break;
                }
                if rolled_back {
                    continue;
                }

                self.current_stack_index += 1;
            }
        }

        // Step 2
        if i < end_index_of_line {
            let mut skip_step2 = false;
            if self.current_stack_index < self.state_stack.len() {
                let eating_info = self.eating_info(
                    line,
                    i,
                    end_index_of_line,
                    first_non_whitespace_index,
                    count_of_precede_spaces,
                );
                let last_state_is_lazy_continuation =
                    self.state_stack.last().is_some_and(|state| {
                        token_ref(&self.root, &state.path).node_type == PARAGRAPH_TYPE
                    });

                if last_state_is_lazy_continuation && eating_info.count_of_precede_spaces >= 4 {
                    skip_step2 = true;
                }
            } else {
                self.current_stack_index = self.state_stack.len().saturating_sub(1);
            }

            if !skip_step2 {
                while i < end_index_of_line
                    && self.state_stack[self.current_stack_index].is_containing_block
                {
                    let mut has_new_opener = false;
                    let eating_info = self.eating_info(
                        line,
                        i,
                        end_index_of_line,
                        first_non_whitespace_index,
                        count_of_precede_spaces,
                    );

                    for hook_idx in 0..self.normal_hook_len {
                        if self.consume_new_opener(
                            hook_idx,
                            &eating_info,
                            &mut i,
                            &mut first_non_whitespace_index,
                            &mut count_of_precede_spaces,
                            &move_forward,
                        ) {
                            has_new_opener = true;
                            break;
                        }
                    }
                    if !has_new_opener {
                        break;
                    }
                }
            }
        }

        // Step 3
        let mut has_lazy_continuation = false;
        if i < end_index_of_line && self.current_stack_index + 1 < self.state_stack.len() {
            let last_index = self.state_stack.len() - 1;
            let last_state = self.state_stack[last_index].clone();
            if let Some(last_hook_idx) = last_state.hook_index {
                let parent_path = self.state_stack[last_index - 1].path.clone();
                let eating_info = self.eating_info(
                    line,
                    i,
                    end_index_of_line,
                    first_non_whitespace_index,
                    count_of_precede_spaces,
                );
                let hook = self.hooks[last_hook_idx].hook.clone();
                let result = hook.borrow_mut().eat_lazy_continuation_text_without_parent(
                    &eating_info,
                    token_mut(&mut self.root, &last_state.path),
                );
                let result = result.unwrap_or_else(|| {
                    let (child_index, mut token) = detach_child_for_parent_snapshot(
                        &mut self.root,
                        &parent_path,
                        &last_state.path,
                    );
                    let parent_token = token_ref(&self.root, &parent_path);
                    let result = hook.borrow_mut().eat_lazy_continuation_text(
                        &eating_info,
                        &mut token,
                        parent_token,
                    );
                    restore_detached_child(&mut self.root, &parent_path, child_index, token);
                    result
                });

                if let EatLazyContinuationTextResult::Opening { next_index } = result {
                    self.current_stack_index = last_index;
                    move_forward(
                        self,
                        &mut i,
                        &mut first_non_whitespace_index,
                        &mut count_of_precede_spaces,
                        next_index,
                        true,
                    );
                    self.current_stack_index = self.state_stack.len() - 1;
                    has_lazy_continuation = true;
                }
            }
        }

        if !has_lazy_continuation {
            self.cut_stale_branch(self.current_stack_index + 1);
        }

        if let Some(fallback_hook_idx) = self.fallback_hook_index {
            if i < end_index_of_line {
                let eating_info = self.eating_info(
                    line,
                    i,
                    end_index_of_line,
                    first_non_whitespace_index,
                    count_of_precede_spaces,
                );
                let _ = self.consume_new_opener(
                    fallback_hook_idx,
                    &eating_info,
                    &mut i,
                    &mut first_non_whitespace_index,
                    &mut count_of_precede_spaces,
                    &move_forward,
                );
            }
        }

        debug_assert!(first_non_whitespace_index >= end_index_of_line);
    }

    pub fn done(mut self) -> BlockToken {
        while self.state_stack.len() > 1 {
            self.popup();
        }
        self.root
    }

    fn into_snapshot(self) -> (BlockToken, Vec<MatchBlockState>, Vec<usize>) {
        (self.root, self.state_stack, self.hook_index_mapping)
    }

    fn eating_info(
        &self,
        line: &PhrasingContentLine,
        start_index: usize,
        end_index: usize,
        first_non_whitespace_index: usize,
        count_of_precede_spaces: usize,
    ) -> PhrasingContentLine {
        PhrasingContentLine {
            node_points: line.node_points.clone(),
            start_index,
            end_index,
            first_non_whitespace_index,
            indent_width: yozora_core_tokenizer::calc_indent_width(
                line.node_points.as_ref(),
                start_index,
                first_non_whitespace_index,
            ),
            count_of_precede_spaces,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn consume_new_opener<F>(
        &mut self,
        hook_idx: usize,
        line: &PhrasingContentLine,
        i: &mut usize,
        first_non_whitespace_index: &mut usize,
        count_of_precede_spaces: &mut usize,
        move_forward: &F,
    ) -> bool
    where
        F: Fn(&mut Self, &mut usize, &mut usize, &mut usize, usize, bool),
    {
        let parent_path = &self.state_stack[self.current_stack_index].path;
        let result = {
            let parent_token = token_ref(&self.root, parent_path);
            self.hooks[hook_idx]
                .hook
                .borrow_mut()
                .eat_opener(line, parent_token)
        };
        let Some(result) = result else {
            return false;
        };

        debug_assert!(result.next_index > *i);

        move_forward(
            self,
            i,
            first_non_whitespace_index,
            count_of_precede_spaces,
            result.next_index,
            false,
        );

        let mut next_token = result.token;
        next_token.tokenizer = self.hooks[hook_idx].name.clone();
        self.push(hook_idx, next_token, result.saturated);
        true
    }

    fn interrupt_sibling<F>(
        &mut self,
        hook_idx: usize,
        line: &PhrasingContentLine,
        i: &mut usize,
        first_non_whitespace_index: &mut usize,
        count_of_precede_spaces: &mut usize,
        move_forward: &F,
    ) -> bool
    where
        F: Fn(&mut Self, &mut usize, &mut usize, &mut usize, usize, bool),
    {
        if self.current_stack_index == 0 {
            return false;
        }

        let sibling_state = self.state_stack[self.current_stack_index].clone();
        if self.hooks[hook_idx].priority <= sibling_state.priority {
            return false;
        }

        let parent_state = self.state_stack[self.current_stack_index - 1].clone();
        let result = {
            let sibling_token = token_ref(&self.root, &sibling_state.path);
            let parent_token = token_ref(&self.root, &parent_state.path);
            self.hooks[hook_idx]
                .hook
                .borrow_mut()
                .eat_and_interrupt_previous_sibling(line, sibling_token, parent_token)
        };
        let Some(result) = result else {
            return false;
        };

        self.cut_stale_branch(self.current_stack_index);

        let parent_token = token_mut(&mut self.root, &parent_state.path);
        let _ = parent_token.children.pop();

        match result.remaining_sibling {
            RemainingSibling::None => {}
            RemainingSibling::One(token) => parent_token.children.push(token),
            RemainingSibling::Many(tokens) => parent_token.children.extend(tokens),
        }

        move_forward(
            self,
            i,
            first_non_whitespace_index,
            count_of_precede_spaces,
            result.next_index,
            false,
        );

        let mut token = result.token;
        token.tokenizer = self.hooks[hook_idx].name.clone();
        self.push(hook_idx, token, result.saturated);
        true
    }

    fn create_rollback_processor(
        &self,
        excluded_hook_index: usize,
        lines: &[PhrasingContentLine],
    ) -> Option<BlockContentProcessor<'a>> {
        if lines.is_empty() {
            return None;
        }

        let mut candidate_hooks = Vec::with_capacity(self.normal_hook_len.saturating_sub(1));
        let mut candidate_mapping = Vec::with_capacity(self.normal_hook_len.saturating_sub(1));
        for hook_index in 0..self.normal_hook_len {
            if hook_index == excluded_hook_index {
                continue;
            }
            candidate_hooks.push(self.hooks[hook_index].clone());
            candidate_mapping.push(self.hook_index_mapping[hook_index]);
        }

        let (fallback_hook, fallback_hook_mapping) =
            self.fallback_hook_index.map_or((None, None), |hook_index| {
                if hook_index == excluded_hook_index {
                    (None, None)
                } else {
                    (
                        Some(self.hooks[hook_index].clone()),
                        Some(self.hook_index_mapping[hook_index]),
                    )
                }
            });

        let mut processor = create_block_content_processor_with_mapping(
            candidate_hooks,
            fallback_hook,
            candidate_mapping,
            fallback_hook_mapping,
        );
        for line in lines {
            processor.consume(line);
        }
        Some(processor)
    }

    fn rollback(
        &mut self,
        excluded_hook_index: usize,
        lines: &[PhrasingContentLine],
        parent_stack_index: usize,
    ) -> bool {
        let Some(processor) = self.create_rollback_processor(excluded_hook_index, lines) else {
            return false;
        };

        let (mut internal_root, internal_state_stack, hook_index_mapping) =
            processor.into_snapshot();

        let parent_path = self.state_stack[parent_stack_index].path.clone();
        let internal_root_offset = {
            let parent_token = token_mut(&mut self.root, &parent_path);
            let start_offset = parent_token.children.len();
            parent_token.children.append(&mut internal_root.children);
            start_offset
        };

        if let Some(position) = &internal_root.position {
            self.refresh_position(position.end);
        }

        for internal_state in internal_state_stack.into_iter().skip(1) {
            let Some((first, remaining_path)) = internal_state.path.split_first() else {
                continue;
            };

            let mut path = parent_path.clone();
            path.push(internal_root_offset + *first);
            path.extend_from_slice(remaining_path);

            self.state_stack.push(MatchBlockState {
                hook_index: internal_state
                    .hook_index
                    .map(|hook_index| hook_index_mapping[hook_index]),
                path,
                is_containing_block: internal_state.is_containing_block,
                priority: internal_state.priority,
            });
        }

        self.current_stack_index = self.state_stack.len().saturating_sub(1);
        true
    }

    fn refresh_position(&mut self, end_point: Point) {
        let mut token = &mut self.root;
        if let Some(position) = token.position.as_mut() {
            position.end = end_point;
        }

        for state in self
            .state_stack
            .iter()
            .take(self.current_stack_index + 1)
            .skip(1)
        {
            let child_index = *state
                .path
                .last()
                .expect("non-root block state should contain a child path");
            token = &mut token.children[child_index];
            if let Some(position) = token.position.as_mut() {
                position.end = end_point;
            }
        }
    }

    fn cut_stale_branch(&mut self, next_top_index: usize) {
        while self.state_stack.len() > next_top_index {
            self.popup();
        }
    }

    fn popup(&mut self) -> Option<MatchBlockState> {
        let top_state = self.state_stack.pop()?;

        if let Some(hook_index) = top_state.hook_index {
            if !self.state_stack.is_empty() {
                let parent_stack_index = self.state_stack.len() - 1;
                let top_path = top_state.path.clone();
                let hook = self.hooks[hook_index].hook.clone();
                let on_close_result = {
                    let mut hook = hook.borrow_mut();
                    let token = token_mut(&mut self.root, &top_path);
                    hook.on_close(token)
                };
                if let Some(result) = on_close_result {
                    match result {
                        OnCloseResult::ClosingAndRollback { lines } => {
                            if let Some(processor) =
                                self.create_rollback_processor(hook_index, &lines)
                            {
                                let mut internal_root = processor.done();
                                let parent_path = self.state_stack[parent_stack_index].path.clone();
                                let parent_token = token_mut(&mut self.root, &parent_path);
                                parent_token.children.append(&mut internal_root.children);
                            }
                        }
                        OnCloseResult::FailedAndRollback { lines } => {
                            let parent_path = self.state_stack[parent_stack_index].path.clone();
                            let parent_token = token_mut(&mut self.root, &parent_path);
                            let _ = parent_token.children.pop();

                            if let Some(processor) =
                                self.create_rollback_processor(hook_index, &lines)
                            {
                                let mut internal_root = processor.done();
                                let parent_token = token_mut(&mut self.root, &parent_path);
                                parent_token.children.append(&mut internal_root.children);
                            }
                        }
                    }
                }
            }
        }

        if self.current_stack_index >= self.state_stack.len() {
            self.current_stack_index = self.state_stack.len().saturating_sub(1);
        }
        Some(top_state)
    }

    fn push(&mut self, hook_index: usize, next_token: BlockToken, saturated: bool) {
        self.cut_stale_branch(self.current_stack_index + 1);

        let parent_path = self.state_stack[self.current_stack_index].path.clone();
        let next_end = next_token.position.as_ref().map(|position| position.end);

        let child_index = {
            let parent_token = token_mut(&mut self.root, &parent_path);
            parent_token.children.push(next_token);
            parent_token.children.len() - 1
        };

        if let Some(end_point) = next_end {
            self.refresh_position(end_point);
        }

        let mut next_path = parent_path;
        next_path.push(child_index);

        self.current_stack_index += 1;
        let is_containing_block = self.hooks[hook_index].hook.borrow().is_containing_block();
        let priority = self.hooks[hook_index].priority;
        self.state_stack.push(MatchBlockState {
            hook_index: Some(hook_index),
            path: next_path,
            is_containing_block,
            priority,
        });

        if saturated {
            let _ = self.popup();
        }
    }
}

fn token_ref<'a>(root: &'a BlockToken, path: &[usize]) -> &'a BlockToken {
    let mut cursor = root;
    for &index in path {
        cursor = &cursor.children[index];
    }
    cursor
}

fn token_mut<'a>(root: &'a mut BlockToken, path: &[usize]) -> &'a mut BlockToken {
    let mut cursor = root;
    for &index in path {
        cursor = &mut cursor.children[index];
    }
    cursor
}

fn detach_child_for_parent_snapshot(
    root: &mut BlockToken,
    parent_path: &[usize],
    child_path: &[usize],
) -> (usize, BlockToken) {
    assert_eq!(
        child_path.len(),
        parent_path.len() + 1,
        "child path should be directly nested in parent path"
    );
    assert_eq!(
        &child_path[..parent_path.len()],
        parent_path,
        "child path should start with parent path"
    );

    let child_index = child_path[parent_path.len()];
    let parent = token_mut(root, parent_path);
    let snapshot = parent.children[child_index].clone();
    let token = std::mem::replace(&mut parent.children[child_index], snapshot);
    (child_index, token)
}

fn restore_detached_child(
    root: &mut BlockToken,
    parent_path: &[usize],
    child_index: usize,
    token: BlockToken,
) {
    token_mut(root, parent_path).children[child_index] = token;
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use yozora_ast::Position;
    use yozora_character::{create_node_point_generator, AsciiCodePoint};
    use yozora_core_tokenizer::{
        BlockToken, EatContinuationTextResult, EatOpenerResult, MatchBlockHook, OnCloseResult,
        PhrasingContentLine,
    };

    use super::{create_block_content_processor, MatchBlockProcessorHook};

    fn make_token(
        tokenizer: &str,
        node_type: &'static str,
        line: &PhrasingContentLine,
    ) -> BlockToken {
        let point = line.node_points[line.first_non_whitespace_index];
        BlockToken::new(
            tokenizer,
            node_type,
            Some(Position {
                start: yozora_ast::Point {
                    line: point.line,
                    column: point.column,
                    offset: Some(point.offset),
                },
                end: yozora_ast::Point {
                    line: point.line,
                    column: point.column + 1,
                    offset: Some(point.offset + 1),
                },
                indent: None,
            }),
        )
    }

    fn starts_with(line: &PhrasingContentLine, code_point: i32) -> bool {
        if line.first_non_whitespace_index >= line.end_index {
            return false;
        }
        line.node_points[line.first_non_whitespace_index].code_point == code_point
    }

    struct EchoHook;

    struct SnapshotHook {
        parent_free: bool,
        lazy: bool,
    }

    impl SnapshotHook {
        fn append_with_parent(&self, token: &mut BlockToken, parent: &BlockToken) {
            assert!(!self.parent_free, "parent-free continuation must be used");
            let before = *token.data_as::<usize>().unwrap();
            let snapshot = parent.children.last().unwrap();
            assert_eq!(snapshot.data_as::<usize>(), Some(&before));
            token.data = Arc::new(before + 1);
            assert_eq!(snapshot.data_as::<usize>(), Some(&before));
        }

        fn append_without_parent(&self, token: &mut BlockToken) {
            let data = Arc::get_mut(&mut token.data)
                .expect("parent-free continuation must not force a token snapshot")
                .downcast_mut::<usize>()
                .unwrap();
            *data += 1;
        }
    }

    impl MatchBlockHook for SnapshotHook {
        fn is_containing_block(&self) -> bool {
            false
        }

        fn eat_opener(
            &mut self,
            line: &PhrasingContentLine,
            _parent: &BlockToken,
        ) -> Option<EatOpenerResult> {
            Some(EatOpenerResult {
                token: make_token("snapshot", "paragraph", line).with_data(1usize),
                next_index: line.end_index,
                saturated: false,
            })
        }

        fn eat_continuation_text_without_parent(
            &mut self,
            line: &PhrasingContentLine,
            token: &mut BlockToken,
        ) -> Option<EatContinuationTextResult> {
            if !self.parent_free {
                return None;
            }
            if self.lazy {
                return Some(EatContinuationTextResult::NotMatched);
            }
            self.append_without_parent(token);
            Some(EatContinuationTextResult::Opening {
                next_index: line.end_index,
            })
        }

        fn eat_lazy_continuation_text_without_parent(
            &mut self,
            line: &PhrasingContentLine,
            token: &mut BlockToken,
        ) -> Option<yozora_core_tokenizer::EatLazyContinuationTextResult> {
            if !self.parent_free {
                return None;
            }
            self.append_without_parent(token);
            Some(
                yozora_core_tokenizer::EatLazyContinuationTextResult::Opening {
                    next_index: line.end_index,
                },
            )
        }

        fn eat_continuation_text(
            &mut self,
            line: &PhrasingContentLine,
            token: &mut BlockToken,
            parent: &BlockToken,
        ) -> EatContinuationTextResult {
            if self.lazy {
                return EatContinuationTextResult::NotMatched;
            }
            self.append_with_parent(token, parent);
            EatContinuationTextResult::Opening {
                next_index: line.end_index,
            }
        }

        fn eat_lazy_continuation_text(
            &mut self,
            line: &PhrasingContentLine,
            token: &mut BlockToken,
            parent: &BlockToken,
        ) -> yozora_core_tokenizer::EatLazyContinuationTextResult {
            self.append_with_parent(token, parent);
            yozora_core_tokenizer::EatLazyContinuationTextResult::Opening {
                next_index: line.end_index,
            }
        }
    }

    #[test]
    fn continuations_preserve_parent_snapshots_and_allow_parent_free_mutation() {
        for parent_free in [false, true] {
            for lazy in [false, true] {
                let hook = MatchBlockProcessorHook::new(
                    "snapshot",
                    -1,
                    Box::new(SnapshotHook { parent_free, lazy }),
                );
                let mut processor = create_block_content_processor(Vec::new(), Some(hook));
                for _ in 0..64 {
                    processor.consume(&make_line("text"));
                }
                let root = processor.done();
                assert_eq!(root.children.len(), 1);
                assert_eq!(root.children[0].data_as::<usize>(), Some(&64));
            }
        }
    }

    impl MatchBlockHook for EchoHook {
        fn is_containing_block(&self) -> bool {
            true
        }

        fn eat_opener(
            &mut self,
            line: &PhrasingContentLine,
            _parent_token: &BlockToken,
        ) -> Option<EatOpenerResult> {
            if !starts_with(line, AsciiCodePoint::LOWERCASE_X as i32) {
                return None;
            }

            Some(EatOpenerResult {
                token: make_token("echo", "paragraph", line),
                next_index: line.end_index,
                saturated: true,
            })
        }
    }

    struct FailedRollbackHook;

    impl MatchBlockHook for FailedRollbackHook {
        fn is_containing_block(&self) -> bool {
            true
        }

        fn eat_opener(
            &mut self,
            line: &PhrasingContentLine,
            _parent_token: &BlockToken,
        ) -> Option<EatOpenerResult> {
            if !starts_with(line, AsciiCodePoint::LOWERCASE_A as i32) {
                return None;
            }

            Some(EatOpenerResult {
                token: make_token("a", "container", line),
                next_index: line.end_index,
                saturated: false,
            })
        }

        fn eat_continuation_text(
            &mut self,
            line: &PhrasingContentLine,
            _token: &mut BlockToken,
            _parent_token: &BlockToken,
        ) -> EatContinuationTextResult {
            EatContinuationTextResult::FailedAndRollback {
                lines: vec![line.clone()],
            }
        }
    }

    struct RollbackTargetHook;

    impl MatchBlockHook for RollbackTargetHook {
        fn is_containing_block(&self) -> bool {
            true
        }

        fn eat_opener(
            &mut self,
            line: &PhrasingContentLine,
            _parent_token: &BlockToken,
        ) -> Option<EatOpenerResult> {
            if !starts_with(line, AsciiCodePoint::LOWERCASE_B as i32) {
                return None;
            }

            Some(EatOpenerResult {
                token: make_token("b", "container", line),
                next_index: line.end_index,
                saturated: false,
            })
        }

        fn eat_continuation_text(
            &mut self,
            line: &PhrasingContentLine,
            _token: &mut BlockToken,
            _parent_token: &BlockToken,
        ) -> EatContinuationTextResult {
            EatContinuationTextResult::Opening {
                next_index: line.end_index,
            }
        }
    }

    struct FailedOnCloseHook;

    impl MatchBlockHook for FailedOnCloseHook {
        fn is_containing_block(&self) -> bool {
            true
        }

        fn eat_opener(
            &mut self,
            line: &PhrasingContentLine,
            _parent_token: &BlockToken,
        ) -> Option<EatOpenerResult> {
            if !starts_with(line, AsciiCodePoint::LOWERCASE_C as i32) {
                return None;
            }

            Some(EatOpenerResult {
                token: make_token("c", "container", line),
                next_index: line.end_index,
                saturated: true,
            })
        }

        fn on_close(&mut self, _token: &mut BlockToken) -> Option<OnCloseResult> {
            Some(OnCloseResult::FailedAndRollback {
                lines: vec![make_line("d")],
            })
        }
    }

    struct ClosingOnCloseHook;

    impl MatchBlockHook for ClosingOnCloseHook {
        fn is_containing_block(&self) -> bool {
            true
        }

        fn eat_opener(
            &mut self,
            line: &PhrasingContentLine,
            _parent_token: &BlockToken,
        ) -> Option<EatOpenerResult> {
            if !starts_with(line, AsciiCodePoint::LOWERCASE_C as i32) {
                return None;
            }

            Some(EatOpenerResult {
                token: make_token("c", "container", line),
                next_index: line.end_index,
                saturated: true,
            })
        }

        fn on_close(&mut self, _token: &mut BlockToken) -> Option<OnCloseResult> {
            Some(OnCloseResult::ClosingAndRollback {
                lines: vec![make_line("d")],
            })
        }
    }

    struct RollbackFromCloseHook;

    impl MatchBlockHook for RollbackFromCloseHook {
        fn is_containing_block(&self) -> bool {
            true
        }

        fn eat_opener(
            &mut self,
            line: &PhrasingContentLine,
            _parent_token: &BlockToken,
        ) -> Option<EatOpenerResult> {
            if !starts_with(line, AsciiCodePoint::LOWERCASE_D as i32) {
                return None;
            }

            Some(EatOpenerResult {
                token: make_token("d", "container", line),
                next_index: line.end_index,
                saturated: true,
            })
        }
    }

    struct InterruptibleHook;

    impl MatchBlockHook for InterruptibleHook {
        fn is_containing_block(&self) -> bool {
            true
        }

        fn eat_opener(
            &mut self,
            line: &PhrasingContentLine,
            _parent_token: &BlockToken,
        ) -> Option<EatOpenerResult> {
            if !starts_with(line, AsciiCodePoint::LOWERCASE_L as i32) {
                return None;
            }

            Some(EatOpenerResult {
                token: make_token("low", "container", line),
                next_index: line.end_index,
                saturated: false,
            })
        }
    }

    struct InterruptorHook;

    impl MatchBlockHook for InterruptorHook {
        fn is_containing_block(&self) -> bool {
            true
        }

        fn eat_opener(
            &mut self,
            _line: &PhrasingContentLine,
            _parent_token: &BlockToken,
        ) -> Option<EatOpenerResult> {
            None
        }

        fn eat_and_interrupt_previous_sibling(
            &mut self,
            line: &PhrasingContentLine,
            _prev_sibling_token: &BlockToken,
            _parent_token: &BlockToken,
        ) -> Option<yozora_core_tokenizer::EatAndInterruptPreviousSiblingResult> {
            if !starts_with(line, AsciiCodePoint::LOWERCASE_H as i32) {
                return None;
            }

            Some(
                yozora_core_tokenizer::EatAndInterruptPreviousSiblingResult {
                    token: make_token("high", "container", line),
                    next_index: line.end_index,
                    saturated: true,
                    remaining_sibling: yozora_core_tokenizer::RemainingSibling::None,
                },
            )
        }
    }

    struct LazyContinuationHook;

    impl MatchBlockHook for LazyContinuationHook {
        fn is_containing_block(&self) -> bool {
            false
        }

        fn eat_opener(
            &mut self,
            line: &PhrasingContentLine,
            _parent_token: &BlockToken,
        ) -> Option<EatOpenerResult> {
            if !starts_with(line, AsciiCodePoint::LOWERCASE_P as i32) {
                return None;
            }

            Some(EatOpenerResult {
                token: make_token("lazy", "paragraph", line),
                next_index: line.end_index,
                saturated: false,
            })
        }

        fn eat_lazy_continuation_text(
            &mut self,
            line: &PhrasingContentLine,
            _token: &mut BlockToken,
            _parent_token: &BlockToken,
        ) -> yozora_core_tokenizer::EatLazyContinuationTextResult {
            if !starts_with(line, AsciiCodePoint::LOWERCASE_X as i32) {
                return yozora_core_tokenizer::EatLazyContinuationTextResult::NotMatched;
            }

            yozora_core_tokenizer::EatLazyContinuationTextResult::Opening {
                next_index: line.end_index,
            }
        }
    }

    struct FallbackHook;

    impl MatchBlockHook for FallbackHook {
        fn is_containing_block(&self) -> bool {
            true
        }

        fn eat_opener(
            &mut self,
            line: &PhrasingContentLine,
            _parent_token: &BlockToken,
        ) -> Option<EatOpenerResult> {
            if !starts_with(line, AsciiCodePoint::LOWERCASE_Z as i32) {
                return None;
            }

            Some(EatOpenerResult {
                token: make_token("fallback", "paragraph", line),
                next_index: line.end_index,
                saturated: true,
            })
        }
    }

    fn make_line(input: &str) -> PhrasingContentLine {
        let chunks = create_node_point_generator(input);
        let points = Arc::new(chunks.first().cloned().unwrap_or_default());
        let end_index = points.len();

        let mut first_non_whitespace_index = 0usize;
        let mut count_of_precede_spaces = 0usize;
        while first_non_whitespace_index < end_index {
            let code_point = points[first_non_whitespace_index].code_point;
            if yozora_character::is_space_character(code_point) {
                count_of_precede_spaces += 1;
                first_non_whitespace_index += 1;
                continue;
            }
            if !yozora_character::is_whitespace_character(code_point) {
                break;
            }
            first_non_whitespace_index += 1;
        }

        PhrasingContentLine {
            node_points: Arc::clone(&points),
            start_index: 0,
            end_index,
            first_non_whitespace_index,
            indent_width: yozora_core_tokenizer::calc_indent_width(
                points.as_ref(),
                0,
                first_non_whitespace_index,
            ),
            count_of_precede_spaces,
        }
    }

    #[test]
    fn should_match_one_line_into_one_child() {
        let hooks = vec![MatchBlockProcessorHook::new("echo", 1, Box::new(EchoHook))];
        let mut processor = create_block_content_processor(hooks, None);
        let line = make_line("x");

        processor.consume(&line);
        let root = processor.done();

        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].tokenizer.as_ref(), "echo");
    }

    #[test]
    fn should_restore_open_state_after_failed_and_rollback() {
        let hooks = vec![
            MatchBlockProcessorHook::new("a", 10, Box::new(FailedRollbackHook)),
            MatchBlockProcessorHook::new("b", 8, Box::new(RollbackTargetHook)),
        ];
        let mut processor = create_block_content_processor(hooks, None);

        processor.consume(&make_line("a"));
        assert_eq!(processor.state_stack.len(), 2);
        assert_eq!(processor.state_stack[1].hook_index, Some(0));

        processor.consume(&make_line("b"));

        assert_eq!(processor.root.children.len(), 1);
        assert_eq!(processor.root.children[0].tokenizer.as_ref(), "b");
        assert!(processor.state_stack.len() >= 2);
        assert!(processor
            .state_stack
            .iter()
            .skip(1)
            .all(|state| state.hook_index == Some(1)));
    }

    #[test]
    fn should_replace_previous_sibling_when_on_close_failed_and_rollback() {
        let hooks = vec![
            MatchBlockProcessorHook::new("c", 10, Box::new(FailedOnCloseHook)),
            MatchBlockProcessorHook::new("d", 8, Box::new(RollbackFromCloseHook)),
        ];
        let mut processor = create_block_content_processor(hooks, None);

        processor.consume(&make_line("c"));
        let root = processor.done();

        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].tokenizer.as_ref(), "d");
    }

    #[test]
    fn should_append_rollback_tokens_when_on_close_closing_and_rollback() {
        let hooks = vec![
            MatchBlockProcessorHook::new("c", 10, Box::new(ClosingOnCloseHook)),
            MatchBlockProcessorHook::new("d", 8, Box::new(RollbackFromCloseHook)),
        ];
        let mut processor = create_block_content_processor(hooks, None);

        processor.consume(&make_line("c"));
        let root = processor.done();

        assert_eq!(root.children.len(), 2);
        assert_eq!(root.children[0].tokenizer.as_ref(), "c");
        assert_eq!(root.children[1].tokenizer.as_ref(), "d");
    }

    #[test]
    fn should_interrupt_previous_sibling_when_priority_is_higher() {
        let hooks = vec![
            MatchBlockProcessorHook::new("low", 1, Box::new(InterruptibleHook)),
            MatchBlockProcessorHook::new("high", 10, Box::new(InterruptorHook)),
        ];
        let mut processor = create_block_content_processor(hooks, None);

        processor.consume(&make_line("l"));
        processor.consume(&make_line("h"));
        let root = processor.done();

        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].tokenizer.as_ref(), "high");
    }

    #[test]
    fn should_keep_block_open_when_lazy_continuation_matched() {
        let hooks = vec![MatchBlockProcessorHook::new(
            "lazy",
            1,
            Box::new(LazyContinuationHook),
        )];
        let mut processor = create_block_content_processor(hooks, None);

        processor.consume(&make_line("p"));
        processor.consume(&make_line("x"));

        assert_eq!(processor.root.children.len(), 1);
        assert_eq!(processor.root.children[0].tokenizer.as_ref(), "lazy");
        assert_eq!(processor.state_stack.len(), 2);
        assert_eq!(processor.state_stack[1].hook_index, Some(0));
    }

    #[test]
    fn should_consume_fallback_opener_when_normal_hooks_not_matched() {
        let hooks = vec![MatchBlockProcessorHook::new("echo", 1, Box::new(EchoHook))];
        let fallback_hook = Some(MatchBlockProcessorHook::new(
            "fallback",
            -1,
            Box::new(FallbackHook),
        ));
        let mut processor = create_block_content_processor(hooks, fallback_hook);

        processor.consume(&make_line("z"));
        let root = processor.done();

        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].tokenizer.as_ref(), "fallback");
    }
}
