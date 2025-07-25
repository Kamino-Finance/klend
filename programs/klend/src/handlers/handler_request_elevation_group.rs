use anchor_lang::prelude::*;

use crate::{
    lending_market::lending_operations, utils::FatAccountLoader, LendingError, LendingMarket,
    Obligation, ReferrerTokenState, Reserve,
};

pub fn process(ctx: Context<RequestElevationGroup>, new_elevation_group: u8) -> Result<()> {
    let obligation = &mut ctx.accounts.obligation.load_mut()?;
    let lending_market = ctx.accounts.lending_market.load()?;
    let slot = Clock::get()?.slot;
    let deposit_count = obligation.active_deposits_count();
    let borrow_count = obligation.active_borrows_count();
    let reserves_count = borrow_count + deposit_count;

    let expected_remaining_accounts = if obligation.has_referrer() {
        reserves_count + borrow_count
    } else {
        reserves_count
    };

    if ctx.remaining_accounts.iter().len() != expected_remaining_accounts {
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

    lending_operations::request_elevation_group(
        &crate::ID,
        obligation,
        &lending_market,
        slot,
        new_elevation_group,
        deposit_reserves_iter,
        borrow_reserves_iter,
        referrer_token_states_iter,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct RequestElevationGroup<'info> {
    pub owner: Signer<'info>,

    #[account(mut,
        has_one = lending_market,
        has_one = owner
    )]
    pub obligation: AccountLoader<'info, Obligation>,

    pub lending_market: AccountLoader<'info, LendingMarket>,
}
