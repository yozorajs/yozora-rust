use crate::constant::ascii::AsciiCodePoint;
use crate::constant::unicode::pc::UnicodePcCodePoint;
use crate::constant::unicode::pd::UnicodePdCodePoint;
use crate::constant::unicode::pe::UnicodePeCodePoint;
use crate::constant::unicode::pf::UnicodePfCodePoint;
use crate::constant::unicode::pi::UnicodePiCodePoint;
use crate::constant::unicode::po::UnicodePoCodePoint;
use crate::constant::unicode::ps::UnicodePsCodePoint;
use crate::constant::virtual_code_point::VirtualCodePoint;
use crate::types::CodePoint;
use crate::util::charset::ascii::{
    ascii_control_characters, ascii_punctuation_characters, is_ascii_control_character,
};

pub fn whitespace_characters() -> Vec<CodePoint> {
    vec![
        AsciiCodePoint::VT as i32,
        AsciiCodePoint::FF as i32,
        AsciiCodePoint::SPACE as i32,
        VirtualCodePoint::Space as i32,
        VirtualCodePoint::LineEnd as i32,
    ]
}

pub fn is_whitespace_character(code_point: CodePoint) -> bool {
    matches!(
        code_point,
        x if x == AsciiCodePoint::VT as i32
            || x == AsciiCodePoint::FF as i32
            || x == AsciiCodePoint::SPACE as i32
            || x == VirtualCodePoint::Space as i32
            || x == VirtualCodePoint::LineEnd as i32
    )
}

pub fn space_characters() -> Vec<CodePoint> {
    vec![AsciiCodePoint::SPACE as i32, VirtualCodePoint::Space as i32]
}

pub fn is_space_character(code_point: CodePoint) -> bool {
    code_point == AsciiCodePoint::SPACE as i32 || code_point == VirtualCodePoint::Space as i32
}

pub fn is_line_ending(code_point: CodePoint) -> bool {
    code_point == VirtualCodePoint::LineEnd as i32
}

pub fn punctuation_characters() -> Vec<CodePoint> {
    let mut chars = ascii_punctuation_characters().to_vec();
    chars.extend_from_slice(UnicodePcCodePoint::VALUES);
    chars.extend_from_slice(UnicodePdCodePoint::VALUES);
    chars.extend_from_slice(UnicodePeCodePoint::VALUES);
    chars.extend_from_slice(UnicodePfCodePoint::VALUES);
    chars.extend_from_slice(UnicodePiCodePoint::VALUES);
    chars.extend_from_slice(UnicodePoCodePoint::VALUES);
    chars.extend_from_slice(UnicodePsCodePoint::VALUES);
    chars.sort_unstable();
    chars.dedup();
    chars
}

pub fn is_punctuation_character(code_point: CodePoint) -> bool {
    ascii_punctuation_characters().contains(&code_point)
        || UnicodePcCodePoint::VALUES.contains(&code_point)
        || UnicodePdCodePoint::VALUES.contains(&code_point)
        || UnicodePeCodePoint::VALUES.contains(&code_point)
        || UnicodePfCodePoint::VALUES.contains(&code_point)
        || UnicodePiCodePoint::VALUES.contains(&code_point)
        || UnicodePoCodePoint::VALUES.contains(&code_point)
        || UnicodePsCodePoint::VALUES.contains(&code_point)
}

pub fn control_characters() -> &'static [CodePoint] {
    ascii_control_characters()
}

pub fn is_control_character(code_point: CodePoint) -> bool {
    is_ascii_control_character(code_point)
}

pub fn is_space_like(code_point: CodePoint) -> bool {
    is_space_character(code_point) || is_line_ending(code_point)
}
