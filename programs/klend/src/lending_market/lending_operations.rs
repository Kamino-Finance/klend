use std::{
    cell::RefMut,
    cmp::min,
    ops::{Add, Div, Mul},
};

use anchor_lang::{err, prelude::*, require, solana_program::clock::Slot, Result};
use borsh::BorshDeserialize;
use solana_program::clock::{self, Clock};

use self::utils::{
    calculate_obligation_collateral_market_value, calculate_obligation_liquidity_market_value,
    check_elevation_group_borrowing_enabled, check_non_elevation_group_borrowing_enabled,
    check_obligation_collateral_deposit_reserve, check_obligation_fully_refreshed_and_not_null,
    check_obligation_liquidity_borrow_reserve, check_same_elevation_group, get_elevation_group,
    get_max_ltv_and_liquidation_threshold, post_borrow_obligation_invariants,
    post_deposit_obligation_invariants, post_repay_obligation_invariants,
    post_withdraw_obligation_invariants, update_elevation_group_debt_trackers_on_repay,
    validate_obligation_asset_tiers,
};
use super::{
    validate_referrer_token_state,
    withdrawal_cap_operations::utils::{add_to_withdrawal_accum, sub_from_withdrawal_accum},
};
use crate::{
    approximate_compounded_interest,
    fraction::FractionExtra,
    liquidation_operations,
    state::{
        obligation::Obligation, CalculateBorrowResult, CalculateLiquidationResult,
        CalculateRepayResult, Reserve,
    },
    utils::{
        borrow_rate_curve::BorrowRateCurve, AnyAccountLoader, BigFraction, Fraction,
        GetPriceResult, ELEVATION_GROUP_NONE, PROGRAM_VERSION,
    },
    xmsg, AssetTier, ElevationGroup, LendingError, LendingMarket, LiquidateAndRedeemResult,
    LiquidateObligationResult, ObligationCollateral, PriceStatusFlags, ReferrerTokenState,
    RefreshObligationBorrowsResult, RefreshObligationDepositsResult, ReserveConfig, ReserveStatus,
    UpdateConfigMode, WithdrawResult,
};

pub fn refresh_reserve(
    reserve: &mut Reserve,
    clock: &Clock,
    price: Option<GetPriceResult>,
    referral_fee_bps: u16,
) -> Result<()> {
    let slot = clock.slot;

    reserve.accrue_interest(slot, referral_fee_bps)?;

    let price_status = if let Some(GetPriceResult {
        price,
        status,
        timestamp,
    }) = price
    {
        reserve.liquidity.market_price_sf = price.to_bits();
        reserve.liquidity.market_price_last_updated_ts = timestamp;

        Some(status)
    } else if !is_saved_price_age_valid(reserve, clock.unix_timestamp) {
        Some(PriceStatusFlags::empty())
    } else {
        None
    };

    reserve.last_update.update_slot(slot, price_status);

    reserve.config.reserved_2 = [0; 2];
    reserve.config.reserved_3 = [0; 8];

    Ok(())
}

pub fn is_saved_price_age_valid(reserve: &Reserve, current_ts: clock::UnixTimestamp) -> bool {
    let current_ts: u64 = current_ts.try_into().expect("Negative timestamp");
    let price_last_updated_ts = reserve.liquidity.market_price_last_updated_ts;
    let price_max_age = reserve.config.token_info.max_age_price_seconds;

    current_ts.saturating_sub(price_last_updated_ts) < price_max_age
}

pub fn is_price_refresh_needed(
    reserve: &Reserve,
    market: &LendingMarket,
    current_ts: clock::UnixTimestamp,
) -> bool {
    let current_ts = current_ts as u64;
    let price_last_updated_ts = reserve.liquidity.market_price_last_updated_ts;
    let price_max_age = reserve.config.token_info.max_age_price_seconds;
    let price_refresh_trigger_to_max_age_pct: u64 =
        market.price_refresh_trigger_to_max_age_pct.into();
    let price_refresh_trigger_to_max_age_secs =
        price_max_age * price_refresh_trigger_to_max_age_pct / 100;

    current_ts.saturating_sub(price_last_updated_ts) >= price_refresh_trigger_to_max_age_secs
}

pub fn refresh_reserve_limit_timestamps(reserve: &mut Reserve, slot: Slot) -> Result<()> {
    reserve.update_deposit_limit_crossed_slot(slot)?;
    reserve.update_borrow_limit_crossed_slot(slot)?;
    Ok(())
}

pub fn deposit_reserve_liquidity(
    reserve: &mut Reserve,
    clock: &Clock,
    liquidity_amount: u64,
) -> Result<u64> {
    if liquidity_amount == 0 {
        msg!("Liquidity amount provided cannot be zero");
        return err!(LendingError::InvalidAmount);
    }

    if reserve
        .last_update
        .is_stale(clock.slot, PriceStatusFlags::NONE)?
    {
        msg!("Reserve is stale and must be refreshed in the current slot");
        return err!(LendingError::ReserveStale);
    }

    let liquidity_amount_f = Fraction::from(liquidity_amount);
    let deposit_limit_f = Fraction::from(reserve.config.deposit_limit);
    let reserve_liquidity_supply_f = reserve.liquidity.total_supply()?;

    let new_reserve_liquidity_supply_f = liquidity_amount_f + reserve_liquidity_supply_f;

    if new_reserve_liquidity_supply_f > deposit_limit_f {
        msg!(
            "Cannot deposit liquidity above the reserve deposit limit. New total deposit: {} > limit: {}",
            new_reserve_liquidity_supply_f,
            reserve.config.deposit_limit
        );
        return err!(LendingError::DepositLimitExceeded);
    }

    sub_from_withdrawal_accum(
        &mut reserve.config.deposit_withdrawal_cap,
        liquidity_amount,
        u64::try_from(clock.unix_timestamp).unwrap(),
    )?;

    let collateral_amount = reserve.deposit_liquidity(liquidity_amount)?;

    reserve.last_update.mark_stale();

    Ok(collateral_amount)
}

#[allow(clippy::too_many_arguments)]
pub fn borrow_obligation_liquidity<'info, T>(
    lending_market: &LendingMarket,
    borrow_reserve: &mut Reserve,
    obligation: &mut Obligation,
    liquidity_amount: u64,
    clock: &Clock,
    borrow_reserve_pk: Pubkey,
    referrer_token_state: Option<RefMut<ReferrerTokenState>>,
    deposit_reserves_iter: impl Iterator<Item = T>,
) -> Result<CalculateBorrowResult>
where
    T: AnyAccountLoader<'info, Reserve>,
{
    if liquidity_amount == 0 {
        msg!("Liquidity amount provided cannot be zero");
        return err!(LendingError::InvalidAmount);
    }

    if borrow_reserve
        .last_update
        .is_stale(clock.slot, PriceStatusFlags::ALL_CHECKS)?
    {
        msg!(
            "Borrow reserve is stale and must be refreshed in the current slot, price_status: {:08b}",
            borrow_reserve.last_update.get_price_status().0
        );
        return err!(LendingError::ReserveStale);
    }

    if lending_market.is_borrowing_disabled() {
        msg!("Borrowing is disabled");
        return err!(LendingError::BorrowingDisabled);
    }

    let current_utilization = borrow_reserve.liquidity.utilization_rate()?;
    let reserve_liquidity_borrowed_f = borrow_reserve.liquidity.total_borrow();
    let liquidity_amount_f = Fraction::from(liquidity_amount);
    let borrow_limit_f = Fraction::from(borrow_reserve.config.borrow_limit);

    let new_borrowed_amount_f = liquidity_amount_f + reserve_liquidity_borrowed_f;
    if liquidity_amount != u64::MAX && new_borrowed_amount_f > borrow_limit_f {
        msg!(
            "Cannot borrow above the borrow limit. New total borrow: {} > limit: {}",
            new_borrowed_amount_f.to_display(),
            borrow_reserve.config.borrow_limit
        );
        return err!(LendingError::BorrowLimitExceeded);
    }
    check_obligation_fully_refreshed_and_not_null(obligation, clock.slot)?;

    let remaining_borrow_value = obligation.remaining_borrow_value();
    if remaining_borrow_value == Fraction::ZERO {
        msg!("Remaining borrow value is zero");
        return err!(LendingError::BorrowTooLarge);
    }

    check_same_elevation_group(obligation, borrow_reserve)?;

    check_elevation_group_borrowing_enabled(lending_market, obligation)?;
    check_non_elevation_group_borrowing_enabled(obligation)?;

    let remaining_reserve_capacity = borrow_limit_f.saturating_sub(reserve_liquidity_borrowed_f);

    if remaining_reserve_capacity == Fraction::ZERO {
        msg!("Borrow reserve is at full capacity");
        return err!(LendingError::BorrowLimitExceeded);
    }

    let CalculateBorrowResult {
        borrow_amount_f,
        receive_amount,
        borrow_fee,
        referrer_fee,
    } = borrow_reserve.calculate_borrow(
        liquidity_amount,
        remaining_borrow_value,
        remaining_reserve_capacity,
        lending_market.referral_fee_bps,
        obligation.elevation_group != ELEVATION_GROUP_NONE,
        referrer_token_state.is_some(),
    )?;

    let borrow_amount = borrow_amount_f.to_ceil();
    msg!("Requested {}, allowed {}", liquidity_amount, borrow_amount);

    add_to_withdrawal_accum(
        &mut borrow_reserve.config.debt_withdrawal_cap,
        borrow_amount,
        u64::try_from(clock.unix_timestamp).unwrap(),
    )?;

    if receive_amount == 0 {
        msg!("Borrow amount is too small to receive liquidity after fees");
        return err!(LendingError::BorrowTooSmall);
    }

    borrow_reserve.liquidity.borrow(borrow_amount_f)?;
    borrow_reserve.last_update.mark_stale();

    let cumulative_borrow_rate_bf =
        BigFraction::from(borrow_reserve.liquidity.cumulative_borrow_rate_bsf);

    let borrow_index = {
        let (obligation_liquidity, borrow_index) = obligation.find_or_add_liquidity_to_borrows(
            borrow_reserve_pk,
            cumulative_borrow_rate_bf,
            borrow_reserve.config.get_asset_tier(),
        )?;

        obligation_liquidity.borrow(borrow_amount_f);

        borrow_index
    };

    if let Some(mut referrer_token_state) = referrer_token_state {
        if lending_market.referral_fee_bps > 0 {
            add_referrer_fee(
                borrow_reserve,
                &mut referrer_token_state,
                Fraction::from_num(referrer_fee),
            )?;

            borrow_reserve.liquidity.available_amount += referrer_fee;
        }
    }

    obligation.has_debt = 1;
    obligation.last_update.mark_stale();

    let new_utilization_rate = borrow_reserve.liquidity.utilization_rate()?;
    let utilization_limit = borrow_reserve
        .config
        .utilization_limit_block_borrowing_above;
    if new_utilization_rate >= Fraction::from_percent(utilization_limit) && utilization_limit != 0 {
        msg!(
            "Borrowing above utilization rate is disabled, current {}, new {}, limit {}",
            current_utilization.to_display(),
            new_utilization_rate.to_display(),
            utilization_limit
        );
        return err!(LendingError::BorrowingAboveUtilizationRateDisabled);
    }

    validate_obligation_asset_tiers(obligation)?;

    let elevation_group = lending_market.get_elevation_group(obligation.elevation_group)?;
    utils::update_elevation_group_debt_trackers_on_borrow(
        borrow_amount,
        obligation,
        borrow_index,
        elevation_group,
        &borrow_reserve_pk,
        borrow_reserve,
        deposit_reserves_iter,
    )?;

    post_borrow_obligation_invariants(
        borrow_amount_f,
        obligation,
        borrow_reserve,
        Fraction::from_bits(obligation.borrows[borrow_index].market_value_sf),
        Fraction::from_bits(lending_market.min_net_value_in_obligation_sf),
    )?;

    Ok(CalculateBorrowResult {
        borrow_amount_f,
        receive_amount,
        borrow_fee,
        referrer_fee,
    })
}

pub fn deposit_obligation_collateral(
    lending_market: &LendingMarket,
    deposit_reserve: &mut Reserve,
    obligation: &mut Obligation,
    slot: Slot,
    collateral_amount: u64,
    deposit_reserve_pk: Pubkey,
) -> Result<()> {
    if collateral_amount == 0 {
        msg!("Collateral amount provided cannot be zero");
        return err!(LendingError::InvalidAmount);
    }

    if deposit_reserve
        .last_update
        .is_stale(slot, PriceStatusFlags::NONE)?
    {
        msg!("Deposit reserve is stale and must be refreshed in the current slot");
        return err!(LendingError::ReserveStale);
    }

    if deposit_reserve.config.disable_usage_as_coll_outside_emode > 0
        && obligation.elevation_group == ELEVATION_GROUP_NONE
        && obligation.borrow_factor_adjusted_debt_value_sf > 0
    {
        msg!("Deposit reserve is disabled for usage as collateral outside elevation group");
        return err!(LendingError::DepositDisabledOutsideElevationGroup);
    }

    check_same_elevation_group(obligation, deposit_reserve)?;
    let elevation_group = get_elevation_group(obligation.elevation_group, lending_market)?;
    let pre_deposit_count = obligation.deposits_count();
    let total_borrowed_amount = obligation.get_borrowed_amount_if_single_token();
    let asset_tier = deposit_reserve.config.get_asset_tier();

    let new_deposit_initializer = |obligation_collateral: &mut ObligationCollateral| -> Result<()> {
        utils::update_elevation_group_debt_trackers_on_new_deposit(
            total_borrowed_amount,
            obligation_collateral,
            pre_deposit_count,
            elevation_group,
            &deposit_reserve_pk,
            deposit_reserve,
        )
    };

    let pre_collateral_market_value_f = {
        let obligation_collateral = obligation.find_or_add_collateral_to_deposits(
            deposit_reserve_pk,
            asset_tier,
            new_deposit_initializer,
        )?;

        obligation_collateral.deposit(collateral_amount)?;

        Fraction::from_bits(obligation_collateral.market_value_sf)
    };

    obligation.last_update.mark_stale();

    deposit_reserve.last_update.mark_stale();

    validate_obligation_asset_tiers(obligation)?;
    post_deposit_obligation_invariants(
        deposit_reserve
            .collateral_exchange_rate()?
            .fraction_collateral_to_liquidity(Fraction::from(collateral_amount)),
        obligation,
        deposit_reserve,
        pre_collateral_market_value_f,
        Fraction::from_bits(lending_market.min_net_value_in_obligation_sf),
    )?;

    Ok(())
}

