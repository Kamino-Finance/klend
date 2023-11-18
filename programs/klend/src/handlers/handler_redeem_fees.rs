use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token::{Token, TokenAccount};

use crate::{
    gen_signer_seeds,
    lending_market::lending_operations,
    state::{LendingMarket, Reserve},
    utils::{seeds, token_transfer},
};

pub fn process(ctx: Context<RedeemFees>) -> Result<()> {
    let clock = &Clock::get()?;

    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let lending_market = &ctx.accounts.lending_market.load()?;
    let lending_market_key = ctx.accounts.lending_market.key();

    let authority_signer_seeds =
        gen_signer_seeds!(lending_market_key, lending_market.bump_seed as u8);

    let withdraw_amount = lending_operations::redeem_fees(reserve, clock.slot)?;

    token_transfer::withdraw_fees_from_reserve(
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.reserve_supply_liquidity.to_account_info(),
        ctx.accounts
            .reserve_liquidity_fee_receiver
            .to_account_info(),
        ctx.accounts.lending_market_authority.to_account_info(),
        authority_signer_seeds,
        withdraw_amount,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct RedeemFees<'info> {
    #[account(mut,
        has_one = lending_market)]
    pub reserve: AccountLoader<'info, Reserve>,
    #[account(mut, address = reserve.load()?.liquidity.fee_vault)]
    pub reserve_liquidity_fee_receiver: Box<Account<'info, TokenAccount>>,
    #[account(mut, address = reserve.load()?.liquidity.supply_vault)]
    pub reserve_supply_liquidity: Box<Account<'info, TokenAccount>>,

    pub lending_market: AccountLoader<'info, LendingMarket>,
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    pub token_program: Program<'info, Token>,
}
