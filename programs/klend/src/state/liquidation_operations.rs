use std::cmp::{max, min, Ordering};

use anchor_lang::{err, prelude::msg, Result};

use crate::{
    fraction::FractionExtra,
    utils::{
        fraction::fraction, secs, Fraction, DUST_LAMPORT_THRESHOLD, ELEVATION_GROUP_NONE,
        MIN_AUTODELEVERAGE_BONUS_BPS,
    },
    xmsg, CalculateLiquidationResult, LendingError, LendingMarket, LiquidationParams, Obligation,
    ObligationCollateral, ObligationLiquidity, Reserve, ReserveConfig,
};

pub fn max_liquidatable_borrowed_amount(
    obligation: &Obligation,
    liquidation_max_debt_close_factor_pct: u8,
    market_max_liquidatable_debt_value_at_once: u64,
    liquidity: &ObligationLiquidity,
    user_ltv: Fraction,
    insolvency_risk_ltv_pct: u8,
) -> Fraction {
    let obligation_debt_for_liquidity_mv = Fraction::from_bits(liquidity.market_value_sf);

    let total_obligation_debt_mv = Fraction::from_bits(obligation.borrowed_assets_market_value_sf);

    let liquidation_max_debt_close_factor_rate =
        if user_ltv > Fraction::from_percent(insolvency_risk_ltv_pct) {
            Fraction::ONE
        } else {
            Fraction::from_percent(liquidation_max_debt_close_factor_pct)
        };

    let market_max_liquidatable_debt_value_at_once =
        Fraction::from_num(market_max_liquidatable_debt_value_at_once);

    let calculated_liquidatable_mv =
        total_obligation_debt_mv * liquidation_max_debt_close_factor_rate;

    let max_liquidatable_mv = calculated_liquidatable_mv
        .min(obligation_debt_for_liquidity_mv)
        .min(market_max_liquidatable_debt_value_at_once);

    let max_liquidation_ratio = max_liquidatable_mv / obligation_debt_for_liquidity_mv;

    let borrowed_amount = Fraction::from_bits(liquidity.borrowed_amount_sf);
    borrowed_amount * max_liquidation_ratio
}

#[allow(clippy::too_many_arguments)]
pub fn calculate_liquidation(
    collateral_reserve: &Reserve,
    debt_reserve: &Reserve,
    debt_amount_to_liquidate: u64,
    lending_market: &LendingMarket,
    obligation: &Obligation,
    liquidity: &ObligationLiquidity,
    collateral: &ObligationCollateral,
    timestamp: u64,
    is_debt_reserve_highest_borrow_factor: bool,
    is_collateral_reserve_lowest_liquidation_ltv: bool,
    max_allowed_ltv_override_pct_opt: Option<u64>,
) -> Result<CalculateLiquidationResult> {
    if obligation.deposited_value_sf == 0 {
        msg!("Deposited value backing a loan cannot be 0");
        return err!(LendingError::InvalidObligationCollateral);
    }

    let LiquidationParams {
        user_ltv,
        liquidation_bonus_rate,
    } = get_liquidation_params(
        lending_market,
        collateral_reserve,
        debt_reserve,
        obligation,
        timestamp,
        is_debt_reserve_highest_borrow_factor,
        is_collateral_reserve_lowest_liquidation_ltv,
        max_allowed_ltv_override_pct_opt,
    )?;

    let bonus_rate = liquidation_bonus_rate + Fraction::ONE;

    let borrowed_amount_f = Fraction::from_bits(liquidity.borrowed_amount_sf);

    let borrowed_value_f = Fraction::from_bits(liquidity.market_value_sf);

    let debt_amount_to_liquidate =
        Fraction::from_num(debt_amount_to_liquidate).min(borrowed_amount_f);

    let is_below_min_full_liquidation_value_threshold =
        borrowed_value_f < lending_market.min_full_liquidation_value_threshold;

    let debt_liquidation_amount_f = if is_below_min_full_liquidation_value_threshold {
        if debt_amount_to_liquidate < borrowed_amount_f {
            msg!(
                "Liquidator-provided debt repay amount {} is too small to satisfy the required full liquidation {}",
                debt_amount_to_liquidate,
                borrowed_amount_f
            );
            return err!(LendingError::RepayTooSmallForFullLiquidation);
        }
        borrowed_amount_f
    } else {
        max_liquidatable_borrowed_amount(
            obligation,
            lending_market.liquidation_max_debt_close_factor_pct,
            lending_market.max_liquidatable_debt_market_value_at_once,
            liquidity,
            user_ltv,
            lending_market.insolvency_risk_unhealthy_ltv_pct,
        )
        .min(debt_amount_to_liquidate)
    };

    let liquidation_ratio = debt_liquidation_amount_f / borrowed_amount_f;

    let total_liquidation_value_including_bonus = borrowed_value_f * liquidation_ratio * bonus_rate;

    let (settle_amount, repay_amount, withdraw_amount) = calculate_liquidation_amounts(
        total_liquidation_value_including_bonus,
        collateral,
        debt_liquidation_amount_f,
        is_below_min_full_liquidation_value_threshold,
    );

    xmsg!(
        "Obligation is liquidated with liquidation bonus: {} bps, liquidation amount (rounded): {}",
        liquidation_bonus_rate.to_bps::<u32>().unwrap(),
        settle_amount.round().to_num::<u64>()
    );

    Ok(CalculateLiquidationResult {
        settle_amount_f: settle_amount,
        repay_amount,
        withdraw_amount,
        liquidation_bonus_rate,
    })
}

