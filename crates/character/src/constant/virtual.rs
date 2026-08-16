#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum VirtualCodePoint {
    LineEnd = -0x0001,
    Space = -0x0002,
}

impl VirtualCodePoint {
    pub const LINE_END: i32 = Self::LineEnd as i32;
    pub const SPACE: i32 = Self::Space as i32;
}
