use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};

use crate::{
    check_refresh_ixs,
    lending_market::lending_operations,
    state::{obligation::Obligation, LendingMarket, Reserve},
    utils::FatAccountLoader,
    ReserveFarmKind,
};

pub fn process(ctx: Context<SocializeLoss>, liquidity_amount: u64) -> Result<()> {
    check_refresh_ixs!(ctx, reserve, ReserveFarmKind::Debt);

    let clock = Clock::get()?;

    let repay_reserve = &mut ctx.accounts.reserve.load_mut()?;
    let obligation = &mut ctx.accounts.obligation.load_mut()?;

    lending_operations::socialize_loss(
        repay_reserve,
        &ctx.accounts.reserve.key(),
        obligation,
        liquidity_amount,
        clock.slot,
        ctx.remaining_accounts.iter().map(|a| {
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

    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
