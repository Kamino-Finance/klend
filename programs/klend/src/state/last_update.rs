use std::cmp::Ordering;

use anchor_lang::{prelude::*, solana_program::clock::Slot, Result};

use crate::LendingError;

pub const STALE_AFTER_SLOTS_ELAPSED: u64 = 1;

#[derive(AnchorSerialize, AnchorDeserialize, Debug, Default)]
#[zero_copy]
#[repr(C)]
pub struct LastUpdate {
    pub slot: u64,
    pub stale: u8,
    pub placeholder: [u8; 7],
}

impl LastUpdate {
    pub fn new(slot: Slot) -> Self {
        Self {
            slot,
            stale: true as u8,
            placeholder: [0; 7],
        }
    }

    pub fn slots_elapsed(&self, slot: Slot) -> Result<u64> {
        let slots_elapsed = slot
            .checked_sub(self.slot)
            .ok_or_else(|| error!(LendingError::MathOverflow))?;
        Ok(slots_elapsed)
    }

    pub fn update_slot(&mut self, slot: Slot) {
        self.slot = slot;
        self.stale = false as u8;
    }

    pub fn mark_stale(&mut self) {
        self.stale = true as u8;
    }

    pub fn is_stale(&self, slot: Slot) -> Result<bool> {
        Ok(self.is_marked_stale() || self.slots_elapsed(slot)? >= STALE_AFTER_SLOTS_ELAPSED)
    }

    pub fn is_marked_stale(&self) -> bool {
        self.stale != (false as u8)
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
