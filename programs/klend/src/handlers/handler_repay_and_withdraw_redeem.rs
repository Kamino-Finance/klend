use anchor_lang::{prelude::*, Accounts};

use crate::{
    check_refresh_ixs, handler_refresh_obligation,
    handler_repay_obligation_liquidity::{self, *},
    handler_withdraw_obligation_collateral_and_redeem_reserve_collateral::{self, *},
    lending_market::lending_operations,
    utils::seeds::BASE_SEED_REFERRER_TOKEN_STATE,
    LendingError, RefreshObligation, RefreshObligationBumps, ReserveFarmKind,
};

pub fn process(
    ctx: Context<RepayAndWithdraw>,
    repay_amount: u64,
    withdraw_collateral_amount: u64,
) -> Result<()> {
    panic!("This operation is not yet supported");

    check_refresh_ixs!(
        ctx.accounts.repay_accounts,
        ctx.accounts.repay_accounts.repay_reserve,
        ctx.accounts.withdraw_accounts.withdraw_reserve,
        ReserveFarmKind::Debt,
        ReserveFarmKind::Collateral
    );

    require_keys_eq!(
        ctx.accounts.repay_accounts.owner.key(),
        ctx.accounts.withdraw_accounts.owner.key(),
        LendingError::ObligationOwnersMustMatch
    );

    require_keys_eq!(
        ctx.accounts.repay_accounts.obligation.key(),
        ctx.accounts.withdraw_accounts.obligation.key(),
        LendingError::ObligationsMustMatch
    );

    require_keys_eq!(
        ctx.accounts.repay_accounts.lending_market.key(),
        ctx.accounts.withdraw_accounts.lending_market.key(),
        LendingError::LendingMarketsMustMatch
    );

    let repay_reserve_key = ctx.accounts.repay_accounts.repay_reserve.key();
    let withdraw_reserve_key = ctx.accounts.withdraw_accounts.withdraw_reserve.key();

    let previous_borrow_count;
    let deposit_count;
    let referrer;
    let mut has_referrer = false;
    {
        let obligation = ctx.accounts.withdraw_accounts.obligation.load()?;

        deposit_count = obligation.deposits_count();
        previous_borrow_count = obligation.borrows_count();
        referrer = obligation.referrer;

        if referrer != Pubkey::default() {
            has_referrer = true;
        }

        let deposit_reserves_iter: Vec<_> = ctx
            .remaining_accounts
            .iter()
            .take(deposit_count)
            .cloned()
            .collect();

        drop(obligation);
        let repay_ctx = Context {
            program_id: ctx.program_id,
            accounts: &mut ctx.accounts.repay_accounts,
            remaining_accounts: deposit_reserves_iter.as_slice(),
            bumps: RepayObligationLiquidityBumps {},
        };

        handler_repay_obligation_liquidity::process(repay_ctx, repay_amount, true)?;
    }

    let borrow_count_post_repay = {
        let obligation = ctx.accounts.repay_accounts.obligation.load()?;
        let borrow_count_post_repay = obligation.borrows_count();
        drop(obligation);

        if borrow_count_post_repay == previous_borrow_count
            || repay_reserve_key == withdraw_reserve_key
        {
            let clock = Clock::get()?;

            let lending_market = ctx.accounts.repay_accounts.lending_market.load()?;

            let repay_reserve = &mut ctx.accounts.repay_accounts.repay_reserve.load_mut()?;

            lending_operations::refresh_reserve(
                repay_reserve,
                &clock,
                None,
                lending_market.referral_fee_bps,
            )?;
        }

        borrow_count_post_repay
    };

    {
        let mut reserves_iter: Vec<AccountInfo>;
        let remaining_accounts = if previous_borrow_count == borrow_count_post_repay {
            ctx.remaining_accounts
        } else {
            let mut referrer_to_skip = Pubkey::default();
            if has_referrer {
                referrer_to_skip = Pubkey::find_program_address(
                    &[
                        BASE_SEED_REFERRER_TOKEN_STATE,
                        &referrer.as_ref(),
                        &repay_reserve_key.as_ref(),
                    ],
                    ctx.program_id,
                )
                .0;
            }
            let mut found_repay_reserve = false;
            reserves_iter = ctx
                .remaining_accounts
                .iter()
                .rev()
                .filter(|account| {
                    if account.key() == referrer_to_skip {
                        false
                    } else if account.key() == repay_reserve_key && !found_repay_reserve {
                        found_repay_reserve = true;
                        false
                    } else {
                        true
                    }
                })
                .cloned()
                .collect::<Vec<AccountInfo>>();

            reserves_iter.reverse();

            reserves_iter.as_slice()
        };

        let refresh_obligation_ctx = Context {
            program_id: ctx.program_id,
            accounts: &mut RefreshObligation {
                obligation: ctx.accounts.repay_accounts.obligation.clone(),
                lending_market: ctx.accounts.repay_accounts.lending_market.clone(),
            },
            remaining_accounts,
            bumps: RefreshObligationBumps {},
        };

        handler_refresh_obligation::process(refresh_obligation_ctx)?;
    }

    {
        let withdraw_ctx = Context {
            program_id: ctx.program_id,
            accounts: &mut ctx.accounts.withdraw_accounts,
            remaining_accounts: ctx.remaining_accounts,
            bumps: WithdrawObligationCollateralAndRedeemReserveCollateralBumps {},
        };

        handler_withdraw_obligation_collateral_and_redeem_reserve_collateral::process(
            withdraw_ctx,
            withdraw_collateral_amount,
            true,
            true,
        )?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct RepayAndWithdraw<'info> {
    pub repay_accounts: RepayObligationLiquidity<'info>,
    pub withdraw_accounts: WithdrawObligationCollateralAndRedeemReserveCollateral<'info>,
}
