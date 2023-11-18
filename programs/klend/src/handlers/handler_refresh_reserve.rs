use anchor_lang::{prelude::*, Accounts, Result};

use crate::{
    fraction::FractionExtra,
    lending_market::lending_operations,
    state::Reserve,
    utils::{constraints, prices::get_price, PROGRAM_VERSION},
    LendingError, LendingMarket,
};

pub fn process(ctx: Context<RefreshReserve>) -> Result<()> {
    let clock = &Clock::get()?;
    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let lending_market = &ctx.accounts.lending_market.load()?;

    constraints::check_remaining_accounts(&ctx)?;

    require!(
        reserve.version == PROGRAM_VERSION as u64,
        LendingError::ReserveDeprecated
    );

    if lending_operations::is_price_refresh_needed(reserve, lending_market, clock.unix_timestamp) {
        reserve.config.token_info.validate_token_info_config(
            &ctx.accounts.pyth_oracle,
            &ctx.accounts.switchboard_price_oracle,
            &ctx.accounts.switchboard_twap_oracle,
            &ctx.accounts.scope_prices,
        )?;

        let (price, price_timestamp) = get_price(
            &reserve.config.token_info,
            ctx.accounts.pyth_oracle.as_ref(),
            ctx.accounts.switchboard_price_oracle.as_ref(),
            ctx.accounts.switchboard_twap_oracle.as_ref(),
            ctx.accounts.scope_prices.as_ref(),
            clock.unix_timestamp,
        )?;

        lending_operations::refresh_reserve_price(reserve, price, price_timestamp)?;
    }

    lending_operations::refresh_reserve_interest(
        reserve,
        clock.slot,
        lending_market.referral_fee_bps,
    )?;
    lending_operations::refresh_reserve_limit_timestamps(reserve, clock.slot)?;

    msg!(
        "Token: {} Price: {}",
        &reserve.config.token_info.symbol(),
        reserve.liquidity.get_market_price_f().to_display()
    );

    Ok(())
}

#[derive(Accounts)]
pub struct RefreshReserve<'info> {
    #[account(mut,
        has_one = lending_market,
    )]
    pub reserve: AccountLoader<'info, Reserve>,

    pub lending_market: AccountLoader<'info, LendingMarket>,

    pub pyth_oracle: Option<AccountInfo<'info>>,

    pub switchboard_price_oracle: Option<AccountInfo<'info>>,
    pub switchboard_twap_oracle: Option<AccountInfo<'info>>,

    pub scope_prices: Option<AccountInfo<'info>>,
}
