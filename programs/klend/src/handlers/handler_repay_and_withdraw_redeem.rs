use std::ops::Deref;

use anchor_lang::{prelude::*, Accounts};

use crate::{
    gen_signer_seeds,
    handler_refresh_obligation_farms_for_reserve::*,
    handler_repay_obligation_liquidity::*,
    handler_withdraw_obligation_collateral_and_redeem_reserve_collateral::*,
    lending_market::{lending_checks, lending_operations},
    refresh_farms,
    utils::{accounts::ObligationRemainingAccounts, token_transfer},
    xmsg, LendingAction, LendingError, RepayAndWithdrawRedeemResult, ReserveFarmKind,
    WithdrawObligationCollateralAndRedeemReserveCollateralAccounts,
};

pub fn process<'info>(
    ctx: Context<'_, '_, '_, 'info, RepayAndWithdraw<'info>>,
    repay_amount: u64,
    withdraw_collateral_amount: u64,
) -> Result<()> {
    process_impl(
        &ctx.accounts.repay_accounts,
        &ctx.accounts.withdraw_accounts,
        ctx.remaining_accounts,
        repay_amount,
        withdraw_collateral_amount,
    )?;

    refresh_farms!(
        ctx.accounts.withdraw_accounts,
        [
            (
                ctx.accounts.withdraw_accounts.withdraw_reserve,
                ctx.accounts.collateral_farms_accounts,
                Collateral,
            ),
            (
                ctx.accounts.repay_accounts.repay_reserve,
                ctx.accounts.debt_farms_accounts,
                Debt,
            ),
        ],
    );

    Ok(())
}

