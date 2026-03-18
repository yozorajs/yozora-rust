pub(crate) fn match_text_slice(source: &str, start_index: usize, end_index: usize) -> &str {
    &source[start_index..end_index]
}
