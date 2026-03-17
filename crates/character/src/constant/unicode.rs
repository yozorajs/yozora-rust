#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum UnicodeCodePoint {
    ReplacementCharacter = 0x0fffd,
}
