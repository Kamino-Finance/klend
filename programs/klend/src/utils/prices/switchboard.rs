use anchor_lang::{
    err, error,
    prelude::{msg, AccountInfo},
    Result,
};
use switchboard_v2::AggregatorAccountData;

use super::{utils::price_to_fraction, CONFIDENCE_FACTOR};
use crate::{
    utils::{
        prices::types::{TimestampedPrice, TimestampedPriceWithTwap},
        NULL_PUBKEY,
    },
    LendingError,
};

pub(super) fn get_switchboard_price_and_twap(
    switchboard_price_feed_info: &AccountInfo,
    switchboard_twap_feed_info: Option<&AccountInfo>,
) -> Result<TimestampedPriceWithTwap> {
    let price = get_switchboard_price(switchboard_price_feed_info)?;
    let twap = switchboard_twap_feed_info
        .as_ref()
        .map(|account| get_switchboard_price(account))
        .transpose()?;
    Ok(TimestampedPriceWithTwap { price, twap })
}

fn get_switchboard_price(switchboard_feed_info: &AccountInfo) -> Result<TimestampedPrice> {
    if *switchboard_feed_info.key == NULL_PUBKEY {
        return err!(LendingError::NoPriceFound);
    }
    let switchboard_data = &switchboard_feed_info.try_borrow_data()?;
    let feed = AggregatorAccountData::new_from_bytes(switchboard_data)?;
    let timestamp = u64::try_from(feed.latest_confirmed_round.round_open_timestamp).unwrap();

    let price_switchboard_desc = feed.get_result()?;

    if price_switchboard_desc.mantissa <= 0 {
        msg!("Switchboard oracle price is negative which is not allowed");
        return err!(LendingError::PriceIsZero);
    }

    let stdev_mantissa = feed.latest_confirmed_round.std_deviation.mantissa;
    let stdev_scale = feed.latest_confirmed_round.std_deviation.scale;

    let price_load = Box::new(move || {
        validate_switchboard_confidence(
            price_switchboard_desc.mantissa,
            price_switchboard_desc.scale,
            stdev_mantissa,
            stdev_scale,
            CONFIDENCE_FACTOR,
        )?;

        let base_value = u128::try_from(price_switchboard_desc.mantissa).map_err(|_| {
            msg!("Switchboard oracle price is negative which is not allowed");
            error!(LendingError::InvalidOracleConfig)
        })?;

        let base_price = super::Price {
            value: base_value,
            exp: price_switchboard_desc.scale,
        };

        Ok(price_to_fraction(base_price))
    });

    Ok(TimestampedPrice {
        price_load,
        timestamp,
    })
}

fn validate_switchboard_confidence(
    price_mantissa: i128,
    price_scale: u32,
    stdev_mantissa: i128,
    stdev_scale: u32,
    oracle_confidence_factor: u64,
) -> Result<()> {
    let (scale_op, scale_diff): (&dyn Fn(i128, i128) -> Option<i128>, _) =
        if price_scale >= stdev_scale {
            (
                &i128::checked_mul,
                price_scale.checked_sub(stdev_scale).unwrap(),
            )
        } else {
            (
                &i128::checked_div,
                stdev_scale.checked_sub(price_scale).unwrap(),
            )
        };

    let scaling_factor = 10_i128
        .checked_pow(scale_diff)
        .ok_or_else(|| error!(LendingError::MathOverflow))?;

    let stdev_x_confidence_factor_scaled = stdev_mantissa
        .checked_mul(oracle_confidence_factor.into())
        .and_then(|a| scale_op(a, scaling_factor))
        .ok_or_else(|| error!(LendingError::MathOverflow))?;

    if stdev_x_confidence_factor_scaled >= price_mantissa {
        msg!(
            "Validation of confidence interval for switchboard v2 feed failed.\n\
             Price mantissa: {price_mantissa}, Price scale: {price_scale}\n\
             stdev mantissa: {stdev_mantissa}, stdev_scale: {stdev_scale}",
        );
        err!(LendingError::PriceConfidenceTooWide)
    } else {
        Ok(())
    }
}
