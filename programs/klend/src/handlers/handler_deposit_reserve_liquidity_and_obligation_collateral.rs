use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};
use anchor_spl::token::Token;
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface};

use crate::{
    check_refresh_ixs, gen_signer_seeds,
    lending_market::{lending_checks, lending_operations},
    state::{nested_accounts::*, obligation::Obligation, LendingMarket, Reserve},
    utils::{seeds, token_transfer},
    LendingAction, ReserveFarmKind,
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

    lending_checks::deposit_reserve_liquidity_and_obligation_collateral_checks(
        &DepositReserveLiquidityAndObligationCollateralAccounts {
            user_source_liquidity: ctx.accounts.user_source_liquidity.clone(),
            reserve: ctx.accounts.reserve.clone(),
            reserve_liquidity_mint: ctx.accounts.reserve_liquidity_mint.clone(),
        },
    )?;

    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let obligation = &mut ctx.accounts.obligation.load_mut()?;

    let lending_market = &ctx.accounts.lending_market.load()?;
    let lending_market_key = ctx.accounts.lending_market.key();
    let clock = Clock::get()?;

    let authority_signer_seeds =
        gen_signer_seeds!(lending_market_key, lending_market.bump_seed as u8);

    let initial_reserve_token_balance = token_interface::accessor::amount(
        &ctx.accounts.reserve_liquidity_supply.to_account_info(),
    )?;
    let initial_reserve_available_liquidity = reserve.liquidity.available_amount;
    let collateral_amount =
        lending_operations::deposit_reserve_liquidity(reserve, &clock, liquidity_amount)?;

    lending_operations::refresh_reserve(reserve, &clock, None, lending_market.referral_fee_bps)?;

    lending_operations::deposit_obligation_collateral(
        lending_market,
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

    token_transfer::deposit_reserve_liquidity_and_obligation_collateral_transfer(
        ctx.accounts.user_source_liquidity.to_account_info(),
        ctx.accounts.reserve_liquidity_supply.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.reserve_liquidity_mint.to_account_info(),
        ctx.accounts.liquidity_token_program.to_account_info(),
        ctx.accounts.reserve_collateral_mint.to_account_info(),
        ctx.accounts
            .reserve_destination_deposit_collateral
            .to_account_info(),
        ctx.accounts.collateral_token_program.to_account_info(),
        ctx.accounts.lending_market_authority.clone(),
        authority_signer_seeds,
        liquidity_amount,
        ctx.accounts.reserve_liquidity_mint.decimals,
        collateral_amount,
    )?;

    lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
        token_interface::accessor::amount(&ctx.accounts.reserve_liquidity_supply.to_account_info())
            .unwrap(),
        reserve.liquidity.available_amount,
        initial_reserve_token_balance,
        initial_reserve_available_liquidity,
        LendingAction::Additive(liquidity_amount),
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

    #[account(mut, has_one = lending_market)]
    pub reserve: AccountLoader<'info, Reserve>,

    #[account(mut,
        address = reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = liquidity_token_program,
    )]
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut,
        address = reserve.load()?.liquidity.supply_vault,
    )]
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, address = reserve.load()?.collateral.mint_pubkey)]
    pub reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, address = reserve.load()?.collateral.supply_vault)]
    pub reserve_destination_deposit_collateral: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = reserve.load()?.liquidity.mint_pubkey,
        token::authority = owner,
    )]
    pub user_source_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,

    pub placeholder_user_destination_collateral: Option<AccountInfo<'info>>,

    pub collateral_token_program: Program<'info, Token>,
    pub liquidity_token_program: Interface<'info, TokenInterface>,

    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
