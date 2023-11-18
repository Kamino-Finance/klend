mod checks;
mod pyth;
mod scope;
mod switchboard;
mod types;
mod utils;

use anchor_lang::{prelude::*, solana_program::clock};
use types::{Price, TimestampedPrice};

use self::{
    checks::get_validated_price, pyth::get_pyth_price_and_twap, scope::get_scope_price_and_twap,
    switchboard::get_switchboard_price_and_twap, types::TimestampedPriceWithTwap,
};
use crate::{utils::Fraction, LendingError, TokenInfo};

const MAX_CONFIDENCE_PERCENTAGE: u64 = 2u64;

const CONFIDENCE_FACTOR: u64 = 100 / MAX_CONFIDENCE_PERCENTAGE;

pub fn get_price(
    token_info: &TokenInfo,
    pyth_price_account_info: Option<&AccountInfo>,
    switchboard_price_feed_info: Option<&AccountInfo>,
    switchboard_price_twap_info: Option<&AccountInfo>,
    scope_prices_info: Option<&AccountInfo>,
    unix_timestamp: clock::UnixTimestamp,
) -> Result<(Fraction, u64)> {
    let price = get_most_recent_price_and_twap(
        token_info,
        pyth_price_account_info,
        switchboard_price_feed_info,
        switchboard_price_twap_info,
        scope_prices_info,
    )?;

    get_validated_price(price, token_info, unix_timestamp)
}

fn get_most_recent_price_and_twap(
    token_info: &TokenInfo,
    pyth_price_account_info: Option<&AccountInfo>,
    switchboard_price_feed_info: Option<&AccountInfo>,
    switchboard_price_twap_info: Option<&AccountInfo>,
    scope_prices_info: Option<&AccountInfo>,
) -> Result<TimestampedPriceWithTwap> {
    let pyth_price = if token_info.pyth_configuration.is_enabled() {
        pyth_price_account_info.and_then(|a| get_pyth_price_and_twap(a).ok())
    } else {
        None
    };

    let switchboard_price_twap_info_opt = if token_info.is_twap_enabled() {
        switchboard_price_twap_info
    } else {
        None
    };

    let switchboard_price = if token_info.switchboard_configuration.is_enabled() {
        switchboard_price_feed_info
            .and_then(|a| get_switchboard_price_and_twap(a, switchboard_price_twap_info_opt).ok())
    } else {
        None
    };

    let scope_price = if token_info.scope_configuration.is_enabled() {
        scope_prices_info
            .and_then(|a| get_scope_price_and_twap(a, &token_info.scope_configuration).ok())
    } else {
        None
    };

    let most_recent_price = [pyth_price, switchboard_price, scope_price]
        .into_iter()
        .flatten()
        .reduce(|current, candidate| {
            if candidate.price.timestamp > current.price.timestamp {
                candidate
            } else {
                current
            }
        });

    most_recent_price.ok_or_else(|| {
        msg!("No price feed available");
        error!(LendingError::PriceNotValid)
    })
}
