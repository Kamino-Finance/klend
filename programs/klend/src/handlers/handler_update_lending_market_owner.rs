use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};

use crate::{check_advance_nonce_ix_if_needed, state::LendingMarket, LendingError};

pub fn process(ctx: Context<UpdateLendingMarketOwner>) -> Result<()> {
    check_advance_nonce_ix_if_needed!(ctx.accounts);

    let market = &mut ctx.accounts.lending_market.load_mut()?;

    require!(
        !market.is_immutable(),
        LendingError::OperationNotPermittedMarketImmutable
    );

    market.lending_market_owner = market.lending_market_owner_cached;

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateLendingMarketOwner<'info> {
    lending_market_owner_cached: Signer<'info>,

    #[account(mut, has_one = lending_market_owner_cached)]
    pub lending_market: AccountLoader<'info, LendingMarket>,

    /// CHECK: Sysvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
