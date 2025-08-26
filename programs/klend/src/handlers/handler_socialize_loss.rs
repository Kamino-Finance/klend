use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};

use crate::{
    check_refresh_ixs,
    handler_refresh_obligation_farms_for_reserve::*,
    lending_market::lending_operations,
    refresh_farms,
    state::{obligation::Obligation, LendingMarket, Reserve},
    utils::{seeds, FatAccountLoader},
    ReserveFarmKind,
};

pub fn process_v1(ctx: Context<SocializeLoss>, liquidity_amount: u64) -> Result<()> {
    check_refresh_ixs!(ctx.accounts, ctx.accounts.reserve, ReserveFarmKind::Debt);
    process_impl(ctx.accounts, ctx.remaining_accounts, liquidity_amount)
}

pub fn process_v2(ctx: Context<SocializeLossV2>, liquidity_amount: u64) -> Result<()> {
    process_impl(
        &ctx.accounts.socialize_loss_accounts,
        ctx.remaining_accounts,
        liquidity_amount,
    )?;
    refresh_farms!(
        &ctx.accounts.socialize_loss_accounts,
        &ctx.accounts.lending_market_authority,
        [(
            ctx.accounts.socialize_loss_accounts.reserve,
            ctx.accounts.farms_accounts,
            Debt,
        )],
    );
    Ok(())
}

fn process_impl(
    accounts: &SocializeLoss,
    remaining_accounts: &[AccountInfo],
    liquidity_amount: u64,
) -> Result<()> {
    let clock = Clock::get()?;

    let repay_reserve = &mut accounts.reserve.load_mut()?;
    let obligation = &mut accounts.obligation.load_mut()?;

    lending_operations::socialize_loss(
        repay_reserve,
        &accounts.reserve.key(),
        obligation,
        liquidity_amount,
        clock.slot,
        remaining_accounts.iter().map(|a| {
            FatAccountLoader::try_from(a).expect("Remaining account is not a valid deposit reserve")
        }),
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct SocializeLoss<'info> {
   
    pub risk_council: Signer<'info>,

    #[account(mut,
        has_one = lending_market
    )]
    pub obligation: AccountLoader<'info, Obligation>,

    #[account(has_one = risk_council)]
    pub lending_market: AccountLoader<'info, LendingMarket>,

    #[account(mut,
        has_one = lending_market
    )]
    pub reserve: AccountLoader<'info, Reserve>,

    /// CHECK: Sysvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct SocializeLossV2<'info> {
    pub socialize_loss_accounts: SocializeLoss<'info>,
    pub farms_accounts: OptionalObligationFarmsAccounts<'info>,
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, socialize_loss_accounts.lending_market.key().as_ref()],
        bump = socialize_loss_accounts.lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,
    pub farms_program: Program<'info, farms::program::Farms>,
}
