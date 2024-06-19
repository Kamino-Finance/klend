use std::cmp::Ordering;

use anchor_lang::{prelude::*, solana_program::clock::Slot, Result};
use bitflags::bitflags;
use borsh::{BorshDeserialize, BorshSerialize};

use crate::LendingError;

pub const STALE_AFTER_SLOTS_ELAPSED: u64 = 1;

#[derive(
    BorshSerialize,
    BorshDeserialize,
    Debug,
    Default,
    PartialEq,
    Eq,
    Clone,
    Copy,
    bytemuck::Zeroable,
    bytemuck::Pod,
)]
#[repr(transparent)]
pub struct PriceStatusFlags(pub u8);

#[rustfmt::skip]
bitflags! {
    impl PriceStatusFlags: u8 {
        const PRICE_LOADED =        0b_0000_0001;
        const PRICE_AGE_CHECKED =   0b_0000_0010;
        const TWAP_CHECKED =        0b_0000_0100;
        const TWAP_AGE_CHECKED =    0b_0000_1000;
        const HEURISTIC_CHECKED =   0b_0001_0000;
        const PRICE_USAGE_ALLOWED = 0b_0010_0000;
    }
}

impl PriceStatusFlags {
    pub const ALL_CHECKS: PriceStatusFlags = PriceStatusFlags::all();

    pub const NONE: PriceStatusFlags = PriceStatusFlags::empty();

    pub const LIQUIDATION_CHECKS: PriceStatusFlags = PriceStatusFlags::PRICE_LOADED
        .union(PriceStatusFlags::PRICE_AGE_CHECKED)
        .union(PriceStatusFlags::PRICE_USAGE_ALLOWED);
}

#[derive(BorshDeserialize, BorshSerialize, Debug)]
#[zero_copy]
#[repr(C)]
pub struct LastUpdate {
    slot: u64,
    stale: u8,
    price_status: u8,

    placeholder: [u8; 6],
}

impl Default for LastUpdate {
    fn default() -> Self {
        Self::new(0)
    }
}

impl LastUpdate {
    pub fn new(slot: Slot) -> Self {
        Self {
            slot,
            stale: true as u8,
            price_status: PriceStatusFlags::empty().0,
            placeholder: [0; 6],
        }
    }

    pub fn slots_elapsed(&self, slot: Slot) -> Result<u64> {
        let slots_elapsed = slot
            .checked_sub(self.slot)
            .ok_or_else(|| error!(LendingError::MathOverflow))?;
        Ok(slots_elapsed)
    }

    pub fn update_slot(&mut self, slot: Slot, price_status: impl Into<Option<PriceStatusFlags>>) {
        let price_status: Option<PriceStatusFlags> = price_status.into();
        self.slot = slot;
        self.stale = false as u8;
        if let Some(price_status) = price_status {
            self.price_status = price_status.bits();
        }
    }

    pub fn mark_stale(&mut self) {
        self.stale = true as u8;
    }

    pub fn is_stale(&self, slot: Slot, min_price_status: PriceStatusFlags) -> Result<bool> {
        let is_price_status_ok = self.get_price_status().contains(min_price_status);
        Ok(self.stale != (false as u8)
            || self.slots_elapsed(slot)? >= STALE_AFTER_SLOTS_ELAPSED
            || !is_price_status_ok)
    }

    pub fn get_price_status(&self) -> PriceStatusFlags {
        PriceStatusFlags::from_bits_truncate(self.price_status)
    }
}

impl PartialEq for LastUpdate {
    fn eq(&self, other: &Self) -> bool {
        self.slot == other.slot
    }
}

impl PartialOrd for LastUpdate {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.slot.partial_cmp(&other.slot)
    }
}
