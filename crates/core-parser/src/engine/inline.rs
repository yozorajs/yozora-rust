use yozora_core_tokenizer::engine::{
    DelimiterType, InlineToken, IsDelimiterPairResult, MatchInlineHook, ProcessDelimiterPairResult,
    TokenDelimiter,
};

pub struct MatchInlineProcessorHook<'a> {
    pub name: String,
    pub priority: i32,
    pub hook: Box<dyn MatchInlineHook + 'a>,
}

impl<'a> MatchInlineProcessorHook<'a> {
    pub fn new(
        name: impl Into<String>,
        priority: i32,
        hook: Box<dyn MatchInlineHook + 'a>,
    ) -> Self {
        Self {
            name: name.into(),
            priority,
            hook,
        }
    }
}

#[derive(Debug, Clone)]
struct DelimiterItem {
    hook_index: usize,
    delimiter: TokenDelimiter,
    inactive: bool,
    token_stack_index: usize,
}

#[derive(Default)]
struct SinglePriorityDelimiterProcessor {
    ht_index: usize,
    higher_priority_tokens: Vec<InlineToken>,
    delimiter_stack: Vec<DelimiterItem>,
    token_stack: Vec<InlineToken>,
}

impl SinglePriorityDelimiterProcessor {
    fn reset(&mut self, higher_priority_tokens: &[InlineToken]) {
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

    fn find_nearest_paired_delimiter(
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
            let result = hooks[hook_index].hook.is_delimiter_pair(
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

                let pre_pair_result = hooks[hook_index].hook.is_delimiter_pair(
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

                        self.delimiter_stack[i].inactive = true;
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
                } = hooks[hook_index].hook.process_delimiter_pair(
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
                .process_single_delimiter(&remain_closer_delimiter);
            for token in &mut tokens {
                token.tokenizer = hook_name.clone();
            }
            self.token_stack.append(&mut tokens);
            return None;
        }

        Some(remain_closer_delimiter)
    }

    fn process(
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
                let mut tokens = hooks[hook_index].hook.process_single_delimiter(&delimiter);
                for token in &mut tokens {
                    token.tokenizer = hook_name.clone();
                }
                self.token_stack.append(&mut tokens);
            }
        }
    }

