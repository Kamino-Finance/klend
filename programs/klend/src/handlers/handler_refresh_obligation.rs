use anchor_lang::{prelude::*, Accounts};

use crate::{
    lending_market::lending_operations,
    state::{obligation::Obligation, LendingMarket},
    LendingError, ReferrerTokenState, Reserve,
};

pub fn process(ctx: Context<RefreshObligation>) -> Result<()> {
    let obligation = &mut ctx.accounts.obligation.load_mut()?;
    let clock = &Clock::get()?;
    let lending_market = &ctx.accounts.lending_market.load()?;
    let borrow_count = obligation.borrows_count();
    let reserves_count = borrow_count + obligation.deposits_count();

    let expected_remaining_accounts = if obligation.has_referrer() {
        reserves_count + borrow_count
    } else {
        reserves_count
    };

    if ctx.remaining_accounts.iter().len() != expected_remaining_accounts {
        msg!(
            "expected_remaining_accounts={} obligation.has_referrer()={} reserves_count={} borrow_count={}",
            expected_remaining_accounts,
            obligation.has_referrer(),
            reserves_count,
            borrow_count
        );
        return err!(LendingError::InvalidAccountInput);
    }

    let reserves_iter = ctx
        .remaining_accounts
        .iter()
        .take(reserves_count)
        .map(|account_info| AccountLoader::<Reserve>::try_from(account_info).unwrap());

    let referrer_token_states_iter = ctx
        .remaining_accounts
        .iter()
        .skip(reserves_count)
        .map(|account_info| AccountLoader::<ReferrerTokenState>::try_from(account_info).unwrap());

    lending_operations::refresh_obligation(
        obligation,
        lending_market,
        clock.slot,
        reserves_iter,
        referrer_token_states_iter,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct RefreshObligation<'info> {
    pub lending_market: AccountLoader<'info, LendingMarket>,
    #[account(mut, has_one = lending_market)]
    pub obligation: AccountLoader<'info, Obligation>,
}
