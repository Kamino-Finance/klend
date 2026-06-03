//! Minimal zero-copy integer wrapper.
//!
//! Vendored from [`spl_pod::primitives::PodU128`] (byte-for-byte layout compatible) so that
//! `solana-zk-sdk` — which `spl-pod >= 0.4` pulls in unconditionally, and which requires
//! Rust >= 1.82 (`core::iter::repeat_n`) — stays out of this crate's dependency closure.
//! Keeping it out is what lets the published MSRV be 1.81.
//!
//! [`spl_pod::primitives::PodU128`]: https://docs.rs/spl-pod/latest/spl_pod/primitives/struct.PodU128.html

use bytemuck::{Pod, Zeroable};

/// A `u128` stored as little-endian bytes, so it can be a field inside `#[repr(C)]` `Pod` structs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
#[repr(transparent)]
pub struct PodU128(pub [u8; 16]);

impl PodU128 {
    /// Build from a native `u128` (little-endian); usable in `const` context.
    pub const fn from_primitive(n: u128) -> Self {
        Self(n.to_le_bytes())
    }
}

impl From<u128> for PodU128 {
    fn from(n: u128) -> Self {
        Self::from_primitive(n)
    }
}

impl From<PodU128> for u128 {
    fn from(pod: PodU128) -> Self {
        u128::from_le_bytes(pod.0)
    }
}
