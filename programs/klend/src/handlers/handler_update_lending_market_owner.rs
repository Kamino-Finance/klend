use anchor_lang::{prelude::*, Accounts};

use crate::state::LendingMarket;

pub fn process(ctx: Context<UpdateLendingMarketOwner>) -> Result<()> {
    let market = &mut ctx.accounts.lending_market.load_mut()?;

    market.lending_market_owner = market.lending_market_owner_cached;

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateLendingMarketOwner<'info> {
    lending_market_owner_cached: Signer<'info>,

    #[account(mut, has_one = lending_market_owner_cached)]
    pub lending_market: AccountLoader<'info, LendingMarket>,
}
