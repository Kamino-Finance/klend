use anchor_lang::prelude::*;

use crate::utils::{Fraction, U128};

#[derive(Clone, Copy, Debug, AnchorDeserialize, AnchorSerialize, Default)]
pub(crate) struct Price<T>
where
    T: Into<U128>,
{
    pub value: T,

    pub exp: u32,
}

pub(super) struct TimestampedPrice {
    pub price_load: Box<dyn FnOnce() -> Result<Fraction>>,
    pub timestamp: u64,
}

pub(super) struct TimestampedPriceWithTwap {
    pub price: TimestampedPrice,
    pub twap: Option<TimestampedPrice>,
}
