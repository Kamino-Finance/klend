use anchor_lang::{prelude::*, solana_program::sysvar, Accounts};
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface};

use crate::{
    gen_signer_seeds,
    lending_market::{flash_ixs, lending_checks, lending_operations},
    state::{LendingMarket, Reserve},
    utils::{seeds, token_transfer},
    LendingAction, ReferrerTokenState,
};

pub fn process(ctx: Context<FlashBorrowReserveLiquidity>, liquidity_amount: u64) -> Result<()> {
    lending_checks::flash_borrow_reserve_liquidity_checks(&ctx)?;
    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let lending_market = &ctx.accounts.lending_market.load()?;
    let lending_market_key = ctx.accounts.lending_market.key();
    let authority_signer_seeds =
        gen_signer_seeds!(lending_market_key, lending_market.bump_seed as u8);

    let initial_reserve_token_balance = token_interface::accessor::amount(
        &ctx.accounts.reserve_source_liquidity.to_account_info(),
    )?;
    let initial_reserve_available_liquidity = reserve.liquidity.available_amount;

    flash_ixs::flash_borrow_checks(&ctx, liquidity_amount)?;

    lending_operations::refresh_reserve(
        reserve,
        &Clock::get()?,
        None,
        lending_market.referral_fee_bps,
    )?;

    lending_operations::flash_borrow_reserve_liquidity(reserve, liquidity_amount)?;

    token_transfer::borrow_obligation_liquidity_transfer(
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.reserve_liquidity_mint.to_account_info(),
        ctx.accounts.reserve_source_liquidity.to_account_info(),
        ctx.accounts.user_destination_liquidity.to_account_info(),
        ctx.accounts.lending_market_authority.to_account_info(),
        authority_signer_seeds,
        liquidity_amount,
        ctx.accounts.reserve_liquidity_mint.decimals,
    )?;

    lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
        token_interface::accessor::amount(&ctx.accounts.reserve_source_liquidity.to_account_info())
            .unwrap(),
        reserve.liquidity.available_amount,
        initial_reserve_token_balance,
        initial_reserve_available_liquidity,
        LendingAction::Subtractive(liquidity_amount),
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct FlashBorrowReserveLiquidity<'info> {
    pub user_transfer_authority: Signer<'info>,

    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    pub lending_market: AccountLoader<'info, LendingMarket>,

    #[account(mut,
        has_one = lending_market
    )]
    pub reserve: AccountLoader<'info, Reserve>,

    #[account(
        address = reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = token_program,
    )]
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut,
        address = reserve.load()?.liquidity.supply_vault,
    )]
    pub reserve_source_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub user_destination_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        address = reserve.load()?.liquidity.fee_vault
    )]
    pub reserve_liquidity_fee_receiver: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub referrer_token_state: Option<AccountLoader<'info, ReferrerTokenState>>,

    #[account(mut)]
    pub referrer_account: Option<AccountInfo<'info>>,

    #[account(address = sysvar::instructions::ID)]
    pub sysvar_info: AccountInfo<'info>,
    pub token_program: Interface<'info, TokenInterface>,
}
