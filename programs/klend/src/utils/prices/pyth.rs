use super::{
    types::{Price, TimestampedPriceWithTwap},
    utils, TimestampedPrice,
};
use crate::LendingError;
use anchor_lang::prelude::*;
use pyth_solana_receiver_sdk::price_update::Price as PythPrice;
use pyth_solana_receiver_sdk::price_update::{PriceFeedMessage, PriceUpdateV2, VerificationLevel};
use std::convert::TryFrom;

pub(super) fn get_pyth_price_and_twap(
    pyth_price_info: &AccountInfo,
) -> Result<TimestampedPriceWithTwap> {
    let price_feed = load_price_feed_from_account_info(pyth_price_info)?;

    let (price, twap) = into_pyth_price_and_twap(price_feed);

    validate_pyth_confidence(&price, super::CONFIDENCE_FACTOR)?;
    validate_pyth_confidence(&twap, super::CONFIDENCE_FACTOR)?;

    Ok(TimestampedPriceWithTwap {
        price: price.into(),
        twap: Some(twap.into()),
    })
}

fn load_price_feed_from_account_info(pyth_price_info: &AccountInfo) -> Result<PriceFeedMessage> {
    let price_update_data = pyth_price_info.data.borrow();
    let PriceUpdateV2 {
        write_authority: _,
        verification_level,
        price_message,
        posted_slot: _,
    } = PriceUpdateV2::try_deserialize(&mut price_update_data.as_ref())?;
    if !verification_level.gte(VerificationLevel::Full) {
        return err!(LendingError::PriceNotValid);
    }
    Ok(price_message)
}

fn into_pyth_price_and_twap(price_feed: PriceFeedMessage) -> (PythPrice, PythPrice) {
    let PriceFeedMessage {
        feed_id: _,
        price,
        conf,
        exponent,
        publish_time,
        prev_publish_time: _,
        ema_price,
        ema_conf,
    } = price_feed;
    (
        PythPrice {
            price,
            conf,
            exponent,
            publish_time,
        },
        PythPrice {
            price: ema_price,
            conf: ema_conf,
            exponent,
            publish_time,
        },
    )
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
    let scaled_conf: u64 = conf.checked_mul(oracle_confidence_factor).unwrap();
    if scaled_conf > price {
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
        let exp = pyth_price.exponent.checked_abs().unwrap() as u32;

        let price = Price { value, exp };

        let timestamp = pyth_price.publish_time.try_into().unwrap();

        let price_load = Box::new(move || Ok(utils::price_to_fraction(price)));

        TimestampedPrice {
            price_load,
            timestamp,
        }
    }
}
