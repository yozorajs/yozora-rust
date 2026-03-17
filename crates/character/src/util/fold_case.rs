pub fn fold_case(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        match ch {
            // CommonMark uses Unicode case fold; this special-case keeps
            // reference identifiers compatible with `ẞ` <-> `SS` matching.
            'ß' | 'ẞ' => out.push_str("ss"),
            _ => out.extend(ch.to_lowercase()),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::fold_case;

    #[test]
    fn fold_german_sharp_s() {
        assert_eq!(fold_case("ẞ"), "ss");
        assert_eq!(fold_case("Straße"), "strasse");
    }
}
