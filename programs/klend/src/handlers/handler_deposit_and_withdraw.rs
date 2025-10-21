use anchor_lang::{prelude::*, Accounts};

use crate::{
    handler_deposit_reserve_liquidity_and_obligation_collateral::{self, *},
    handler_refresh_obligation,
    handler_refresh_obligation_farms_for_reserve::*,
    handler_withdraw_obligation_collateral_and_redeem_reserve_collateral::{self, *},
    lending_market::lending_operations,
    refresh_farms, LendingError, LtvMaxWithdrawalCheck, MaxReservesAsCollateralCheck,
    RefreshObligation, RefreshObligationBumps, ReserveFarmKind,
};

pub fn process(
    ctx: Context<DepositAndWithdraw>,
    liquidity_amount: u64,
    withdraw_collateral_amount: u64,
) -> Result<()> {
    let initial_ltv = {
        let obligation = ctx.accounts.deposit_accounts.obligation.load()?;
       
        require_gt!(
            obligation.deposited_value_sf,
            0,
            LendingError::ObligationDepositsEmpty
        );
        obligation.loan_to_value()
    };

    {
        handler_deposit_reserve_liquidity_and_obligation_collateral::process_impl(
            &ctx.accounts.deposit_accounts,
            liquidity_amount,
            MaxReservesAsCollateralCheck::Skip,
        )?;
    }

    {
        let clock = Clock::get()?;

        let lending_market = ctx.accounts.deposit_accounts.lending_market.load()?;
        let mut reserve = ctx.accounts.deposit_accounts.reserve.load_mut()?;
        lending_operations::refresh_reserve(
            &mut reserve,
            &clock,
            None,
            lending_market.referral_fee_bps,
        )?;
        let timestamp = u64::try_from(clock.unix_timestamp).unwrap();
        lending_operations::refresh_reserve_limit_timestamps(&mut reserve, timestamp);
    }

    {
        let refresh_obligation_ctx = Context {
            program_id: ctx.program_id,
            accounts: &mut RefreshObligation {
                obligation: ctx.accounts.deposit_accounts.obligation.clone(),
                lending_market: ctx.accounts.deposit_accounts.lending_market.clone(),
            },
            remaining_accounts: ctx.remaining_accounts,
            bumps: RefreshObligationBumps {},
        };

        handler_refresh_obligation::process(
            refresh_obligation_ctx,
            MaxReservesAsCollateralCheck::Skip,
        )?;
    }

    let is_obligation_closed = {
        handler_withdraw_obligation_collateral_and_redeem_reserve_collateral::process_impl(
            &ctx.accounts.withdraw_accounts,
            withdraw_collateral_amount,
            LtvMaxWithdrawalCheck::LiquidationThreshold,
        )?
    };

    {
        let clock = Clock::get()?;

        let lending_market = ctx.accounts.withdraw_accounts.lending_market.load()?;
        let mut reserve = ctx.accounts.withdraw_accounts.withdraw_reserve.load_mut()?;
        lending_operations::refresh_reserve(
            &mut reserve,
            &clock,
            None,
            lending_market.referral_fee_bps,
        )?;
        let timestamp = u64::try_from(clock.unix_timestamp).unwrap();
        lending_operations::refresh_reserve_limit_timestamps(&mut reserve, timestamp);
    }

    if !is_obligation_closed {
        let is_full_withdrawal = {
            let obligation = ctx.accounts.withdraw_accounts.obligation.load()?;
            let final_deposit_amount = obligation
                .find_collateral_in_deposits(ctx.accounts.withdraw_accounts.withdraw_reserve.key())
                .map_or(0, |collateral| collateral.deposited_amount);
            final_deposit_amount == 0
        };

        let remaining_accounts: Vec<AccountInfo> = if is_full_withdrawal {
            let mut withdraw_reserve_found = false;
            ctx.remaining_accounts
                .iter()
                .filter_map(|account| {
                    if account.key() == ctx.accounts.withdraw_accounts.withdraw_reserve.key()
                        && !withdraw_reserve_found
                    {
                        withdraw_reserve_found = true;
                        None
                    } else {
                        Some(account.clone())
                    }
                })
                .collect()
        } else {
            ctx.remaining_accounts.to_vec()
        };

        let refresh_obligation_ctx = Context {
            program_id: ctx.program_id,
            accounts: &mut RefreshObligation {
                obligation: ctx.accounts.deposit_accounts.obligation.clone(),
                lending_market: ctx.accounts.deposit_accounts.lending_market.clone(),
            },
            remaining_accounts: remaining_accounts.as_slice(),
            bumps: RefreshObligationBumps {},
        };

        handler_refresh_obligation::process(
            refresh_obligation_ctx,
            MaxReservesAsCollateralCheck::Perform,
        )?;

        let mut obligation = ctx.accounts.withdraw_accounts.obligation.load_mut()?;
        obligation.last_update.mark_stale();

        let mut withdraw_reserve = ctx.accounts.withdraw_accounts.withdraw_reserve.load_mut()?;
        withdraw_reserve.last_update.mark_stale();
        let lending_market = ctx.accounts.withdraw_accounts.lending_market.load()?;

        lending_operations::utils::post_deposit_and_withdraw_obligation_enforcements(
            &obligation,
            &withdraw_reserve,
            &lending_market,
            initial_ltv,
        )?;
    }

    refresh_farms!(
        ctx.accounts.deposit_accounts,
        [
            (
                ctx.accounts.deposit_accounts.reserve,
                ctx.accounts.deposit_farms_accounts,
                Collateral,
            ),
            (
                ctx.accounts.withdraw_accounts.withdraw_reserve,
                ctx.accounts.withdraw_farms_accounts,
                Collateral,
            ),
        ],
    );

    Ok(())
}

#[derive(Accounts)]
pub struct DepositAndWithdraw<'info> {
    #[account(
        constraint = deposit_accounts.owner.key()          == withdraw_accounts.owner.key()          @ LendingError::ObligationOwnersMustMatch,
        constraint = deposit_accounts.obligation.key()     == withdraw_accounts.obligation.key()     @ LendingError::ObligationsMustMatch,
        constraint = deposit_accounts.lending_market.key() == withdraw_accounts.lending_market.key() @ LendingError::LendingMarketsMustMatch,
    )]
    pub deposit_accounts: DepositReserveLiquidityAndObligationCollateral<'info>,
    pub withdraw_accounts: WithdrawObligationCollateralAndRedeemReserveCollateral<'info>,
    pub deposit_farms_accounts: OptionalObligationFarmsAccounts<'info>,
    pub withdraw_farms_accounts: OptionalObligationFarmsAccounts<'info>,
    pub farms_program: Program<'info, farms::program::Farms>,
}
