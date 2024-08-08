use anchor_lang::{prelude::*, Accounts};
use farms::program::Farms;

use crate::{
    lending_market::farms_ixs,
    state::{obligation::Obligation, LendingMarket},
    utils::{seeds, PROGRAM_VERSION},
    LendingError, Reserve, ReserveStatus,
};

pub fn process(ctx: Context<InitObligationFarmsForReserve>, mode: u8) -> Result<()> {
    let reserve = &ctx.accounts.reserve.load()?;
    let obligation = &ctx.accounts.obligation.to_account_info().key;

    require!(
        reserve.config.status() != ReserveStatus::Obsolete,
        LendingError::ReserveObsolete
    );

    require!(
        reserve.version == PROGRAM_VERSION as u64,
        LendingError::ReserveDeprecated
    );

    let farm = reserve.get_farm(mode.try_into().unwrap());
    require!(
        farm == ctx.accounts.reserve_farm_state.key(),
        LendingError::InvalidAccountInput
    );

    farms_ixs::cpi_initialize_farmer_delegated(&ctx, obligation, farm)?;

    Ok(())
}

#[derive(Accounts)]
pub struct InitObligationFarmsForReserve<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,

    pub owner: AccountInfo<'info>,

    #[account(
        mut,
        has_one = lending_market,
        has_one = owner
    )]
    pub obligation: AccountLoader<'info, Obligation>,

    #[account(
        mut,
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(
        mut,
        has_one = lending_market
    )]
    pub reserve: AccountLoader<'info, Reserve>,

    #[account(mut)]
    pub reserve_farm_state: AccountInfo<'info>,

    #[account(mut)]
    pub obligation_farm: AccountInfo<'info>,

    pub lending_market: AccountLoader<'info, LendingMarket>,

    pub farms_program: Program<'info, Farms>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}