#[allow(clippy::too_many_arguments)]
pub fn get_liquidation_params(
    lending_market: &LendingMarket,
    collateral_reserve: &Reserve,
    debt_reserve: &Reserve,
    obligation: &Obligation,
    timestamp: u64,
    is_debt_reserve_highest_borrow_factor: bool,
    is_collateral_reserve_lowest_liquidation_ltv: bool,
    max_allowed_ltv_override_pct_opt: Option<u64>,
) -> Result<LiquidationParams> {
    if let Some(params) = check_liquidate_obligation(
        lending_market,
        collateral_reserve,
        debt_reserve,
        obligation,
        max_allowed_ltv_override_pct_opt,
    ) {
        if !is_debt_reserve_highest_borrow_factor {
            xmsg!("Debt reserve is not the highest borrow factor reserve, obligation cannot be liquidated");
            return err!(LendingError::LiquidationBorrowFactorPriority,);
        }

        if !is_collateral_reserve_lowest_liquidation_ltv {
            xmsg!(
                "Collateral reserve is not the lowest LTV reserve, obligation cannot be liquidated"
            );
            return err!(LendingError::LiquidationLowestLTVPriority);
        }

        xmsg!(
            "Obligation is eligible for liquidation with liquidation bonus: {}bps",
            params.liquidation_bonus_rate.to_bps::<u64>().unwrap()
        );
        Ok(params)
    } else if let Some(params) = check_individual_autodeleverage_obligation(
        lending_market,
        collateral_reserve,
        debt_reserve,
        obligation,
        timestamp,
    ) {
        xmsg!(
            "Obligation is eligible for target-ltv deleveraging with liquidation bonus: {}bps",
            params.liquidation_bonus_rate.to_bps::<u64>().unwrap()
        );
        Ok(params)
    } else if let Some(params) = check_market_wide_autodeleverage_obligation(
        lending_market,
        collateral_reserve,
        debt_reserve,
        obligation,
        timestamp,
    ) {
        xmsg!(
            "Obligation is eligible for market-wide deleveraging with liquidation bonus: {}bps",
            params.liquidation_bonus_rate.to_bps::<u64>().unwrap()
        );
        Ok(params)
    } else {
        xmsg!(
            "Obligation is healthy and cannot be liquidated, LTV: {}",
            obligation.loan_to_value()
        );
        return err!(LendingError::ObligationHealthy);
    }
}