pub fn withdraw_obligation_collateral(
    lending_market: &LendingMarket,
    withdraw_reserve: &mut Reserve,
    obligation: &mut Obligation,
    collateral_amount: u64,
    slot: Slot,
    withdraw_reserve_pk: Pubkey,
) -> Result<u64> {
    if collateral_amount == 0 {
        return err!(LendingError::InvalidAmount);
    }

    let is_borrows_empty = obligation.borrows_empty();

    let required_price_status = if is_borrows_empty {
        PriceStatusFlags::NONE
    } else {
        PriceStatusFlags::ALL_CHECKS
    };

    if withdraw_reserve
        .last_update
        .is_stale(slot, required_price_status)?
    {
        msg!(
            "Withdraw reserve is stale and must be refreshed in the current slot, price status: {:08b}",
            withdraw_reserve.last_update.get_price_status().0
        );
        return err!(LendingError::ReserveStale);
    }

    if obligation
        .last_update
        .is_stale(slot, required_price_status)?
    {
        msg!(
            "Obligation is stale and must be refreshed in the current slot, price status: {:08b}",
            obligation.last_update.get_price_status().0
        );
        return err!(LendingError::ObligationStale);
    }

    let collateral_index = obligation.position_of_collateral_in_deposits(withdraw_reserve_pk)?;
    let collateral = &obligation.deposits[collateral_index];
    if collateral.deposited_amount == 0 {
        return err!(LendingError::ObligationCollateralEmpty);
    }

    check_elevation_group_borrowing_enabled(lending_market, obligation)?;

    if obligation.num_of_obsolete_reserves > 0
        && withdraw_reserve.config.status() == ReserveStatus::Active
    {
        return err!(LendingError::ObligationInDeprecatedReserve);
    }

    let withdraw_amount = if is_borrows_empty {
        if collateral_amount == u64::MAX {
            collateral.deposited_amount
        } else {
            collateral.deposited_amount.min(collateral_amount)
        }
    } else if obligation.deposited_value_sf == 0 {
        msg!("Obligation deposited value is zero");
        return err!(LendingError::ObligationDepositsZero);
    } else {
        let (reserve_loan_to_value_pct, _) = get_max_ltv_and_liquidation_threshold(
            withdraw_reserve,
            get_elevation_group(obligation.elevation_group, lending_market)?,
        )?;

        let max_withdraw_value = obligation.max_withdraw_value(reserve_loan_to_value_pct)?;

        if max_withdraw_value == Fraction::ZERO {
            msg!("Maximum withdraw value is zero");
            return err!(LendingError::WithdrawTooLarge);
        }

        let collateral_value = Fraction::from_bits(collateral.market_value_sf);
        let withdraw_amount = if collateral_amount == u64::MAX {
            let withdraw_value = max_withdraw_value.min(collateral_value);
            let withdraw_ratio = withdraw_value / collateral_value;

            let ratioed_amount_f = withdraw_ratio * u128::from(collateral.deposited_amount);
            let ratioed_amount: u64 = ratioed_amount_f.to_floor();

            min(collateral.deposited_amount, ratioed_amount)
        } else {
            let withdraw_amount = collateral_amount.min(collateral.deposited_amount);
            let withdraw_ratio =
                Fraction::from(withdraw_amount) / u128::from(collateral.deposited_amount);
            let withdraw_value = collateral_value * withdraw_ratio;
            if withdraw_value > max_withdraw_value {
                msg!("Withdraw value cannot exceed maximum withdraw value, collateral_amount={}, collateral.deposited_amount={} withdraw_pct={}, collateral_value={}, max_withdraw_value={} withdraw_value={}",
                    collateral_amount,
                    collateral.deposited_amount,
                    withdraw_ratio,
                    collateral_value,
                    max_withdraw_value,
                    withdraw_value);
                return err!(LendingError::WithdrawTooLarge);
            }
            withdraw_amount
        };

        if withdraw_amount == 0 {
            msg!("Withdraw amount is too small to transfer collateral");
            return err!(LendingError::WithdrawTooSmall);
        }
        withdraw_amount
    };

    let previous_debt_in_elevation_group =
        collateral.borrowed_amount_against_this_collateral_in_elevation_group;
    let is_full_withdrawal = obligation.withdraw(withdraw_amount, collateral_index)?;
    obligation.last_update.mark_stale();

    if is_full_withdrawal == WithdrawResult::Full {
        utils::update_elevation_group_debt_trackers_on_full_withdraw(
            previous_debt_in_elevation_group,
            obligation.elevation_group,
            withdraw_reserve,
        )?;
    }

    post_withdraw_obligation_invariants(
        withdraw_reserve
            .collateral_exchange_rate()?
            .fraction_collateral_to_liquidity(Fraction::from(withdraw_amount)),
        obligation,
        withdraw_reserve,
        Fraction::from_bits(obligation.deposits[collateral_index].market_value_sf),
        Fraction::from_bits(lending_market.min_net_value_in_obligation_sf),
    )?;

    Ok(withdraw_amount)
}

pub fn redeem_reserve_collateral(
    reserve: &mut Reserve,
    collateral_amount: u64,
    clock: &Clock,
    add_amount_to_withdrawal_caps: bool,
) -> Result<u64> {
    if collateral_amount == 0 {
        msg!("Collateral amount provided cannot be zero");
        return err!(LendingError::InvalidAmount);
    }

    if reserve
        .last_update
        .is_stale(clock.slot, PriceStatusFlags::NONE)?
    {
        msg!("Reserve is stale and must be refreshed in the current slot");
        return err!(LendingError::ReserveStale);
    }

    let liquidity_amount = reserve.redeem_collateral(collateral_amount)?;
    refresh_reserve_limit_timestamps(reserve, clock.slot)?;
    reserve.last_update.mark_stale();

    if add_amount_to_withdrawal_caps {
        add_to_withdrawal_accum(
            &mut reserve.config.deposit_withdrawal_cap,
            liquidity_amount,
            u64::try_from(clock.unix_timestamp).unwrap(),
        )?;
    }

    Ok(liquidity_amount)
}

pub fn redeem_fees(reserve: &mut Reserve, slot: Slot) -> Result<u64> {
    if reserve.last_update.is_stale(slot, PriceStatusFlags::NONE)? {
        msg!(
            "reserve is stale and must be refreshed in the current slot, price status: {:08b}",
            reserve.last_update.get_price_status().0
        );
        return err!(LendingError::ReserveStale);
    }

    let withdraw_amount = reserve.calculate_redeem_fees()?;

    if withdraw_amount == 0 {
        return err!(LendingError::InsufficientProtocolFeesToRedeem);
    }

    reserve.liquidity.redeem_fees(withdraw_amount)?;
    reserve.last_update.mark_stale();

    Ok(withdraw_amount)
}

pub fn repay_obligation_liquidity<'info, T>(
    repay_reserve: &mut Reserve,
    obligation: &mut Obligation,
    clock: &Clock,
    liquidity_amount: u64,
    repay_reserve_pk: Pubkey,
    lending_market: &LendingMarket,
    deposit_reserves_iter: impl Iterator<Item = T>,
) -> Result<u64>
where
    T: AnyAccountLoader<'info, Reserve>,
{
    if liquidity_amount == 0 {
        msg!("Liquidity amount provided cannot be zero");
        return err!(LendingError::InvalidAmount);
    }

    if repay_reserve
        .last_update
        .is_stale(clock.slot, PriceStatusFlags::NONE)?
    {
        msg!("Repay reserve is stale and must be refreshed in the current slot");
        return err!(LendingError::ReserveStale);
    }

    let (liquidity, liquidity_index) =
        obligation.find_liquidity_in_borrows_mut(repay_reserve_pk)?;
    if liquidity.borrowed_amount_sf == 0 {
        msg!("Liquidity borrowed amount is zero");
        return err!(LendingError::ObligationLiquidityEmpty);
    }

    let cumulative_borrow_rate =
        BigFraction::from(repay_reserve.liquidity.cumulative_borrow_rate_bsf);
    liquidity.accrue_interest(cumulative_borrow_rate)?;

    let CalculateRepayResult {
        settle_amount_f: settle_amount,
        repay_amount,
    } = repay_reserve.calculate_repay(
        liquidity_amount,
        Fraction::from_bits(liquidity.borrowed_amount_sf),
    )?;

    if repay_amount == 0 {
        msg!("Repay amount is too small to transfer liquidity");
        return err!(LendingError::RepayTooSmall);
    }

    sub_from_withdrawal_accum(
        &mut repay_reserve.config.debt_withdrawal_cap,
        repay_amount,
        u64::try_from(clock.unix_timestamp).unwrap(),
    )?;

    update_elevation_group_debt_trackers_on_repay(
        repay_amount,
        obligation,
        liquidity_index,
        repay_reserve,
        deposit_reserves_iter,
    )?;

    repay_reserve.liquidity.repay(repay_amount, settle_amount)?;
    repay_reserve.last_update.mark_stale();

    obligation.repay(settle_amount, liquidity_index)?;
    obligation.update_has_debt();
    obligation.last_update.mark_stale();

    post_repay_obligation_invariants(
        settle_amount,
        obligation,
        repay_reserve,
        Fraction::from_bits(obligation.borrows[liquidity_index].market_value_sf),
        Fraction::from_bits(lending_market.min_net_value_in_obligation_sf),
    )?;

    Ok(repay_amount)
}

pub fn request_elevation_group<'info, T, U>(
    obligation: &mut Obligation,
    lending_market: &LendingMarket,
    slot: Slot,
    new_elevation_group: u8,
    deposit_reserves_iter: impl Iterator<Item = T> + Clone,
    borrow_reserves_iter: impl Iterator<Item = T> + Clone,
    mut referrer_token_states_iter: impl Iterator<Item = U>,
) -> Result<()>
where
    T: AnyAccountLoader<'info, Reserve>,
    U: AnyAccountLoader<'info, ReferrerTokenState>,
{
    check_obligation_fully_refreshed_and_not_null(obligation, slot)?;

    require!(
        obligation.elevation_group != new_elevation_group,
        LendingError::ElevationGroupAlreadyActivated
    );

    reset_elevation_group_debts(
        obligation,
        get_elevation_group(obligation.elevation_group, lending_market)?,
        deposit_reserves_iter.clone(),
        borrow_reserves_iter.clone(),
    )?;

    let elevation_group = get_elevation_group(new_elevation_group, lending_market)?;

    if let Some(elevation_group) = elevation_group {
        require!(
            !elevation_group.new_loans_disabled(),
            LendingError::ElevationGroupNewLoansDisabled
        );

        require!(
            elevation_group.debt_reserve != Pubkey::default(),
            LendingError::ElevationGroupWithoutDebtReserve
        );

        require_gt!(
            elevation_group.max_reserves_as_collateral,
            0,
            LendingError::ElevationGroupMaxCollateralReserveZero
        );
    }

    let RefreshObligationBorrowsResult {
        borrow_factor_adjusted_debt_value_f: borrow_factor_adjusted_debt_value,
        borrowed_amount_in_elevation_group,
        ..
    } = refresh_obligation_borrows(
        obligation,
        lending_market,
        slot,
        elevation_group,
        borrow_reserves_iter.clone(),
        &mut referrer_token_states_iter,
    )?;

    let RefreshObligationDepositsResult {
        allowed_borrow_value_f: allowed_borrow_value,
        ..
    } = refresh_obligation_deposits(
        obligation,
        lending_market,
        slot,
        elevation_group,
        deposit_reserves_iter.clone(),
        borrowed_amount_in_elevation_group,
    )?;

    if allowed_borrow_value < borrow_factor_adjusted_debt_value {
        msg!("The obligation is not healthy enough to support the new elevation group");
        return Err(
            error!(LendingError::UnhealthyElevationGroupLtv).with_values((
                allowed_borrow_value.to_display(),
                borrow_factor_adjusted_debt_value.to_display(),
            )),
        );
    }

    msg!(
        "Previous elevation group: {} . Requested elevation group for: {}",
        obligation.elevation_group,
        new_elevation_group
    );

    obligation.elevation_group = new_elevation_group;
    obligation.last_update.mark_stale();

    utils::check_elevation_group_borrow_limit_constraints(
        obligation,
        elevation_group,
        deposit_reserves_iter,
        borrow_reserves_iter,
    )?;

    Ok(())
}

