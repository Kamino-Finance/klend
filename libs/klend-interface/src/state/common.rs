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
    pub placeholder: [u8; 6],
}

/// 256-bit fraction stored as 4 × u64 limbs.
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct BigFractionBytes {
    pub value: [u64; 4],
    pub padding: [u64; 2],
}
