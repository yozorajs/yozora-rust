use crate::constant::ascii::AsciiCodePoint;
use crate::constant::unicode_sets::{
    UNICODE_PC, UNICODE_PD, UNICODE_PE, UNICODE_PF, UNICODE_PI, UNICODE_PO, UNICODE_PS,
};
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
    chars.extend_from_slice(UNICODE_PC);
    chars.extend_from_slice(UNICODE_PD);
    chars.extend_from_slice(UNICODE_PE);
    chars.extend_from_slice(UNICODE_PF);
    chars.extend_from_slice(UNICODE_PI);
    chars.extend_from_slice(UNICODE_PO);
    chars.extend_from_slice(UNICODE_PS);
    chars.sort_unstable();
    chars.dedup();
    chars
}

pub fn is_punctuation_character(code_point: CodePoint) -> bool {
    ascii_punctuation_characters().contains(&code_point)
        || UNICODE_PC.contains(&code_point)
        || UNICODE_PD.contains(&code_point)
        || UNICODE_PE.contains(&code_point)
        || UNICODE_PF.contains(&code_point)
        || UNICODE_PI.contains(&code_point)
        || UNICODE_PO.contains(&code_point)
        || UNICODE_PS.contains(&code_point)
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
