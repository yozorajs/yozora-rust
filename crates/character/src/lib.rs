pub mod constant;
pub mod types;
pub mod util;

pub use constant::ascii::AsciiCodePoint;
pub use constant::unicode::UnicodeCodePoint;
pub use constant::virtual_code_point::VirtualCodePoint;
pub use types::{CodePoint, NodeInterval, NodePoint};
pub use util::character::{
    control_characters, is_control_character, is_line_ending, is_punctuation_character,
    is_space_character, is_space_like, is_whitespace_character, punctuation_characters,
    space_characters, whitespace_characters,
};
pub use util::charset::ascii::{
    ascii_control_characters, ascii_punctuation_characters, ascii_whitespace_characters,
    is_alphanumeric, is_ascii_character, is_ascii_control_character, is_ascii_digit_character,
    is_ascii_letter, is_ascii_lower_letter, is_ascii_punctuation_character, is_ascii_upper_letter,
    is_ascii_whitespace_character,
};
pub use util::charset::unicode::{is_unicode_whitespace_character, unicode_whitespace_characters};
pub use util::entity_reference::{eat_entity_reference, EntityReference};
pub use util::fold_case::fold_case;
pub use util::han::{strip_chinese_characters, tighten_chinese_characters};
pub use util::node_point::{
    calc_escaped_string_from_node_points, calc_string_from_node_points,
    calc_trim_boundary_of_code_points, create_node_point_generator,
};
pub use util::searcher::{collect_code_points_from_slice, create_code_point_searcher};
