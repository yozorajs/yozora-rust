use yozora_ast::TEXT_TYPE;
use yozora_core_tokenizer::{DelimiterType, InlineToken, TokenDelimiter};

pub(crate) fn find_text_delimiter(start_index: usize, end_index: usize) -> TokenDelimiter {
    TokenDelimiter {
        delimiter_type: DelimiterType::Full,
        start_index,
        end_index,
        thickness: end_index.saturating_sub(start_index),
        original_thickness: end_index.saturating_sub(start_index),
    }
}

pub(crate) fn process_single_delimiter(delimiter: &TokenDelimiter) -> Vec<InlineToken> {
    vec![InlineToken::new(
        "",
        TEXT_TYPE,
        (delimiter.start_index, delimiter.end_index),
    )]
}
