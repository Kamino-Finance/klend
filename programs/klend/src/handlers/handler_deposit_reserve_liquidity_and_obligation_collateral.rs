use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};
use anchor_spl::token::{Mint, Token, TokenAccount};

use crate::{
    check_refresh_ixs, gen_signer_seeds,
    lending_market::{lending_checks, lending_operations},
    state::{nested_accounts::*, obligation::Obligation, LendingMarket, Reserve},
    utils::{seeds, token_transfer},
    ReserveFarmKind,
};

pub fn process(
    ctx: Context<DepositReserveLiquidityAndObligationCollateral>,
    liquidity_amount: u64,
) -> Result<()> {
    check_refresh_ixs!(ctx, reserve, ReserveFarmKind::Collateral);
    msg!(
        "DepositReserveLiquidityAndObligationCollateral Reserve {} amount {}",
        ctx.accounts.reserve.key(),
        liquidity_amount
    );

    lending_checks::deposit_reserve_liquidity_checks(&DepositReserveLiquidityAccounts {
        user_source_liquidity: ctx.accounts.user_source_liquidity.clone(),
        user_destination_collateral: ctx.accounts.user_destination_collateral.clone(),
        reserve: ctx.accounts.reserve.clone(),
        reserve_liquidity_supply: ctx.accounts.reserve_liquidity_supply.clone(),
        reserve_collateral_mint: ctx.accounts.reserve_collateral_mint.clone(),
        lending_market: ctx.accounts.lending_market.clone(),
        lending_market_authority: ctx.accounts.lending_market_authority.clone(),
        owner: ctx.accounts.owner.clone(),
        token_program: ctx.accounts.token_program.clone(),
    })?;
    lending_checks::deposit_obligation_collateral_checks(&DepositObligationCollateralAccounts {
        obligation: ctx.accounts.obligation.clone(),
        deposit_reserve: ctx.accounts.reserve.clone(),
        reserve_destination_collateral: ctx.accounts.reserve_destination_deposit_collateral.clone(),
        user_source_collateral: ctx.accounts.user_destination_collateral.clone(),
        lending_market: ctx.accounts.lending_market.clone(),
        obligation_owner: ctx.accounts.owner.clone(),
        token_program: ctx.accounts.token_program.clone(),
    })?;

    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let obligation = &mut ctx.accounts.obligation.load_mut()?;

    let lending_market = &ctx.accounts.lending_market.load()?;
    let lending_market_key = ctx.accounts.lending_market.key();
    let clock = &Clock::get()?;

    let authority_signer_seeds =
        gen_signer_seeds!(lending_market_key, lending_market.bump_seed as u8);

    lending_operations::refresh_reserve_interest(
        reserve,
        clock.slot,
        lending_market.referral_fee_bps,
    )?;
    let collateral_amount =
        lending_operations::deposit_reserve_liquidity(reserve, clock, liquidity_amount)?;

    lending_operations::refresh_reserve_interest(
        reserve,
        clock.slot,
        lending_market.referral_fee_bps,
    )?;
    lending_operations::deposit_obligation_collateral(
        reserve,
        obligation,
        clock.slot,
        collateral_amount,
        ctx.accounts.reserve.key(),
    )?;

    msg!(
        "pnl: Deposit reserve liquidity {} and obligation collateral {}",
        liquidity_amount,
        collateral_amount
    );

    token_transfer::deposit_reserve_liquidity_transfer(
        ctx.accounts.user_source_liquidity.to_account_info(),
        ctx.accounts.reserve_liquidity_supply.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.reserve_collateral_mint.to_account_info(),
        ctx.accounts.user_destination_collateral.to_account_info(),
        ctx.accounts.lending_market_authority.clone(),
        authority_signer_seeds,
        liquidity_amount,
        collateral_amount,
    )?;

    token_transfer::deposit_obligation_collateral_transfer(
        ctx.accounts.user_destination_collateral.to_account_info(),
        ctx.accounts
            .reserve_destination_deposit_collateral
            .to_account_info(),
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
        collateral_amount,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct DepositReserveLiquidityAndObligationCollateral<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut,
        has_one = lending_market,
        has_one = owner
    )]
    pub obligation: AccountLoader<'info, Obligation>,

    pub lending_market: AccountLoader<'info, LendingMarket>,
       #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(mut,
        has_one = lending_market
    )]
    pub reserve: AccountLoader<'info, Reserve>,
    #[account(mut,
        address = reserve.load()?.liquidity.supply_vault
    )]
    pub reserve_liquidity_supply: Box<Account<'info, TokenAccount>>,
    #[account(mut,
        address = reserve.load()?.collateral.mint_pubkey
    )]
    pub reserve_collateral_mint: Box<Account<'info, Mint>>,

    #[account(mut,
        address = reserve.load()?.collateral.supply_vault
    )]
    pub reserve_destination_deposit_collateral: Box<Account<'info, TokenAccount>>,

    #[account(mut,
        token::mint = reserve.load()?.liquidity.mint_pubkey
    )]
    pub user_source_liquidity: Box<Account<'info, TokenAccount>>,
    #[account(mut,
        token::mint = reserve_collateral_mint.key()
    )]
    pub user_destination_collateral: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,

       #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
