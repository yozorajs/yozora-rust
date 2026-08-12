use yozora_core_tokenizer::{
    DelimiterType, InlineToken, IsDelimiterPairResult, ProcessDelimiterPairResult, TokenDelimiter,
};

use super::types::MatchInlineProcessorHook;

#[derive(Debug, Clone)]
struct DelimiterItem {
    hook_index: usize,
    delimiter: TokenDelimiter,
    inactive: bool,
    token_stack_index: usize,
}

#[derive(Default)]
pub(super) struct SinglePriorityDelimiterProcessor {
    ht_index: usize,
    higher_priority_tokens: Vec<InlineToken>,
    delimiter_stack: Vec<DelimiterItem>,
    hook_delimiter_stacks: Vec<Vec<usize>>,
    token_stack: Vec<InlineToken>,
}

impl SinglePriorityDelimiterProcessor {
    pub(super) fn reset(&mut self, higher_priority_tokens: &[InlineToken]) {
        self.higher_priority_tokens.clear();
        self.higher_priority_tokens
            .extend(higher_priority_tokens.iter().cloned());

        self.ht_index = 0;
        self.clear_delimiter_stacks();
        self.token_stack.clear();
    }

    fn clear_delimiter_stacks(&mut self) {
        self.delimiter_stack.clear();
        self.hook_delimiter_stacks.clear();
    }

    fn cut_stale_branch(&mut self, start_stack_index: usize) {
        let mut target_len = start_stack_index;
        while target_len > 0 && self.delimiter_stack[target_len - 1].inactive {
            target_len -= 1;
        }

        while self.delimiter_stack.len() > target_len {
            let stack_index = self.delimiter_stack.len() - 1;
            let item = self
                .delimiter_stack
                .pop()
                .expect("delimiter stack should not be empty");
            let hook_delimiter_stack = self
                .hook_delimiter_stacks
                .get_mut(item.hook_index)
                .expect("hook delimiter stack should exist");
            assert_eq!(
                hook_delimiter_stack.last().copied(),
                Some(stack_index),
                "[DelimiterProcessor] hook delimiter stack is out of sync."
            );
            hook_delimiter_stack.pop();
        }
    }

    fn push(&mut self, hook_index: usize, delimiter: TokenDelimiter) {
        let stack_index = self.delimiter_stack.len();
        self.delimiter_stack.push(DelimiterItem {
            hook_index,
            delimiter,
            inactive: false,
            token_stack_index: self.token_stack.len(),
        });
        if self.hook_delimiter_stacks.len() <= hook_index {
            self.hook_delimiter_stacks
                .resize_with(hook_index + 1, Vec::new);
        }
        self.hook_delimiter_stacks[hook_index].push(stack_index);
    }

    fn append_higher_priority_tokens(
        target: &mut Vec<InlineToken>,
        higher_priority_tokens: &[InlineToken],
        mut start_index: usize,
        delimiter: &TokenDelimiter,
    ) -> usize {
        while start_index < higher_priority_tokens.len() {
            let token = &higher_priority_tokens[start_index];
            if token.start_index >= delimiter.end_index {
                break;
            }

            if token.start_index < delimiter.start_index {
                target.push(token.clone());
            }
            start_index += 1;
        }
        start_index
    }

