use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};
use farms::program::Farms;

use crate::{
    check_advance_nonce_ix_if_needed,
    lending_market::farms_ixs,
    state::{LendingMarket, Reserve},
    utils::seeds,
    xmsg, ReserveFarmKind,
};

pub fn process(ctx: Context<InitFarmsForReserve>, mode: u8) -> Result<()> {
    check_advance_nonce_ix_if_needed!(ctx.accounts);

    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let farm = ctx.accounts.farm_state.key();

    let mode: ReserveFarmKind = mode.try_into().unwrap();

    xmsg!(
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
    /// CHECK: Checked through create_program_address
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(mut,
        has_one = lending_market
    )]
    pub reserve: AccountLoader<'info, Reserve>,

    pub farms_program: Program<'info, Farms>,
    /// CHECK: GlobalConfig account checked on farms CPI
    pub farms_global_config: AccountInfo<'info>,

    /// CHECK: FarmState account is initialized on farms CPI
    #[account(mut)]
    pub farm_state: AccountInfo<'info>,

    /// CHECK: PDA initialized on farms CPI
    pub farms_vault_authority: AccountInfo<'info>,

    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,

    /// CHECK: Sysvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
