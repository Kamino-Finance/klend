use anchor_lang::{prelude::*, Accounts};
use farms::program::Farms;

use crate::{
    lending_market::farms_ixs,
    state::{LendingMarket, Reserve},
    utils::seeds,
    ReserveFarmKind,
};

pub fn process(ctx: Context<InitFarmsForReserve>, mode: u8) -> Result<()> {
    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let farm = ctx.accounts.farm_state.key();

    let mode: ReserveFarmKind = mode.try_into().unwrap();

    msg!(
        "InitFarmsForReserve Reserve {:?} mode {:?}",
        ctx.accounts.reserve.key(),
        mode
    );

    reserve.add_farm(&farm, mode);

    farms_ixs::cpi_initialize_farm_delegated(&ctx)?;

    Ok(())
}

#[derive(Accounts)]
pub struct InitFarmsForReserve<'info> {
    #[account(mut)]
    pub lending_market_owner: Signer<'info>,
    #[account(has_one = lending_market_owner)]
    pub lending_market: AccountLoader<'info, LendingMarket>,
    #[account(
        mut,
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(mut,
        has_one = lending_market
    )]
    pub reserve: AccountLoader<'info, Reserve>,

    pub farms_program: Program<'info, Farms>,
    pub farms_global_config: AccountInfo<'info>,

    #[account(mut)]
    pub farm_state: AccountInfo<'info>,

    pub farms_vault_authority: AccountInfo<'info>,

    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}
