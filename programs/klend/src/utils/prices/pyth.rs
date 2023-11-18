use std::convert::TryFrom;

use anchor_lang::prelude::*;
use pyth_sdk_solana::{load_price_feed_from_account_info, Price as PythPrice};

use super::{
    types::{Price, TimestampedPriceWithTwap},
    utils, TimestampedPrice,
};
use crate::LendingError;

pub(super) fn get_pyth_price_and_twap(
    pyth_price_info: &AccountInfo,
) -> Result<TimestampedPriceWithTwap> {
    let price_feed = load_price_feed_from_account_info(pyth_price_info).map_err(|e| {
        msg!("Error loading price pyth feed: {:?}", e);
        error!(LendingError::PriceNotValid)
    })?;

    let price = price_feed.get_price_unchecked();
    let twap = price_feed.get_ema_price_unchecked();

    validate_pyth_confidence(&price, super::CONFIDENCE_FACTOR)?;

    Ok(TimestampedPriceWithTwap {
        price: price.into(),
        twap: Some(twap.into()),
    })
}

pub(super) fn validate_pyth_confidence(
    pyth_price: &PythPrice,
    oracle_confidence_factor: u64,
) -> Result<()> {
    let price = u64::try_from(pyth_price.price).unwrap();
    if price == 0 {
        return err!(LendingError::PriceIsZero);
    }
    let conf: u64 = pyth_price.conf;
    let conf_50x: u64 = conf.checked_mul(oracle_confidence_factor).unwrap();
    if conf_50x > price {
        msg!(
            "Confidence interval check failed on pyth account {} {} {}",
            conf,
            price,
            oracle_confidence_factor,
        );
        return err!(LendingError::PriceConfidenceTooWide);
    };
    Ok(())
}

impl From<PythPrice> for TimestampedPrice {
    fn from(pyth_price: PythPrice) -> Self {
        let value = u64::try_from(pyth_price.price).unwrap();
        let exp = pyth_price.expo.checked_abs().unwrap() as u32;

        let price = Price { value, exp };

        let timestamp = pyth_price.publish_time.try_into().unwrap();

        let price_load = Box::new(move || Ok(utils::price_to_fraction(price)));

        TimestampedPrice {
            price_load,
            timestamp,
        }
    }
}
