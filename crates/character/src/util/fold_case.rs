use crate::constant::folding_case::FOLDING_CASE_CODE_MAP;

pub fn fold_case(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    for character in text.chars() {
        match FOLDING_CASE_CODE_MAP.binary_search_by_key(&(character as u32), |entry| entry.0) {
            Ok(index) => result.push_str(FOLDING_CASE_CODE_MAP[index].1),
            Err(_) => result.push(character),
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::fold_case;

    #[test]
    fn follows_unicode_case_folding_map() {
        assert_eq!(fold_case("ẞ"), "ss");
        assert_eq!(fold_case("SS"), "SS");
        assert_eq!(fold_case("\u{1c80}"), "в");
    }
}
