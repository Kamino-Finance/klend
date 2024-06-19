use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};
use anchor_spl::token::Token;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface};

use crate::{
    check_cpi, gen_signer_seeds,
    lending_market::{lending_checks, lending_operations},
    state::{LendingMarket, RedeemReserveCollateralAccounts, Reserve},
    utils::{seeds, token_transfer},
    LendingAction,
};

pub fn process(ctx: Context<RedeemReserveCollateral>, collateral_amount: u64) -> Result<()> {
    check_cpi!(ctx);
    lending_checks::redeem_reserve_collateral_checks(&RedeemReserveCollateralAccounts {
        user_source_collateral: ctx.accounts.user_source_collateral.clone(),
        user_destination_liquidity: ctx.accounts.user_destination_liquidity.clone(),
        reserve: ctx.accounts.reserve.clone(),
        reserve_liquidity_mint: ctx.accounts.reserve_liquidity_mint.clone(),
        reserve_collateral_mint: ctx.accounts.reserve_collateral_mint.clone(),
        reserve_liquidity_supply: ctx.accounts.reserve_liquidity_supply.clone(),
        lending_market: ctx.accounts.lending_market.clone(),
        lending_market_authority: ctx.accounts.lending_market_authority.clone(),
        owner: ctx.accounts.owner.clone(),
        collateral_token_program: ctx.accounts.collateral_token_program.clone(),
        liquidity_token_program: ctx.accounts.liquidity_token_program.clone(),
    })?;

    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let lending_market = &ctx.accounts.lending_market.load()?;
    let clock = Clock::get()?;

    let lending_market_key = ctx.accounts.lending_market.key();
    let authority_signer_seeds =
        gen_signer_seeds!(lending_market_key.as_ref(), lending_market.bump_seed as u8);

    let initial_reserve_token_balance = token_interface::accessor::amount(
        &ctx.accounts.reserve_liquidity_supply.to_account_info(),
    )?;
    let initial_reserve_available_liquidity = reserve.liquidity.available_amount;

    lending_operations::refresh_reserve(reserve, &clock, None, lending_market.referral_fee_bps)?;
    let withdraw_liquidity_amount =
        lending_operations::redeem_reserve_collateral(reserve, collateral_amount, &clock, true)?;

    msg!(
        "pnl: Redeeming reserve collateral {}",
        withdraw_liquidity_amount
    );

    token_transfer::redeem_reserve_collateral_transfer(
        ctx.accounts.collateral_token_program.to_account_info(),
        ctx.accounts.liquidity_token_program.to_account_info(),
        ctx.accounts.reserve_liquidity_mint.to_account_info(),
        ctx.accounts.reserve_collateral_mint.to_account_info(),
        ctx.accounts.user_source_collateral.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.reserve_liquidity_supply.to_account_info(),
        ctx.accounts.user_destination_liquidity.to_account_info(),
        ctx.accounts.lending_market_authority.clone(),
        authority_signer_seeds,
        collateral_amount,
        withdraw_liquidity_amount,
        ctx.accounts.reserve_liquidity_mint.decimals,
    )?;

    lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
        token_interface::accessor::amount(&ctx.accounts.reserve_liquidity_supply.to_account_info())
            .unwrap(),
        reserve.liquidity.available_amount,
        initial_reserve_token_balance,
        initial_reserve_available_liquidity,
        LendingAction::Subtractive(withdraw_liquidity_amount),
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct RedeemReserveCollateral<'info> {
    pub owner: Signer<'info>,

    pub lending_market: AccountLoader<'info, LendingMarket>,

    #[account(mut,
        has_one = lending_market
    )]
    pub reserve: AccountLoader<'info, Reserve>,
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(mut,
        address = reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = liquidity_token_program,
    )]
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut,
        address = reserve.load()?.collateral.mint_pubkey,
    )]
    pub reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut,
        address = reserve.load()?.liquidity.supply_vault,
    )]
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = reserve_collateral_mint
    )]
    pub user_source_collateral: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut,
        token::mint = reserve.load()?.liquidity.mint_pubkey,
    )]
    pub user_destination_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,

    pub collateral_token_program: Program<'info, Token>,
    pub liquidity_token_program: Interface<'info, TokenInterface>,

    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
