#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum VirtualCodePoint {
    LineEnd = -0x0001,
    Space = -0x0002,
}
