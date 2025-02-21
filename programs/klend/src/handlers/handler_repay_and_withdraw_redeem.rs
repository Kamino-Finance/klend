use anchor_lang::{prelude::*, Accounts};

use crate::{
    handler_refresh_obligation,
    handler_refresh_obligation_farms_for_reserve::*,
    handler_repay_obligation_liquidity::{self, *},
    handler_withdraw_obligation_collateral_and_redeem_reserve_collateral::{self, *},
    lending_market::lending_operations,
    refresh_farms,
    utils::seeds::pda,
    LendingError, LtvMaxWithdrawalCheck, MaxReservesAsCollateralCheck, RefreshObligation,
    RefreshObligationBumps, ReserveFarmKind,
};

pub fn process(
    ctx: Context<RepayAndWithdraw>,
    repay_amount: u64,
    withdraw_collateral_amount: u64,
) -> Result<()> {
    process_impl(
        &ctx.accounts.repay_accounts,
        &ctx.accounts.withdraw_accounts,
        ctx.remaining_accounts,
        ctx.program_id,
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

fn process_impl(
    repay_accounts: &RepayObligationLiquidity,
    withdraw_accounts: &WithdrawObligationCollateralAndRedeemReserveCollateral,
    remaining_accounts: &[AccountInfo],
    program_id: &Pubkey,
    repay_amount: u64,
    withdraw_collateral_amount: u64,
) -> Result<()> {
    let repay_reserve_key = repay_accounts.repay_reserve.key();
    let withdraw_reserve_key = withdraw_accounts.withdraw_reserve.key();
    let clock = Clock::get()?;
    let lending_market = repay_accounts.lending_market.load()?;

    let previous_borrow_count;
    let deposit_count;
    let referrer;
    let initial_ltv;
    let has_referrer;
    {
        let obligation = withdraw_accounts.obligation.load()?;

        deposit_count = obligation.deposits_count();
        previous_borrow_count = obligation.borrows_count();
        referrer = obligation.referrer;
        initial_ltv = obligation.loan_to_value();
        has_referrer = obligation.has_referrer();

        drop(obligation);

        let deposit_reserves_iter = remaining_accounts.iter().take(deposit_count);

        handler_repay_obligation_liquidity::process_impl(
            repay_accounts,
            deposit_reserves_iter,
            repay_amount,
        )?;
    }

    let borrow_count_post_repay = {
        let obligation = repay_accounts.obligation.load()?;
        let borrow_count_post_repay = obligation.borrows_count();
        drop(obligation);

        if borrow_count_post_repay == previous_borrow_count
            || repay_reserve_key == withdraw_reserve_key
        {
            let repay_reserve = &mut repay_accounts.repay_reserve.load_mut()?;

            lending_operations::refresh_reserve(
                repay_reserve,
                &clock,
                None,
                lending_market.referral_fee_bps,
            )?;
        }

        borrow_count_post_repay
    };

    let mut remaining_accounts_post_repay = {
        let remaining_accounts = if previous_borrow_count == borrow_count_post_repay {
            remaining_accounts.to_vec()
        } else {
            let referrer_to_skip = if has_referrer {
                pda::referrer_token_state(referrer, repay_reserve_key).0
            } else {
                Pubkey::default()
            };

            let mut reserves_iter: Vec<AccountInfo> = remaining_accounts
                .iter()
                .rev()
                .scan(false, |found_repay_reserve, account| {
                    let is_repay_reserve = account.key() == repay_reserve_key;

                    let accounts_to_include = account.key() != referrer_to_skip
                        && (!is_repay_reserve || *found_repay_reserve);

                    *found_repay_reserve = *found_repay_reserve || is_repay_reserve;

                    if accounts_to_include {
                        Some(Some(account.clone()))
                    } else {
                        Some(None)
                    }
                })
                .flatten()
                .collect::<Vec<_>>();
            reserves_iter.reverse();
            reserves_iter
        };

        let refresh_obligation_ctx = Context {
            program_id,
            accounts: &mut RefreshObligation {
                obligation: repay_accounts.obligation.clone(),
                lending_market: repay_accounts.lending_market.clone(),
            },
            remaining_accounts: remaining_accounts.as_slice(),
            bumps: RefreshObligationBumps {},
        };

        handler_refresh_obligation::process(
            refresh_obligation_ctx,
            MaxReservesAsCollateralCheck::Perform,
        )?;

        remaining_accounts
    };

    let obligation_was_closed = {
        handler_withdraw_obligation_collateral_and_redeem_reserve_collateral::process_impl(
            withdraw_accounts,
            withdraw_collateral_amount,
            LtvMaxWithdrawalCheck::LiquidationThreshold,
        )?
    };

    if !obligation_was_closed {
        let (final_deposit_amount, withdraw_reserve_key_is_repay_reserve) = {
            let obligation = withdraw_accounts.obligation.load()?;
            let final_deposit_amount = obligation
                .find_collateral_in_deposits(withdraw_reserve_key)
                .map_or(0, |collateral| collateral.deposited_amount);

            let withdraw_reserve_key_is_repay_reserve = obligation
                .find_liquidity_in_borrows(withdraw_reserve_key)
                .is_ok();

            (final_deposit_amount, withdraw_reserve_key_is_repay_reserve)
        };

        let is_full_withdrawal = final_deposit_amount == 0;

        if !is_full_withdrawal || withdraw_reserve_key_is_repay_reserve {
            let withdraw_reserve = &mut withdraw_accounts.withdraw_reserve.load_mut()?;
            lending_operations::refresh_reserve(
                withdraw_reserve,
                &clock,
                None,
                lending_market.referral_fee_bps,
            )?;
        }

        let remaining_accounts_post_withdrawal = if is_full_withdrawal {
            let withdraw_reserve_index = remaining_accounts_post_repay
                .iter()
                .position(|account| account.key() == withdraw_reserve_key)
                .unwrap();
            remaining_accounts_post_repay.remove(withdraw_reserve_index);
            remaining_accounts_post_repay
        } else {
            remaining_accounts_post_repay
        };

        let refresh_obligation_ctx = Context {
            program_id,
            accounts: &mut RefreshObligation {
                obligation: repay_accounts.obligation.clone(),
                lending_market: repay_accounts.lending_market.clone(),
            },
            remaining_accounts: remaining_accounts_post_withdrawal.as_slice(),
            bumps: RefreshObligationBumps {},
        };

        handler_refresh_obligation::process(
            refresh_obligation_ctx,
            MaxReservesAsCollateralCheck::Perform,
        )?;

        let mut obligation = withdraw_accounts.obligation.load_mut()?;
        obligation.last_update.mark_stale();

        let mut withdraw_reserve = withdraw_accounts.withdraw_reserve.load_mut()?;
        withdraw_reserve.last_update.mark_stale();
        lending_operations::utils::post_repay_and_withdraw_obligation_enforcements(
            &obligation,
            &withdraw_reserve,
            initial_ltv,
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
