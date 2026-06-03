use bytemuck::{Pod, Zeroable};
use solana_pubkey::Pubkey;
use spl_discriminator::SplDiscriminate;

use super::pod::PodU128;

/// Per-referrer, per-reserve fee tracking account.
#[derive(Debug, Clone, Copy, Pod, Zeroable, SplDiscriminate)]
#[discriminator_hash_input("account:ReferrerTokenState")]
#[repr(C)]
pub struct ReferrerTokenState {
    pub referrer: Pubkey,
    pub mint: Pubkey,
    /// Unclaimed fees (scaled fraction).
    pub amount_unclaimed_sf: PodU128,
    /// Total accumulated fees (scaled fraction).
    pub amount_cumulative_sf: PodU128,
    pub bump: u64,
    pub padding: [u64; 31],
}

const _: () = assert!(core::mem::size_of::<ReferrerTokenState>() == 352);

impl ReferrerTokenState {
    /// Unclaimed referrer fees as a raw u128 scaled fraction.
    pub fn amount_unclaimed(&self) -> u128 {
        u128::from(self.amount_unclaimed_sf)
    }

    /// Total accumulated referrer fees as a raw u128 scaled fraction.
    pub fn amount_cumulative(&self) -> u128 {
        u128::from(self.amount_cumulative_sf)
    }
}

/// User metadata — links a wallet to its referrer.
#[derive(Debug, Clone, Copy, Zeroable, Pod, SplDiscriminate)]
#[discriminator_hash_input("account:UserMetadata")]
#[repr(C)]
pub struct UserMetadata {
    /// Referrer address (`Pubkey::default()` = no referrer).
    pub referrer: Pubkey,
    pub bump: u64,
    pub user_lookup_table: Pubkey,
    pub owner: Pubkey,
    pub padding_1: [u64; 51],
    pub padding_2: [u64; 64],
}

const _: () = assert!(core::mem::size_of::<UserMetadata>() == 1024);

impl UserMetadata {
    /// Returns the referrer, or `None` if no referrer is set.
    pub fn referrer(&self) -> Option<Pubkey> {
        if self.referrer == Pubkey::default() {
            None
        } else {
            Some(self.referrer)
        }
    }
}

/// Referrer state — maps a referrer to their short URL.
#[derive(Debug, Clone, Copy, Zeroable, Pod, SplDiscriminate)]
#[discriminator_hash_input("account:ReferrerState")]
#[repr(C)]
pub struct ReferrerState {
    pub short_url: Pubkey,
    pub owner: Pubkey,
}

const _: () = assert!(core::mem::size_of::<ReferrerState>() == 64);
