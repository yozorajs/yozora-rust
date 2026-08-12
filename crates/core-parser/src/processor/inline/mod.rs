mod single_priority;
mod types;

pub use types::MatchInlineProcessorHook;

use single_priority::SinglePriorityDelimiterProcessor;
use yozora_core_tokenizer::{DelimiterType, InlineToken, TokenDelimiter};
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
        let delimiter = hooks[hook_index].find_delimiter((start_index, end_index));
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
            hooks[hook_index].reset();
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

#[cfg(test)]
mod tests {
    use super::{match_inline_tokens, MatchInlineProcessorHook};
    use yozora_core_tokenizer::{
        DelimiterType, FindDelimiterGenerator, InlineToken, IsDelimiterPairResult, MatchInlineHook,
        ProcessDelimiterPairResult, TokenDelimiter,
    };

    struct DummyHook {
        delimiters: Vec<TokenDelimiter>,
        paired: bool,
    }

    impl DummyHook {
        fn new(delimiters: Vec<TokenDelimiter>, paired: bool) -> Self {
            Self { delimiters, paired }
        }
    }

    impl<'a> MatchInlineHook<'a> for DummyHook {
        fn find_delimiter(&self) -> Box<dyn FindDelimiterGenerator + 'a> {
            let delimiters = self.delimiters.clone();

            struct DummyDelimiterGenerator {
                delimiters: Vec<TokenDelimiter>,
                cursor: usize,
            }

            impl FindDelimiterGenerator for DummyDelimiterGenerator {
                fn next(&mut self, range_index: (usize, usize)) -> Option<TokenDelimiter> {
                    let (start_index, end_index) = range_index;
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
            }

            Box::new(DummyDelimiterGenerator {
                delimiters,
                cursor: 0,
            })
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
        assert_eq!(tokens[0].tokenizer.as_ref(), "em");
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
        assert_eq!(tokens[0].tokenizer.as_ref(), "hp");
        assert_eq!(tokens[1].tokenizer.as_ref(), "full");
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
        assert_eq!(tokens[0].tokenizer.as_ref(), "high");
        assert_eq!(tokens[1].tokenizer.as_ref(), "low");
    }
}
