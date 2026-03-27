use anchor_lang::Result;

use crate::{
    utils::{Fraction, FractionExtra, SECONDS_PER_DAY, SECONDS_PER_HOUR, SLOTS_PER_SECOND},
    LendingError,
};

pub fn to_days_fractional(secs: u64) -> Fraction {
    Fraction::from(secs) / u128::from(SECONDS_PER_DAY)
}

pub fn from_days(days: u64) -> u64 {
    days.checked_mul(SECONDS_PER_DAY).unwrap()
}

pub fn from_hours(hours: u64) -> u64 {
    hours.checked_mul(SECONDS_PER_HOUR).unwrap()
}

pub fn checked_secs_to_slots(secs: Fraction) -> Result<u64> {
    let slots = secs.full_mul_int_ratio_ceil(u128::from(SLOTS_PER_SECOND), 1);
    slots
        .try_to_ceil::<u64>()
        .ok_or(LendingError::MathOverflow.into())
}

pub fn estimate_slot_after_period(current_slot: u64, secs: Fraction) -> Result<u64> {
    let elapsed_slot_estimate = checked_secs_to_slots(secs)?;
    current_slot
        .checked_add(elapsed_slot_estimate)
        .ok_or(LendingError::MathOverflow.into())
}
