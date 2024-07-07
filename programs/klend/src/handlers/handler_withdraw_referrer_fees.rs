use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::utils::constraints;
use crate::{
    gen_signer_seeds,
    lending_market::lending_operations,
    state::{LendingMarket, Reserve},
    utils::{
        seeds::{self, BASE_SEED_REFERRER_TOKEN_STATE},
        token_transfer,
    },
    ReferrerTokenState,
};

pub fn process(ctx: Context<WithdrawReferrerFees>) -> Result<()> {
    constraints::token_2022::validate_liquidity_token_extensions(
        &ctx.accounts.reserve_liquidity_mint.to_account_info(),
        &ctx.accounts.referrer_token_account.to_account_info(),
    )?;

    let clock = &Clock::get()?;

    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let referrer_token_state = &mut ctx.accounts.referrer_token_state.load_mut()?;
    let lending_market = &ctx.accounts.lending_market.load()?;
    let lending_market_key = ctx.accounts.lending_market.key();

    let authority_signer_seeds =
        gen_signer_seeds!(lending_market_key, lending_market.bump_seed as u8);

    let withdraw_amount =
        lending_operations::withdraw_referrer_fees(reserve, clock.slot, referrer_token_state)?;

    token_transfer::withdraw_fees_from_reserve(
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.reserve_liquidity_mint.to_account_info(),
        ctx.accounts.reserve_supply_liquidity.to_account_info(),
        ctx.accounts.referrer_token_account.to_account_info(),
        ctx.accounts.lending_market_authority.to_account_info(),
        authority_signer_seeds,
        withdraw_amount,
        ctx.accounts.reserve_liquidity_mint.decimals,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawReferrerFees<'info> {
    #[account(mut)]
    pub referrer: Signer<'info>,

    #[account(mut,
        seeds = [BASE_SEED_REFERRER_TOKEN_STATE, referrer.key().as_ref(), reserve.key().as_ref()],
        bump = referrer_token_state.load()?.bump.try_into().unwrap()
    )]
    pub referrer_token_state: AccountLoader<'info, ReferrerTokenState>,

    #[account(mut,
        has_one = lending_market
    )]
    pub reserve: AccountLoader<'info, Reserve>,

    #[account(mut,
        address = reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = token_program,
    )]
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut,
        address = reserve.load()?.liquidity.supply_vault,
    )]
    pub reserve_supply_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, token::mint = reserve.load()?.liquidity.mint_pubkey)]
    pub referrer_token_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub lending_market: AccountLoader<'info, LendingMarket>,
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    pub token_program: Interface<'info, TokenInterface>,
}
