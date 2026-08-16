#[path = "pc.rs"]
pub mod pc;
#[path = "pd.rs"]
pub mod pd;
#[path = "pe.rs"]
pub mod pe;
#[path = "pf.rs"]
pub mod pf;
#[path = "pi.rs"]
pub mod pi;
#[path = "po.rs"]
pub mod po;
#[path = "ps.rs"]
pub mod ps;
#[path = "zs.rs"]
pub mod zs;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum UnicodeCodePoint {
    ReplacementCharacter = 0x0fffd,
}

impl UnicodeCodePoint {
    pub const REPLACEMENT_CHARACTER: i32 = Self::ReplacementCharacter as i32;
}
