#[allow(non_camel_case_types)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum UnicodeZsCodePoint {
    SPACE = 0x00020,
    NO_BREAK_SPACE = 0x000a0,
    OGHAM_SPACE_MARK = 0x01680,
    EN_QUAD = 0x02000,
    EM_QUAD = 0x02001,
    EN_SPACE = 0x02002,
    EM_SPACE = 0x02003,
    THREE_PER_EM_SPACE = 0x02004,
    FOUR_PER_EM_SPACE = 0x02005,
    SIX_PER_EM_SPACE = 0x02006,
    FIGURE_SPACE = 0x02007,
    PUNCTUATION_SPACE = 0x02008,
    THIN_SPACE = 0x02009,
    HAIR_SPACE = 0x0200a,
    NARROW_NO_BREAK_SPACE = 0x0202f,
    MEDIUM_MATHEMATICAL_SPACE = 0x0205f,
    IDEOGRAPHIC_SPACE = 0x03000,
}

impl UnicodeZsCodePoint {
    pub const VALUES: &'static [i32] = &[
        Self::SPACE as i32,
        Self::NO_BREAK_SPACE as i32,
        Self::OGHAM_SPACE_MARK as i32,
        Self::EN_QUAD as i32,
        Self::EM_QUAD as i32,
        Self::EN_SPACE as i32,
        Self::EM_SPACE as i32,
        Self::THREE_PER_EM_SPACE as i32,
        Self::FOUR_PER_EM_SPACE as i32,
        Self::SIX_PER_EM_SPACE as i32,
        Self::FIGURE_SPACE as i32,
        Self::PUNCTUATION_SPACE as i32,
        Self::THIN_SPACE as i32,
        Self::HAIR_SPACE as i32,
        Self::NARROW_NO_BREAK_SPACE as i32,
        Self::MEDIUM_MATHEMATICAL_SPACE as i32,
        Self::IDEOGRAPHIC_SPACE as i32,
    ];
}
