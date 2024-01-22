use anchor_lang::{prelude::*, solana_program::clock};

use super::{types::TimestampedPriceWithTwap, utils::price_to_fraction, Price};
use crate::{
    utils::{Fraction, FULL_BPS},
    LendingError, PriceHeuristic, TokenInfo,
};

pub(super) fn get_validated_price(
    price_and_twap: TimestampedPriceWithTwap,
    token_info: &TokenInfo,
    unix_timestamp: clock::UnixTimestamp,
) -> Result<(Fraction, u64)> {
    let unix_timestamp = u64::try_from(unix_timestamp).unwrap();

    let TimestampedPriceWithTwap { price, twap } = price_and_twap;

    let price_dec = (price.price_load)()?;

    check_price_age(
        price.timestamp,
        token_info.max_age_price_seconds,
        unix_timestamp,
    )
    .map_err(|e| {
        let price_label = token_info.symbol();
        msg!("Price is too old token=[{price_label}]",);
        e
    })?;

    if token_info.is_twap_enabled() {
        let twap = twap.ok_or_else(|| error!(LendingError::InvalidTwapPrice))?;
        check_price_age(
            twap.timestamp,
            token_info.max_age_twap_seconds,
            unix_timestamp,
        )
        .map_err(|e| {
            let price_label = token_info.symbol();
            msg!("Price twap is too old token=[{price_label}]",);
            e
        })?;

        let twap_dec = (twap.price_load)()?;
        check_twap_in_tolerance(price_dec, twap_dec, token_info)?;
    }

    check_price_heuristics(price_dec, &token_info.heuristic)?;
    Ok((price_dec, price.timestamp))
}

fn check_price_age(
    price_timestamp: u64,
    max_age_seconds: u64,
    current_timestamp: u64,
) -> Result<()> {
    let age_seconds = current_timestamp.saturating_sub(price_timestamp);
    if age_seconds > max_age_seconds {
        msg!("Price is too old age={age_seconds} max_age={max_age_seconds}",);
        err!(LendingError::PriceTooOld)
    } else {
        Ok(())
    }
}

fn is_within_tolerance(px: Fraction, twap: Fraction, acceptable_tolerance_bps: u64) -> bool {
    let abs_diff = Fraction::abs_diff(px, twap);

    let diff_bps_scaled = abs_diff * u128::from(FULL_BPS);
    let tolerance_scaled = px * u128::from(acceptable_tolerance_bps);
    diff_bps_scaled < tolerance_scaled
}

fn check_twap_in_tolerance(price: Fraction, twap: Fraction, token_info: &TokenInfo) -> Result<()> {
    let acceptable_twap_tolerance_bps = token_info.max_twap_divergence_bps;

    if !is_within_tolerance(price, twap, acceptable_twap_tolerance_bps) {
        let token_span = token_info.symbol();
        msg!(
            "Price is too far from TWAP \
              token={token_span} \
              price={price} \
              twap={twap} \
              tolerance_bps={acceptable_twap_tolerance_bps}",
        );
        return Err(LendingError::PriceTooDivergentFromTwap.into());
    }
    Ok(())
}

fn check_price_heuristics(token_price: Fraction, heuristic: &PriceHeuristic) -> Result<()> {
    if heuristic.lower > 0 {
        let lower_heuristic = Price {
            value: heuristic.lower,
            exp: heuristic.exp.try_into().unwrap(),
        };

        let lower_heuristic = price_to_fraction(lower_heuristic);

        if token_price < lower_heuristic {
            return err!(LendingError::PriceIsLowerThanHeuristic);
        }
    }

    if heuristic.upper > 0 {
        let upper_heuristic = Price {
            value: heuristic.upper,
            exp: heuristic.exp.try_into().unwrap(),
        };

        let upper_heuristic = price_to_fraction(upper_heuristic);

        if upper_heuristic < token_price {
            return err!(LendingError::PriceIsBiggerThanHeuristic);
        }
    }

    Ok(())
}
