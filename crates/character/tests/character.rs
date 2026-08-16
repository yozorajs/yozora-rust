use yozora_character::{
    ascii_control_characters, ascii_punctuation_characters, ascii_whitespace_characters,
    calc_escaped_string_from_node_points, calc_string_from_node_points,
    calc_trim_boundary_of_code_points, control_characters, create_node_point_generator, fold_case,
    is_ascii_control_character, is_ascii_digit_character, is_ascii_punctuation_character,
    is_ascii_whitespace_character, is_control_character, is_punctuation_character,
    is_space_character, is_unicode_whitespace_character, is_whitespace_character,
    punctuation_characters, space_characters, strip_chinese_characters, tighten_chinese_characters,
    unicode_whitespace_characters, whitespace_characters, AsciiCodePoint, NodePoint,
    UnicodeZsCodePoint, VirtualCodePoint,
};

fn collect(input: &str) -> Vec<NodePoint> {
    create_node_point_generator(input)
        .into_iter()
        .flatten()
        .collect()
}

#[test]
fn character_sets_and_predicates_are_consistent() {
    for code_point in ascii_whitespace_characters() {
        assert!(is_ascii_whitespace_character(*code_point));
    }
    for code_point in ascii_punctuation_characters() {
        assert!(is_ascii_punctuation_character(*code_point));
        assert!(is_punctuation_character(*code_point));
    }
    for code_point in ascii_control_characters() {
        assert!(is_ascii_control_character(*code_point));
        assert!(is_control_character(*code_point));
    }
    for code_point in AsciiCodePoint::DIGIT0 as i32..=AsciiCodePoint::DIGIT9 as i32 {
        assert!(is_ascii_digit_character(code_point));
    }

    assert_eq!(control_characters(), ascii_control_characters());
    assert_eq!(
        space_characters(),
        [AsciiCodePoint::SPACE as i32, VirtualCodePoint::Space as i32]
    );
    assert_eq!(
        whitespace_characters(),
        [
            AsciiCodePoint::VT as i32,
            AsciiCodePoint::FF as i32,
            AsciiCodePoint::SPACE as i32,
            VirtualCodePoint::Space as i32,
            VirtualCodePoint::LineEnd as i32,
        ]
    );
    for code_point in punctuation_characters() {
        assert!(is_punctuation_character(code_point));
    }
    for code_point in space_characters() {
        assert!(is_space_character(code_point));
    }
    for code_point in whitespace_characters() {
        assert!(is_whitespace_character(code_point));
    }

    for code_point in 0..=AsciiCodePoint::DELETE as i32 {
        assert_eq!(
            is_ascii_whitespace_character(code_point),
            ascii_whitespace_characters().contains(&code_point),
            "ASCII whitespace U+{code_point:04X}"
        );
        assert_eq!(
            is_ascii_punctuation_character(code_point),
            ascii_punctuation_characters().contains(&code_point),
            "ASCII punctuation U+{code_point:04X}"
        );
        assert_eq!(
            is_ascii_digit_character(code_point),
            (AsciiCodePoint::DIGIT0 as i32..=AsciiCodePoint::DIGIT9 as i32).contains(&code_point),
            "ASCII digit U+{code_point:04X}"
        );
        assert_eq!(
            is_ascii_control_character(code_point),
            ascii_control_characters().contains(&code_point),
            "ASCII control U+{code_point:04X}"
        );
        assert_eq!(
            is_space_character(code_point),
            space_characters().contains(&code_point),
            "space U+{code_point:04X}"
        );
        assert_eq!(
            is_whitespace_character(code_point),
            whitespace_characters().contains(&code_point),
            "whitespace U+{code_point:04X}"
        );
        assert_eq!(
            is_control_character(code_point),
            control_characters().contains(&code_point),
            "control U+{code_point:04X}"
        );
    }
}

#[test]
fn unicode_whitespace_set_matches_predicate() {
    let values = unicode_whitespace_characters();
    for code_point in &values {
        assert!(is_unicode_whitespace_character(*code_point));
    }
    for code_point in UnicodeZsCodePoint::VALUES {
        assert!(values.contains(code_point));
    }
    for code_point in [
        AsciiCodePoint::HT as i32,
        AsciiCodePoint::LF as i32,
        AsciiCodePoint::FF as i32,
        AsciiCodePoint::CR as i32,
        VirtualCodePoint::Space as i32,
        VirtualCodePoint::LineEnd as i32,
    ] {
        assert!(values.contains(&code_point));
    }
    for code_point in 0..=AsciiCodePoint::DELETE as i32 {
        assert_eq!(
            is_unicode_whitespace_character(code_point),
            values.contains(&code_point),
            "U+{code_point:04X}"
        );
    }
}

