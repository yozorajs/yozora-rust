pub mod pc;
pub mod pd;
pub mod pe;
pub mod pf;
pub mod pi;
pub mod po;
pub mod ps;
pub mod zs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum UnicodeCodePoint {
    ReplacementCharacter = 0x0fffd,
}

impl UnicodeCodePoint {
    pub const REPLACEMENT_CHARACTER: i32 = Self::ReplacementCharacter as i32;
}
