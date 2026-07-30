use anchor_lang::prelude::*;

use crate::{
    state::{PriceStatusFlags, Reserve},
    utils::{constraints, COLLATERAL_MINT_DECIMALS, PROGRAM_VERSION},
    LendingError,
};



const ONE_CTOKEN_LAMPORTS: u64 = 10u64.pow(COLLATERAL_MINT_DECIMALS as u32);










#[derive(AnchorSerialize, AnchorDeserialize, Debug, Clone, PartialEq, Eq)]
pub struct ExchangeRateWithDecimals {
    pub exchange_rate_sf: u128,
    pub mint_decimals: u8,
}
















pub fn process(ctx: Context<CalculateCTokenExchangeRate>) -> Result<ExchangeRateWithDecimals> {
    let clock = Clock::get()?;

    constraints::check_remaining_accounts(&ctx)?;

    let reserve = ctx.accounts.reserve.load()?;

    require!(
        reserve.version == PROGRAM_VERSION as u64,
        LendingError::ReserveDeprecated
    );

    require!(
        !reserve
            .last_update
            .is_stale(clock.slot, PriceStatusFlags::NONE)?,
        LendingError::ReserveStale
    );

   
    let exchange_rate = reserve.collateral_exchange_rate();
    let liquidity_amount =
        exchange_rate.fraction_collateral_to_liquidity(ONE_CTOKEN_LAMPORTS.into());

    Ok(ExchangeRateWithDecimals {
        exchange_rate_sf: liquidity_amount.to_bits(),
        mint_decimals: u8::try_from(reserve.liquidity.mint_decimals)
            .map_err(|_| error!(LendingError::IntegerOverflow))?,
    })
}

#[derive(Accounts)]
pub struct CalculateCTokenExchangeRate<'info> {
    pub reserve: AccountLoader<'info, Reserve>,
}
