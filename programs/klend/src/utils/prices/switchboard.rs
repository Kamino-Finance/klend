use anchor_lang::{
    err, error,
    prelude::{msg, AccountInfo},
    Result,
};
use sbod_itf::accounts::PullFeedAccountData;
use solana_program::clock::{Clock, DEFAULT_MS_PER_SLOT};

use crate::{
    utils::{
        prices::{
            types::{TimestampedPrice, TimestampedPriceWithTwap},
            utils::price_to_fraction,
            CONFIDENCE_FACTOR,
        },
        FatAccountLoader, NULL_PUBKEY,
    },
    LendingError,
};

pub(super) fn get_switchboard_price_and_twap(
    switchboard_price_feed_info: &AccountInfo,
    switchboard_twap_feed_info: Option<&AccountInfo>,
    clock: &Clock,
) -> Result<TimestampedPriceWithTwap> {
    let price = get_switchboard_price(switchboard_price_feed_info, clock)?;
    let twap = switchboard_twap_feed_info
        .as_ref()
        .map(|account| get_switchboard_price(account, clock))
        .transpose()?;
    Ok(TimestampedPriceWithTwap { price, twap })
}

fn get_switchboard_price(
    switchboard_feed_info: &AccountInfo,
    clock: &Clock,
) -> Result<TimestampedPrice> {
    if *switchboard_feed_info.key == NULL_PUBKEY {
        return err!(LendingError::NoPriceFound);
    }
    let feed_acc: FatAccountLoader<'_, PullFeedAccountData> =
        FatAccountLoader::try_from(switchboard_feed_info)?;
    let feed = feed_acc.load()?;

   
   
    let last_updated_slot = feed.result.slot;

   
    let elapsed_slots = clock.slot.saturating_sub(last_updated_slot);
    let timestamp = u64::try_from(clock.unix_timestamp)
        .unwrap_or(0)
        .saturating_sub(elapsed_slots * DEFAULT_MS_PER_SLOT / 1000);

    let price_switchboard_desc = feed
        .result
        .value()
        .ok_or(error!(LendingError::SwitchboardV2Error))?;

    if price_switchboard_desc.mantissa() <= 0 {
        msg!("Switchboard oracle price is zero or negative which is not allowed");
        return err!(LendingError::PriceIsZero);
    }
    let price_switchboard_desc_mantissa = u128::try_from(price_switchboard_desc.mantissa())
        .expect("a `<= 0` check above guarantees this");
    let price_switchboard_desc_scale = price_switchboard_desc.scale();

    let stdev = feed
        .result
        .std_dev()
        .ok_or(error!(LendingError::SwitchboardV2Error))?;
    let stdev_mantissa = u128::try_from(stdev.mantissa()).map_err(|_| {
        msg!("Switchboard standard deviation is negative which is against its math definition");
        error!(LendingError::SwitchboardV2Error)
    })?;
    let stdev_scale = stdev.scale();

    let price_load = Box::new(move || {
        validate_switchboard_confidence(
            price_switchboard_desc_mantissa,
            price_switchboard_desc_scale,
            stdev_mantissa,
            stdev_scale,
            CONFIDENCE_FACTOR,
        )?;

        let base_price = super::Price {
            value: price_switchboard_desc_mantissa,
            exp: price_switchboard_desc_scale,
        };

        Ok(price_to_fraction(base_price))
    });

    Ok(TimestampedPrice {
        price_load,
        timestamp,
    })
}

fn validate_switchboard_confidence(
    price_mantissa: u128,
    price_scale: u32,
    stdev_mantissa: u128,
    stdev_scale: u32,
    oracle_confidence_factor: u64,
) -> Result<()> {
   
    let (scale_op, scale_diff): (&dyn Fn(u128, u128) -> Option<u128>, _) =
        if price_scale >= stdev_scale {
            (
                &u128::checked_mul,
                price_scale.checked_sub(stdev_scale).unwrap(),
            )
        } else {
            (
                &u128::checked_div,
                stdev_scale.checked_sub(price_scale).unwrap(),
            )
        };

    let scaling_factor = 10_u128
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

