use anchor_lang::{prelude::*, Accounts};

use crate::{
    lending_market::lending_operations,
    state::{obligation::Obligation, LendingMarket},
};

pub fn process(
    ctx: Context<MarkObligationForDeleveraging>,
    autodeleverage_target_ltv_pct: u8,
) -> Result<()> {
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
   
    pub risk_council: Signer<'info>,

    #[account(mut,
        has_one = lending_market
    )]
    pub obligation: AccountLoader<'info, Obligation>,

    #[account(has_one = risk_council)]
    pub lending_market: AccountLoader<'info, LendingMarket>,
}