pub fn check_liquidate_obligation(
    lending_market: &LendingMarket,
    collateral_reserve: &Reserve,
    debt_reserve: &Reserve,
    obligation: &Obligation,
    max_allowed_ltv_override_pct_opt: Option<u64>,
) -> Option<LiquidationParams> {
    let user_ltv = obligation.loan_to_value();
    let user_no_bf_ltv = obligation.no_bf_loan_to_value();
    let max_allowed_ltv_user = obligation.unhealthy_loan_to_value();
    let max_allowed_ltv_override_opt = max_allowed_ltv_override_pct_opt.map(Fraction::from_percent);
    let max_allowed_ltv = max_allowed_ltv_override_opt.unwrap_or(max_allowed_ltv_user);

    if user_ltv >= max_allowed_ltv {
        xmsg!("Obligation is eligible for liquidation, borrowed value (scaled): {}, unhealthy borrow value (scaled): {}, LTV: {}%/{}%, max_allowed_ltv_user {}%, max_allowed_ltv_override {:?}%",
            Fraction::from_bits(obligation.borrow_factor_adjusted_debt_value_sf).to_display(),
            Fraction::from_bits(obligation.unhealthy_borrow_value_sf).to_display(),
            user_ltv.to_percent::<u64>().unwrap(),
            max_allowed_ltv.to_percent::<u64>().unwrap(),
            max_allowed_ltv_user.to_percent::<u64>().unwrap(),
            max_allowed_ltv_override_pct_opt,
        );

        let emode_max_liquidation_bonus_bps = get_emode_max_liquidation_bonus(
            lending_market,
            &collateral_reserve.config,
            &debt_reserve.config,
            obligation,
        );

        return Some(LiquidationParams {
            user_ltv,
            liquidation_bonus_rate: calculate_liquidation_bonus(
                &collateral_reserve.config,
                &debt_reserve.config,
                max_allowed_ltv,
                user_ltv,
                user_no_bf_ltv,
                emode_max_liquidation_bonus_bps,
            ),
        });
    }
    None
}

fn get_emode_max_liquidation_bonus(
    lending_market: &LendingMarket,
    collateral_reserve: &ReserveConfig,
    debt_reserve: &ReserveConfig,
    obligation: &Obligation,
) -> u16 {
    if obligation.elevation_group != ELEVATION_GROUP_NONE
        && collateral_reserve
            .elevation_groups
            .contains(&obligation.elevation_group)
        && debt_reserve
            .elevation_groups
            .contains(&obligation.elevation_group)
    {
        let elevation_group = lending_market
            .get_elevation_group(obligation.elevation_group)
            .unwrap()
            .unwrap();

        if elevation_group.max_liquidation_bonus_bps > collateral_reserve.max_liquidation_bonus_bps
            || elevation_group.max_liquidation_bonus_bps > debt_reserve.max_liquidation_bonus_bps
            || elevation_group.max_liquidation_bonus_bps == 0
        {
            u16::MAX
        } else {
            elevation_group.max_liquidation_bonus_bps
        }
    } else {
        u16::MAX
    }
}

fn calculate_liquidation_amounts(
    total_liquidation_value_including_bonus: Fraction,
    collateral: &ObligationCollateral,
    debt_liquidation_amount: Fraction,
    is_below_min_full_liquidation_value_threshold: bool,
) -> (Fraction, u64, u64) {
    let collateral_value = Fraction::from_bits(collateral.market_value_sf);
    match total_liquidation_value_including_bonus.cmp(&collateral_value) {
        Ordering::Greater => {
            let repay_ratio = collateral_value / total_liquidation_value_including_bonus;

            let repay_amount_f = debt_liquidation_amount * repay_ratio;

            let settle_amount = if is_below_min_full_liquidation_value_threshold {
                debt_liquidation_amount
            } else {
                repay_amount_f
            };

            let repay_amount = settle_amount.to_ceil();

            let withdraw_amount = collateral.deposited_amount;
            (settle_amount, repay_amount, withdraw_amount)
        }
        Ordering::Equal => {
            let settle_amount = debt_liquidation_amount;
            let repay_amount = settle_amount.to_ceil();
            let withdraw_amount = collateral.deposited_amount;
            (settle_amount, repay_amount, withdraw_amount)
        }
        Ordering::Less => {
            let settle_amount = debt_liquidation_amount;
            let repay_amount = settle_amount.to_ceil();
            let withdraw_pct = total_liquidation_value_including_bonus / collateral_value;
            let withdraw_amount_f = Fraction::from_num(collateral.deposited_amount) * withdraw_pct;

            let withdraw_amount = if is_below_min_full_liquidation_value_threshold
                && withdraw_amount_f < DUST_LAMPORT_THRESHOLD
            {
                DUST_LAMPORT_THRESHOLD
            } else {
                withdraw_amount_f.to_floor()
            };
            (settle_amount, repay_amount, withdraw_amount)
        }
    }
}

