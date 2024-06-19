use anchor_lang::{prelude::*, Accounts};

use crate::{
    lending_market::lending_operations,
    state::{obligation::Obligation, LendingMarket},
    utils::FatAccountLoader,
    LendingError, ReferrerTokenState, Reserve,
};

pub fn process(ctx: Context<RefreshObligation>) -> Result<()> {
    let obligation = &mut ctx.accounts.obligation.load_mut()?;
    let clock = &Clock::get()?;
    let lending_market = &ctx.accounts.lending_market.load()?;
    let borrow_count = obligation.borrows_count();
    let deposit_count = obligation.deposits_count();
    let reserves_count = borrow_count + deposit_count;

    let expected_remaining_accounts = if obligation.has_referrer() {
        reserves_count + borrow_count
    } else {
        reserves_count
    };

    if ctx.remaining_accounts.len() != expected_remaining_accounts {
        msg!(
            "expected_remaining_accounts={} obligation.has_referrer()={} reserves_count={} borrow_count={}",
            expected_remaining_accounts,
            obligation.has_referrer(),
            reserves_count,
            borrow_count
        );
        return err!(LendingError::InvalidAccountInput);
    }

    let deposit_reserves_iter = ctx
        .remaining_accounts
        .iter()
        .take(deposit_count)
        .map(|account_info| FatAccountLoader::<Reserve>::try_from(account_info).unwrap());

    let borrow_reserves_iter = ctx
        .remaining_accounts
        .iter()
        .skip(deposit_count)
        .take(borrow_count)
        .map(|account_info| FatAccountLoader::<Reserve>::try_from(account_info).unwrap());

    let referrer_token_states_iter =
        ctx.remaining_accounts
            .iter()
            .skip(reserves_count)
            .map(|account_info| {
                FatAccountLoader::<ReferrerTokenState>::try_from(account_info).unwrap()
            });

    lending_operations::refresh_obligation(
        obligation,
        lending_market,
        clock.slot,
        deposit_reserves_iter,
        borrow_reserves_iter,
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
