use anchor_lang::prelude::*;

use crate::{
    lending_market::{lending_checks, lending_operations},
    utils::accounts::ObligationRemainingAccounts,
    LendingMarket, Obligation,
};

pub fn process(ctx: Context<RequestElevationGroup>, new_elevation_group: u8) -> Result<()> {
    let obligation = &mut ctx.accounts.obligation.load_mut()?;
    let lending_market = ctx.accounts.lending_market.load()?;
    let clock = Clock::get()?;
    let other_accounts = ObligationRemainingAccounts::parse(obligation, ctx.remaining_accounts)?;

   
    for reserve in other_accounts.all_reserves() {
        lending_checks::check_reserve_emergency_mode(&*reserve.load()?)?;
    }

    lending_operations::request_elevation_group(
        &crate::ID,
        obligation,
        &lending_market,
        &clock,
        new_elevation_group,
        other_accounts.deposit_reserves(),
        other_accounts.borrow_reserves(),
        other_accounts.referrer_token_states(),
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