fn calculate_liquidation_bonus(
    collateral_reserve_config: &ReserveConfig,
    debt_reserve_config: &ReserveConfig,
    max_allowed_ltv: Fraction,
    user_ltv: Fraction,
    user_no_bf_ltv: Fraction,
    emode_max_liquidation_bonus_bps: u16,
) -> Fraction {
    let bad_debt_ltv = Fraction::ONE;

    if user_no_bf_ltv >= fraction!(0.99) {
        let liquidation_bonus_bad_debt_bps = min(
            collateral_reserve_config.bad_debt_liquidation_bonus_bps,
            debt_reserve_config.bad_debt_liquidation_bonus_bps,
        );

        let liquidation_bonus_bad_debt = Fraction::from_bps(liquidation_bonus_bad_debt_bps);

        let capped_bonus = if user_no_bf_ltv < bad_debt_ltv {
            let diff_to_bad_debt = bad_debt_ltv - user_no_bf_ltv;
            max(liquidation_bonus_bad_debt, diff_to_bad_debt)
        } else {
            liquidation_bonus_bad_debt
        };

        return capped_bonus;
    }

    let unhealthy_factor = user_ltv - max_allowed_ltv;

    let max_bonus_bps = max(
        collateral_reserve_config.max_liquidation_bonus_bps,
        debt_reserve_config.max_liquidation_bonus_bps,
    );

    let max_bonus_bps = min(max_bonus_bps, emode_max_liquidation_bonus_bps);
    let max_bonus = Fraction::from_bps(max_bonus_bps);

    let min_reserve_bonus_bps = max(
        collateral_reserve_config.min_liquidation_bonus_bps,
        debt_reserve_config.min_liquidation_bonus_bps,
    );

    let min_reserve_bonus = Fraction::from_bps(min_reserve_bonus_bps);

    let min_bonus = max(min_reserve_bonus, unhealthy_factor);

    let collared_bonus = min(min_bonus, max_bonus);

    let diff_to_bad_debt = bad_debt_ltv - user_no_bf_ltv;

    min(collared_bonus, diff_to_bad_debt)
}

fn check_individual_autodeleverage_obligation(
    lending_market: &LendingMarket,
    collateral_reserve: &Reserve,
    debt_reserve: &Reserve,
    obligation: &Obligation,
    timestamp: u64,
) -> Option<LiquidationParams> {
    if !obligation.is_marked_for_deleveraging() {
        return None;
    }
    if !lending_market.is_autodeleverage_enabled() {
        xmsg!("Obligation is marked for auto-deleveraging, but the feature is disabled");
        return None;
    }
    if lending_market.individual_autodeleverage_margin_call_period_secs == 0 {
        xmsg!("Obligation is marked for auto-deleveraging, but the feature is misconfigured");
        return None;
    }
    let user_ltv = obligation.loan_to_value();
    let autodeleverage_target_ltv =
        Fraction::from_percent(obligation.autodeleverage_target_ltv_pct);
    if user_ltv <= autodeleverage_target_ltv {
        xmsg!("Obligation is marked for auto-deleveraging, but its LTV is already below target");
        return None;
    }
    let secs_since_margin_call_started =
        timestamp.saturating_sub(obligation.autodeleverage_margin_call_started_timestamp);
    let secs_since_deleveraging_started = get_secs_since_deleveraging_started(
        lending_market.individual_autodeleverage_margin_call_period_secs,
        secs_since_margin_call_started,
    )?;
    let days_since_deleveraging_started = secs::to_days_fractional(secs_since_deleveraging_started);
    let selected_reserve_config = [&collateral_reserve.config, &debt_reserve.config]
        .into_iter()
        .max_by_key(|reserve| {
            (
                reserve.max_liquidation_bonus_bps,
                reserve.deleveraging_bonus_increase_bps_per_day,
            )
        })
        .expect("must exist for a statically-constructed non-empty array");
    let liquidation_bonus_rate = calculate_autodeleverage_bonus(
        selected_reserve_config.deleveraging_bonus_increase_bps_per_day,
        selected_reserve_config.max_liquidation_bonus_bps,
        get_emode_max_liquidation_bonus(
            lending_market,
            &collateral_reserve.config,
            &debt_reserve.config,
            obligation,
        ),
        days_since_deleveraging_started,
        &obligation.no_bf_loan_to_value(),
    );
    xmsg!("Auto-deleveraging individual target LTV: {user_ltv}/{autodeleverage_target_ltv}, secs: {secs_since_deleveraging_started} ({days_since_deleveraging_started} days), liquidation bonus: {liquidation_bonus_rate}", );
    Some(LiquidationParams {
        user_ltv,
        liquidation_bonus_rate,
    })
}