    fn done(&mut self, hooks: &mut [MatchInlineProcessorHook<'_>]) -> Vec<InlineToken> {
        let mut tokens = Vec::new();
        for item in &self.delimiter_stack {
            let hook_name = hooks[item.hook_index].name.clone();
            let mut next_tokens = hooks[item.hook_index]
                .hook
                .process_single_delimiter(&item.delimiter);
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

#[derive(Clone)]
struct NearestDelimiterItem {
    hook_index: usize,
    delimiter: TokenDelimiter,
}

fn find_nearest_delimiters(
    start_index: usize,
    end_index: usize,
    hook_indices: &[usize],
    hooks: &mut [MatchInlineProcessorHook<'_>],
    processor: &SinglePriorityDelimiterProcessor,
) -> (Vec<NearestDelimiterItem>, Option<usize>) {
    let mut nearest_delimiters = Vec::new();
    let mut nearest_start: Option<usize> = None;

    for &hook_index in hook_indices {
        let delimiter = hooks[hook_index]
            .hook
            .find_delimiter(start_index, end_index);
        let Some(delimiter) = delimiter else {
            continue;
        };

        if let Some(nearest_start) = nearest_start {
            if delimiter.start_index > nearest_start {
                continue;
            }
            if delimiter.start_index < nearest_start {
                nearest_delimiters.clear();
            }
        }

        nearest_start = Some(delimiter.start_index);
        nearest_delimiters.push(NearestDelimiterItem {
            hook_index,
            delimiter,
        });
    }

    if nearest_delimiters.is_empty() {
        return (Vec::new(), None);
    }

    let default_next_index = nearest_start.map(|x| x + 1);

    if nearest_delimiters.len() > 1 {
        let mut potential_closer_count = 0usize;
        for item in &nearest_delimiters {
            let delimiter_type = item.delimiter.delimiter_type;
            if delimiter_type == DelimiterType::Full {
                return (vec![item.clone()], Some(item.delimiter.end_index));
            }
            if matches!(delimiter_type, DelimiterType::Both | DelimiterType::Closer) {
                potential_closer_count += 1;
            }
        }

        if potential_closer_count > 1 {
            let mut valid_closer_index: Option<usize> = None;
            let mut valid_paired_opener_start = 0usize;

            for (index, item) in nearest_delimiters.iter().enumerate() {
                if !matches!(
                    item.delimiter.delimiter_type,
                    DelimiterType::Both | DelimiterType::Closer
                ) {
                    continue;
                }

                if let Some(opener) =
                    processor.find_nearest_paired_delimiter(item.hook_index, &item.delimiter, hooks)
                {
                    if valid_closer_index.is_none()
                        || opener.start_index > valid_paired_opener_start
                    {
                        valid_closer_index = Some(index);
                        valid_paired_opener_start = opener.start_index;
                    }
                }
            }

            let items = if let Some(valid_closer_index) = valid_closer_index {
                vec![nearest_delimiters[valid_closer_index].clone()]
            } else {
                nearest_delimiters
                    .into_iter()
                    .filter(|item| item.delimiter.delimiter_type != DelimiterType::Closer)
                    .collect::<Vec<_>>()
            };
            return (items, default_next_index);
        }
    }

    (nearest_delimiters, default_next_index)
}

fn group_hooks_by_priority(hooks: &[MatchInlineProcessorHook<'_>]) -> Vec<Vec<usize>> {
    let mut groups = Vec::new();

    let mut i = 0usize;
    while i < hooks.len() {
        let mut group = Vec::new();
        let priority = hooks[i].priority;
        while i < hooks.len() && hooks[i].priority == priority {
            group.push(i);
            i += 1;
        }
        groups.push(group);
    }

    groups
}

pub fn match_inline_tokens(
    hooks: &mut [MatchInlineProcessorHook<'_>],
    higher_priority_tokens: &[InlineToken],
    start_index: usize,
    end_index: usize,
) -> Vec<InlineToken> {
    if hooks.is_empty() {
        return higher_priority_tokens.to_vec();
    }

    let hook_groups = group_hooks_by_priority(hooks);
    let mut processor = SinglePriorityDelimiterProcessor::default();

    let mut tokens = higher_priority_tokens.to_vec();
    for hook_indices in hook_groups {
        for &hook_index in &hook_indices {
            hooks[hook_index].hook.reset();
        }

        let mut token_index = 0usize;
        processor.reset(&tokens);

        let mut i = start_index;
        while i < end_index {
            let mut next_end_index = end_index;
            while token_index < tokens.len() {
                let token = &tokens[token_index];
                if i < token.start_index {
                    next_end_index = token.start_index;
                    break;
                }
                if i < token.end_index {
                    i = token.end_index;
                }
                token_index += 1;
            }

            let (items, next_index) =
                find_nearest_delimiters(i, next_end_index, &hook_indices, hooks, &processor);
            if items.is_empty() {
                i = next_index.unwrap_or(next_end_index);
                continue;
            }

            i += 1;
            for item in items {
                i = i.max(item.delimiter.end_index);
                processor.process(item.hook_index, item.delimiter, hooks);
            }
        }

        tokens = processor.done(hooks);
    }

    tokens
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

#[cfg(test)]
mod tests {
    use super::{match_inline_tokens, MatchInlineProcessorHook};
    use yozora_core_tokenizer::engine::{
        DelimiterType, InlineToken, IsDelimiterPairResult, MatchInlineHook,
        ProcessDelimiterPairResult, TokenDelimiter,
    };

    struct DummyHook {
        delimiters: Vec<TokenDelimiter>,
        cursor: usize,
        paired: bool,
    }

    impl DummyHook {
        fn new(delimiters: Vec<TokenDelimiter>, paired: bool) -> Self {
            Self {
                delimiters,
                cursor: 0,
                paired,
            }
        }
    }

    impl MatchInlineHook for DummyHook {
        fn reset(&mut self) {
            self.cursor = 0;
        }

        fn find_delimiter(
            &mut self,
            start_index: usize,
            end_index: usize,
        ) -> Option<TokenDelimiter> {
            while self.cursor < self.delimiters.len() {
                let delimiter = self.delimiters[self.cursor].clone();
                if delimiter.start_index < start_index {
                    self.cursor += 1;
                    continue;
                }
                if delimiter.start_index >= end_index {
                    return None;
                }
                self.cursor += 1;
                return Some(delimiter);
            }
            None
        }

        fn is_delimiter_pair(
            &self,
            _opener_delimiter: &TokenDelimiter,
            _closer_delimiter: &TokenDelimiter,
            _internal_tokens: &[InlineToken],
        ) -> IsDelimiterPairResult {
            if self.paired {
                IsDelimiterPairResult::Paired
            } else {
                IsDelimiterPairResult::NotPaired {
                    opener: false,
                    closer: false,
                }
            }
        }

        fn process_delimiter_pair(
            &self,
            opener_delimiter: &TokenDelimiter,
            closer_delimiter: &TokenDelimiter,
            _internal_tokens: &[InlineToken],
        ) -> ProcessDelimiterPairResult {
            ProcessDelimiterPairResult {
                tokens: vec![InlineToken::new(
                    "",
                    "emphasis",
                    (opener_delimiter.start_index, closer_delimiter.end_index),
                )],
                remain_opener_delimiter: None,
                remain_closer_delimiter: None,
            }
        }

        fn process_single_delimiter(&self, delimiter: &TokenDelimiter) -> Vec<InlineToken> {
            vec![InlineToken::new(
                "",
                "text",
                (delimiter.start_index, delimiter.end_index),
            )]
        }
    }

    fn delimiter(
        delimiter_type: DelimiterType,
        start_index: usize,
        end_index: usize,
    ) -> TokenDelimiter {
        TokenDelimiter {
            delimiter_type,
            start_index,
            end_index,
            thickness: 1,
            original_thickness: 1,
        }
    }

    #[test]
    fn should_pair_delimiters_into_one_token() {
        let mut hooks = vec![MatchInlineProcessorHook::new(
            "em",
            1,
            Box::new(DummyHook::new(
                vec![
                    delimiter(DelimiterType::Opener, 0, 1),
                    delimiter(DelimiterType::Closer, 2, 3),
                ],
                true,
            )),
        )];

        let tokens = match_inline_tokens(&mut hooks, &[], 0, 3);

        assert_eq!(tokens.len(), 1);
        assert_eq!(tokens[0].tokenizer, "em");
        assert_eq!(tokens[0].start_index, 0);
        assert_eq!(tokens[0].end_index, 3);
    }

    #[test]
    fn should_keep_higher_priority_token_order() {
        let mut hooks = vec![MatchInlineProcessorHook::new(
            "full",
            1,
            Box::new(DummyHook::new(
                vec![delimiter(DelimiterType::Full, 3, 4)],
                false,
            )),
        )];

        let higher_priority_tokens = vec![InlineToken::new("hp", "inlineCode", (1, 2))];
        let tokens = match_inline_tokens(&mut hooks, &higher_priority_tokens, 0, 4);

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].tokenizer, "hp");
        assert_eq!(tokens[1].tokenizer, "full");
    }

    #[test]
    fn should_process_priority_groups_from_high_to_low() {
        let mut hooks = vec![
            MatchInlineProcessorHook::new(
                "high",
                10,
                Box::new(DummyHook::new(
                    vec![delimiter(DelimiterType::Full, 0, 1)],
                    false,
                )),
            ),
            MatchInlineProcessorHook::new(
                "low",
                1,
                Box::new(DummyHook::new(
                    vec![delimiter(DelimiterType::Full, 2, 3)],
                    false,
                )),
            ),
        ];

        let tokens = match_inline_tokens(&mut hooks, &[], 0, 3);

        assert_eq!(tokens.len(), 2);
        assert_eq!(tokens[0].tokenizer, "high");
        assert_eq!(tokens[1].tokenizer, "low");
    }
}
