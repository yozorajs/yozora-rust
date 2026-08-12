use std::collections::HashSet;
use std::sync::Arc;

use crate::Escaper;

pub const LINE_REGEX: &str = r"\r\n|\n|\r";

pub fn split_lines(value: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut start = 0usize;
    let bytes = value.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'\r' || bytes[index] == b'\n' {
            lines.push(&value[start..index]);
            if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
                index += 1;
            }
            start = index + 1;
        }
        index += 1;
    }
    lines.push(&value[start..]);
    lines
}

pub fn create_character_escaper(characters: &[char]) -> Escaper {
    let characters = characters.iter().copied().collect::<HashSet<_>>();
    Arc::new(move |text: &str| {
        let mut result = String::with_capacity(text.len());
        let mut backslash_count = 0usize;

        for character in text.chars() {
            if characters.contains(&character) && backslash_count.is_multiple_of(2) {
                result.push('\\');
            }
            result.push(character);
            if character == '\\' {
                backslash_count += 1;
            } else {
                backslash_count = 0;
            }
        }
        result
    })
}

pub fn minmax(value: i64, min: i64, max: i64) -> i64 {
    value.clamp(min, max)
}

pub trait ContinuousSymbolMatcher {
    fn matches(&self, character: char) -> bool;
}

impl ContinuousSymbolMatcher for char {
    fn matches(&self, character: char) -> bool {
        character == *self
    }
}

impl<F> ContinuousSymbolMatcher for F
where
    F: Fn(char) -> bool,
{
    fn matches(&self, character: char) -> bool {
        self(character)
    }
}

pub fn find_max_continuous_symbol<M>(value: &str, symbol_matcher: M) -> usize
where
    M: ContinuousSymbolMatcher,
{
    let mut maximum = 0usize;
    let mut current = 0usize;
    for character in value.chars() {
        if symbol_matcher.matches(character) {
            current += 1;
            maximum = maximum.max(current);
        } else {
            current = 0;
        }
    }
    maximum
}

#[cfg(test)]
mod tests {
    use super::{find_max_continuous_symbol, split_lines};

    #[test]
    fn line_regex_semantics_treat_crlf_as_one_separator() {
        assert_eq!(split_lines("a\r\nb\rc\n"), ["a", "b", "c", ""]);
    }

    #[test]
    fn continuous_symbol_accepts_character_predicates() {
        assert_eq!(find_max_continuous_symbol("a```b~~", '`'), 3);
        assert_eq!(find_max_continuous_symbol("a```b~~", |ch| ch == '~'), 2);
    }
}
