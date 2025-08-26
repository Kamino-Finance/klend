use std::{cell::Ref, convert::TryInto};

use anchor_lang::{__private::bytemuck, prelude::*, Discriminator};
pub use scope::OraclePrices as ScopePrices;

use super::{
    types::{TimestampedPrice, TimestampedPriceWithTwap},
    utils::price_to_fraction,
};
use crate::{
    dbg_msg,
    utils::{prices::Price, MAX_PRICE_DECIMALS_U256, NULL_PUBKEY, TARGET_PRICE_DECIMALS, U256},
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

   
    if chain_len == 1 {
        let price = price_chain_raw[0].unwrap();
        let price_load = Box::new(move || Ok(price_to_fraction(price.0)));
        return Ok(TimestampedPrice {
            price_load,
            timestamp: price.1,
        });
    }

    let oldest_timestamp = price_chain_raw
        .iter()
        .take(chain_len)
        .flatten()
        .map(|x| x.1)
        .min()
        .unwrap();

    let init_price: Price<U256> = Price {
        value: U256::from(1_u64),
        exp: 0,
    };
    let price_load = Box::new(move || {
        let price_chain = &price_chain_raw[..chain_len];
        let base_price = price_chain
            .iter()
            .flatten()
            .map(|x| x.0.size_up::<U256>())
            .try_fold(init_price, |acc, x| {
                let (current_price, next_price) = if acc.exp + x.exp > MAX_PRICE_DECIMALS_U256 {
                    (
                        acc.reduce_exp_lossy(TARGET_PRICE_DECIMALS)?,
                        x.reduce_exp_lossy(TARGET_PRICE_DECIMALS)?,
                    )
                } else {
                    (acc, x)
                };
                let value = current_price.value.checked_mul(next_price.value)?;
                let exp = current_price.exp + next_price.exp;
                Some(Price { value, exp })
            })
            .ok_or_else(|| dbg_msg!(LendingError::MathOverflow))?;

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