    pub(super) fn find_nearest_paired_delimiter(
        &self,
        hook_index: usize,
        closer_delimiter: &TokenDelimiter,
        hooks: &[MatchInlineProcessorHook<'_>],
    ) -> Option<TokenDelimiter> {
        let hook_delimiter_stack = self.hook_delimiter_stacks.get(hook_index)?;

        for &stack_index in hook_delimiter_stack.iter().rev() {
            let item = &self.delimiter_stack[stack_index];
            if item.inactive {
                continue;
            }

            let opener_delimiter = &item.delimiter;
            let mut internal_tokens = self.token_stack[item.token_stack_index..].to_vec();
            Self::append_higher_priority_tokens(
                &mut internal_tokens,
                &self.higher_priority_tokens,
                self.ht_index,
                closer_delimiter,
            );
            let result = hooks[hook_index].is_delimiter_pair(
                opener_delimiter,
                closer_delimiter,
                &internal_tokens,
            );
            match result {
                IsDelimiterPairResult::Paired => return Some(opener_delimiter.clone()),
                IsDelimiterPairResult::NotPaired { closer, .. } => {
                    if !closer {
                        return None;
                    }
                }
            }
        }

        None
    }

    fn consume(
        &mut self,
        hook_index: usize,
        closer_delimiter: TokenDelimiter,
        hooks: &mut [MatchInlineProcessorHook<'_>],
    ) -> Option<TokenDelimiter> {
        if self.delimiter_stack.is_empty() {
            return Some(closer_delimiter);
        }

        let hook_name = hooks[hook_index].name.clone();

        let mut remain_closer_delimiter = Some(closer_delimiter);
        let mut internal_tokens: Vec<InlineToken> = Vec::new();

        let mut hook_stack_index = self
            .hook_delimiter_stacks
            .get(hook_index)
            .map_or(0, Vec::len);
        while hook_stack_index > 0 {
            hook_stack_index -= 1;
            let delimiter_stack_index = self.hook_delimiter_stacks[hook_index][hook_stack_index];
            if self.delimiter_stack[delimiter_stack_index].inactive {
                continue;
            }

            let opener_token_stack_index =
                self.delimiter_stack[delimiter_stack_index].token_stack_index;
            if opener_token_stack_index < self.token_stack.len() {
                let mut suffix = self.token_stack.split_off(opener_token_stack_index);
                suffix.append(&mut internal_tokens);
                internal_tokens = suffix;
            }

            let mut remain_opener_delimiter = Some(
                self.delimiter_stack[delimiter_stack_index]
                    .delimiter
                    .clone(),
            );

            while let (Some(opener_delimiter), Some(closer_delimiter)) = (
                remain_opener_delimiter.clone(),
                remain_closer_delimiter.clone(),
            ) {
                if closer_delimiter.delimiter_type == DelimiterType::Opener {
                    self.push(hook_index, closer_delimiter);
                    remain_closer_delimiter = None;
                    break;
                }

                if closer_delimiter.delimiter_type == DelimiterType::Full {
                    break;
                }

                let pre_pair_result = hooks[hook_index].is_delimiter_pair(
                    &opener_delimiter,
                    &closer_delimiter,
                    &internal_tokens,
                );

                if let IsDelimiterPairResult::NotPaired { opener, closer } = pre_pair_result {
                    if !opener {
                        let mut tokens = hooks[hook_index]
                            .hook
                            .process_single_delimiter(&opener_delimiter);
                        for token in &mut tokens {
                            token.tokenizer = hook_name.clone();
                        }
                        if !tokens.is_empty() {
                            tokens.append(&mut internal_tokens);
                            internal_tokens = tokens;
                        }

                        self.delimiter_stack[delimiter_stack_index].inactive = true;
                    }

                    if !closer {
                        let mut tokens = hooks[hook_index]
                            .hook
                            .process_single_delimiter(&closer_delimiter);
                        for token in &mut tokens {
                            token.tokenizer = hook_name.clone();
                        }
                        if !tokens.is_empty() {
                            internal_tokens.append(&mut tokens);
                        }
                        remain_closer_delimiter = None;
                    }
                    break;
                }

                let ProcessDelimiterPairResult {
                    mut tokens,
                    remain_opener_delimiter: next_opener,
                    remain_closer_delimiter: next_closer,
                } = hooks[hook_index].process_delimiter_pair(
                    &opener_delimiter,
                    &closer_delimiter,
                    &internal_tokens,
                );

                for token in &mut tokens {
                    if token.tokenizer.is_empty() {
                        token.tokenizer = hook_name.clone();
                    }
                }

                internal_tokens = tokens;
                remain_opener_delimiter = next_opener;
                remain_closer_delimiter = next_closer;

                self.cut_stale_branch(delimiter_stack_index);
                hook_stack_index = hook_stack_index.min(
                    self.hook_delimiter_stacks
                        .get(hook_index)
                        .map_or(0, Vec::len),
                );
                if let Some(opener_delimiter) = remain_opener_delimiter.clone() {
                    self.push(hook_index, opener_delimiter);
                }
            }

            let should_break = remain_closer_delimiter.is_none()
                || remain_closer_delimiter
                    .as_ref()
                    .is_some_and(|d| d.delimiter_type == DelimiterType::Full);
            if should_break {
                break;
            }
        }

        self.token_stack.append(&mut internal_tokens);

        let remain_closer_delimiter = remain_closer_delimiter?;

        if matches!(
            remain_closer_delimiter.delimiter_type,
            DelimiterType::Full | DelimiterType::Closer
        ) {
            let mut tokens = hooks[hook_index]
                .hook
                .process_single_delimiter(&remain_closer_delimiter);
            for token in &mut tokens {
                token.tokenizer = hook_name.clone();
            }
            self.token_stack.append(&mut tokens);
            return None;
        }

        Some(remain_closer_delimiter)
    }

    pub(super) fn process(
        &mut self,
        hook_index: usize,
        delimiter: TokenDelimiter,
        hooks: &mut [MatchInlineProcessorHook<'_>],
    ) {
        self.ht_index = Self::append_higher_priority_tokens(
            &mut self.token_stack,
            &self.higher_priority_tokens,
            self.ht_index,
            &delimiter,
        );

        match delimiter.delimiter_type {
            DelimiterType::Opener => {
                self.push(hook_index, delimiter);
            }
            DelimiterType::Both => {
                if let Some(remain_delimiter) = self.consume(hook_index, delimiter, hooks) {
                    self.push(hook_index, remain_delimiter);
                }
            }
            DelimiterType::Closer => {
                let _ = self.consume(hook_index, delimiter, hooks);
            }
            DelimiterType::Full => {
                let hook_name = hooks[hook_index].name.clone();
                let mut tokens = hooks[hook_index].process_single_delimiter(&delimiter);
                for token in &mut tokens {
                    token.tokenizer = hook_name.clone();
                }
                self.token_stack.append(&mut tokens);
            }
        }
    }

    pub(super) fn done(&mut self, hooks: &mut [MatchInlineProcessorHook<'_>]) -> Vec<InlineToken> {
        let mut tokens = Vec::new();
        for item in &self.delimiter_stack {
            if item.inactive {
                continue;
            }
            let hook_name = hooks[item.hook_index].name.clone();
            let mut next_tokens = hooks[item.hook_index]
                .hook
                .process_single_delimiter(&item.delimiter);
            for token in &mut next_tokens {
                token.tokenizer = hook_name.clone();
            }
            tokens.append(&mut next_tokens);
        }

        self.clear_delimiter_stacks();

        if !tokens.is_empty() {
            self.token_stack = merge_sorted_token_stack(&self.token_stack, &tokens);
        }

        let mut result = self.token_stack.clone();
        result.extend(self.higher_priority_tokens[self.ht_index..].iter().cloned());
        result
    }
}

fn merge_sorted_token_stack(tokens1: &[InlineToken], tokens2: &[InlineToken]) -> Vec<InlineToken> {
    if tokens1.is_empty() {
        return tokens2.to_vec();
    }
    if tokens2.is_empty() {
        return tokens1.to_vec();
    }

    let mut out = Vec::with_capacity(tokens1.len() + tokens2.len());
    let mut i1 = 0usize;
    let mut i2 = 0usize;

    while i1 < tokens1.len() && i2 < tokens2.len() {
        if tokens1[i1].start_index < tokens2[i2].start_index {
            out.push(tokens1[i1].clone());
            i1 += 1;
        } else {
            out.push(tokens2[i2].clone());
            i2 += 1;
        }
    }

    while i1 < tokens1.len() {
        out.push(tokens1[i1].clone());
        i1 += 1;
    }
    while i2 < tokens2.len() {
        out.push(tokens2[i2].clone());
        i2 += 1;
    }

    out
}