fn reset_elevation_group_debts<'info, T>(
    obligation: &mut Obligation,
    elevation_group: Option<&ElevationGroup>,
    mut deposit_reserves_iter: impl Iterator<Item = T> + Clone,
    mut borrow_reserves_iter: impl Iterator<Item = T> + Clone,
) -> Result<()>
where
    T: AnyAccountLoader<'info, Reserve>,
{
    if let Some(elevation_group) = elevation_group {
        let elevation_group_index = elevation_group.get_index();
        let mut obligation_deposits_iter = obligation
            .deposits
            .iter_mut()
            .filter(|deposit| deposit.deposit_reserve != Pubkey::default());

        for (deposit, reserve) in obligation_deposits_iter
            .by_ref()
            .zip(deposit_reserves_iter.by_ref())
        {
            require_keys_eq!(
                deposit.deposit_reserve,
                reserve.get_pubkey(),
                LendingError::InvalidAccountInput
            );

            let mut reserve = reserve.get_mut()?;

            reserve.borrowed_amounts_against_this_reserve_in_elevation_groups
                [elevation_group_index] = reserve
                .borrowed_amounts_against_this_reserve_in_elevation_groups[elevation_group_index]
                .saturating_sub(deposit.borrowed_amount_against_this_collateral_in_elevation_group);

            deposit.borrowed_amount_against_this_collateral_in_elevation_group = 0;
        }

        require!(
            obligation_deposits_iter.next().is_none(),
            LendingError::InvalidAccountInput
        );
        require!(
            deposit_reserves_iter.next().is_none(),
            LendingError::InvalidAccountInput
        );
    } else {
        let mut obligation_borrows_iter = obligation
            .borrows
            .iter_mut()
            .filter(|borrow| borrow.borrow_reserve != Pubkey::default());

        for (borrow, reserve) in obligation_borrows_iter
            .by_ref()
            .zip(borrow_reserves_iter.by_ref())
        {
            let mut reserve = reserve.get_mut()?;
            reserve.borrowed_amount_outside_elevation_group = reserve
                .borrowed_amount_outside_elevation_group
                .saturating_sub(borrow.borrowed_amount_outside_elevation_groups);

            borrow.borrowed_amount_outside_elevation_groups = 0;
        }

        require!(
            obligation_borrows_iter.next().is_none(),
            LendingError::InvalidAccountInput
        );
        require!(
            borrow_reserves_iter.next().is_none(),
            LendingError::InvalidAccountInput
        );
    }

    Ok(())
}

pub fn refresh_obligation_deposits<'info, T>(
    obligation: &mut Obligation,
    lending_market: &LendingMarket,
    slot: Slot,
    elevation_group: Option<&ElevationGroup>,
    mut reserves_iter: impl Iterator<Item = T>,
    borrowed_amount_in_elevation_group: Option<u64>,
) -> Result<RefreshObligationDepositsResult>
where
    T: AnyAccountLoader<'info, Reserve>,
{
    let mut lowest_deposit_liquidation_ltv_threshold = u8::MAX;
    let mut deposited_value = Fraction::ZERO;
    let mut allowed_borrow_value = Fraction::ZERO;
    let mut unhealthy_borrow_value = Fraction::ZERO;
    let mut num_of_obsolete_reserves = 0;
    let mut prices_state = PriceStatusFlags::all();
    let mut borrowing_disabled = false;
    let mut collaterals_count = 0;

    let elevation_group_and_borrowed_amount: Option<(&ElevationGroup, u64)> = match (
        elevation_group,
        borrowed_amount_in_elevation_group,
    ) {
        (Some(elevation_group), Some(borrowed_amount)) => Some((elevation_group, borrowed_amount)),
        (None, None) => None,
        _ => {
            panic!("Elevation group and borrowed amount must be both set or both unset when refreshing deposits.");
        }
    };

    for (index, deposit) in obligation
        .deposits
        .iter_mut()
        .enumerate()
        .filter(|(_, deposit)| deposit.deposit_reserve != Pubkey::default())
    {
        let deposit_reserve = reserves_iter
            .next()
            .ok_or(error!(LendingError::InvalidAccountInput))?;

        let deposit_reserve_info_key = deposit_reserve.get_pubkey();

        let mut deposit_reserve = deposit_reserve
            .get_mut()
            .map_err(|_| error!(LendingError::InvalidAccountInput))?;

        if elevation_group.is_none()
            && deposit_reserve.config.disable_usage_as_coll_outside_emode > 0
        {
            borrowing_disabled = true;
        }

        if deposit_reserve.config.status() == ReserveStatus::Obsolete {
            num_of_obsolete_reserves += 1;
        }

        check_obligation_collateral_deposit_reserve(
            deposit,
            &deposit_reserve,
            deposit_reserve_info_key,
            index,
            slot,
        )?;

        if deposit.deposited_amount > 0 {
            collaterals_count += 1;
        }

        if let Some((elevation_group, debt_amount)) = elevation_group_and_borrowed_amount {
            let elevation_group_index = elevation_group.get_index();
            require!(
                deposit_reserve
                    .config
                    .elevation_groups
                    .contains(&elevation_group.id),
                LendingError::InconsistentElevationGroup
            );

            require_keys_neq!(
                deposit_reserve_info_key,
                elevation_group.debt_reserve,
                LendingError::ElevationGroupDebtReserveAsCollateral
            );

            deposit_reserve.borrowed_amounts_against_this_reserve_in_elevation_groups
                [elevation_group_index] = deposit_reserve
                .borrowed_amounts_against_this_reserve_in_elevation_groups[elevation_group_index]
                .saturating_sub(deposit.borrowed_amount_against_this_collateral_in_elevation_group);
            deposit_reserve.borrowed_amounts_against_this_reserve_in_elevation_groups
                [elevation_group_index] += debt_amount;
            deposit.borrowed_amount_against_this_collateral_in_elevation_group = debt_amount;
        }

        let market_value_f =
            calculate_obligation_collateral_market_value(&deposit_reserve, deposit)?;
        deposit.market_value_sf = market_value_f.to_bits();

        let (coll_ltv_pct, coll_liquidation_threshold_pct) =
            get_max_ltv_and_liquidation_threshold(&deposit_reserve, elevation_group)?;

        if market_value_f >= lending_market.min_value_skip_liquidation_ltv_bf_checks
            && coll_liquidation_threshold_pct > 0
        {
            lowest_deposit_liquidation_ltv_threshold =
                lowest_deposit_liquidation_ltv_threshold.min(coll_liquidation_threshold_pct);
        }

        deposited_value = deposited_value.add(market_value_f);
        allowed_borrow_value += market_value_f * Fraction::from_percent(coll_ltv_pct);
        unhealthy_borrow_value +=
            market_value_f * Fraction::from_percent(coll_liquidation_threshold_pct);

        obligation.deposits_asset_tiers[index] = deposit_reserve.config.asset_tier;

        prices_state &= deposit_reserve.last_update.get_price_status();

        xmsg!(
            "Deposit: {} amount: {} value: {}",
            &deposit_reserve.config.token_info.symbol(),
            deposit_reserve
                .collateral_exchange_rate()?
                .fraction_collateral_to_liquidity(deposit.deposited_amount.into())
                .to_display(),
            market_value_f.to_display()
        );
    }

    if let Some(elevation_group) = elevation_group {
        require_gte!(
            elevation_group.max_reserves_as_collateral,
            collaterals_count,
            LendingError::ObligationCollateralExceedsElevationGroupLimit
        );
    }

    Ok(RefreshObligationDepositsResult {
        lowest_deposit_liquidation_ltv_threshold,
        num_of_obsolete_reserves,
        deposited_value_f: deposited_value,
        allowed_borrow_value_f: allowed_borrow_value,
        unhealthy_borrow_value_f: unhealthy_borrow_value,
        prices_state,
        borrowing_disabled,
    })
}

pub fn refresh_obligation_borrows<'info, T, U>(
    obligation: &mut Obligation,
    lending_market: &LendingMarket,
    slot: u64,
    elevation_group: Option<&ElevationGroup>,
    mut reserves_iter: impl Iterator<Item = T>,
    mut referrer_token_states_iter: impl Iterator<Item = U>,
) -> Result<RefreshObligationBorrowsResult>
where
    T: AnyAccountLoader<'info, Reserve>,
    U: AnyAccountLoader<'info, ReferrerTokenState>,
{
    let mut borrowed_assets_market_value = Fraction::ZERO;
    let mut borrow_factor_adjusted_debt_value = Fraction::ZERO;
    let mut prices_state = PriceStatusFlags::all();
    let mut highest_borrow_factor_f = Fraction::ONE;

    let obligation_has_referrer = obligation.has_referrer();
    let mut borrowed_amounts_accumulator_for_elevation_group = 0_u64;
    let mut num_borrow_reserves = 0;

    for (index, borrow) in obligation
        .borrows
        .iter_mut()
        .enumerate()
        .filter(|(_, borrow)| borrow.borrow_reserve != Pubkey::default())
    {
        num_borrow_reserves += 1;
        let borrow_reserve = reserves_iter
            .next()
            .ok_or(error!(LendingError::InvalidAccountInput))?;

        let borrow_reserve_info_key = borrow_reserve.get_pubkey();

        let borrow_reserve = &mut borrow_reserve
            .get_mut()
            .map_err(|_| error!(LendingError::InvalidAccountInput))?;

        check_obligation_liquidity_borrow_reserve(
            borrow,
            borrow_reserve,
            borrow_reserve_info_key,
            index,
            slot,
        )?;

        let cumulative_borrow_rate_bf =
            BigFraction::from(borrow_reserve.liquidity.cumulative_borrow_rate_bsf);

        let previous_borrowed_amount_f = Fraction::from_bits(borrow.borrowed_amount_sf);

        borrow.accrue_interest(cumulative_borrow_rate_bf)?;

        let borrowed_amount_f = Fraction::from_bits(borrow.borrowed_amount_sf);
        let borrowed_amount = borrowed_amount_f.to_ceil::<u64>();
        borrowed_amounts_accumulator_for_elevation_group += borrowed_amount;
        {
            if let Some(elevation_group) = elevation_group {
                require!(
                    borrow_reserve
                        .config
                        .elevation_groups
                        .contains(&elevation_group.id),
                    LendingError::InconsistentElevationGroup
                );
                require_keys_eq!(
                    borrow_reserve_info_key,
                    elevation_group.debt_reserve,
                    LendingError::ElevationGroupHasAnotherDebtReserve
                );
            } else {
                borrow_reserve.borrowed_amount_outside_elevation_group = borrow_reserve
                    .borrowed_amount_outside_elevation_group
                    .saturating_sub(borrow.borrowed_amount_outside_elevation_groups);
                borrow_reserve.borrowed_amount_outside_elevation_group += borrowed_amount;
                borrow.borrowed_amount_outside_elevation_groups = borrowed_amount;
            }
        }

        accumulate_referrer_fees(
            borrow_reserve_info_key,
            borrow_reserve,
            &obligation.referrer,
            lending_market.referral_fee_bps,
            obligation.last_update.slots_elapsed(slot)?,
            borrowed_amount_f,
            previous_borrowed_amount_f,
            obligation_has_referrer,
            &mut referrer_token_states_iter,
        )?;

        let market_value_f = calculate_obligation_liquidity_market_value(borrow_reserve, borrow)?;

        borrow.market_value_sf = market_value_f.to_bits();

        borrowed_assets_market_value += market_value_f;

        let borrow_factor_f = borrow_reserve.borrow_factor_f(elevation_group.is_some());

        if market_value_f >= lending_market.min_value_skip_liquidation_ltv_bf_checks {
            highest_borrow_factor_f = highest_borrow_factor_f.max(borrow_factor_f);
        }

        let borrow_factor_adjusted_market_value: Fraction = market_value_f * borrow_factor_f;
        borrow.borrow_factor_adjusted_market_value_sf =
            borrow_factor_adjusted_market_value.to_bits();

        borrow_factor_adjusted_debt_value += borrow_factor_adjusted_market_value;

        obligation.borrows_asset_tiers[index] = borrow_reserve.config.asset_tier;

        obligation.has_debt = 1;

        prices_state &= borrow_reserve.last_update.get_price_status();

        xmsg!(
            "Borrow: {} amount: {} value: {} value_bf: {}",
            &borrow_reserve.config.token_info.symbol(),
            Fraction::from_bits(borrow.borrowed_amount_sf),
            market_value_f.to_display(),
            borrow_factor_adjusted_market_value.to_display()
        );
    }

    let borrowed_amount_in_elevation_group = if elevation_group.is_some() {
        require!(
            num_borrow_reserves <= 1,
            LendingError::InconsistentElevationGroup
        );
        Some(borrowed_amounts_accumulator_for_elevation_group)
    } else {
        None
    };

    Ok(RefreshObligationBorrowsResult {
        borrowed_assets_market_value_f: borrowed_assets_market_value,
        borrow_factor_adjusted_debt_value_f: borrow_factor_adjusted_debt_value,
        borrowed_amount_in_elevation_group,
        prices_state,
        highest_borrow_factor_pct: highest_borrow_factor_f.to_percent::<u64>().unwrap(),
    })
}

