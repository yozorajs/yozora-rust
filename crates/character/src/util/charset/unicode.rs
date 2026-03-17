use crate::constant::ascii::AsciiCodePoint;
use crate::constant::unicode_sets::UNICODE_ZS;
use crate::constant::virtual_code_point::VirtualCodePoint;
use crate::types::CodePoint;

pub fn unicode_whitespace_characters() -> Vec<CodePoint> {
    let mut values = vec![
        AsciiCodePoint::HT as i32,
        AsciiCodePoint::LF as i32,
        AsciiCodePoint::FF as i32,
        AsciiCodePoint::CR as i32,
        VirtualCodePoint::Space as i32,
        VirtualCodePoint::LineEnd as i32,
    ];
    values.extend_from_slice(UNICODE_ZS);
    values.sort_unstable();
    values.dedup();
    values
}

pub fn is_unicode_whitespace_character(code_point: CodePoint) -> bool {
    if code_point == VirtualCodePoint::Space as i32
        || code_point == VirtualCodePoint::LineEnd as i32
    {
        return true;
    }

    if code_point == AsciiCodePoint::HT as i32
        || code_point == AsciiCodePoint::LF as i32
        || code_point == AsciiCodePoint::FF as i32
        || code_point == AsciiCodePoint::CR as i32
    {
        return true;
    }

    UNICODE_ZS.contains(&code_point)
}
