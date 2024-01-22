use std::{
    cmp::min,
    ops::{Add, Div, Mul},
};

use anchor_lang::{
    err,
    prelude::{msg, Pubkey},
    require,
    solana_program::clock::Slot,
    Result,
};
use borsh::BorshDeserialize;
use solana_program::clock::{self, Clock};

use self::utils::{
    calculate_obligation_collateral_market_value, calculate_obligation_liquidity_market_value,
    check_elevation_group_borrowing_enabled, check_obligation_collateral_deposit_reserve,
    check_obligation_liquidity_borrow_reserve, check_obligation_refreshed_and_not_null,
    check_same_elevation_group, get_elevation_group, get_max_ltv_and_liquidation_threshold,
    validate_obligation_asset_tiers,
};
use super::{
    validate_referrer_token_state,
    withdrawal_cap_operations::utils::{add_to_withdrawal_accum, sub_from_withdrawal_accum},
};
use crate::{
    fraction::FractionExtra,
    liquidation_operations,
    state::{
        obligation::Obligation, CalculateBorrowResult, CalculateLiquidationResult,
        CalculateRepayResult, Reserve,
    },
    utils::{
        borrow_rate_curve::BorrowRateCurve, AnyAccountLoader, BigFraction, Fraction,
        ELEVATION_GROUP_NONE, PROGRAM_VERSION,
    },
    xmsg, AssetTier, LendingError, LendingMarket, LiquidateAndRedeemResult,
    LiquidateObligationResult, ReferrerTokenState, RefreshObligationBorrowsResult,
    RefreshObligationDepositsResult, ReserveConfig, ReserveStatus, UpdateConfigMode,
};

pub fn refresh_reserve_interest(
    reserve: &mut Reserve,
    slot: Slot,
    referral_fee_bps: u16,
) -> Result<()> {
    reserve.accrue_interest(slot, referral_fee_bps)?;
    reserve.last_update.update_slot(slot);

    Ok(())
}