pub fn refresh_obligation<'info, T, U>(
    obligation: &mut Obligation,
    lending_market: &LendingMarket,
    slot: Slot,
    mut deposit_reserves_iter: impl Iterator<Item = T>,
    mut borrow_reserves_iter: impl Iterator<Item = T>,
    mut referrer_token_states_iter: impl Iterator<Item = U>,
) -> Result<()>
where
    T: AnyAccountLoader<'info, Reserve>,
    U: AnyAccountLoader<'info, ReferrerTokenState>,
{
    let elevation_group = get_elevation_group(obligation.elevation_group, lending_market)?;

    let RefreshObligationBorrowsResult {
        borrow_factor_adjusted_debt_value_f,
        borrowed_assets_market_value_f,
        prices_state: borrows_prices_state,
        borrowed_amount_in_elevation_group,
        highest_borrow_factor_pct,
    } = refresh_obligation_borrows(
        obligation,
        lending_market,
        slot,
        elevation_group,
        &mut borrow_reserves_iter,
        &mut referrer_token_states_iter,
    )?;

    let RefreshObligationDepositsResult {
        lowest_deposit_liquidation_ltv_threshold,
        num_of_obsolete_reserves,
        deposited_value_f,
        allowed_borrow_value_f: allowed_borrow_value,
        unhealthy_borrow_value_f: unhealthy_borrow_value,
        prices_state: deposits_prices_state,
        borrowing_disabled,
    } = refresh_obligation_deposits(
        obligation,
        lending_market,
        slot,
        elevation_group,
        &mut deposit_reserves_iter,
        borrowed_amount_in_elevation_group,
    )?;

    obligation.borrowed_assets_market_value_sf = borrowed_assets_market_value_f.to_bits();

    obligation.deposited_value_sf = deposited_value_f.to_bits();

    obligation.borrow_factor_adjusted_debt_value_sf = borrow_factor_adjusted_debt_value_f.to_bits();

    obligation.allowed_borrow_value_sf = min(
        allowed_borrow_value,
        Fraction::from(lending_market.global_allowed_borrow_value),
    )
    .to_bits();

    obligation.unhealthy_borrow_value_sf = min(
        unhealthy_borrow_value,
        Fraction::from(lending_market.global_unhealthy_borrow_value),
    )
    .to_bits();

    obligation.lowest_reserve_deposit_liquidation_ltv =
        lowest_deposit_liquidation_ltv_threshold.into();

    obligation.num_of_obsolete_reserves = num_of_obsolete_reserves;

    obligation.borrowing_disabled = borrowing_disabled.into();
    obligation.highest_borrow_factor_pct = highest_borrow_factor_pct;

    let prices_state = deposits_prices_state.intersection(borrows_prices_state);
    obligation.last_update.update_slot(slot, Some(prices_state));

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn liquidate_and_redeem<'info, T>(
    lending_market: &LendingMarket,
    repay_reserve: &dyn AnyAccountLoader<Reserve>,
    withdraw_reserve: &dyn AnyAccountLoader<Reserve>,
    obligation: &mut Obligation,
    clock: &Clock,
    liquidity_amount: u64,
    min_acceptable_received_liquidity_amount: u64,
    max_allowed_ltv_override_pct_opt: Option<u64>,
    deposit_reserves_iter: impl Iterator<Item = T>,
) -> Result<LiquidateAndRedeemResult>
where
    T: AnyAccountLoader<'info, Reserve>,
{
    let LiquidateObligationResult {
        repay_amount,
        withdraw_collateral_amount,
        withdraw_amount,
        liquidation_bonus_rate,
        ..
    } = liquidate_obligation(
        lending_market,
        repay_reserve,
        withdraw_reserve,
        obligation,
        clock,
        liquidity_amount,
        max_allowed_ltv_override_pct_opt,
        deposit_reserves_iter,
    )?;

    let withdraw_reserve = &mut withdraw_reserve.get_mut()?;

    let total_withdraw_liquidity_amount = post_liquidate_redeem(
        withdraw_reserve,
        repay_amount,
        withdraw_amount,
        withdraw_collateral_amount,
        liquidation_bonus_rate,
        min_acceptable_received_liquidity_amount,
        clock,
    )?;

    Ok(LiquidateAndRedeemResult {
        repay_amount,
        withdraw_amount,
        total_withdraw_liquidity_amount,
        withdraw_collateral_amount,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn liquidate_obligation<'info, T>(
    lending_market: &LendingMarket,
    repay_reserve: &dyn AnyAccountLoader<Reserve>,
    withdraw_reserve: &dyn AnyAccountLoader<Reserve>,
    obligation: &mut Obligation,
    clock: &Clock,
    liquidity_amount: u64,
    max_allowed_ltv_override_pct_opt: Option<u64>,
    deposit_reserves_iter: impl Iterator<Item = T>,
) -> Result<LiquidateObligationResult>
where
    T: AnyAccountLoader<'info, Reserve>,
{
    xmsg!(
        "Liquidating liquidation_close_factor_pct: {}, liquidation_max_value: {}",
        lending_market.liquidation_max_debt_close_factor_pct,
        lending_market.max_liquidatable_debt_market_value_at_once
    );
    let repay_reserve_ref = repay_reserve.get()?;
    let withdraw_reserve_ref = withdraw_reserve.get()?;

    let slot = clock.slot;

    if withdraw_reserve_ref.config.loan_to_value_pct == 0
        || withdraw_reserve_ref.config.liquidation_threshold_pct == 0
    {
        xmsg!("Max LTV of the withdraw reserve is 0 and can't be used for liquidation");
        return err!(LendingError::CollateralNonLiquidatable);
    }

    utils::assert_obligation_liquidatable(
        &repay_reserve_ref,
        &withdraw_reserve_ref,
        obligation,
        liquidity_amount,
        slot,
    )?;

    let (liquidity, liquidity_index) =
        obligation.find_liquidity_in_borrows(repay_reserve.get_pubkey())?;
    if liquidity.borrow_factor_adjusted_market_value_sf == 0 {
        msg!("Obligation borrow value is zero");
        return err!(LendingError::ObligationLiquidityEmpty);
    }

    let collateral_index =
        obligation.position_of_collateral_in_deposits(withdraw_reserve.get_pubkey())?;
    let collateral = &obligation.deposits[collateral_index];
    if collateral.market_value_sf == 0 {
        msg!("Obligation deposit value is zero");
        return err!(LendingError::ObligationCollateralEmpty);
    }

    let is_debt_reserve_highest_borrow_factor =
        repay_reserve_ref.config.borrow_factor_pct >= obligation.highest_borrow_factor_pct;

    let elevation_group = get_elevation_group(obligation.elevation_group, lending_market)?;
    let (_, collateral_liquidation_threshold) =
        get_max_ltv_and_liquidation_threshold(&withdraw_reserve_ref, elevation_group)?;

    let is_collateral_reserve_lowest_liquidation_ltv = collateral_liquidation_threshold as u64
        <= obligation.lowest_reserve_deposit_liquidation_ltv;

    let CalculateLiquidationResult {
        settle_amount_f: settle_amount,
        repay_amount,
        withdraw_amount,
        liquidation_bonus_rate,
    } = liquidation_operations::calculate_liquidation(
        &withdraw_reserve_ref,
        &repay_reserve_ref,
        liquidity_amount,
        lending_market,
        obligation,
        liquidity,
        collateral,
        slot,
        is_debt_reserve_highest_borrow_factor,
        is_collateral_reserve_lowest_liquidation_ltv,
        max_allowed_ltv_override_pct_opt,
    )?;

    let is_full_withdrawal = collateral.deposited_amount == withdraw_amount;

    drop(repay_reserve_ref);
    drop(withdraw_reserve_ref);

    let previous_borrowed_amount_against_this_collateral_in_elevation_group;
    {
        let mut repay_reserve_ref_mut = repay_reserve.get_mut()?;

        utils::update_elevation_group_debt_trackers_on_repay(
            repay_amount,
            obligation,
            liquidity_index,
            &mut repay_reserve_ref_mut,
            deposit_reserves_iter,
        )?;

        previous_borrowed_amount_against_this_collateral_in_elevation_group = obligation.deposits
            [collateral_index]
            .borrowed_amount_against_this_collateral_in_elevation_group;

        utils::repay_and_withdraw_from_obligation_post_liquidation(
            obligation,
            &mut repay_reserve_ref_mut,
            settle_amount,
            withdraw_amount,
            repay_amount,
            liquidity_index,
            collateral_index,
        )?;
    }

    let mut withdraw_reserve_ref_mut = withdraw_reserve.get_mut()?;
    let withdraw_collateral_amount = {
        refresh_reserve(
            &mut withdraw_reserve_ref_mut,
            clock,
            None,
            lending_market.referral_fee_bps,
        )?;
        let collateral_exchange_rate = withdraw_reserve_ref_mut.collateral_exchange_rate()?;
        let max_redeemable_collateral = collateral_exchange_rate
            .liquidity_to_collateral(withdraw_reserve_ref_mut.liquidity.available_amount);
        min(withdraw_amount, max_redeemable_collateral)
    };

    if is_full_withdrawal {
        utils::update_elevation_group_debt_trackers_on_full_withdraw(
            previous_borrowed_amount_against_this_collateral_in_elevation_group,
            obligation.elevation_group,
            &mut withdraw_reserve_ref_mut,
        )?;
    }

    Ok(LiquidateObligationResult {
        settle_amount_f: settle_amount,
        repay_amount,
        withdraw_amount,
        withdraw_collateral_amount,
        liquidation_bonus_rate,
    })
}

pub(crate) fn post_liquidate_redeem(
    withdraw_reserve: &mut Reserve,
    repay_amount: u64,
    withdraw_amount: u64,
    withdraw_collateral_amount: u64,
    liquidation_bonus_rate: Fraction,
    min_acceptable_received_liquidity_amount: u64,
    clock: &Clock,
) -> Result<Option<(u64, u64)>> {
    if withdraw_collateral_amount != 0 {
        let withdraw_liquidity_amount =
            redeem_reserve_collateral(withdraw_reserve, withdraw_collateral_amount, clock, false)?;
        let protocol_fee = liquidation_operations::calculate_protocol_liquidation_fee(
            withdraw_liquidity_amount,
            liquidation_bonus_rate,
            withdraw_reserve.config.protocol_liquidation_fee_pct,
        );
        let net_withdraw_liquidity_amount = withdraw_liquidity_amount - protocol_fee;
        msg!(
            "pnl: Liquidator repaid {} and withdrew {} collateral with fees {}",
            repay_amount,
            net_withdraw_liquidity_amount,
            protocol_fee
        );

        if net_withdraw_liquidity_amount < min_acceptable_received_liquidity_amount {
            return err!(LendingError::LiquidationRewardTooSmall);
        }

        Ok(Some((withdraw_liquidity_amount, protocol_fee)))
    } else {
        let theoretical_withdraw_liquidity_amount = withdraw_reserve
            .collateral_exchange_rate()?
            .collateral_to_liquidity(withdraw_amount);

        if theoretical_withdraw_liquidity_amount < min_acceptable_received_liquidity_amount {
            return err!(LendingError::LiquidationRewardTooSmall);
        }

        msg!(
            "pnl: Liquidator repaid {} and withdrew {} ctokens",
            repay_amount,
            withdraw_amount
        );
        Ok(None)
    }
}

pub fn flash_borrow_reserve_liquidity(reserve: &mut Reserve, liquidity_amount: u64) -> Result<()> {
    if reserve.config.fees.flash_loan_fee_sf == u64::MAX {
        msg!("Flash loans are disabled for this reserve");
        return err!(LendingError::FlashLoansDisabled);
    }

    let liquidity_amount_f = Fraction::from(liquidity_amount);

    reserve.liquidity.borrow(liquidity_amount_f)?;
    reserve.last_update.mark_stale();

    Ok(())
}

pub fn flash_repay_reserve_liquidity<'info, T>(
    lending_market: &LendingMarket,
    reserve: &mut Reserve,
    liquidity_amount: u64,
    slot: Slot,
    referrer_token_state_loader: Option<&T>,
) -> Result<(u64, u64)>
where
    T: AnyAccountLoader<'info, ReferrerTokenState>,
{
    let flash_loan_amount = liquidity_amount;

    let flash_loan_amount_f = Fraction::from(flash_loan_amount);
    let (protocol_fee, referrer_fee) = reserve.config.fees.calculate_flash_loan_fees(
        flash_loan_amount_f,
        lending_market.referral_fee_bps,
        referrer_token_state_loader.is_some(),
    )?;

    reserve
        .liquidity
        .repay(flash_loan_amount, flash_loan_amount_f)?;
    refresh_reserve_limit_timestamps(reserve, slot)?;
    reserve.last_update.mark_stale();

    if let Some(referrer_token_state_loader) = referrer_token_state_loader {
        if lending_market.referral_fee_bps > 0 {
            let referrer_token_state = &mut referrer_token_state_loader.get_mut()?;

            add_referrer_fee(
                reserve,
                referrer_token_state,
                Fraction::from_num(referrer_fee),
            )?;

            reserve.liquidity.available_amount += referrer_fee;
        }
    }

    let flash_loan_amount_with_referral_fee = flash_loan_amount + referrer_fee;

    Ok((flash_loan_amount_with_referral_fee, protocol_fee))
}

pub fn socialize_loss<'info, T>(
    reserve: &mut Reserve,
    reserve_pk: &Pubkey,
    obligation: &mut Obligation,
    liquidity_amount: u64,
    slot: u64,
    deposit_reserves_iter: impl Iterator<Item = T>,
) -> Result<Fraction>
where
    T: AnyAccountLoader<'info, Reserve>,
{
    if reserve
        .last_update
        .is_stale(slot, PriceStatusFlags::ALL_CHECKS)?
    {
        msg!(
            "Reserve is stale and must be refreshed in the current slot, price status: {:08b}",
            reserve.last_update.get_price_status().0
        );
        return Err(LendingError::ReserveStale.into());
    }

    if obligation
        .last_update
        .is_stale(slot, PriceStatusFlags::ALL_CHECKS)?
    {
        msg!(
            "Obligation is stale and must be refreshed in the current slot, price status: {:08b}",
            obligation.last_update.get_price_status().0
        );
        return Err(LendingError::ObligationStale.into());
    }

    if !obligation.deposits_empty() {
        msg!("Obligation hasn't been fully liquidated!");
        return Err(LendingError::CannotSocializeObligationWithCollateral.into());
    }

    if obligation.deposits_empty() && obligation.borrows_empty() {
        msg!("Obligation has no deposits or borrows");
        return Err(LendingError::ObligationEmpty.into());
    }

    let (liquidity, liquidity_index) = obligation.find_liquidity_in_borrows(*reserve_pk)?;

    let liquidity_amount_f = Fraction::from(liquidity_amount);
    let borrowed_amount_f = Fraction::from_bits(liquidity.borrowed_amount_sf);
    let forgive_amount_f = min(liquidity_amount_f, borrowed_amount_f);

    if forgive_amount_f >= reserve.liquidity.total_supply().unwrap() {
        msg!("Reserve becomes deprecated");
        reserve.version = u64::MAX;
    }

    msg!("Forgiving debt amount {}", forgive_amount_f);

    utils::update_elevation_group_debt_trackers_on_repay(
        forgive_amount_f.to_ceil(),
        obligation,
        liquidity_index,
        reserve,
        deposit_reserves_iter,
    )?;

    reserve.liquidity.forgive_debt(forgive_amount_f)?;
    reserve.last_update.mark_stale();

    obligation.repay(forgive_amount_f, liquidity_index)?;
    obligation.update_has_debt();
    obligation.last_update.mark_stale();

    Ok(forgive_amount_f)
}