fn check_market_wide_autodeleverage_obligation(
    lending_market: &LendingMarket,
    collateral_reserve: &Reserve,
    debt_reserve: &Reserve,
    obligation: &Obligation,
    timestamp: u64,
) -> Option<LiquidationParams> {
    if !lending_market.is_autodeleverage_enabled() {
        return None;
    }

    if collateral_reserve.config.is_autodeleverage_enabled() {
        let params = get_secs_since_autodeleverage_obligation_collateral_deposit_limit_crossed(
            collateral_reserve,
            timestamp,
        )
        .and_then(|secs_since_deleveraging_started| {
            get_autodeleverage_liquidation_params(
                collateral_reserve,
                obligation,
                secs_since_deleveraging_started,
            )
        });
        if params.is_some() {
            return params;
        }
    }

    if debt_reserve.config.is_autodeleverage_enabled() {
        let params = get_secs_since_autodeleverage_obligation_debt_borrow_limit_crossed(
            debt_reserve,
            timestamp,
        )
        .and_then(|secs_since_deleveraging_started| {
            get_autodeleverage_liquidation_params(
                debt_reserve,
                obligation,
                secs_since_deleveraging_started,
            )
        });
        if params.is_some() {
            return params;
        }
    }

    None
}

fn get_autodeleverage_liquidation_params(
    autodeleverage_reserve: &Reserve,
    obligation: &Obligation,
    secs_since_deleveraging_started: u64,
) -> Option<LiquidationParams> {
    let days_since_deleveraging_started = secs::to_days_fractional(secs_since_deleveraging_started);
    let autodeleverage_ltv_threshold =
        calculate_autodeleverage_threshold(autodeleverage_reserve, days_since_deleveraging_started);
    let user_ltv = obligation.loan_to_value();
    if user_ltv.ge(&autodeleverage_ltv_threshold) {
        let liquidation_bonus_rate = calculate_autodeleverage_bonus(
            autodeleverage_reserve
                .config
                .deleveraging_bonus_increase_bps_per_day,
            autodeleverage_reserve.config.max_liquidation_bonus_bps,
            u16::MAX,
            days_since_deleveraging_started,
            &obligation.no_bf_loan_to_value(),
        );

        xmsg!("Auto-deleveraging LTV threshold crossed: {user_ltv}/{autodeleverage_ltv_threshold}, seconds: {secs_since_deleveraging_started} ({days_since_deleveraging_started} days), liquidation bonus: {liquidation_bonus_rate}", );
        Some(LiquidationParams {
            user_ltv,
            liquidation_bonus_rate,
        })
    } else {
        xmsg!("LTV is below the current auto-deleverage threshold: {user_ltv}/{autodeleverage_ltv_threshold}, seconds since deleveraging started: {secs_since_deleveraging_started}", );
        None
    }
}

fn get_secs_since_autodeleverage_obligation_collateral_deposit_limit_crossed(
    collateral_reserve: &Reserve,
    timestamp: u64,
) -> Option<u64> {
    if collateral_reserve.deposit_limit_crossed() {
        if collateral_reserve.liquidity.deposit_limit_crossed_timestamp == 0 {
            xmsg!("Reserve deposit limit crossed but timestamp not set - need to call refresh reserve?");
            None
        } else {
            xmsg!("Reserve is eligible for collateral auto-deleveraging");
            let secs_since_margin_call_started = timestamp
                .saturating_sub(collateral_reserve.liquidity.deposit_limit_crossed_timestamp);
            get_secs_since_deleveraging_started(
                collateral_reserve
                    .config
                    .deleveraging_margin_call_period_secs,
                secs_since_margin_call_started,
            )
        }
    } else {
        xmsg!("Reserve deposit limit not crossed");
        None
    }
}

