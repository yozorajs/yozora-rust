use yozora_character::{is_whitespace_character, AsciiCodePoint, NodePoint, VirtualCodePoint};

const MAX_LABEL_CONTENT_LENGTH: usize = 999;

pub fn eat_footnote_label(
    node_points: &[NodePoint],
    first_non_whitespace_index: usize,
    end_index: usize,
) -> isize {
    let mut i = first_non_whitespace_index;

    if i + 1 >= end_index
        || node_points[i].code_point != AsciiCodePoint::OPEN_BRACKET as i32
        || node_points[i + 1].code_point != AsciiCodePoint::CARET as i32
    {
        return -1;
    }

    let mut is_empty = true;
    let first_content_index = i + 2;
    let last_index = std::cmp::min(
        end_index,
        first_content_index + MAX_LABEL_CONTENT_LENGTH + 1,
    );
    i = first_content_index;
    while i < last_index {
        let code_point = node_points[i].code_point;
        match code_point {
            x if x == AsciiCodePoint::BACKSLASH as i32 => {
                is_empty = false;
                i += 1;
                if i >= end_index || node_points[i].code_point == VirtualCodePoint::LineEnd as i32 {
                    return -1;
                }
            }
            x if x == AsciiCodePoint::OPEN_BRACKET as i32 => return -1,
            x if x == AsciiCodePoint::CLOSE_BRACKET as i32 => {
                return if is_empty { -1 } else { (i + 1) as isize };
            }
            x if x == VirtualCodePoint::LineEnd as i32 => return -1,
            _ => {
                if is_empty && !is_whitespace_character(code_point) {
                    is_empty = false;
                }
            }
        }
        i += 1;
    }

    -1
}

#[cfg(test)]
mod tests {
    use super::*;
    use yozora_character::create_node_point_generator;

    fn label_end(content: &str) -> isize {
        let node_points = create_node_point_generator(content)
            .pop()
            .unwrap_or_default();
        eat_footnote_label(&node_points, 0, node_points.len())
    }

    #[test]
    fn handles_backslash_escaped_brackets() {
        assert_eq!(label_end(r"[^valid footnote\] \[ \] label\]]"), 33);
        assert_eq!(label_end(r"[^\]]"), 5);
        assert_eq!(label_end("[^invalid footnote []"), -1);
    }

    #[test]
    fn requires_label_on_one_line() {
        assert_eq!(label_end("[^valid footnote]"), 17);
        assert_eq!(label_end("[^invalid \n footnote]"), -1);
        assert_eq!(label_end("[^invalid footnote\n]"), -1);
        assert_eq!(label_end("[^invalid\\\nfootnote]"), -1);
    }

    #[test]
    fn rejects_unclosed_label() {
        assert_eq!(label_end("[^valid footnote"), -1);
    }

    #[test]
    fn rejects_empty_label() {
        assert_eq!(label_end("[^   ]"), -1);
    }

    #[test]
    fn limits_label_content_to_999_characters() {
        for size in [998, 999] {
            let label = format!("[^{}]", "a".repeat(size));
            assert_eq!(label_end(&label), label.len() as isize);
        }
        assert_eq!(label_end(&format!("[^{}]", "a".repeat(1_000))), -1);
    }
}
