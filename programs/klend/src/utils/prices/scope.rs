use std::{cell::Ref, convert::TryInto};

use anchor_lang::{__private::bytemuck, prelude::*, Discriminator};
pub use scope::OraclePrices as ScopePrices;

use super::{
    types::{TimestampedPrice, TimestampedPriceWithTwap},
    utils::price_to_fraction,
};
use crate::{
    dbg_msg,
    utils::{prices::Price, NULL_PUBKEY, U128},
    LendingError, Result, ScopeConfiguration,
};

pub(super) fn get_scope_price_and_twap(
    scope_price_account: &AccountInfo,
    conf: &ScopeConfiguration,
) -> Result<TimestampedPriceWithTwap> {
    let scope_prices = get_price_account(scope_price_account)?;
    let price = get_price_usd(&scope_prices, conf.price_chain)?;
    let twap = if conf.has_twap() {
        get_price_usd(&scope_prices, conf.twap_chain)
            .map_err(|e| msg!("No valid twap found for scope price, error: {:?}", e))
            .ok()
    } else {
        None
    };
    Ok(TimestampedPriceWithTwap { price, twap })
}

type ScopePriceId = u16;

type ScopeConversionChain = [ScopePriceId; 4];

impl From<scope::Price> for Price<u64> {
    fn from(price: scope::Price) -> Self {
        Self {
            value: price.value,
            exp: price.exp.try_into().unwrap(),
        }
    }
}

impl From<Price<u64>> for scope::Price {
    fn from(val: Price<u64>) -> Self {
        Self {
            value: val.value,
            exp: val.exp.into(),
        }
    }
}

fn get_price_account<'a>(scope_price_account: &'a AccountInfo) -> Result<Ref<'a, ScopePrices>> {
    if *scope_price_account.key == NULL_PUBKEY {
        return Err(LendingError::InvalidOracleConfig.into());
    }

    let data = scope_price_account.try_borrow_data()?;

    let disc_bytes = &data[0..8];
    if disc_bytes != ScopePrices::discriminator() {
        return Err(LendingError::CouldNotDeserializeScope.into());
    }

    Ok(Ref::map(data, |data| bytemuck::from_bytes(&data[8..])))
}

fn get_price_usd(
    scope_prices: &ScopePrices,
    tokens_chain: ScopeConversionChain,
) -> Result<TimestampedPrice> {
    if tokens_chain == [0, 0, 0, 0] {
        msg!("Scope chain is not initialized properly");
        return err!(LendingError::PriceNotValid);
    }
    let price_chain_raw = tokens_chain.map(|token_id| get_base_price(scope_prices, token_id));

    let chain_len = price_chain_raw.iter().take_while(|v| v.is_some()).count();

    if chain_len == 0 {
        msg!("Scope chain is empty");
        return err!(LendingError::NoPriceFound);
    }

    let price_chain = &price_chain_raw[..chain_len];

    if chain_len == 1 {
        let price = price_chain[0].unwrap();
        let price_load = Box::new(move || Ok(price_to_fraction(price.0)));
        return Ok(TimestampedPrice {
            price_load,
            timestamp: price.1,
        });
    }

    let oldest_timestamp = price_chain.iter().flatten().map(|x| x.1).min().unwrap();

    let price_load = Box::new(move || {
        let price_chain = &price_chain_raw[..chain_len];
        let total_decimals: u32 = price_chain
            .iter()
            .flatten()
            .try_fold(0u32, |acc, price| acc.checked_add(price.0.exp))
            .ok_or_else(|| dbg_msg!(LendingError::MathOverflow))?;

        let product = price_chain
            .iter()
            .flatten()
            .try_fold(U128::from(1u128), |acc, price| {
                acc.checked_mul(price.0.value.into())
            })
            .ok_or_else(|| dbg_msg!(LendingError::MathOverflow))?;

        let base_price = Price {
            value: product,
            exp: total_decimals,
        };

        Ok(price_to_fraction(base_price))
    });

    Ok(TimestampedPrice {
        price_load,
        timestamp: oldest_timestamp,
    })
}

fn get_base_price(scope_prices: &ScopePrices, token: ScopePriceId) -> Option<(Price<u64>, u64)> {
    scope_prices
        .prices
        .get(usize::from(token))
        .map(|price| (price.price.into(), price.unix_timestamp))
}
