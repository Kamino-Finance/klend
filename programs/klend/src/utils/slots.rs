use crate::utils::{Fraction, SLOTS_PER_DAY, SLOTS_PER_HOUR, SLOTS_PER_MINUTE, SLOTS_PER_SECOND};

pub fn to_minutes(slots: u64) -> u64 {
    slots.checked_div(SLOTS_PER_MINUTE).unwrap()
}

pub fn to_secs(slots: u64) -> u64 {
    slots.checked_div(SLOTS_PER_SECOND).unwrap()
}

pub fn to_hours(slots: u64) -> u64 {
    slots.checked_div(SLOTS_PER_HOUR).unwrap()
}

pub fn to_days_fractional(slots: u64) -> Fraction {
    Fraction::from(slots) / u128::from(SLOTS_PER_DAY)
}

pub fn from_secs(seconds: u64) -> u64 {
    seconds.checked_mul(SLOTS_PER_SECOND).unwrap()
}

pub fn from_hours(hours: u64) -> u64 {
    hours.checked_mul(SLOTS_PER_HOUR).unwrap()
}

pub fn from_days(days: u64) -> u64 {
    days.checked_mul(SLOTS_PER_DAY).unwrap()
}
