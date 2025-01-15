use crate::utils::{Fraction, SECONDS_PER_DAY, SECONDS_PER_HOUR};

pub fn to_days_fractional(secs: u64) -> Fraction {
    Fraction::from(secs) / u128::from(SECONDS_PER_DAY)
}

pub fn from_days(days: u64) -> u64 {
    days.checked_mul(SECONDS_PER_DAY).unwrap()
}

pub fn from_hours(hours: u64) -> u64 {
    hours.checked_mul(SECONDS_PER_HOUR).unwrap()
}
