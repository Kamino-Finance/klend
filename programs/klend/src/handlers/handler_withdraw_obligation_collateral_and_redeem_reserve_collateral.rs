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
    state::{obligation::Obligation, LendingMarket, Reserve},
    utils::{close_account_loader, seeds, token_transfer},
    LendingAction, ReserveFarmKind, WithdrawObligationCollateralAndRedeemReserveCollateralAccounts,
};

pub fn process(
    ctx: Context<WithdrawObligationCollateralAndRedeemReserveCollateral>,
    collateral_amount: u64,
) -> Result<()> {
    let close_obligation = {
        check_refresh_ixs!(ctx, withdraw_reserve, ReserveFarmKind::Collateral);

        lending_checks::withdraw_obligation_collateral_and_redeem_reserve_collateral_checks(
            &WithdrawObligationCollateralAndRedeemReserveCollateralAccounts {
                user_destination_liquidity: ctx.accounts.user_destination_liquidity.clone(),
                withdraw_reserve: ctx.accounts.withdraw_reserve.clone(),
                reserve_liquidity_mint: ctx.accounts.reserve_liquidity_mint.clone(),
            },
        )?;

        let reserve = &mut ctx.accounts.withdraw_reserve.load_mut()?;

        let obligation = &mut ctx.accounts.obligation.load_mut()?;
        let lending_market = &ctx.accounts.lending_market.load()?;
        let lending_market_key = ctx.accounts.lending_market.key();
        let clock = &Clock::get()?;

        let authority_signer_seeds =
            gen_signer_seeds!(lending_market_key.as_ref(), lending_market.bump_seed as u8);

        let initial_reserve_token_balance = token_interface::accessor::amount(
            &ctx.accounts.reserve_liquidity_supply.to_account_info(),
        )?;
        let initial_reserve_available_liquidity = reserve.liquidity.available_amount;
        let withdraw_obligation_amount = lending_operations::withdraw_obligation_collateral(
            lending_market,
            reserve,
            obligation,
            collateral_amount,
            clock.slot,
            ctx.accounts.withdraw_reserve.key(),
        )?;
        let withdraw_liquidity_amount = lending_operations::redeem_reserve_collateral(
            reserve,
            withdraw_obligation_amount,
            clock,
            true,
        )?;
        msg!(
            "pnl: Withdraw obligation collateral {} and redeem reserve collateral {}",
            withdraw_obligation_amount,
            withdraw_liquidity_amount
        );

        token_transfer::withdraw_and_redeem_reserve_collateral_transfer(
            ctx.accounts.collateral_token_program.to_account_info(),
            ctx.accounts.liquidity_token_program.to_account_info(),
            ctx.accounts.reserve_liquidity_mint.to_account_info(),
            ctx.accounts.reserve_collateral_mint.to_account_info(),
            ctx.accounts.reserve_source_collateral.to_account_info(),
            ctx.accounts.reserve_liquidity_supply.to_account_info(),
            ctx.accounts.user_destination_liquidity.to_account_info(),
            ctx.accounts.lending_market_authority.clone(),
            authority_signer_seeds,
            withdraw_obligation_amount,
            withdraw_liquidity_amount,
            ctx.accounts.reserve_liquidity_mint.decimals,
        )?;

        lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
            token_interface::accessor::amount(
                &ctx.accounts.reserve_liquidity_supply.to_account_info(),
            )
            .unwrap(),
            reserve.liquidity.available_amount,
            initial_reserve_token_balance,
            initial_reserve_available_liquidity,
            LendingAction::Subtractive(withdraw_liquidity_amount),
        )?;

        obligation.deposits_empty() && obligation.borrows_empty()
    };

    close_account_loader(
        close_obligation,
        &ctx.accounts.owner,
        &ctx.accounts.obligation,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawObligationCollateralAndRedeemReserveCollateral<'info> {
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
    pub withdraw_reserve: AccountLoader<'info, Reserve>,

    #[account(mut,
        address = withdraw_reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = liquidity_token_program,
    )]
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, address = withdraw_reserve.load()?.collateral.supply_vault)]
    pub reserve_source_collateral: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, address = withdraw_reserve.load()?.collateral.mint_pubkey)]
    pub reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut,
        address = withdraw_reserve.load()?.liquidity.supply_vault,
    )]
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = withdraw_reserve.load()?.liquidity.mint_pubkey,
        token::authority = owner,
    )]
    pub user_destination_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,

    pub placeholder_user_destination_collateral: Option<AccountInfo<'info>>,

    pub collateral_token_program: Program<'info, Token>,
    pub liquidity_token_program: Interface<'info, TokenInterface>,

    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