fn get_secs_since_autodeleverage_obligation_debt_borrow_limit_crossed(
    debt_reserve: &Reserve,
    timestamp: u64,
) -> Option<u64> {
    if debt_reserve.borrow_limit_crossed() {
        if debt_reserve.liquidity.borrow_limit_crossed_timestamp == 0 {
            xmsg!("Reserve borrow limit crossed but timestamp not set - need to call refresh reserve?");
            None
        } else {
            xmsg!("Reserve is eligible for debt auto-deleveraging");
            let secs_since_margin_call_started =
                timestamp.saturating_sub(debt_reserve.liquidity.borrow_limit_crossed_timestamp);
            get_secs_since_deleveraging_started(
                debt_reserve.config.deleveraging_margin_call_period_secs,
                secs_since_margin_call_started,
            )
        }
    } else {
        xmsg!("Reserve borrow limit not crossed");
        None
    }
}

fn get_secs_since_deleveraging_started(
    deleveraging_margin_call_period_secs: u64,
    secs_since_margin_call_started: u64,
) -> Option<u64> {
    let secs_since_deleveraging_started =
        secs_since_margin_call_started.checked_sub(deleveraging_margin_call_period_secs);
    if secs_since_deleveraging_started.is_none() {
        xmsg!("Reserve is eligible for auto-deleveraging, but margin call period not expired ({secs_since_margin_call_started}/{deleveraging_margin_call_period_secs} seconds)");
    }
    secs_since_deleveraging_started
}

fn calculate_autodeleverage_threshold(
    autodeleverage_reserve: &Reserve,
    days_since_deleveraging_started: Fraction,
) -> Fraction {
    let daily_ltv_threshold_decrease = Fraction::from_bps(
        autodeleverage_reserve
            .config
            .deleveraging_threshold_decrease_bps_per_day,
    );
    let ltv_threshold_reduction = daily_ltv_threshold_decrease * days_since_deleveraging_started;
    Fraction::ONE.saturating_sub(ltv_threshold_reduction)
}

fn calculate_autodeleverage_bonus(
    deleveraging_bonus_increase_bps_per_day: u64,
    reserve_max_bonus_bps: u16,
    emode_max_liquidation_bonus_bps: u16,
    days_since_deleveraging_started: Fraction,
    user_no_bf_ltv: &Fraction,
) -> Fraction {
    let static_min_bonus = Fraction::from_bps(MIN_AUTODELEVERAGE_BONUS_BPS);
    let daily_bonus_increase = Fraction::from_bps(deleveraging_bonus_increase_bps_per_day);

    let liquidation_bonus =
        static_min_bonus + (daily_bonus_increase * days_since_deleveraging_started);

    let configured_max_bonus =
        Fraction::from_bps(min(reserve_max_bonus_bps, emode_max_liquidation_bonus_bps));

    let diff_to_bad_debt = Fraction::ONE.saturating_sub(*user_no_bf_ltv);
    let effective_max_bonus = min(configured_max_bonus, diff_to_bad_debt);

    if liquidation_bonus > effective_max_bonus {
        xmsg!("After {days_since_deleveraging_started} days, at user_no_bf_ltv = {user_no_bf_ltv}, the autodeleverage bonus should be {liquidation_bonus}, but it is capped at {effective_max_bonus}", );
        effective_max_bonus
    } else {
        liquidation_bonus
    }
}

pub fn calculate_protocol_liquidation_fee(
    amount_liquidated: u64,
    liquidation_bonus: Fraction,
    protocol_liquidation_fee_pct: u8,
) -> u64 {
    let protocol_liquidation_fee_rate = Fraction::from_percent(protocol_liquidation_fee_pct);
    let amount_liquidated = Fraction::from(amount_liquidated);

    let bonus_rate = liquidation_bonus + Fraction::ONE;

    let bonus = amount_liquidated - (amount_liquidated / bonus_rate);

    let protocol_fee = bonus * protocol_liquidation_fee_rate;
    let protocol_fee: u64 = protocol_fee.to_ceil();

    max(protocol_fee, 1)
}