pub fn add_referrer_fee(
    borrow_reserve: &mut Reserve,
    referrer_token_state: &mut ReferrerTokenState,
    referrer_fee: Fraction,
) -> Result<()> {
    let referrer_fee_sf = referrer_fee.to_sf();
    referrer_token_state.amount_cumulative_sf += referrer_fee_sf;
    referrer_token_state.amount_unclaimed_sf += referrer_fee_sf;

    borrow_reserve.liquidity.accumulated_referrer_fees_sf += referrer_fee_sf;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn accumulate_referrer_fees<'info, T>(
    borrow_reserve_info_key: Pubkey,
    borrow_reserve: &mut Reserve,
    obligation_referrer: &Pubkey,
    lending_market_referral_fee_bps: u16,
    slots_elapsed: u64,
    borrowed_amount_f: Fraction,
    previous_borrowed_amount_f: Fraction,
    obligation_has_referrer: bool,
    referrer_token_states_iter: &mut impl Iterator<Item = T>,
) -> Result<()>
where
    T: AnyAccountLoader<'info, ReferrerTokenState>,
{
    let absolute_referral_rate =
        Fraction::from_bits(borrow_reserve.liquidity.absolute_referral_rate_sf);

    if absolute_referral_rate == Fraction::ZERO {
        return Ok(());
    }

    let fixed_rate = approximate_compounded_interest(
        Fraction::from_bps(borrow_reserve.config.host_fixed_interest_rate_bps),
        slots_elapsed,
    );
    let net_new_debt = borrowed_amount_f - previous_borrowed_amount_f;
    let net_new_fixed_debt = previous_borrowed_amount_f * fixed_rate - previous_borrowed_amount_f;
    if net_new_fixed_debt > net_new_debt {
        return Err(LendingError::CannotCalculateReferralAmountDueToSlotsMismatch.into());
    }
    let net_new_variable_debt_f = net_new_debt - net_new_fixed_debt;

    let referrer_fee_f = net_new_variable_debt_f * absolute_referral_rate;

    let referrer_fee_capped_sf = min(
        referrer_fee_f.to_bits(),
        borrow_reserve.liquidity.pending_referrer_fees_sf,
    );

    borrow_reserve.liquidity.pending_referrer_fees_sf -= referrer_fee_capped_sf;

    if obligation_has_referrer && lending_market_referral_fee_bps > 0 {
        let referrer_token_state_loader = referrer_token_states_iter
            .next()
            .ok_or(error!(LendingError::InvalidAccountInput))?;
        let referrer_token_state = &mut referrer_token_state_loader
            .get_mut()
            .map_err(|_| error!(LendingError::InvalidAccountInput))?;

        validate_referrer_token_state(
            referrer_token_state,
            referrer_token_state_loader.get_pubkey(),
            borrow_reserve.liquidity.mint_pubkey,
            *obligation_referrer,
            borrow_reserve_info_key,
        )?;

        add_referrer_fee(
            borrow_reserve,
            referrer_token_state,
            Fraction::from_sf(referrer_fee_capped_sf),
        )?;
    } else {
        borrow_reserve.liquidity.accumulated_protocol_fees_sf += referrer_fee_capped_sf;
    }

    Ok(())
}

pub fn withdraw_referrer_fees(
    reserve: &mut Reserve,
    slot: Slot,
    referrer_token_state: &mut ReferrerTokenState,
) -> Result<u64> {
    if reserve
        .last_update
        .is_stale(slot, PriceStatusFlags::ALL_CHECKS)?
    {
        msg!(
            "reserve is stale and must be refreshed in the current slot, price status: {:08b}",
            reserve.last_update.get_price_status().0
        );
        return err!(LendingError::ReserveStale);
    }

    let withdraw_amount = reserve.get_withdraw_referrer_fees(referrer_token_state)?;

    if withdraw_amount == 0 {
        return err!(LendingError::InsufficientReferralFeesToRedeem);
    }

    reserve
        .liquidity
        .withdraw_referrer_fees(withdraw_amount, referrer_token_state)?;
    reserve.last_update.mark_stale();

    Ok(withdraw_amount)
}

