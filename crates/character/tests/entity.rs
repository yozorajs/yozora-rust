use yozora_character::{
    create_node_point_generator, eat_entity_reference, is_punctuation_character,
    UnicodePcCodePoint, UnicodePdCodePoint, UnicodePeCodePoint, UnicodePfCodePoint,
    UnicodePiCodePoint, UnicodePoCodePoint, UnicodePsCodePoint, ENTITY_REFERENCES,
    ENTITY_REFERENCE_TRIE,
};

#[test]
fn every_v2_4_0_named_entity_is_searchable() {
    for (key, value) in ENTITY_REFERENCES {
        let points = create_node_point_generator(*key)
            .into_iter()
            .next()
            .expect("entity should produce points");
        let result = ENTITY_REFERENCE_TRIE
            .search(&points, 0, points.len())
            .unwrap_or_else(|| panic!("entity not found: {key}"));
        assert_eq!(result.next_index, points.len(), "{key}");
        assert_eq!(result.value, *value, "{key}");
    }
}

#[test]
fn every_v2_4_0_unicode_punctuation_is_recognized() {
    for code_point in [
        UnicodePcCodePoint::VALUES,
        UnicodePdCodePoint::VALUES,
        UnicodePeCodePoint::VALUES,
        UnicodePfCodePoint::VALUES,
        UnicodePiCodePoint::VALUES,
        UnicodePoCodePoint::VALUES,
        UnicodePsCodePoint::VALUES,
    ]
    .into_iter()
    .flatten()
    {
        assert!(is_punctuation_character(*code_point), "U+{code_point:04X}");
    }
}

#[test]
fn numeric_entities_match_scalar_rules() {
    for (source, expected) in [
        ("&#992;", "Ϡ"),
        ("&#xcab;", "ಫ"),
        ("&#0;", "�"),
        ("&#xDFFF;", "�"),
    ] {
        let points = create_node_point_generator(source)
            .into_iter()
            .next()
            .unwrap();
        assert_eq!(
            eat_entity_reference(&points, 1, points.len())
                .expect("numeric entity should match")
                .value,
            expected
        );
    }
    for source in ["&#;", "&#x;", "&#X;"] {
        let points = create_node_point_generator(source)
            .into_iter()
            .next()
            .unwrap();
        assert!(eat_entity_reference(&points, 1, points.len()).is_none());
    }
}