fn process_impl<'info>(
    repay_accounts: &RepayObligationLiquidity<'info>,
    withdraw_accounts: &WithdrawObligationCollateralAndRedeemReserveCollateral<'info>,
    remaining_accounts: &[AccountInfo<'info>],
    repay_amount: u64,
    withdraw_collateral_amount: u64,
) -> Result<()> {
   
    lending_checks::repay_obligation_liquidity_checks(repay_accounts)?;
    lending_checks::withdraw_obligation_collateral_and_redeem_reserve_collateral_checks(
        &WithdrawObligationCollateralAndRedeemReserveCollateralAccounts {
            user_destination_liquidity: withdraw_accounts.user_destination_liquidity.clone(),
            withdraw_reserve: withdraw_accounts.withdraw_reserve.clone(),
            reserve_liquidity_mint: withdraw_accounts.reserve_liquidity_mint.clone(),
        },
    )?;

   
    let clock = Clock::get()?;
    let lending_market_address = repay_accounts.lending_market.key();
    let lending_market = repay_accounts.lending_market.load()?;
    let mut obligation = withdraw_accounts.obligation.load_mut()?;
    let other_accounts = ObligationRemainingAccounts::parse(&obligation, remaining_accounts)?;

   
    let debt_before = lending_checks::capture_reserve_accounting_and_balance(
        repay_accounts.repay_reserve.load()?.deref(),
        &repay_accounts.reserve_destination_liquidity,
    )?;
    let withdraw_before = lending_checks::capture_reserve_accounting_and_balance(
        withdraw_accounts.withdraw_reserve.load()?.deref(),
        &withdraw_accounts.reserve_liquidity_supply,
    )?;

   
    let RepayAndWithdrawRedeemResult {
        repay_amount: actual_repay_amount,
        early_repay_penalty,
        withdraw_obligation_amount,
        withdraw_liquidity_amount,
        obligation_closed,
    } = lending_operations::repay_and_withdraw_redeem(
        &lending_market,
        &repay_accounts.repay_reserve,
        &withdraw_accounts.withdraw_reserve,
        &mut obligation,
        &clock,
        repay_amount,
        withdraw_collateral_amount,
        other_accounts.deposit_reserves(),
        other_accounts.borrow_reserves(),
        other_accounts.referrer_token_states(),
    )?;
    let repay_amount_with_penalty = actual_repay_amount + early_repay_penalty;

   
    xmsg!(
        "pnl: Repaying obligation liquidity {} liquidity_amount {}",
        repay_amount_with_penalty,
        repay_amount,
    );
    xmsg!(
        "pnl: Withdraw obligation collateral {} and redeem reserve collateral {}",
        withdraw_obligation_amount,
        withdraw_liquidity_amount,
    );

   
    token_transfer::repay_obligation_liquidity_transfer(
        repay_accounts.token_program.to_account_info(),
        repay_accounts.reserve_liquidity_mint.to_account_info(),
        repay_accounts.user_source_liquidity.to_account_info(),
        repay_accounts
            .reserve_destination_liquidity
            .to_account_info(),
        repay_accounts.owner.to_account_info(),
        repay_amount_with_penalty,
        repay_accounts.reserve_liquidity_mint.decimals,
    )?;

    let authority_signer_seeds = gen_signer_seeds!(
        lending_market_address.as_ref(),
        lending_market.bump_seed as u8
    );
    token_transfer::withdraw_and_redeem_reserve_collateral_transfer(
        withdraw_accounts.collateral_token_program.to_account_info(),
        withdraw_accounts.liquidity_token_program.to_account_info(),
        withdraw_accounts.reserve_liquidity_mint.to_account_info(),
        withdraw_accounts.reserve_collateral_mint.to_account_info(),
        withdraw_accounts
            .reserve_source_collateral
            .to_account_info(),
        withdraw_accounts.reserve_liquidity_supply.to_account_info(),
        withdraw_accounts
            .user_destination_liquidity
            .to_account_info(),
        withdraw_accounts.lending_market_authority.clone(),
        authority_signer_seeds,
        withdraw_obligation_amount,
        withdraw_liquidity_amount,
        withdraw_accounts.reserve_liquidity_mint.decimals,
    )?;

   
    if obligation_closed {
        drop(obligation);
        withdraw_accounts
            .obligation
            .close(withdraw_accounts.owner.to_account_info())?;
    }

   
    let withdraw_after = lending_checks::capture_reserve_accounting_and_balance(
        withdraw_accounts.withdraw_reserve.load()?.deref(),
        &withdraw_accounts.reserve_liquidity_supply,
    )?;
    let debt_after = lending_checks::capture_reserve_accounting_and_balance(
        repay_accounts.repay_reserve.load()?.deref(),
        &repay_accounts.reserve_destination_liquidity,
    )?;

   
    if repay_accounts.repay_reserve.key() == withdraw_accounts.withdraw_reserve.key() {
        lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
            withdraw_after.vault_balance,
            withdraw_after.total_available_liquidity_amount,
            withdraw_before.vault_balance,
            withdraw_before.total_available_liquidity_amount,
            LendingAction::net_of(repay_amount_with_penalty, withdraw_liquidity_amount),
        )?;
    } else {
        lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
            debt_after.vault_balance,
            debt_after.total_available_liquidity_amount,
            debt_before.vault_balance,
            debt_before.total_available_liquidity_amount,
            LendingAction::Additive(repay_amount_with_penalty),
        )?;
        lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
            withdraw_after.vault_balance,
            withdraw_after.total_available_liquidity_amount,
            withdraw_before.vault_balance,
            withdraw_before.total_available_liquidity_amount,
            LendingAction::Subtractive(withdraw_liquidity_amount),
        )?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct RepayAndWithdraw<'info> {
    #[account(
        constraint = repay_accounts.owner.key()          == withdraw_accounts.owner.key()          @ LendingError::ObligationOwnersMustMatch,
        constraint = repay_accounts.obligation.key()     == withdraw_accounts.obligation.key()     @ LendingError::ObligationsMustMatch,
        constraint = repay_accounts.lending_market.key() == withdraw_accounts.lending_market.key() @ LendingError::LendingMarketsMustMatch,
    )]
    pub repay_accounts: RepayObligationLiquidity<'info>,
    pub withdraw_accounts: WithdrawObligationCollateralAndRedeemReserveCollateral<'info>,
    pub collateral_farms_accounts: OptionalObligationFarmsAccounts<'info>,
    pub debt_farms_accounts: OptionalObligationFarmsAccounts<'info>,
    pub farms_program: Program<'info, farms::program::Farms>,
}
