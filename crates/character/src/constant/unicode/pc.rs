#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum UnicodePcCodePoint {
    LOW_LINE = 0x005f,
    UNDERTIE = 0x203f,
    CHARACTER_TIE = 0x2040,
    INVERTED_UNDERTIE = 0x2054,
    PRESENTATION_FORM_FOR_VERTICAL_LOW_LINE = 0xfe33,
    PRESENTATION_FORM_FOR_VERTICAL_WAVY_LOW_LINE = 0xfe34,
    DASHED_LOW_LINE = 0xfe4d,
    CENTRELINE_LOW_LINE = 0xfe4e,
    WAVY_LOW_LINE = 0xfe4f,
    FULLWIDTH_LOW_LINE = 0xff3f,
}

impl UnicodePcCodePoint {
    pub const VALUES: &'static [i32] = &[
        Self::LOW_LINE as i32,
        Self::UNDERTIE as i32,
        Self::CHARACTER_TIE as i32,
        Self::INVERTED_UNDERTIE as i32,
        Self::PRESENTATION_FORM_FOR_VERTICAL_LOW_LINE as i32,
        Self::PRESENTATION_FORM_FOR_VERTICAL_WAVY_LOW_LINE as i32,
        Self::DASHED_LOW_LINE as i32,
        Self::CENTRELINE_LOW_LINE as i32,
        Self::WAVY_LOW_LINE as i32,
        Self::FULLWIDTH_LOW_LINE as i32,
    ];
}
