use std::collections::HashSet;
use std::sync::Arc;

use crate::Escaper;

pub fn split_lines(value: &str) -> Vec<&str> {
    value.split(['\r', '\n']).collect()
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

pub fn find_max_continuous_symbol(value: &str, symbol: char) -> usize {
    let mut maximum = 0usize;
    let mut current = 0usize;
    for character in value.chars() {
        if character == symbol {
            current += 1;
            maximum = maximum.max(current);
        } else {
            current = 0;
        }
    }
    maximum
}