pub fn refresh_reserve_price(reserve: &mut Reserve, price: Fraction, timestamp: u64) -> Result<()> {
    reserve.liquidity.market_price_sf = price.to_bits();
    reserve.liquidity.market_price_last_updated_ts = timestamp;

    Ok(())
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

    if reserve.last_update.is_stale(clock.slot)? {
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
        return err!(LendingError::InvalidAmount);
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

pub fn borrow_obligation_liquidity(
    lending_market: &LendingMarket,
    borrow_reserve: &mut Reserve,
    obligation: &mut Obligation,
    liquidity_amount: u64,
    clock: &Clock,
    borrow_reserve_pk: Pubkey,
) -> Result<CalculateBorrowResult> {
    if liquidity_amount == 0 {
        msg!("Liquidity amount provided cannot be zero");
        return err!(LendingError::InvalidAmount);
    }

    if borrow_reserve.last_update.is_stale(clock.slot)? {
        msg!("Borrow reserve is stale and must be refreshed in the current slot");
        return err!(LendingError::ReserveStale);
    }

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
        return err!(LendingError::InvalidAmount);
    }
    check_obligation_refreshed_and_not_null(obligation, clock.slot)?;

    let remaining_borrow_value = obligation.remaining_borrow_value();
    if remaining_borrow_value == Fraction::ZERO {
        msg!("Remaining borrow value is zero");
        return err!(LendingError::BorrowTooLarge);
    }

    check_same_elevation_group(obligation, borrow_reserve)?;

    check_elevation_group_borrowing_enabled(lending_market, obligation)?;

    let remaining_reserve_capacity = borrow_limit_f.saturating_sub(reserve_liquidity_borrowed_f);

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
        obligation.elevation_group,
    )?;

    add_to_withdrawal_accum(
        &mut borrow_reserve.config.debt_withdrawal_cap,
        borrow_amount_f.to_floor(),
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

    let obligation_liquidity = obligation.find_or_add_liquidity_to_borrows(
        borrow_reserve_pk,
        cumulative_borrow_rate_bf,
        borrow_reserve.config.get_asset_tier(),
    )?;

    obligation_liquidity.borrow(borrow_amount_f);
    obligation.has_debt = 1;
    obligation.last_update.mark_stale();

    validate_obligation_asset_tiers(obligation)?;

    Ok(CalculateBorrowResult {
        borrow_amount_f,
        receive_amount,
        borrow_fee,
        referrer_fee,
    })
}

pub fn deposit_obligation_collateral(
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

    if deposit_reserve.last_update.is_stale(slot)? {
        msg!("Deposit reserve is stale and must be refreshed in the current slot");
        return err!(LendingError::ReserveStale);
    }

    check_same_elevation_group(obligation, deposit_reserve)?;

    obligation
        .find_or_add_collateral_to_deposits(
            deposit_reserve_pk,
            deposit_reserve.config.get_asset_tier(),
        )?
        .deposit(collateral_amount)?;
    obligation.last_update.mark_stale();

    deposit_reserve.last_update.mark_stale();

    validate_obligation_asset_tiers(obligation)?;

    Ok(())
}

pub fn withdraw_obligation_collateral(
    lending_market: &LendingMarket,
    withdraw_reserve: &Reserve,
    obligation: &mut Obligation,
    collateral_amount: u64,
    slot: Slot,
    withdraw_reserve_pk: Pubkey,
) -> Result<u64> {
    if collateral_amount == 0 {
        return err!(LendingError::InvalidAmount);
    }

    if withdraw_reserve.last_update.is_stale(slot)? {
        msg!("Withdraw reserve is stale and must be refreshed in the current slot");
        return err!(LendingError::ReserveStale);
    }

    if obligation.last_update.is_stale(slot)? {
        msg!("Obligation is stale and must be refreshed in the current slot");
        return err!(LendingError::ObligationStale);
    }

    let (collateral, collateral_index) =
        obligation.find_collateral_in_deposits(withdraw_reserve_pk)?;
    if collateral.deposited_amount == 0 {
        return err!(LendingError::ObligationCollateralEmpty);
    }

    check_elevation_group_borrowing_enabled(lending_market, obligation)?;

    if obligation.num_of_obsolete_reserves > 0
        && withdraw_reserve.config.status() == ReserveStatus::Active
    {
        return err!(LendingError::ObligationInDeprecatedReserve);
    }

    let withdraw_amount = if obligation.borrows_empty() {
        if collateral_amount == u64::MAX {
            collateral.deposited_amount
        } else {
            collateral.deposited_amount.min(collateral_amount)
        }
    } else if obligation.deposited_value_sf == 0 {
        msg!("Obligation deposited value is zero");
        return err!(LendingError::ObligationDepositsZero);
    } else {
        let (loan_to_value_pct, _) = get_max_ltv_and_liquidation_threshold(
            lending_market,
            withdraw_reserve,
            obligation.elevation_group,
        )?;

        let max_withdraw_value = obligation.max_withdraw_value(loan_to_value_pct)?;

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

    obligation.withdraw(withdraw_amount, collateral_index)?;
    obligation.last_update.mark_stale();

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

    if reserve.last_update.is_stale(clock.slot)? {
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
    if reserve.last_update.is_stale(slot)? {
        msg!("reserve is stale and must be refreshed in the current slot");
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

pub fn repay_obligation_liquidity(
    repay_reserve: &mut Reserve,
    obligation: &mut Obligation,
    clock: &Clock,
    liquidity_amount: u64,
    repay_reserve_pk: Pubkey,
) -> Result<u64> {
    if liquidity_amount == 0 {
        msg!("Liquidity amount provided cannot be zero");
        return err!(LendingError::InvalidAmount);
    }

    if repay_reserve.last_update.is_stale(clock.slot)? {
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

    repay_reserve.liquidity.repay(repay_amount, settle_amount)?;
    repay_reserve.last_update.mark_stale();

    obligation.repay(settle_amount, liquidity_index)?;
    obligation.update_has_debt();
    obligation.last_update.mark_stale();

    Ok(repay_amount)
}

pub fn request_elevation_group<'info, T, U>(
    obligation: &mut Obligation,
    lending_market: &LendingMarket,
    slot: Slot,
    new_elevation_group: u8,
    mut reserves_iter: impl Iterator<Item = T>,
    mut referrer_token_states_iter: impl Iterator<Item = U>,
) -> Result<()>
where
    T: AnyAccountLoader<'info, Reserve>,
    U: AnyAccountLoader<'info, ReferrerTokenState>,
{
    check_obligation_refreshed_and_not_null(obligation, slot)?;

    require!(
        obligation.elevation_group != new_elevation_group,
        LendingError::ElevationGroupAlreadyActivated
    );

    {
        let elevation_group = get_elevation_group(new_elevation_group, lending_market).unwrap();

        if elevation_group.new_loans_disabled() && new_elevation_group != ELEVATION_GROUP_NONE {
            return err!(LendingError::ElevationGroupNewLoansDisabled);
        }
    }

    let RefreshObligationDepositsResult {
        allowed_borrow_value_f: allowed_borrow_value,
        ..
    } = refresh_obligation_deposits(
        obligation,
        lending_market,
        slot,
        new_elevation_group,
        &mut reserves_iter,
    )?;

    let RefreshObligationBorrowsResult {
        borrow_factor_adjusted_debt_value_f: borrow_factor_adjusted_debt_value,
        ..
    } = refresh_obligation_borrows(
        obligation,
        slot,
        new_elevation_group,
        &mut reserves_iter,
        &mut referrer_token_states_iter,
    )?;

    if allowed_borrow_value < borrow_factor_adjusted_debt_value {
        msg!("The obligation is not healthy enough to support the new elevation group");
        return err!(LendingError::UnhealthyElevationGroupLtv);
    }

    msg!(
        "Previous elevation group: {:?} . Requested elevation group for: {}",
        obligation.elevation_group,
        new_elevation_group
    );

    obligation.elevation_group = new_elevation_group;
    obligation.last_update.mark_stale();

    Ok(())
}

pub fn refresh_obligation_deposits<'info, T>(
    obligation: &mut Obligation,
    lending_market: &LendingMarket,
    slot: Slot,
    elevation_group: u8,
    mut reserves_iter: impl Iterator<Item = T>,
) -> Result<RefreshObligationDepositsResult>
where
    T: AnyAccountLoader<'info, Reserve>,
{
    let mut lowest_deposit_ltv_accumulator = u8::MAX;
    let mut deposited_value = Fraction::ZERO;
    let mut allowed_borrow_value = Fraction::ZERO;
    let mut unhealthy_borrow_value = Fraction::ZERO;
    let mut num_of_obsolete_reserves = 0;

    for (index, deposit) in obligation
        .deposits
        .iter_mut()
        .enumerate()
        .filter(|(_, deposit)| deposit.deposit_reserve != Pubkey::default())
    {
        let deposit_reserve = reserves_iter
            .next()
            .ok_or(LendingError::InvalidAccountInput)?;

        let deposit_reserve_info_key = deposit_reserve.get_pubkey();

        let deposit_reserve = deposit_reserve
            .get()
            .map_err(|_| LendingError::InvalidAccountInput)?;

        if elevation_group != ELEVATION_GROUP_NONE
            && !deposit_reserve
                .config
                .elevation_groups
                .contains(&elevation_group)
        {
            return err!(LendingError::InconsistentElevationGroup);
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

        let market_value_f =
            calculate_obligation_collateral_market_value(&deposit_reserve, deposit)?;
        deposit.market_value_sf = market_value_f.to_bits();

        let (coll_ltv_pct, coll_liquidation_threshold_pct) = get_max_ltv_and_liquidation_threshold(
            lending_market,
            &deposit_reserve,
            elevation_group,
        )?;

        lowest_deposit_ltv_accumulator = min(
            lowest_deposit_ltv_accumulator.min(deposit_reserve.config.loan_to_value_pct),
            coll_ltv_pct,
        );

        deposited_value = deposited_value.add(market_value_f);
        allowed_borrow_value += market_value_f * Fraction::from_percent(coll_ltv_pct);
        unhealthy_borrow_value +=
            market_value_f * Fraction::from_percent(coll_liquidation_threshold_pct);

        obligation.deposits_asset_tiers[index] = deposit_reserve.config.asset_tier;

        msg!(
            "Deposit: {} amount: {} value: {}",
            &deposit_reserve.config.token_info.symbol(),
            deposit_reserve
                .collateral_exchange_rate()?
                .fraction_collateral_to_liquidity(deposit.deposited_amount.into())
                .to_display(),
            market_value_f.to_display()
        );
    }

    Ok(RefreshObligationDepositsResult {
        lowest_deposit_ltv_accumulator,
        num_of_obsolete_reserves,
        deposited_value_f: deposited_value,
        allowed_borrow_value_f: allowed_borrow_value,
        unhealthy_borrow_value_f: unhealthy_borrow_value,
    })
}

pub fn refresh_obligation_borrows<'info, T, U>(
    obligation: &mut Obligation,
    slot: u64,
    elevation_group: u8,
    mut reserves_iter: impl Iterator<Item = T>,
    mut referrer_token_states_iter: impl Iterator<Item = U>,
) -> Result<RefreshObligationBorrowsResult>
where
    T: AnyAccountLoader<'info, Reserve>,
    U: AnyAccountLoader<'info, ReferrerTokenState>,
{
    let mut borrowed_assets_market_value = Fraction::ZERO;
    let mut borrow_factor_adjusted_debt_value = Fraction::ZERO;

    let obligation_has_referrer = obligation.has_referrer();

    for (index, borrow) in obligation
        .borrows
        .iter_mut()
        .enumerate()
        .filter(|(_, borrow)| borrow.borrow_reserve != Pubkey::default())
    {
        let borrow_reserve = reserves_iter
            .next()
            .ok_or(LendingError::InvalidAccountInput)?;

        let borrow_reserve_info_key = borrow_reserve.get_pubkey();

        let borrow_reserve = &mut borrow_reserve
            .get_mut()
            .map_err(|_| LendingError::InvalidAccountInput)?;

        check_obligation_liquidity_borrow_reserve(
            borrow,
            borrow_reserve,
            borrow_reserve_info_key,
            index,
            slot,
        )?;

        if elevation_group != ELEVATION_GROUP_NONE
            && !borrow_reserve
                .config
                .elevation_groups
                .contains(&elevation_group)
        {
            return err!(LendingError::InconsistentElevationGroup);
        }

        let cumulative_borrow_rate_bf =
            BigFraction::from(borrow_reserve.liquidity.cumulative_borrow_rate_bsf);

        let previous_borrowed_amount_f = Fraction::from_bits(borrow.borrowed_amount_sf);

        borrow.accrue_interest(cumulative_borrow_rate_bf)?;

        let absolute_referral_rate =
            Fraction::from_bits(borrow_reserve.liquidity.absolute_referral_rate_sf);
        let net_new_debt_f =
            Fraction::from_bits(borrow.borrowed_amount_sf) - previous_borrowed_amount_f;

        let referrer_fee_f = net_new_debt_f * absolute_referral_rate;
        let referrer_fee_capped_sf = min(
            referrer_fee_f.to_bits(),
            borrow_reserve.liquidity.pending_referrer_fees_sf,
        );

        borrow_reserve.liquidity.pending_referrer_fees_sf -= referrer_fee_capped_sf;

        if obligation_has_referrer {
            let referrer_token_state_loader = referrer_token_states_iter
                .next()
                .ok_or(LendingError::InvalidAccountInput)?;
            let referrer_token_state = &mut referrer_token_state_loader
                .get_mut()
                .map_err(|_| LendingError::InvalidAccountInput)?;

            validate_referrer_token_state(
                referrer_token_state,
                referrer_token_state_loader.get_pubkey(),
                borrow_reserve.liquidity.mint_pubkey,
                obligation.referrer,
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

        let market_value_f = calculate_obligation_liquidity_market_value(borrow_reserve, borrow)?;

        borrow.market_value_sf = market_value_f.to_bits();

        borrowed_assets_market_value += market_value_f;

        let borrow_factor_adjusted_market_value: Fraction = if elevation_group != 0 {
            market_value_f
        } else {
            market_value_f * borrow_reserve.config.get_borrow_factor()
        };

        borrow.borrow_factor_adjusted_market_value_sf =
            borrow_factor_adjusted_market_value.to_bits();

        borrow_factor_adjusted_debt_value += borrow_factor_adjusted_market_value;

        obligation.borrows_asset_tiers[index] = borrow_reserve.config.asset_tier;

        obligation.has_debt = 1;

        msg!(
            "Borrow: {} amount: {} value: {} value_bf: {}",
            &borrow_reserve.config.token_info.symbol(),
            Fraction::from_bits(borrow.borrowed_amount_sf),
            market_value_f.to_display(),
            borrow_factor_adjusted_market_value.to_display()
        );
    }

    Ok(RefreshObligationBorrowsResult {
        borrowed_assets_market_value_f: borrowed_assets_market_value,
        borrow_factor_adjusted_debt_value_f: borrow_factor_adjusted_debt_value,
    })
}

pub fn refresh_obligation<'info, T, U>(
    obligation: &mut Obligation,
    lending_market: &LendingMarket,
    slot: Slot,
    mut reserves_iter: impl Iterator<Item = T>,
    mut referrer_token_states_iter: impl Iterator<Item = U>,
) -> Result<()>
where
    T: AnyAccountLoader<'info, Reserve>,
    U: AnyAccountLoader<'info, ReferrerTokenState>,
{
    let RefreshObligationDepositsResult {
        lowest_deposit_ltv_accumulator,
        num_of_obsolete_reserves,
        deposited_value_f,
        allowed_borrow_value_f: allowed_borrow_value,
        unhealthy_borrow_value_f: unhealthy_borrow_value,
    } = refresh_obligation_deposits(
        obligation,
        lending_market,
        slot,
        obligation.elevation_group,
        &mut reserves_iter,
    )?;

    let RefreshObligationBorrowsResult {
        borrow_factor_adjusted_debt_value_f,
        borrowed_assets_market_value_f,
    } = refresh_obligation_borrows(
        obligation,
        slot,
        obligation.elevation_group,
        &mut reserves_iter,
        &mut referrer_token_states_iter,
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

    obligation.lowest_reserve_deposit_ltv = lowest_deposit_ltv_accumulator.into();

    obligation.num_of_obsolete_reserves = num_of_obsolete_reserves;

    obligation.last_update.update_slot(slot);

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn liquidate_and_redeem(
    lending_market: &LendingMarket,
    repay_reserve: &dyn AnyAccountLoader<Reserve>,
    withdraw_reserve: &dyn AnyAccountLoader<Reserve>,
    obligation: &mut Obligation,
    clock: &Clock,
    liquidity_amount: u64,
    min_acceptable_received_collateral_amount: u64,
    max_allowed_ltv_override_pct_opt: Option<u64>,
) -> Result<LiquidateAndRedeemResult> {
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
        clock.slot,
        liquidity_amount,
        min_acceptable_received_collateral_amount,
        max_allowed_ltv_override_pct_opt,
    )?;

    let withdraw_reserve = &mut withdraw_reserve.get_mut()?;

    let total_withdraw_liquidity_amount = post_liquidate_redeem(
        withdraw_reserve,
        repay_amount,
        withdraw_collateral_amount,
        liquidation_bonus_rate,
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
pub fn liquidate_obligation(
    lending_market: &LendingMarket,
    repay_reserve: &dyn AnyAccountLoader<Reserve>,
    withdraw_reserve: &dyn AnyAccountLoader<Reserve>,
    obligation: &mut Obligation,
    slot: Slot,
    liquidity_amount: u64,
    min_acceptable_received_collateral_amount: u64,
    max_allowed_ltv_override_pct_opt: Option<u64>,
) -> Result<LiquidateObligationResult> {
    xmsg!(
        "Liquidating liquidation_close_factor_pct: {}, liquidation_max_value: {}",
        lending_market.liquidation_max_debt_close_factor_pct,
        lending_market.max_liquidatable_debt_market_value_at_once
    );
    let repay_reserve_ref = repay_reserve.get()?;
    let withdraw_reserve_ref = withdraw_reserve.get()?;

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

    let (collateral, collateral_index) =
        obligation.find_collateral_in_deposits(withdraw_reserve.get_pubkey())?;
    if collateral.market_value_sf == 0 {
        msg!("Obligation deposit value is zero");
        return err!(LendingError::ObligationCollateralEmpty);
    }

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
        max_allowed_ltv_override_pct_opt,
    )?;

    drop(repay_reserve_ref);
    drop(withdraw_reserve_ref);

    {
        let mut repay_reserve_ref_mut = repay_reserve.get_mut()?;

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

    let withdraw_collateral_amount = {
        let mut withdraw_reserve_ref_mut = withdraw_reserve.get_mut()?;
        refresh_reserve_interest(
            &mut withdraw_reserve_ref_mut,
            slot,
            lending_market.referral_fee_bps,
        )?;
        let collateral_exchange_rate = withdraw_reserve_ref_mut.collateral_exchange_rate()?;
        let max_redeemable_collateral = collateral_exchange_rate
            .liquidity_to_collateral(withdraw_reserve_ref_mut.liquidity.available_amount);
        min(withdraw_amount, max_redeemable_collateral)
    };

    if withdraw_collateral_amount < min_acceptable_received_collateral_amount {
        msg!("Withdraw amount below minimum acceptable collateral amount");
        return err!(LendingError::LiquidationSlippageError);
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
    withdraw_collateral_amount: u64,
    liquidation_bonus_rate: Fraction,
    clock: &Clock,
) -> Result<Option<(u64, u64)>> {
    if withdraw_collateral_amount != 0 {
        let withdraw_liquidity_amount =
            redeem_reserve_collateral(withdraw_reserve, withdraw_collateral_amount, clock, false)?;
        let protocol_fee = liquidation_operations::calculate_protocol_liquidation_fee(
            withdraw_liquidity_amount,
            liquidation_bonus_rate,
            withdraw_reserve.config.protocol_liquidation_fee_pct,
        )?;
        msg!(
            "pnl: Liquidator repaid {} and withdrew {} collateral with fees {}",
            repay_amount,
            withdraw_liquidity_amount.checked_sub(protocol_fee).unwrap(),
            protocol_fee
        );
        Ok(Some((withdraw_liquidity_amount, protocol_fee)))
    } else {
        Ok(None)
    }
}

pub fn flash_borrow_reserve_liquidity(reserve: &mut Reserve, liquidity_amount: u64) -> Result<()> {
    if reserve.config.fees.flash_loan_fee_sf == u64::MAX {
        msg!("Flash loans are disabled for this reserve");
        return err!(LendingError::FlashLoansDisabled);
    }

    let borrowed_amount_f = reserve.liquidity.total_borrow();
    let liquidity_amount_f = Fraction::from(liquidity_amount);
    let borrow_limit_f = Fraction::from(reserve.config.borrow_limit);
    let new_total_borrow_f = borrowed_amount_f + liquidity_amount_f;
    if new_total_borrow_f > borrow_limit_f {
        msg!(
            "Cannot borrow above the borrow limit. New total borrow: {} > limit: {}",
            new_total_borrow_f,
            reserve.config.borrow_limit
        );
        return err!(LendingError::InvalidAmount);
    }

    reserve.liquidity.borrow(liquidity_amount_f)?;
    reserve.last_update.mark_stale();

    Ok(())
}

pub fn flash_repay_reserve_liquidity(
    lending_market: &LendingMarket,
    reserve: &mut Reserve,
    liquidity_amount: u64,
    slot: Slot,
) -> Result<(u64, u64, u64)> {
    let flash_loan_amount = liquidity_amount;

    let flash_loan_amount_f = Fraction::from(flash_loan_amount);
    let (total_fee, referral_fee) = reserve
        .config
        .fees
        .calculate_flash_loan_fees(flash_loan_amount_f, lending_market.referral_fee_bps)?;

    reserve
        .liquidity
        .repay(flash_loan_amount, flash_loan_amount_f)?;
    refresh_reserve_limit_timestamps(reserve, slot)?;
    reserve.last_update.mark_stale();

    Ok((flash_loan_amount, total_fee, referral_fee))
}

pub fn socialize_loss(
    lending_market: &LendingMarket,
    reserve: &mut Reserve,
    reserve_pk: &Pubkey,
    obligation: &mut Obligation,
    liquidity_amount: u64,
    slot: u64,
) -> Result<Fraction> {
    refresh_reserve_interest(reserve, slot, lending_market.referral_fee_bps)?;

    if reserve.last_update.is_stale(slot)? {
        msg!("Reserve is stale and must be refreshed in the current slot");
        return Err(LendingError::ReserveStale.into());
    }

    if obligation.last_update.is_stale(slot)? {
        msg!("Obligation is stale and must be refreshed in the current slot");
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

pub fn withdraw_referrer_fees(
    reserve: &mut Reserve,
    slot: Slot,
    referrer_token_state: &mut ReferrerTokenState,
) -> Result<u64> {
    if reserve.last_update.is_stale(slot)? {
        msg!("reserve is stale and must be refreshed in the current slot");
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
            msg!("Prv Value is {:?}", prv);
            msg!("New Value is {:?}", new);
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
        UpdateConfigMode::UpdateMultiplierTagBoost => {
            let tag: usize = value[0].into();
            let multiplier = value[1];
            let cached = reserve.config.multiplier_tag_boost[tag];
            reserve.config.multiplier_tag_boost[tag] = multiplier;

            msg!("Prv multiplier for tag {tag} to {cached}",);
            msg!("New multiplier for tag {tag} to {multiplier}",);
        }
        UpdateConfigMode::UpdateMultiplierSideBoost => {
            let new = [value[0], value[1]];
            let prv = reserve.config.multiplier_side_boost;
            reserve.config.multiplier_side_boost = new;
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
    }

    reserve.last_update.mark_stale();
}

pub mod utils {
    use super::*;
    use crate::{
        fraction::FRACTION_ONE_SCALED,
        state::ReserveConfig,
        utils::{ELEVATION_GROUP_NONE, FULL_BPS, MAX_NUM_ELEVATION_GROUPS},
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

    pub(crate) fn calculate_obligation_collateral_market_value(
        deposit_reserve: &Reserve,
        deposit: &ObligationCollateral,
    ) -> Result<Fraction> {
        let mint_decimal_factor: u128 = 10u64
            .pow(deposit_reserve.liquidity.mint_decimals.try_into().unwrap())
            .into();
        let market_price_f = deposit_reserve.liquidity.get_market_price_f();
        let market_value = deposit_reserve
            .collateral_exchange_rate()?
            .fraction_collateral_to_liquidity(deposit.deposited_amount.into())
            .mul(market_price_f)
            .div(mint_decimal_factor);

        Ok(market_value)
    }

    pub(crate) fn calculate_obligation_liquidity_market_value(
        borrow_reserve: &Reserve,
        borrow: &ObligationLiquidity,
    ) -> Result<Fraction> {
        let mint_decimal_factor =
            10u64.pow(borrow_reserve.liquidity.mint_decimals.try_into().unwrap());

        let borrowed_amount_f = Fraction::from_bits(borrow.borrowed_amount_sf);
        let market_price_f = borrow_reserve.liquidity.get_market_price_f();

        let market_value = borrowed_amount_f * market_price_f / u128::from(mint_decimal_factor);

        Ok(market_value)
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

        if deposit_reserve.last_update.is_stale(slot)? {
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

        if borrow_reserve.last_update.is_stale(slot)? {
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
        let elevation_group = get_elevation_group(obligation.elevation_group, market)?;
        if obligation.elevation_group != ELEVATION_GROUP_NONE
            && elevation_group.new_loans_disabled()
        {
            err!(LendingError::ElevationGroupNewLoansDisabled)
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

    pub fn get_elevation_group(
        elevation_group_id: u8,
        market: &LendingMarket,
    ) -> Result<ElevationGroup> {
        if elevation_group_id > MAX_NUM_ELEVATION_GROUPS {
            return err!(LendingError::InvalidElevationGroup);
        }

        let elevation_group = market.get_elevation_group(elevation_group_id)?;

        if elevation_group_id != ELEVATION_GROUP_NONE
            && (elevation_group.liquidation_threshold_pct == 0 || elevation_group.ltv_pct == 0)
        {
            return err!(LendingError::InvalidElevationGroup);
        }

        Ok(elevation_group)
    }

    pub fn get_max_ltv_and_liquidation_threshold(
        lending_market: &LendingMarket,
        deposit_reserve: &Reserve,
        obligation_elevation_group: u8,
    ) -> Result<(u8, u8)> {
        let elevation_group = get_elevation_group(obligation_elevation_group, lending_market)?;

        if obligation_elevation_group == ELEVATION_GROUP_NONE {
            Ok((
                deposit_reserve.config.loan_to_value_pct,
                deposit_reserve.config.liquidation_threshold_pct,
            ))
        } else {
            Ok((
                elevation_group.ltv_pct,
                elevation_group.liquidation_threshold_pct,
            ))
        }
    }

    pub fn check_obligation_refreshed_and_not_null(
        obligation: &Obligation,
        slot: Slot,
    ) -> Result<()> {
        if obligation.last_update.is_stale(slot)? {
            msg!("Obligation is stale and must be refreshed in the current slot");
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

        if repay_reserve.last_update.is_stale(slot)? {
            msg!("Repay reserve is stale and must be refreshed in the current slot");
            return err!(LendingError::ReserveStale);
        }

        if withdraw_reserve.last_update.is_stale(slot)? {
            msg!("Withdraw reserve is stale and must be refreshed in the current slot");
            return err!(LendingError::ReserveStale);
        }

        if obligation.last_update.is_stale(slot)? {
            msg!("Obligation is stale and must be refreshed in the current slot");
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

    pub fn validate_reserve_config(config: &ReserveConfig, market: &LendingMarket) -> Result<()> {
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

        for elevation_group_id in config.elevation_groups {
            let elevation_group = get_elevation_group(elevation_group_id, market)?;

            if elevation_group_id == ELEVATION_GROUP_NONE {
            } else {
                if elevation_group.max_liquidation_bonus_bps > config.max_liquidation_bonus_bps {
                    msg!("Invalid max liquidation bonus, elevation id liquidation bonus must be less than the config's");
                    return err!(LendingError::InvalidConfig);
                }

                if elevation_group.liquidation_threshold_pct < config.liquidation_threshold_pct {
                    msg!("Invalid liquidation threshold, elevation id liquidation threshold must be greater than the config's");
                    return err!(LendingError::InvalidConfig);
                }

                if elevation_group.ltv_pct < config.loan_to_value_pct {
                    msg!("Invalid ltv ratio, cannot be bigger than the ltv ratio");
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
