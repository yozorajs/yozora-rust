use std::collections::BTreeMap;
use std::sync::LazyLock;

use crate::constant::ascii::AsciiCodePoint;
use crate::constant::entity::ENTITY_REFERENCES;
use crate::constant::unicode::UnicodeCodePoint;
use crate::types::{CodePoint, NodePoint};
use crate::util::charset::ascii::is_ascii_digit_character;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntityReference {
    pub next_index: usize,
    pub value: String,
}

#[derive(Debug, Clone, Default)]
pub struct EntityReferenceTrie {
    entries: BTreeMap<String, String>,
}

impl EntityReferenceTrie {
    pub fn search(
        &self,
        node_points: &[NodePoint],
        start_index: usize,
        end_index: usize,
    ) -> Option<EntityReference> {
        let mut key = String::new();
        for (index, point) in node_points
            .iter()
            .enumerate()
            .take(end_index)
            .skip(start_index)
        {
            key.push(char::from_u32(point.code_point as u32)?);
            if point.code_point != AsciiCodePoint::SEMICOLON as i32 {
                continue;
            }
            return self.entries.get(&key).map(|value| EntityReference {
                next_index: index + 1,
                value: value.clone(),
            });
        }
        None
    }

    pub fn insert(&mut self, keys: &[CodePoint], value: &str) {
        let key = keys
            .iter()
            .filter_map(|code_point| char::from_u32(*code_point as u32))
            .collect::<String>();
        self.entries.insert(key, value.to_string());
    }
}

pub fn create_entity_reference_trie() -> EntityReferenceTrie {
    let entries = ENTITY_REFERENCES
        .iter()
        .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
        .collect();
    EntityReferenceTrie { entries }
}

pub static ENTITY_REFERENCE_TRIE: LazyLock<EntityReferenceTrie> =
    LazyLock::new(create_entity_reference_trie);

pub fn eat_entity_reference(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<EntityReference> {
    if start_index + 1 >= end_index {
        return None;
    }

    if let Some(entity) = ENTITY_REFERENCE_TRIE.search(node_points, start_index, end_index) {
        return Some(entity);
    }

    if node_points[start_index].code_point != AsciiCodePoint::NUMBER_SIGN as i32 {
        return None;
    }

    parse_numeric_entity(node_points, start_index, end_index)
}

fn parse_numeric_entity(
    node_points: &[NodePoint],
    start_index: usize,
    end_index: usize,
) -> Option<EntityReference> {
    let mut index = start_index + 1;
    let mut value: CodePoint = 0;
    let radix = if index < end_index
        && matches!(
            node_points[index].code_point,
            code_point if code_point == AsciiCodePoint::LOWERCASE_X as i32
                || code_point == AsciiCodePoint::UPPERCASE_X as i32
        ) {
        index += 1;
        16
    } else {
        10
    };
    let maximum_digits = if radix == 16 { 6 } else { 7 };
    let mut digit_count = 0usize;

    while index < end_index && digit_count < maximum_digits {
        let code_point = node_points[index].code_point;
        let digit = if radix == 16 {
            match code_point {
                value if is_ascii_digit_character(value) => value - AsciiCodePoint::DIGIT0 as i32,
                value
                    if (AsciiCodePoint::UPPERCASE_A as i32
                        ..=AsciiCodePoint::UPPERCASE_F as i32)
                        .contains(&value) =>
                {
                    value - AsciiCodePoint::UPPERCASE_A as i32 + 10
                }
                value
                    if (AsciiCodePoint::LOWERCASE_A as i32
                        ..=AsciiCodePoint::LOWERCASE_F as i32)
                        .contains(&value) =>
                {
                    value - AsciiCodePoint::LOWERCASE_A as i32 + 10
                }
                _ => break,
            }
        } else if is_ascii_digit_character(code_point) {
            code_point - AsciiCodePoint::DIGIT0 as i32
        } else {
            break;
        };
        value = value * radix + digit;
        digit_count += 1;
        index += 1;
    }

    if digit_count == 0
        || index >= end_index
        || node_points[index].code_point != AsciiCodePoint::SEMICOLON as i32
    {
        return None;
    }
    if value == 0 || value > 0x10ffff || (0xd800..=0xdfff).contains(&value) {
        value = UnicodeCodePoint::ReplacementCharacter as i32;
    }
    Some(EntityReference {
        next_index: index + 1,
        value: char::from_u32(value as u32)
            .unwrap_or('\u{fffd}')
            .to_string(),
    })
}
