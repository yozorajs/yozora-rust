use crate::constant::ascii::AsciiCodePoint;
use crate::types::CodePoint;

pub const ASCII_WHITESPACE_CODES: [CodePoint; 6] = [
    AsciiCodePoint::HT as i32,
    AsciiCodePoint::LF as i32,
    AsciiCodePoint::VT as i32,
    AsciiCodePoint::FF as i32,
    AsciiCodePoint::CR as i32,
    AsciiCodePoint::SPACE as i32,
];

pub const ASCII_PUNCTUATION_CODES: [CodePoint; 32] = [
    AsciiCodePoint::EXCLAMATION_MARK as i32,
    AsciiCodePoint::DOUBLE_QUOTE as i32,
    AsciiCodePoint::NUMBER_SIGN as i32,
    AsciiCodePoint::DOLLAR_SIGN as i32,
    AsciiCodePoint::PERCENT_SIGN as i32,
    AsciiCodePoint::AMPERSAND as i32,
    AsciiCodePoint::SINGLE_QUOTE as i32,
    AsciiCodePoint::OPEN_PARENTHESIS as i32,
    AsciiCodePoint::CLOSE_PARENTHESIS as i32,
    AsciiCodePoint::ASTERISK as i32,
    AsciiCodePoint::PLUS_SIGN as i32,
    AsciiCodePoint::COMMA as i32,
    AsciiCodePoint::MINUS_SIGN as i32,
    AsciiCodePoint::DOT as i32,
    AsciiCodePoint::SLASH as i32,
    AsciiCodePoint::COLON as i32,
    AsciiCodePoint::SEMICOLON as i32,
    AsciiCodePoint::OPEN_ANGLE as i32,
    AsciiCodePoint::EQUALS_SIGN as i32,
    AsciiCodePoint::CLOSE_ANGLE as i32,
    AsciiCodePoint::QUESTION_MARK as i32,
    AsciiCodePoint::AT_SIGN as i32,
    AsciiCodePoint::OPEN_BRACKET as i32,
    AsciiCodePoint::BACKSLASH as i32,
    AsciiCodePoint::CLOSE_BRACKET as i32,
    AsciiCodePoint::CARET as i32,
    AsciiCodePoint::UNDERSCORE as i32,
    AsciiCodePoint::BACKTICK as i32,
    AsciiCodePoint::OPEN_BRACE as i32,
    AsciiCodePoint::VERTICAL_SLASH as i32,
    AsciiCodePoint::CLOSE_BRACE as i32,
    AsciiCodePoint::TILDE as i32,
];

pub const ASCII_CONTROL_CODES: [CodePoint; 33] = [
    AsciiCodePoint::NUL as i32,
    AsciiCodePoint::SOH as i32,
    AsciiCodePoint::STX as i32,
    AsciiCodePoint::ETX as i32,
    AsciiCodePoint::EOT as i32,
    AsciiCodePoint::ENQ as i32,
    AsciiCodePoint::ACK as i32,
    AsciiCodePoint::BEL as i32,
    AsciiCodePoint::BS as i32,
    AsciiCodePoint::HT as i32,
    AsciiCodePoint::LF as i32,
    AsciiCodePoint::VT as i32,
    AsciiCodePoint::FF as i32,
    AsciiCodePoint::CR as i32,
    AsciiCodePoint::SO as i32,
    AsciiCodePoint::SI as i32,
    AsciiCodePoint::DLE as i32,
    AsciiCodePoint::DC1 as i32,
    AsciiCodePoint::DC2 as i32,
    AsciiCodePoint::DC3 as i32,
    AsciiCodePoint::DC4 as i32,
    AsciiCodePoint::NAK as i32,
    AsciiCodePoint::SYN as i32,
    AsciiCodePoint::ETB as i32,
    AsciiCodePoint::CAN as i32,
    AsciiCodePoint::EM as i32,
    AsciiCodePoint::SUB as i32,
    AsciiCodePoint::ESC as i32,
    AsciiCodePoint::FS as i32,
    AsciiCodePoint::GS as i32,
    AsciiCodePoint::RS as i32,
    AsciiCodePoint::US as i32,
    AsciiCodePoint::DELETE as i32,
];

pub fn ascii_whitespace_characters() -> &'static [CodePoint] {
    &ASCII_WHITESPACE_CODES
}

pub fn ascii_punctuation_characters() -> &'static [CodePoint] {
    &ASCII_PUNCTUATION_CODES
}

pub fn ascii_control_characters() -> &'static [CodePoint] {
    &ASCII_CONTROL_CODES
}

pub fn is_ascii_whitespace_character(code_point: CodePoint) -> bool {
    ASCII_WHITESPACE_CODES.contains(&code_point)
}

pub fn is_ascii_punctuation_character(code_point: CodePoint) -> bool {
    ASCII_PUNCTUATION_CODES.contains(&code_point)
}

pub fn is_ascii_digit_character(code_point: CodePoint) -> bool {
    code_point >= AsciiCodePoint::DIGIT0 as i32 && code_point <= AsciiCodePoint::DIGIT9 as i32
}

pub fn is_ascii_lower_letter(code_point: CodePoint) -> bool {
    code_point >= AsciiCodePoint::LOWERCASE_A as i32
        && code_point <= AsciiCodePoint::LOWERCASE_Z as i32
}

pub fn is_ascii_upper_letter(code_point: CodePoint) -> bool {
    code_point >= AsciiCodePoint::UPPERCASE_A as i32
        && code_point <= AsciiCodePoint::UPPERCASE_Z as i32
}

pub fn is_ascii_letter(code_point: CodePoint) -> bool {
    is_ascii_lower_letter(code_point) || is_ascii_upper_letter(code_point)
}

pub fn is_alphanumeric(code_point: CodePoint) -> bool {
    is_ascii_letter(code_point) || is_ascii_digit_character(code_point)
}

pub fn is_ascii_character(code_point: CodePoint) -> bool {
    code_point >= AsciiCodePoint::NUL as i32 && code_point <= AsciiCodePoint::DELETE as i32
}

pub fn is_ascii_control_character(code_point: CodePoint) -> bool {
    ASCII_CONTROL_CODES.contains(&code_point)
}