pub fn update_reserve_config(reserve: &mut Reserve, mode: UpdateConfigMode, value: &[u8]) {
    match mode {
        UpdateConfigMode::UpdateLoanToValuePct => {
            let new = value[0];
            let prv = reserve.config.loan_to_value_pct;
            reserve.config.loan_to_value_pct = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateMaxLiquidationBonusBps => {
            let new: u16 = u16::from_le_bytes(value[..2].try_into().unwrap());
            let prv = reserve.config.max_liquidation_bonus_bps;
            reserve.config.max_liquidation_bonus_bps = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateLiquidationThresholdPct => {
            let new = value[0];
            let prv = reserve.config.liquidation_threshold_pct;
            reserve.config.liquidation_threshold_pct = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateProtocolLiquidationFee => {
            let new = value[0];
            let prv = reserve.config.protocol_liquidation_fee_pct;
            reserve.config.protocol_liquidation_fee_pct = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateProtocolTakeRate => {
            let new = value[0];
            let prv = reserve.config.protocol_take_rate_pct;
            reserve.config.protocol_take_rate_pct = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateFeesBorrowFee => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.fees.borrow_fee_sf;
            reserve.config.fees.borrow_fee_sf = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateFeesFlashLoanFee => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.fees.flash_loan_fee_sf;
            reserve.config.fees.flash_loan_fee_sf = new;
            msg!("Prv Value is {}", Fraction::from_bits(prv.into()));
            msg!("New Value is {}", Fraction::from_bits(new.into()));
        }
        UpdateConfigMode::UpdateFeesReferralFeeBps => {
            msg!("ReferralFee moved to lending_market");
        }
        UpdateConfigMode::UpdateDepositLimit => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.deposit_limit;
            reserve.config.deposit_limit = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateBorrowLimit => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.borrow_limit;
            reserve.config.borrow_limit = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateTokenInfoLowerHeuristic => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.token_info.heuristic.lower;
            reserve.config.token_info.heuristic.lower = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateTokenInfoUpperHeuristic => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.token_info.heuristic.upper;
            reserve.config.token_info.heuristic.upper = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateTokenInfoExpHeuristic => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.token_info.heuristic.exp;
            reserve.config.token_info.heuristic.exp = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateTokenInfoTwapDivergence => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.token_info.max_twap_divergence_bps;
            reserve.config.token_info.max_twap_divergence_bps = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateTokenInfoScopeChain => {
            let value = u64::from_le_bytes(value[..8].try_into().unwrap());
            let x = value.to_le_bytes();
            let end = x
                .chunks_exact(2)
                .map(|x| u16::from_le_bytes(x.try_into().unwrap()))
                .collect::<Vec<u16>>();
            let cached = reserve.config.token_info.scope_configuration.price_chain;
            reserve.config.token_info.scope_configuration.price_chain = end.try_into().unwrap();
            msg!("Prev scope chain is {:?}", cached);
            msg!(
                "Set scope chain to {:?}",
                reserve.config.token_info.scope_configuration.price_chain
            );
        }
        UpdateConfigMode::UpdateTokenInfoScopeTwap => {
            let value = u64::from_le_bytes(value[..8].try_into().unwrap());
            let x = value.to_le_bytes();
            let end = x
                .chunks_exact(2)
                .map(|x| u16::from_le_bytes(x.try_into().unwrap()))
                .collect::<Vec<u16>>();
            let cached = reserve.config.token_info.scope_configuration.twap_chain;
            reserve.config.token_info.scope_configuration.twap_chain = end.try_into().unwrap();
            msg!("Prev twap scope chain is {:?}", cached);
            msg!(
                "Set  twap scope chain to {:?}",
                reserve.config.token_info.scope_configuration.twap_chain
            );
        }
        UpdateConfigMode::UpdateTokenInfoName => {
            let value: [u8; 32] = value[0..32].try_into().unwrap();
            let str_name = std::str::from_utf8(&value).unwrap();
            let cached = reserve.config.token_info.name;
            let cached_name = std::str::from_utf8(&cached).unwrap();
            msg!("Prev token name was {}", cached_name);
            msg!("Setting token name to {}", str_name);
            reserve.config.token_info.name = value;
        }
        UpdateConfigMode::UpdateTokenInfoPriceMaxAge => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.token_info.max_age_price_seconds;
            reserve.config.token_info.max_age_price_seconds = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateTokenInfoTwapMaxAge => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.token_info.max_age_twap_seconds;
            reserve.config.token_info.max_age_twap_seconds = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateScopePriceFeed => {
            let new: [u8; 32] = value[0..32].try_into().unwrap();
            let new = Pubkey::new_from_array(new);
            let prv = reserve.config.token_info.scope_configuration.price_feed;
            reserve.config.token_info.scope_configuration.price_feed = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }

        UpdateConfigMode::UpdatePythPrice => {
            let new: [u8; 32] = value[0..32].try_into().unwrap();
            let new = Pubkey::new_from_array(new);
            let prv = reserve.config.token_info.pyth_configuration.price;
            reserve.config.token_info.pyth_configuration.price = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateSwitchboardFeed => {
            let new: [u8; 32] = value[0..32].try_into().unwrap();
            let new = Pubkey::new_from_array(new);
            let prv = reserve
                .config
                .token_info
                .switchboard_configuration
                .price_aggregator;
            reserve
                .config
                .token_info
                .switchboard_configuration
                .price_aggregator = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateSwitchboardTwapFeed => {
            let new: [u8; 32] = value[0..32].try_into().unwrap();
            let new = Pubkey::new_from_array(new);
            let prv = reserve
                .config
                .token_info
                .switchboard_configuration
                .twap_aggregator;
            reserve
                .config
                .token_info
                .switchboard_configuration
                .twap_aggregator = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateBorrowRateCurve => {
            let new: BorrowRateCurve = BorshDeserialize::deserialize(&mut &value[..]).unwrap();
            let prv = reserve.config.borrow_rate_curve;
            reserve.config.borrow_rate_curve = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateEntireReserveConfig => {
            let new: ReserveConfig = BorshDeserialize::deserialize(&mut &value[..]).unwrap();
            reserve.config = new;
            msg!("New Value is {:?}", value);
        }
        UpdateConfigMode::UpdateDebtWithdrawalCap => {
            let capacity = u64::from_le_bytes(value[..8].try_into().unwrap());
            let interval_length_seconds = u64::from_le_bytes(value[8..16].try_into().unwrap());

            let prev_capacity = reserve.config.debt_withdrawal_cap.config_capacity;
            let prev_length = reserve
                .config
                .debt_withdrawal_cap
                .config_interval_length_seconds;

            reserve.config.debt_withdrawal_cap.config_capacity = capacity.try_into().unwrap();
            reserve
                .config
                .debt_withdrawal_cap
                .config_interval_length_seconds = interval_length_seconds;

            msg!(
                "New capacity is {:?}, interval_length_seconds is {:?}",
                capacity,
                interval_length_seconds
            );
            msg!(
                "Prv capacity is {:?}, interval_length_seconds is {:?}",
                prev_capacity,
                prev_length
            );
        }
        UpdateConfigMode::UpdateDepositWithdrawalCap => {
            let capacity = u64::from_le_bytes(value[..8].try_into().unwrap());
            let interval_length_seconds = u64::from_le_bytes(value[8..16].try_into().unwrap());

            let prev_capacity = reserve.config.deposit_withdrawal_cap.config_capacity;
            let prev_length = reserve
                .config
                .deposit_withdrawal_cap
                .config_interval_length_seconds;

            reserve.config.deposit_withdrawal_cap.config_capacity = capacity.try_into().unwrap();
            reserve
                .config
                .deposit_withdrawal_cap
                .config_interval_length_seconds = interval_length_seconds;

            msg!(
                "Prv capacity is {:?}, interval_length_seconds is {:?}",
                prev_capacity,
                prev_length
            );
            msg!(
                "New capacity is {:?}, interval_length_seconds is {:?}",
                capacity,
                interval_length_seconds
            );
        }
        UpdateConfigMode::UpdateDebtWithdrawalCapCurrentTotal => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.debt_withdrawal_cap.current_total;
            reserve.config.debt_withdrawal_cap.current_total = new.try_into().unwrap();
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateDepositWithdrawalCapCurrentTotal => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.deposit_withdrawal_cap.current_total;
            reserve.config.deposit_withdrawal_cap.current_total = new.try_into().unwrap();
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateBadDebtLiquidationBonusBps => {
            let new: u16 = u16::from_le_bytes(value[..2].try_into().unwrap());
            let prv = reserve.config.bad_debt_liquidation_bonus_bps;
            reserve.config.bad_debt_liquidation_bonus_bps = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateMinLiquidationBonusBps => {
            let new: u16 = u16::from_le_bytes(value[..2].try_into().unwrap());
            let prv = reserve.config.min_liquidation_bonus_bps;
            reserve.config.min_liquidation_bonus_bps = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::DeleveragingMarginCallPeriod => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.deleveraging_margin_call_period_secs;
            reserve.config.deleveraging_margin_call_period_secs = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateBorrowFactor => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.borrow_factor_pct;
            reserve.config.borrow_factor_pct = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateAssetTier => {
            let new = value[0];
            let prv = reserve.config.asset_tier;
            reserve.config.asset_tier = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateElevationGroup => {
            let new: [u8; 20] = value[..20].try_into().unwrap();
            let prv = reserve.config.elevation_groups;
            reserve.config.elevation_groups = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::DeleveragingThresholdSlotsPerBps => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.deleveraging_threshold_slots_per_bps;
            reserve.config.deleveraging_threshold_slots_per_bps = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateReserveStatus => {
            let new = ReserveStatus::try_from(value[0]).unwrap();
            let prv = ReserveStatus::try_from(reserve.config.status).unwrap();
            reserve.config.status = new as u8;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateBorrowLimitOutsideElevationGroup => {
            let new = u64::from_le_bytes(value[..8].try_into().unwrap());
            let prv = reserve.config.borrow_limit_outside_elevation_group;
            reserve.config.borrow_limit_outside_elevation_group = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateBorrowLimitsInElevationGroupAgainstThisReserve => {
            msg!(
                "Prv Value is {:?}",
                reserve
                    .config
                    .borrow_limit_against_this_collateral_in_elevation_group
            );
            reserve
                .config
                .borrow_limit_against_this_collateral_in_elevation_group =
                BorshDeserialize::try_from_slice(value).unwrap();
            msg!(
                "New Value is {:?}",
                reserve
                    .config
                    .borrow_limit_against_this_collateral_in_elevation_group
            );
        }
        UpdateConfigMode::UpdateFarmCollateral => {
            let new: [u8; 32] = value[0..32].try_into().unwrap();
            let new = Pubkey::new_from_array(new);
            let prv = reserve.farm_collateral;
            reserve.farm_collateral = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateFarmDebt => {
            let new: [u8; 32] = value[0..32].try_into().unwrap();
            let new = Pubkey::new_from_array(new);
            let prv = reserve.farm_debt;
            reserve.farm_debt = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateDisableUsageAsCollateralOutsideEmode => {
            let new = value[0];
            let prv = reserve.config.disable_usage_as_coll_outside_emode;
            reserve.config.disable_usage_as_coll_outside_emode = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateBlockBorrowingAboveUtilization => {
            let new = value[0];
            let prv = reserve.config.utilization_limit_block_borrowing_above;
            reserve.config.utilization_limit_block_borrowing_above = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateBlockPriceUsage => {
            let new = value[0];
            let prv = reserve.config.token_info.block_price_usage;
            reserve.config.token_info.block_price_usage = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::UpdateHostFixedInterestRateBps => {
            let new = u16::from_le_bytes(value[..2].try_into().unwrap());
            let prv = reserve.config.host_fixed_interest_rate_bps;
            reserve.config.host_fixed_interest_rate_bps = new;
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
        }
        UpdateConfigMode::DeprecatedUpdateMultiplierSideBoost => {
            panic!("Deprecated endpoint")
        }
        UpdateConfigMode::DeprecatedUpdateMultiplierTagBoost => {
            panic!("Deprecated endpoint")
        }
    }

    reserve.last_update.mark_stale();
}

pub mod utils {
    use anchor_lang::require_neq;

    use super::*;
    use crate::{
        fraction::FRACTION_ONE_SCALED,
        state::ReserveConfig,
        utils::{ten_pow, ELEVATION_GROUP_NONE, FULL_BPS, MAX_NUM_ELEVATION_GROUPS},
        ElevationGroup, ObligationCollateral, ObligationLiquidity,
    };

    pub(crate) fn repay_and_withdraw_from_obligation_post_liquidation(
        obligation: &mut Obligation,
        repay_reserve: &mut Reserve,
        settle_amount_f: Fraction,
        withdraw_amount: u64,
        repay_amount: u64,
        liquidity_index: usize,
        collateral_index: usize,
    ) -> Result<()> {
        if repay_amount == 0 {
            msg!("Liquidation is too small to transfer liquidity");
            return err!(LendingError::LiquidationTooSmall);
        }
        if withdraw_amount == 0 {
            msg!("Liquidation is too small to receive collateral");
            return err!(LendingError::LiquidationTooSmall);
        }

        repay_reserve
            .liquidity
            .repay(repay_amount, settle_amount_f)?;
        repay_reserve.last_update.mark_stale();

        obligation.repay(settle_amount_f, liquidity_index)?;
        obligation.withdraw(withdraw_amount, collateral_index)?;
        obligation.update_has_debt();
        obligation.last_update.mark_stale();

        Ok(())
    }

    pub(crate) fn calculate_market_value_from_liquidity_amount(
        reserve: &Reserve,
        liquidity_amount: Fraction,
    ) -> Result<Fraction> {
        let mint_decimal_factor: u128 =
            ten_pow(reserve.liquidity.mint_decimals.try_into().unwrap()).into();
        let market_price_f = reserve.liquidity.get_market_price_f();
        let market_value = liquidity_amount
            .mul(market_price_f)
            .div(mint_decimal_factor);

        Ok(market_value)
    }

    pub(crate) fn calculate_obligation_collateral_market_value(
        deposit_reserve: &Reserve,
        deposit: &ObligationCollateral,
    ) -> Result<Fraction> {
        let liquidity_amount_from_collateral = deposit_reserve
            .collateral_exchange_rate()?
            .fraction_collateral_to_liquidity(deposit.deposited_amount.into());

        calculate_market_value_from_liquidity_amount(
            deposit_reserve,
            liquidity_amount_from_collateral,
        )
    }

    pub(crate) fn calculate_obligation_liquidity_market_value(
        borrow_reserve: &Reserve,
        borrow: &ObligationLiquidity,
    ) -> Result<Fraction> {
        calculate_market_value_from_liquidity_amount(
            borrow_reserve,
            Fraction::from_bits(borrow.borrowed_amount_sf),
        )
    }

    pub(crate) fn check_obligation_collateral_deposit_reserve(
        deposit: &ObligationCollateral,
        deposit_reserve: &Reserve,
        deposit_reserve_pk: Pubkey,
        index: usize,
        slot: u64,
    ) -> Result<()> {
        if deposit.deposit_reserve != deposit_reserve_pk {
            msg!(
                "Deposit reserve of collateral {} does not match the deposit reserve provided",
                index
            );
            return err!(LendingError::InvalidAccountInput);
        }

        if deposit_reserve
            .last_update
            .is_stale(slot, PriceStatusFlags::NONE)?
        {
            msg!(
                "Deposit reserve {} provided for collateral {} is stale
                and must be refreshed in the current slot. Last Update {:?}",
                deposit.deposit_reserve,
                index,
                deposit_reserve.last_update,
            );
            return err!(LendingError::ReserveStale);
        }

        if deposit_reserve.version != PROGRAM_VERSION as u64 {
            msg!(
                "Deposit reserve {} provided for collateral {} has been deprecated.",
                deposit.deposit_reserve,
                index,
            );
            return err!(LendingError::ReserveDeprecated);
        }

        Ok(())
    }

    pub(crate) fn check_obligation_liquidity_borrow_reserve(
        borrow: &ObligationLiquidity,
        borrow_reserve: &Reserve,
        borrow_reserve_pk: Pubkey,
        index: usize,
        slot: u64,
    ) -> Result<()> {
        if borrow.borrow_reserve != borrow_reserve_pk {
            msg!(
                "Borrow reserve of liquidity {} does not match the borrow reserve provided",
                index
            );
            return err!(LendingError::InvalidAccountInput);
        }

        if borrow_reserve
            .last_update
            .is_stale(slot, PriceStatusFlags::NONE)?
        {
            msg!(
                "Borrow reserve {} provided for liquidity {} is stale
                and must be refreshed in the current slot. Last Update {:?}",
                borrow.borrow_reserve,
                index,
                borrow_reserve.last_update,
            );
            return err!(LendingError::ReserveStale);
        }

        if borrow_reserve.version != PROGRAM_VERSION as u64 {
            msg!(
                "Borrow reserve {} provided for liquidity {} has been deprecated.",
                borrow.borrow_reserve,
                index,
            );
            return err!(LendingError::ReserveDeprecated);
        }

        Ok(())
    }

    pub fn check_elevation_group_borrowing_enabled(
        market: &LendingMarket,
        obligation: &Obligation,
    ) -> Result<()> {
        if let Some(elevation_group) = get_elevation_group(obligation.elevation_group, market)? {
            require!(
                !elevation_group.new_loans_disabled(),
                LendingError::ElevationGroupNewLoansDisabled
            );
        }
        Ok(())
    }

    pub fn check_elevation_group_borrow_limit_constraints<'info, T>(
        obligation: &Obligation,
        elevation_group: Option<&ElevationGroup>,
        mut deposit_reserves_iter: impl Iterator<Item = T>,
        mut borrow_reserves_iter: impl Iterator<Item = T>,
    ) -> Result<()>
    where
        T: AnyAccountLoader<'info, Reserve>,
    {
        {
            let mut borrows_iter = obligation.borrows.iter();
            for (borrow, reserve_acc) in borrows_iter
                .by_ref()
                .filter(|borrow| borrow.borrow_reserve != Pubkey::default())
                .zip(borrow_reserves_iter.by_ref())
            {
                let reserve_pk = reserve_acc.get_pubkey();
                let borrow_reserve = reserve_acc.get()?;
                require_keys_eq!(
                    borrow.borrow_reserve,
                    reserve_pk,
                    LendingError::InvalidAccountInput
                );

                if let Some(elevation_group) = elevation_group {
                    require!(
                        borrow_reserve
                            .config
                            .elevation_groups
                            .contains(&elevation_group.id),
                        LendingError::InconsistentElevationGroup
                    );
                    require_keys_eq!(
                        reserve_pk,
                        elevation_group.debt_reserve,
                        LendingError::ElevationGroupHasAnotherDebtReserve
                    );
                } else {
                    require_gte!(
                        borrow_reserve.config.borrow_limit_outside_elevation_group,
                        borrow_reserve.borrowed_amount_outside_elevation_group,
                        LendingError::ElevationGroupBorrowLimitExceeded
                    );
                }
            }

            require!(
                borrows_iter.next().is_none(),
                LendingError::InvalidAccountInput
            );
            require!(
                borrow_reserves_iter.next().is_none(),
                LendingError::InvalidAccountInput
            );
        }

        {
            let mut deposits_iter = obligation.deposits.iter();
            for (deposit, reserve_acc) in deposits_iter
                .by_ref()
                .filter(|deposit| deposit.deposit_reserve != Pubkey::default())
                .zip(deposit_reserves_iter.by_ref())
            {
                let reserve_pk = reserve_acc.get_pubkey();
                let deposit_reserve = reserve_acc.get()?;
                require_keys_eq!(
                    deposit.deposit_reserve,
                    reserve_pk,
                    LendingError::InvalidAccountInput
                );

                if let Some(elevation_group) = elevation_group {
                    let elevation_group_index = elevation_group.get_index();
                    require!(
                        deposit_reserve
                            .config
                            .elevation_groups
                            .contains(&elevation_group.id),
                        LendingError::InconsistentElevationGroup
                    );
                    require_keys_neq!(
                        reserve_pk,
                        elevation_group.debt_reserve,
                        LendingError::ElevationGroupDebtReserveAsCollateral
                    );

                    require_gte!(
                        deposit_reserve
                            .config
                            .borrow_limit_against_this_collateral_in_elevation_group
                            [elevation_group_index],
                        deposit_reserve.borrowed_amounts_against_this_reserve_in_elevation_groups
                            [elevation_group_index],
                        LendingError::ElevationGroupBorrowLimitExceeded,
                    );
                } else {
                }
            }

            require!(
                deposits_iter.next().is_none(),
                LendingError::InvalidAccountInput
            );
            require!(
                deposit_reserves_iter.next().is_none(),
                LendingError::InvalidAccountInput
            );
        }
        Ok(())
    }

    pub fn update_elevation_group_debt_trackers_on_borrow<'info, T>(
        new_borrowed_amount: u64,
        obligation: &mut Obligation,
        obligation_borrow_index: usize,
        elevation_group: Option<&ElevationGroup>,
        borrow_reserve_pk: &Pubkey,
        borrow_reserve: &mut Reserve,
        mut deposit_reserves_iter: impl Iterator<Item = T>,
    ) -> Result<()>
    where
        T: AnyAccountLoader<'info, Reserve>,
    {
        if let Some(elevation_group) = elevation_group {
            let elevation_group_index = elevation_group.get_index();

            require_keys_eq!(
                elevation_group.debt_reserve,
                *borrow_reserve_pk,
                LendingError::ElevationGroupHasAnotherDebtReserve
            );
            for obligation_deposit in obligation
                .deposits
                .iter_mut()
                .filter(|d| d.deposit_reserve != Pubkey::default())
            {
                let deposit_reserve = deposit_reserves_iter
                    .next()
                    .ok_or_else(|| error!(LendingError::InvalidAccountInput))?;
                require_keys_eq!(
                    deposit_reserve.get_pubkey(),
                    obligation_deposit.deposit_reserve
                );

                let mut deposit_reserve = deposit_reserve.get_mut()?;

                let debt_limit = deposit_reserve
                    .config
                    .borrow_limit_against_this_collateral_in_elevation_group[elevation_group_index];
                let prev_borrowed_amounts_against_this_reserve_in_elevation_groups =
                    deposit_reserve.borrowed_amounts_against_this_reserve_in_elevation_groups
                        [elevation_group_index];
                let new_borrowed_amounts_against_this_reserve_in_elevation_groups =
                    prev_borrowed_amounts_against_this_reserve_in_elevation_groups
                        .checked_add(new_borrowed_amount)
                        .ok_or_else(|| error!(LendingError::ElevationGroupBorrowLimitExceeded))?;

                msg!("Refreshed debt in elevation group reserve {} before {prev_borrowed_amounts_against_this_reserve_in_elevation_groups} after {new_borrowed_amounts_against_this_reserve_in_elevation_groups} limit {debt_limit}",
                    obligation_deposit.deposit_reserve,
                );

                require_gte!(
                    debt_limit,
                    new_borrowed_amounts_against_this_reserve_in_elevation_groups,
                    LendingError::ElevationGroupBorrowLimitExceeded
                );
                deposit_reserve.borrowed_amounts_against_this_reserve_in_elevation_groups
                    [elevation_group_index] =
                    new_borrowed_amounts_against_this_reserve_in_elevation_groups;

                obligation_deposit.borrowed_amount_against_this_collateral_in_elevation_group +=
                    new_borrowed_amount;
            }
        } else {
            let borrow_limit = borrow_reserve.config.borrow_limit_outside_elevation_group;
            msg!(
                "Last refreshed borrows (outside elevation group) {}",
                borrow_reserve.borrowed_amount_outside_elevation_group
            );
            let new_total_borrow_amount = borrow_reserve
                .borrowed_amount_outside_elevation_group
                .checked_add(new_borrowed_amount)
                .ok_or_else(|| error!(LendingError::MathOverflow))?;

            require_gte!(
                borrow_limit,
                new_total_borrow_amount,
                LendingError::BorrowLimitExceeded
            );

            borrow_reserve.borrowed_amount_outside_elevation_group = new_total_borrow_amount;
            obligation.borrows[obligation_borrow_index].borrowed_amount_outside_elevation_groups +=
                new_borrowed_amount;
        }
        Ok(())
    }

    pub fn update_elevation_group_debt_trackers_on_repay<'info, T>(
        repay_amount: u64,
        obligation: &mut Obligation,
        obligation_borrow_index: usize,
        borrow_reserve: &mut Reserve,
        mut deposit_reserves_iter: impl Iterator<Item = T>,
    ) -> Result<()>
    where
        T: AnyAccountLoader<'info, Reserve>,
    {
        if obligation.elevation_group != ELEVATION_GROUP_NONE {
            let elevation_group_index = obligation.elevation_group as usize - 1;
            for obligation_deposit in obligation
                .deposits
                .iter_mut()
                .filter(|d| d.deposit_reserve != Pubkey::default())
            {
                let deposit_reserve = deposit_reserves_iter
                    .next()
                    .ok_or_else(|| error!(LendingError::InvalidAccountInput))?;
                require_keys_eq!(
                    deposit_reserve.get_pubkey(),
                    obligation_deposit.deposit_reserve
                );
                let mut deposit_reserve = deposit_reserve.get_mut()?;
                let debt_limit = deposit_reserve
                    .config
                    .borrow_limit_against_this_collateral_in_elevation_group[elevation_group_index];
                let pre_debt_amount = deposit_reserve
                    .borrowed_amounts_against_this_reserve_in_elevation_groups
                    [elevation_group_index];
                let new_debt_amount = pre_debt_amount.saturating_sub(repay_amount);

                msg!("Refreshed debt in elevation group reserve {} before {pre_debt_amount} after {new_debt_amount} limit {debt_limit}",
                    obligation_deposit.deposit_reserve,
                );
                deposit_reserve.borrowed_amounts_against_this_reserve_in_elevation_groups
                    [elevation_group_index] = new_debt_amount;
                obligation_deposit.borrowed_amount_against_this_collateral_in_elevation_group =
                    obligation_deposit
                        .borrowed_amount_against_this_collateral_in_elevation_group
                        .saturating_sub(repay_amount);
            }
        } else {
            let new_total_borrow_amount = borrow_reserve
                .borrowed_amount_outside_elevation_group
                .saturating_sub(repay_amount);

            msg!(
                "Last refreshed borrows (outside elevation group) {}",
                borrow_reserve.borrowed_amount_outside_elevation_group
            );

            borrow_reserve.borrowed_amount_outside_elevation_group = new_total_borrow_amount;
            obligation.borrows[obligation_borrow_index].borrowed_amount_outside_elevation_groups =
                obligation.borrows[obligation_borrow_index]
                    .borrowed_amount_outside_elevation_groups
                    .saturating_sub(repay_amount);
        }
        Ok(())
    }

    pub fn update_elevation_group_debt_trackers_on_new_deposit(
        total_borrowed: Option<u64>,
        obligation_collateral: &mut ObligationCollateral,
        pre_deposit_count: usize,
        elevation_group: Option<&ElevationGroup>,
        deposit_reserve_pk: &Pubkey,
        deposit_reserve: &mut Reserve,
    ) -> Result<()> {
        if let Some(elevation_group) = elevation_group {
            require_keys_neq!(
                elevation_group.debt_reserve,
                *deposit_reserve_pk,
                LendingError::ElevationGroupDebtReserveAsCollateral
            );

            require_gte!(
                usize::from(elevation_group.max_reserves_as_collateral),
                pre_deposit_count + 1,
                LendingError::ObligationCollateralExceedsElevationGroupLimit
            );

            let elevation_group_index = elevation_group.get_index();

            let total_borrowed = total_borrowed
                .ok_or_else(|| error!(LendingError::ObligationElevationGroupMultipleDebtReserve))?;

            deposit_reserve.borrowed_amounts_against_this_reserve_in_elevation_groups
                [elevation_group_index] += total_borrowed;
            obligation_collateral.borrowed_amount_against_this_collateral_in_elevation_group =
                total_borrowed;
        }
        Ok(())
    }

    pub fn update_elevation_group_debt_trackers_on_full_withdraw(
        previous_debt_in_elevation_group: u64,
        elevation_group_id: u8,
        deposit_reserve: &mut Reserve,
    ) -> Result<()> {
        if elevation_group_id != ELEVATION_GROUP_NONE {
            let elevation_group_index = elevation_group_id as usize - 1;

            deposit_reserve.borrowed_amounts_against_this_reserve_in_elevation_groups
                [elevation_group_index] = deposit_reserve
                .borrowed_amounts_against_this_reserve_in_elevation_groups[elevation_group_index]
                .saturating_sub(previous_debt_in_elevation_group);
        }
        Ok(())
    }

    pub fn check_non_elevation_group_borrowing_enabled(obligation: &Obligation) -> Result<()> {
        if obligation.elevation_group == ELEVATION_GROUP_NONE && obligation.borrowing_disabled > 0 {
            err!(LendingError::BorrowingDisabledOutsideElevationGroup)
        } else {
            Ok(())
        }
    }

    pub fn check_same_elevation_group(obligation: &Obligation, reserve: &Reserve) -> Result<()> {
        if obligation.elevation_group != ELEVATION_GROUP_NONE
            && !reserve
                .config
                .elevation_groups
                .contains(&obligation.elevation_group)
        {
            return err!(LendingError::InconsistentElevationGroup);
        }

        Ok(())
    }

    pub fn post_deposit_obligation_invariants(
        amount: Fraction,
        obligation: &Obligation,
        reserve: &Reserve,
        collateral_asset_mv: Fraction,
        min_accepted_net_value: Fraction,
    ) -> Result<()> {
        let asset_mv = calculate_market_value_from_liquidity_amount(reserve, amount)?;

        let new_total_deposited_mv = Fraction::from_bits(obligation.deposited_value_sf) + asset_mv;

        let new_collateral_asset_mv = collateral_asset_mv + asset_mv;

        let new_ltv = Fraction::from_bits(obligation.borrow_factor_adjusted_debt_value_sf)
            / new_total_deposited_mv;

        if new_collateral_asset_mv > 0 && new_collateral_asset_mv < min_accepted_net_value {
            msg!(
                "Obligation new collateral value after deposit {} for ${}",
                new_collateral_asset_mv.to_display(),
                reserve.token_symbol()
            );
            return err!(LendingError::NetValueRemainingTooSmall);
        }

        if obligation.deposited_value_sf != 0 {
            if new_ltv > obligation.loan_to_value() {
                msg!(
                    "Obligation new LTV after deposit {} of {}",
                    new_ltv.to_display(),
                    reserve.token_symbol()
                );
                return err!(LendingError::WorseLTVBlocked);
            }
        }

        Ok(())
    }

    pub fn post_withdraw_obligation_invariants(
        amount: Fraction,
        obligation: &Obligation,
        reserve: &Reserve,
        collateral_asset_mv: Fraction,
        min_accepted_net_value: Fraction,
    ) -> Result<()> {
        let asset_mv = calculate_market_value_from_liquidity_amount(reserve, amount)?;

        let new_total_deposited_mv = Fraction::from_bits(obligation.deposited_value_sf) - asset_mv;

        if collateral_asset_mv != 0 {
            let new_collateral_asset_mv = collateral_asset_mv - asset_mv;

            if new_collateral_asset_mv > 0 && new_collateral_asset_mv < min_accepted_net_value {
                msg!(
                    "Obligation new collateral value after withdraw {} for {}",
                    new_collateral_asset_mv.to_display(),
                    reserve.token_symbol()
                );
                return err!(LendingError::NetValueRemainingTooSmall);
            }
        }

        if new_total_deposited_mv != 0 {
            if Fraction::from_bits(obligation.borrowed_assets_market_value_sf)
                >= new_total_deposited_mv
            {
                msg!(
                    "Obligation new total deposited market value after withdraw {} of {}",
                    new_total_deposited_mv.to_display(),
                    reserve.token_symbol()
                );
                return err!(LendingError::LiabilitiesBiggerThanAssets);
            }

            let new_ltv = Fraction::from_bits(obligation.borrow_factor_adjusted_debt_value_sf)
                / new_total_deposited_mv;

            let unhealthy_ltv = obligation.unhealthy_loan_to_value();

            if new_ltv > unhealthy_ltv {
                msg!(
                    "Obligation new LTV/new unhealthy LTV after withdraw {:.2}/{:.2} of {}",
                    new_ltv.to_display(),
                    unhealthy_ltv.to_display(),
                    reserve.token_symbol()
                );
                return err!(LendingError::WorseLTVBlocked);
            }
        }

        Ok(())
    }

    pub fn post_borrow_obligation_invariants(
        amount: Fraction,
        obligation: &Obligation,
        reserve: &Reserve,
        liquidity_asset_mv: Fraction,
        min_accepted_net_value: Fraction,
    ) -> Result<()> {
        let asset_mv = calculate_market_value_from_liquidity_amount(reserve, amount)?;

        let new_total_bf_debt_mv =
            Fraction::from_bits(obligation.borrow_factor_adjusted_debt_value_sf)
                + asset_mv
                    * reserve.borrow_factor_f(obligation.elevation_group != ELEVATION_GROUP_NONE);
        let new_total_no_bf_debt_mv =
            Fraction::from_bits(obligation.borrowed_assets_market_value_sf) + asset_mv;
        let new_liquidity_asset_mv = liquidity_asset_mv + asset_mv;

        if new_liquidity_asset_mv > 0 && new_liquidity_asset_mv < min_accepted_net_value {
            msg!(
                "Obligation new borrowed value after borrow {} for {}",
                new_liquidity_asset_mv.to_display(),
                reserve.token_symbol()
            );
            return err!(LendingError::NetValueRemainingTooSmall);
        }
        let new_ltv = new_total_bf_debt_mv / Fraction::from_bits(obligation.deposited_value_sf);

        if new_ltv > obligation.unhealthy_loan_to_value() {
            msg!(
                "Obligation new LTV/new unhealthy LTV after borrow {:.2}/{:.2} of {}",
                new_ltv.to_display(),
                obligation.unhealthy_loan_to_value().to_display(),
                reserve.token_symbol()
            );
            return err!(LendingError::WorseLTVBlocked);
        }

        if new_total_no_bf_debt_mv >= Fraction::from_bits(obligation.deposited_value_sf) {
            msg!(
                "Obligation can't have more liabilities than assets after borrow {} of {}",
                new_total_no_bf_debt_mv.to_display(),
                reserve.token_symbol()
            );
            return err!(LendingError::LiabilitiesBiggerThanAssets);
        }

        Ok(())
    }

    pub fn post_repay_obligation_invariants(
        amount: Fraction,
        obligation: &Obligation,
        reserve: &Reserve,
        liquidity_asset_mv: Fraction,
        min_accepted_net_value: Fraction,
    ) -> Result<()> {
        let asset_mv = calculate_market_value_from_liquidity_amount(reserve, amount)?;
        let new_total_bf_debt_mv =
            Fraction::from_bits(obligation.borrow_factor_adjusted_debt_value_sf)
                - asset_mv
                    * reserve.borrow_factor_f(obligation.elevation_group != ELEVATION_GROUP_NONE);
        let total_deposited_mv = Fraction::from_bits(obligation.deposited_value_sf);

        if liquidity_asset_mv != 0 {
            let new_liquidity_asset_mv = liquidity_asset_mv - asset_mv;

            if new_liquidity_asset_mv > 0 && new_liquidity_asset_mv < min_accepted_net_value {
                msg!(
                    "Obligation new borrowed value after repay {} for {}",
                    new_liquidity_asset_mv.to_display(),
                    reserve.token_symbol()
                );
                return err!(LendingError::NetValueRemainingTooSmall);
            }
        }
        if total_deposited_mv > 0 {
            let new_ltv = new_total_bf_debt_mv / total_deposited_mv;

            if new_ltv > obligation.loan_to_value() {
                msg!(
                    "Obligation new LTV/new unhealthy LTV after repay {:.2}/{:.2} of {}",
                    new_ltv.to_display(),
                    obligation.unhealthy_loan_to_value().to_display(),
                    reserve.token_symbol()
                );
                return err!(LendingError::WorseLTVBlocked);
            }
        }

        Ok(())
    }

    pub fn get_elevation_group(
        elevation_group_id: u8,
        market: &LendingMarket,
    ) -> Result<Option<&ElevationGroup>> {
        if elevation_group_id > MAX_NUM_ELEVATION_GROUPS {
            return err!(LendingError::InvalidElevationGroup);
        }

        let elevation_group = market.get_elevation_group(elevation_group_id)?;

        if let Some(elevation_group) = elevation_group {
            require_neq!(
                elevation_group.liquidation_threshold_pct,
                0,
                LendingError::InvalidElevationGroup
            );
            require_neq!(
                elevation_group.ltv_pct,
                0,
                LendingError::InvalidElevationGroup
            );
        }

        Ok(elevation_group)
    }

    pub fn get_max_ltv_and_liquidation_threshold(
        deposit_reserve: &Reserve,
        elevation_group: Option<&ElevationGroup>,
    ) -> Result<(u8, u8)> {
        if let Some(elevation_group) = elevation_group {
            Ok((
                elevation_group.ltv_pct,
                elevation_group.liquidation_threshold_pct,
            ))
        } else {
            Ok((
                deposit_reserve.config.loan_to_value_pct,
                deposit_reserve.config.liquidation_threshold_pct,
            ))
        }
    }

    pub fn check_obligation_fully_refreshed_and_not_null(
        obligation: &Obligation,
        slot: Slot,
    ) -> Result<()> {
        if obligation
            .last_update
            .is_stale(slot, PriceStatusFlags::ALL_CHECKS)?
        {
            msg!(
            "Obligation is stale and must be refreshed in the current slot, price status: {:08b}",
            obligation.last_update.get_price_status().0
        );
            return err!(LendingError::ObligationStale);
        }
        if obligation.deposits_empty() {
            msg!("Obligation has no deposits to borrow against");
            return err!(LendingError::ObligationDepositsEmpty);
        }
        if obligation.deposited_value_sf == 0 {
            msg!("Obligation deposits have zero value");
            return err!(LendingError::ObligationDepositsZero);
        }

        Ok(())
    }

    pub fn assert_obligation_liquidatable(
        repay_reserve: &Reserve,
        withdraw_reserve: &Reserve,
        obligation: &Obligation,
        liquidity_amount: u64,
        slot: Slot,
    ) -> Result<()> {
        if liquidity_amount == 0 {
            msg!("Liquidity amount provided cannot be zero");
            return err!(LendingError::InvalidAmount);
        }

        if repay_reserve
            .last_update
            .is_stale(slot, PriceStatusFlags::LIQUIDATION_CHECKS)?
        {
            msg!(
                "Repay reserve is stale and must be refreshed in the current slot, price status: {:08b}",
                repay_reserve.last_update.get_price_status().0
            );
            return err!(LendingError::ReserveStale);
        }

        if withdraw_reserve
            .last_update
            .is_stale(slot, PriceStatusFlags::LIQUIDATION_CHECKS)?
        {
            msg!(
                "Withdraw reserve is stale and must be refreshed in the current slot, price status: {:08b}",
                withdraw_reserve.last_update.get_price_status().0
            );
            return err!(LendingError::ReserveStale);
        }

        if obligation
            .last_update
            .is_stale(slot, PriceStatusFlags::LIQUIDATION_CHECKS)?
        {
            msg!(
            "Obligation is stale and must be refreshed in the current slot, price status: {:08b}",
            obligation.last_update.get_price_status().0
        );
            return err!(LendingError::ObligationStale);
        }

        if obligation.deposited_value_sf == 0 {
            msg!("Obligation deposited value is zero");
            return err!(LendingError::ObligationDepositsZero);
        }
        if obligation.borrow_factor_adjusted_debt_value_sf == 0 {
            msg!("Obligation borrowed value is zero");
            return err!(LendingError::ObligationBorrowsZero);
        }

        Ok(())
    }

    pub fn validate_reserve_config(
        config: &ReserveConfig,
        market: &LendingMarket,
        reserve_address: Pubkey,
    ) -> Result<()> {
        if config.loan_to_value_pct >= 100 {
            msg!("Loan to value ratio must be in range [0, 100)");
            return err!(LendingError::InvalidConfig);
        }
        if config.max_liquidation_bonus_bps > FULL_BPS {
            msg!("Liquidation bonus must be in range [0, 100]");
            return err!(LendingError::InvalidConfig);
        }
        if config.liquidation_threshold_pct < config.loan_to_value_pct
            || config.liquidation_threshold_pct > 100
        {
            msg!("Liquidation threshold must be in range [LTV, 100]");
            return err!(LendingError::InvalidConfig);
        }
        if u128::from(config.fees.borrow_fee_sf) >= FRACTION_ONE_SCALED {
            msg!("Borrow fee must be in range [0, 100%]");
            return err!(LendingError::InvalidConfig);
        }
        if config.protocol_liquidation_fee_pct > 100 {
            msg!("Protocol liquidation fee must be in range [0, 100]");
            return err!(LendingError::InvalidConfig);
        }
        if config.protocol_take_rate_pct > 100 {
            msg!("Protocol take rate must be in range [0, 100]");
            return err!(LendingError::InvalidConfig);
        }
        if !config.token_info.is_valid() {
            msg!("Invalid reserve token info");
            return err!(LendingError::InvalidOracleConfig);
        }
        if !config.token_info.is_twap_config_valid() {
            msg!("Invalid reserve token twap config");
            return err!(LendingError::InvalidTwapConfig);
        }

        if config.bad_debt_liquidation_bonus_bps >= 100 {
            msg!("Invalid bad debt liquidation bonus, cannot be more than 1%");
            return err!(LendingError::InvalidConfig);
        }
        if config.min_liquidation_bonus_bps > config.max_liquidation_bonus_bps {
            msg!("Invalid min liquidation bonus");
            return err!(LendingError::InvalidConfig);
        }
        if config.borrow_factor_pct < 100 {
            msg!("Invalid borrow factor, it must be greater or equal to 100");
            return err!(LendingError::InvalidConfig);
        }
        if config.deleveraging_threshold_slots_per_bps == 0 {
            msg!("Invalid deleveraging_threshold_slots_per_bps, must be greater than 0");
            return err!(LendingError::InvalidConfig);
        }
        if config.get_asset_tier() == AssetTier::IsolatedDebt
            && !(config.loan_to_value_pct == 0 && config.liquidation_threshold_pct == 0)
        {
            msg!("LTV ratio and liquidation threshold must be 0 for isolated debt assets");
            return Err(LendingError::InvalidConfig.into());
        }
        if config.get_asset_tier() == AssetTier::IsolatedCollateral && config.borrow_limit != 0 {
            msg!("Borrow limit must be 0 for isolated collateral assets");
            return Err(LendingError::InvalidConfig.into());
        }
        if config.borrow_limit_outside_elevation_group != u64::MAX
            && config.borrow_limit < config.borrow_limit_outside_elevation_group
        {
            msg!("Invalid 'borrow limit', must be at least equal to 'borrow limit outside elevation group' when enabled");
            return err!(LendingError::InvalidConfig);
        }

        for elevation_group_id in config.elevation_groups {
            if let Some(elevation_group) = get_elevation_group(elevation_group_id, market)? {
                if elevation_group.debt_reserve == Pubkey::default() {
                    msg!("Invalid elevation group debt reserve");
                    return err!(LendingError::InvalidConfig);
                }

                if elevation_group.debt_reserve != reserve_address {
                    if elevation_group.max_liquidation_bonus_bps > config.max_liquidation_bonus_bps
                    {
                        msg!("Invalid max liquidation bonus, elevation id liquidation bonus must be less than the config's");
                        return err!(LendingError::InvalidConfig);
                    }

                    if elevation_group.liquidation_threshold_pct < config.liquidation_threshold_pct
                    {
                        msg!("Invalid liquidation threshold, elevation id liquidation threshold must be greater than the config's");
                        return err!(LendingError::InvalidConfig);
                    }

                    if elevation_group.ltv_pct < config.loan_to_value_pct {
                        msg!("Invalid ltv ratio, cannot be bigger than the ltv ratio");
                        return err!(LendingError::InvalidConfig);
                    }
                }

                if elevation_group.max_reserves_as_collateral == 0 {
                    msg!("Invalid elevation group max collateral reserves");
                    return err!(LendingError::InvalidConfig);
                }
            }
        }

        config.borrow_rate_curve.validate()?;
        Ok(())
    }

    pub fn validate_obligation_asset_tiers(obligation: &Obligation) -> Result<()> {
        let deposit_tiers = obligation.get_deposit_asset_tiers();

        let borrow_tiers = obligation.get_borrows_asset_tiers();

        let count_isolated_deposits = deposit_tiers
            .iter()
            .filter(|&tier| *tier == AssetTier::IsolatedCollateral)
            .count();
        let count_isolated_borrows = borrow_tiers
            .iter()
            .filter(|&tier| *tier == AssetTier::IsolatedDebt)
            .count();

        if count_isolated_deposits > 1 {
            msg!("Cannot deposit more than one isolated collateral tier asset");
            return Err(LendingError::IsolatedAssetTierViolation.into());
        }

        if count_isolated_borrows > 1 {
            msg!("Cannot borrow more than one isolated debt tier asset");
            return Err(LendingError::IsolatedAssetTierViolation.into());
        }

        if count_isolated_deposits > 0 && count_isolated_borrows > 0 {
            msg!("Cannot borrow an isolated tier asset while depositing and isolated tier asset");
            return Err(LendingError::IsolatedAssetTierViolation.into());
        }

        if deposit_tiers.len() > 1 && count_isolated_deposits > 0 {
            msg!("Cannot deposit isolated collateral tier asset with other assets");
            return Err(LendingError::IsolatedAssetTierViolation.into());
        }

        if borrow_tiers.len() > 1 && count_isolated_borrows > 0 {
            msg!("Cannot borrow isolated debt tier asset with other assets");
            return Err(LendingError::IsolatedAssetTierViolation.into());
        }

        if deposit_tiers
            .iter()
            .filter(|&tier| *tier == AssetTier::IsolatedDebt)
            .count()
            > 0
        {
            msg!("Cannot deposit an isolated debt tier asset");
            return Err(LendingError::IsolatedAssetTierViolation.into());
        }

        if borrow_tiers
            .iter()
            .filter(|&tier| *tier == AssetTier::IsolatedCollateral)
            .count()
            > 0
        {
            msg!("Cannot borrow an isolated collateral tier asset");
            return Err(LendingError::IsolatedAssetTierViolation.into());
        }

        Ok(())
    }
}
