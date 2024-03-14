use anchor_lang::{prelude::*, Accounts};

use crate::{
    state::{InitLendingMarketParams, LendingMarket},
    utils::seeds,
};

pub fn process(ctx: Context<InitLendingMarket>, quote_currency: [u8; 32]) -> Result<()> {
    let lending_market = &mut ctx.accounts.lending_market.load_init()?;

    lending_market.init(InitLendingMarketParams {
        quote_currency,
        lending_market_owner: ctx.accounts.lending_market_owner.key(),
        bump_seed: ctx.bumps.lending_market_authority,
    });

    Ok(())
}

#[derive(Accounts)]
pub struct InitLendingMarket<'info> {
    #[account(mut)]
    pub lending_market_owner: Signer<'info>,

    #[account(zero)]
    pub lending_market: AccountLoader<'info, LendingMarket>,

    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump
    )]
    pub lending_market_authority: AccountInfo<'info>,

    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}
