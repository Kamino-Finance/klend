use bytemuck::{Pod, Zeroable};

/// Last update state — tracks when an account was last refreshed.
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct LastUpdate {
    /// Last slot when updated.
    pub slot: u64,
    /// True (1) when marked stale, false (0) when slot updated.
    pub stale: u8,
    /// Price status flags (bitfield).
    pub price_status: u8,

    pub alignment_padding: [u8; 2],

    /// Wall-clock timestamp (seconds) of the last update.
    ///
    /// Note: `u32` is used here only because of space constraints; it overflows in year 2106.
    pub timestamp: u32,
}

/// 256-bit fraction stored as 4 × u64 limbs.
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct BigFractionBytes {
    pub value: [u64; 4],
    pub padding: [u64; 2],
}
