use anchor_lang::{prelude::*, Accounts};

use crate::{
    lending_market::lending_operations,
    state::{obligation::Obligation, LendingMarket},
    utils::accounts::ObligationRemainingAccounts,
};

pub fn process(ctx: Context<RefreshObligation>) -> Result<()> {
    let obligation = &mut ctx.accounts.obligation.load_mut()?;
    let clock = &Clock::get()?;
    let lending_market = &ctx.accounts.lending_market.load()?;
    let other_accounts = ObligationRemainingAccounts::parse(obligation, ctx.remaining_accounts)?;

    lending_operations::refresh_obligation(
        &crate::ID,
        obligation,
        lending_market,
        clock,
        other_accounts.deposit_reserves(),
        other_accounts.borrow_reserves(),
        other_accounts.referrer_token_states(),
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct RefreshObligation<'info> {
    pub lending_market: AccountLoader<'info, LendingMarket>,
    #[account(mut, has_one = lending_market)]
    pub obligation: AccountLoader<'info, Obligation>,
   
   
   
   
   
   
   
   
}