#[test]
fn node_point_generator_normalizes_controls() {
    let points = collect("\tA\r\nB\rC\n\0");
    let code_points: Vec<i32> = points.iter().map(|point| point.code_point).collect();
    assert_eq!(
        code_points,
        [
            VirtualCodePoint::Space as i32,
            VirtualCodePoint::Space as i32,
            VirtualCodePoint::Space as i32,
            VirtualCodePoint::Space as i32,
            AsciiCodePoint::UPPERCASE_A as i32,
            VirtualCodePoint::LineEnd as i32,
            AsciiCodePoint::UPPERCASE_B as i32,
            VirtualCodePoint::LineEnd as i32,
            AsciiCodePoint::UPPERCASE_C as i32,
            VirtualCodePoint::LineEnd as i32,
            0xfffd,
        ]
    );
    assert_eq!(points[5].source_width, Some(2));
    assert_eq!(points[7].source_width, None);
    assert_eq!(points[9].source_width, None);
}

#[test]
fn node_point_generation_is_chunk_independent() {
    for content in ["a\r\nb", "a😀b", "a\r\n😀b", "text\r"] {
        let expected = collect(content);
        let mut boundaries: Vec<usize> = content.char_indices().map(|(index, _)| index).collect();
        boundaries.push(content.len());
        for boundary in boundaries {
            let chunks =
                create_node_point_generator(vec![&content[..boundary], &content[boundary..]])
                    .into_iter()
                    .flatten()
                    .collect::<Vec<_>>();
            assert_eq!(chunks, expected, "content={content:?}, boundary={boundary}");
        }

        let char_chunks = content.chars().map(|ch| ch.to_string()).collect::<Vec<_>>();
        let chunks = create_node_point_generator(char_chunks)
            .into_iter()
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(chunks, expected, "content={content:?}");
    }
}

#[test]
fn offsets_remain_utf16_source_indices() {
    let content = "a😀b";
    let points = collect(content);
    let point = points
        .iter()
        .find(|point| point.code_point == AsciiCodePoint::LOWERCASE_B as i32)
        .expect("expected b");
    assert_eq!(point.offset, 3);
}

#[test]
fn reconstructs_tabs_newlines_escapes_and_entities() {
    let points = collect("\tA\n");
    assert_eq!(points[0], points[1]);
    assert_eq!(points[0], points[2]);
    assert_eq!(points[0], points[3]);
    assert_eq!(
        calc_string_from_node_points(&points, 0, points.len(), false),
        "\tA\n"
    );
    assert_eq!(
        calc_string_from_node_points(&points, 0, points.len(), true),
        "A"
    );

    let points = collect("\t\t");
    assert_eq!(
        calc_string_from_node_points(&points, 0, points.len(), false),
        "\t\t"
    );
    assert_eq!(
        calc_escaped_string_from_node_points(&points, 0, points.len(), false),
        "\t\t"
    );
    assert_eq!(calc_string_from_node_points(&points, 2, 6, false), "    ");
    assert_eq!(
        calc_escaped_string_from_node_points(&points, 2, 6, false),
        "    "
    );

    let points = collect(r"\* \a &amp;");
    assert_eq!(
        calc_escaped_string_from_node_points(&points, 0, points.len(), false),
        r"* \a &"
    );

    let points = collect("  hello  ");
    assert_eq!(
        calc_trim_boundary_of_code_points(&points, 0, points.len()),
        (2, 7)
    );
    assert_eq!(
        calc_string_from_node_points(&points, 0, points.len(), true),
        "hello"
    );
}

#[test]
fn fold_case_covers_reference_cyrillic_additions() {
    for (source, target) in [
        ("\u{1C80}", "\u{0432}"),
        ("\u{1C81}", "\u{0434}"),
        ("\u{1C82}", "\u{043E}"),
        ("\u{1C83}", "\u{0441}"),
        ("\u{1C84}", "\u{0442}"),
        ("\u{1C85}", "\u{0442}"),
        ("\u{1C86}", "\u{044A}"),
        ("\u{1C87}", "\u{0463}"),
        ("\u{1C88}", "\u{A64B}"),
    ] {
        assert_eq!(fold_case(source), target);
    }
    assert_eq!(fold_case("A"), "A");
    assert_eq!(fold_case("a"), "a");
}

#[test]
fn strips_and_tightens_chinese_characters() {
    for (source, expected) in [
        ("中文\n中文2", "中文中文2"),
        ("中文；\n中文2", "中文；中文2"),
        ("中\n文\n字", "中文字"),
        ("中\n；\n文", "中；文"),
        ("中文\nEnglish", "中文\nEnglish"),
        ("中文\n.English", "中文\n.English"),
        ("中文；\nEnglish", "中文；\nEnglish"),
        ("English\n中文", "English\n中文"),
        ("English.\n中文", "English.\n中文"),
        ("English\n；中文", "English\n；中文"),
        ("English\nEnglish", "English\nEnglish"),
        ("English.\nEnglish", "English.\nEnglish"),
        ("English\n.English", "English\n.English"),
    ] {
        assert_eq!(strip_chinese_characters(source), expected);
    }

    assert_eq!(tighten_chinese_characters("中 文 字"), "中文字");
    assert_eq!(tighten_chinese_characters("中 \n；\t 文"), "中；文");
    assert_eq!(tighten_chinese_characters("中 English 文"), "中 English 文");
}
