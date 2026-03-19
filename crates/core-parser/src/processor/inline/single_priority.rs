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
    token_stack: Vec<InlineToken>,
}

impl SinglePriorityDelimiterProcessor {
    pub(super) fn reset(&mut self, higher_priority_tokens: &[InlineToken]) {
        self.higher_priority_tokens.clear();
        self.higher_priority_tokens
            .extend(higher_priority_tokens.iter().cloned());

        self.ht_index = 0;
        self.delimiter_stack.clear();
        self.token_stack.clear();
    }

    fn cut_stale_branch(&mut self, start_stack_index: usize) {
        if start_stack_index == 0 {
            self.delimiter_stack.clear();
            return;
        }

        let mut top = start_stack_index - 1;
        while top > 0 && self.delimiter_stack[top].inactive {
            top -= 1;
        }

        if top == 0 && self.delimiter_stack[top].inactive {
            self.delimiter_stack.clear();
            return;
        }

        self.delimiter_stack.truncate(top + 1);
    }

    fn push(&mut self, hook_index: usize, delimiter: TokenDelimiter) {
        self.delimiter_stack.push(DelimiterItem {
            hook_index,
            delimiter,
            inactive: false,
            token_stack_index: self.token_stack.len(),
        });
    }

    pub(super) fn find_nearest_paired_delimiter(
        &self,
        hook_index: usize,
        closer_delimiter: &TokenDelimiter,
        hooks: &[MatchInlineProcessorHook<'_>],
    ) -> Option<TokenDelimiter> {
        if self.delimiter_stack.is_empty() {
            return None;
        }

        for i in (0..self.delimiter_stack.len()).rev() {
            let item = &self.delimiter_stack[i];
            if item.inactive || item.hook_index != hook_index {
                continue;
            }

            let opener_delimiter = &item.delimiter;
            let result = hooks[hook_index].isDelimiterPair(
                opener_delimiter,
                closer_delimiter,
                &self.higher_priority_tokens,
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

        let mut i = self.delimiter_stack.len();
        while i > 0 {
            i -= 1;
            if self.delimiter_stack[i].hook_index != hook_index || self.delimiter_stack[i].inactive
            {
                continue;
            }

            let opener_token_stack_index = self.delimiter_stack[i].token_stack_index;
            if opener_token_stack_index < self.token_stack.len() {
                let mut suffix = self.token_stack.split_off(opener_token_stack_index);
                suffix.append(&mut internal_tokens);
                internal_tokens = suffix;
            }

            let mut remain_opener_delimiter = Some(self.delimiter_stack[i].delimiter.clone());

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

                let pre_pair_result = hooks[hook_index].isDelimiterPair(
                    &opener_delimiter,
                    &closer_delimiter,
                    &internal_tokens,
                );

                if let IsDelimiterPairResult::NotPaired { opener, closer } = pre_pair_result {
                    if !opener {
                        let mut tokens = hooks[hook_index]
                            .hook
                            .processSingleDelimiter(&opener_delimiter);
                        for token in &mut tokens {
                            token.tokenizer = hook_name.clone();
                        }
                        if !tokens.is_empty() {
                            tokens.append(&mut internal_tokens);
                            internal_tokens = tokens;
                        }

                        self.delimiter_stack[i].inactive = true;
                    }

                    if !closer {
                        let mut tokens = hooks[hook_index]
                            .hook
                            .processSingleDelimiter(&closer_delimiter);
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
                    remainOpenerDelimiter: next_opener,
                    remainCloserDelimiter: next_closer,
                } = hooks[hook_index].processDelimiterPair(
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

                self.cut_stale_branch(i);
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

        let Some(remain_closer_delimiter) = remain_closer_delimiter else {
            return None;
        };

        if matches!(
            remain_closer_delimiter.delimiter_type,
            DelimiterType::Full | DelimiterType::Closer
        ) {
            let mut tokens = hooks[hook_index]
                .hook
                .processSingleDelimiter(&remain_closer_delimiter);
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
        while self.ht_index < self.higher_priority_tokens.len() {
            let token = &self.higher_priority_tokens[self.ht_index];
            if token.start_index >= delimiter.end_index {
                break;
            }

            if token.start_index < delimiter.start_index {
                self.token_stack.push(token.clone());
            }
            self.ht_index += 1;
        }

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
                let mut tokens = hooks[hook_index].processSingleDelimiter(&delimiter);
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
            let hook_name = hooks[item.hook_index].name.clone();
            let mut next_tokens = hooks[item.hook_index]
                .hook
                .processSingleDelimiter(&item.delimiter);
            for token in &mut next_tokens {
                token.tokenizer = hook_name.clone();
            }
            tokens.append(&mut next_tokens);
        }

        self.delimiter_stack.clear();

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
