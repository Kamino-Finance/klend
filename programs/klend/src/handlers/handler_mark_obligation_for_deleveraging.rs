use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};

use crate::{
    check_advance_nonce_ix_if_needed,
    lending_market::lending_operations,
    state::{obligation::Obligation, LendingMarket},
};

pub fn process(
    ctx: Context<MarkObligationForDeleveraging>,
    autodeleverage_target_ltv_pct: u8,
) -> Result<()> {
    check_advance_nonce_ix_if_needed!(ctx.accounts);

    let obligation = &mut ctx.accounts.obligation.load_mut()?;
    let lending_market = &ctx.accounts.lending_market.load()?;
    let clock = Clock::get()?;
    lending_operations::mark_obligation_for_deleveraging(
        lending_market,
        obligation,
        autodeleverage_target_ltv_pct,
        u64::try_from(clock.unix_timestamp).unwrap(),
    )
}

#[derive(Accounts)]
pub struct MarkObligationForDeleveraging<'info> {
   
    pub lending_market_owner: Signer<'info>,

    #[account(mut,
        has_one = lending_market
    )]
    pub obligation: AccountLoader<'info, Obligation>,

    #[account(has_one = lending_market_owner)]
    pub lending_market: AccountLoader<'info, LendingMarket>,

    /// CHECK: Sysvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
