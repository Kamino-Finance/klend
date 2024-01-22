use std::cmp::{max, min, Ordering};

use anchor_lang::{err, prelude::msg, Result};
use solana_program::clock::Slot;

use crate::{
    fraction::FractionExtra,
    lending_market::utils::get_max_ltv_and_liquidation_threshold,
    utils::{
        bps_u128_to_fraction, fraction::fraction, slots, Fraction, BANKRUPTCY_THRESHOLD,
        ELEVATION_GROUP_NONE, MIN_AUTODELEVERAGE_BONUS_BPS,
    },
    xmsg, CalculateLiquidationResult, LendingError, LendingMarket, LendingResult,
    LiquidationParams, Obligation, ObligationCollateral, ObligationLiquidity, Reserve,
    ReserveConfig,
};

pub fn max_liquidatable_borrowed_amount(
    obligation: &Obligation,
    liquidation_max_debt_close_factor_pct: u8,
    market_max_liquidatable_debt_value_at_once: u64,
    liquidity: &ObligationLiquidity,
    user_ltv: Fraction,
    insolvency_risk_ltv_pct: u8,
) -> LendingResult<Fraction> {
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
    let max_liquidatable_amount = borrowed_amount * max_liquidation_ratio;

    Ok(max_liquidatable_amount)
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
    current_slot: Slot,
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
        current_slot,
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
        borrowed_amount_f
    } else {
        max_liquidatable_borrowed_amount(
            obligation,
            lending_market.liquidation_max_debt_close_factor_pct,
            lending_market.max_liquidatable_debt_market_value_at_once,
            liquidity,
            user_ltv,
            lending_market.insolvency_risk_unhealthy_ltv_pct,
        )?
        .min(borrowed_amount_f)
        .min(debt_amount_to_liquidate)
    };

    xmsg!(
        "Obligation is liquidated with liquidation bonus: {} bps, liquidation amount (rounded): {}",
        liquidation_bonus_rate.to_bps::<u32>().unwrap(),
        debt_liquidation_amount_f.round().to_num::<u64>()
    );

    let liquidation_ratio = debt_liquidation_amount_f / borrowed_amount_f;

    let total_liquidation_value_including_bonus = borrowed_value_f * liquidation_ratio * bonus_rate;

    let (settle_amount, repay_amount, withdraw_amount) = calculate_liquidation_amounts(
        total_liquidation_value_including_bonus,
        collateral,
        debt_liquidation_amount_f,
        is_below_min_full_liquidation_value_threshold,
    );

    Ok(CalculateLiquidationResult {
        settle_amount_f: settle_amount,
        repay_amount,
        withdraw_amount,
        liquidation_bonus_rate,
    })
}

pub fn get_liquidation_params(
    lending_market: &LendingMarket,
    collateral_reserve: &Reserve,
    debt_reserve: &Reserve,
    obligation: &Obligation,
    slot: Slot,
    max_allowed_ltv_override_pct_opt: Option<u64>,
) -> Result<LiquidationParams> {
    if let Some(params) = check_liquidate_obligation(
        lending_market,
        collateral_reserve,
        debt_reserve,
        obligation,
        max_allowed_ltv_override_pct_opt,
    ) {
        xmsg!(
            "Obligation is eligible for liquidation with liquidation bonus: {}bps",
            params.liquidation_bonus_rate.to_bps::<u64>().unwrap()
        );
        Ok(params)
    } else if let Some(params) = check_autodeleverage_obligation(
        lending_market,
        collateral_reserve,
        debt_reserve,
        obligation,
        slot,
    ) {
        xmsg!(
            "Obligation is eligible for auto-deleveraging liquidation with liquidation bonus: {}bps",
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
                emode_max_liquidation_bonus_bps,
            )
            .unwrap(),
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

            let repay_amount = repay_amount_f.to_ceil();

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
                && withdraw_amount_f < BANKRUPTCY_THRESHOLD
            {
                collateral.deposited_amount
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
    emode_max_liquidation_bonus_bps: u16,
) -> Result<Fraction> {
    let bad_debt_ltv = Fraction::ONE;

    if user_ltv >= fraction!(0.99) {
        let liquidation_bonus_bad_debt_bps = min(
            collateral_reserve_config.bad_debt_liquidation_bonus_bps,
            debt_reserve_config.bad_debt_liquidation_bonus_bps,
        );

        let liquidation_bonus_bad_debt = Fraction::from_bps(liquidation_bonus_bad_debt_bps);

        let capped_bonus = if user_ltv < bad_debt_ltv {
            let diff_to_bad_debt = bad_debt_ltv - user_ltv;
            max(liquidation_bonus_bad_debt, diff_to_bad_debt)
        } else {
            liquidation_bonus_bad_debt
        };

        return Ok(capped_bonus);
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

    let diff_to_bad_debt = bad_debt_ltv - user_ltv;
    let capped_max_liq_bonus_bad_debt = min(collared_bonus, diff_to_bad_debt);

    Ok(capped_max_liq_bonus_bad_debt)
}

pub fn check_autodeleverage_obligation(
    lending_market: &LendingMarket,
    collateral_reserve: &Reserve,
    debt_reserve: &Reserve,
    obligation: &Obligation,
    slot: Slot,
) -> Option<LiquidationParams> {
    if lending_market.autodeleverage_enabled == 0 {
        return None;
    }
    get_slots_since_autodeleverage_obligation_collateral_deposit_limit_crossed(
        collateral_reserve,
        slot,
    )
    .and_then(|slots_since_deleveraging_started| {
        get_autodeleverage_liquidation_params(
            lending_market,
            collateral_reserve,
            obligation,
            slots_since_deleveraging_started,
        )
    })
    .or_else(|| {
        get_slots_since_autodeleverage_obligation_debt_borrow_limit_crossed(debt_reserve, slot)
            .and_then(|slots_since_deleveraging_started| {
                get_autodeleverage_liquidation_params(
                    lending_market,
                    debt_reserve,
                    obligation,
                    slots_since_deleveraging_started,
                )
            })
    })
}

fn get_autodeleverage_liquidation_params(
    lending_market: &LendingMarket,
    autodeleverage_reserve: &Reserve,
    obligation: &Obligation,
    slots_since_deleveraging_started: u64,
) -> Option<LiquidationParams> {
    let (ltv_reduction_bps, autodeleverage_ltv_threshold) = calculate_autodeleverage_threshold(
        lending_market,
        autodeleverage_reserve,
        slots_since_deleveraging_started,
        obligation.elevation_group,
    )
    .unwrap();
    let user_ltv = obligation.loan_to_value();
    if user_ltv.ge(&autodeleverage_ltv_threshold) {
        let (days_since_deleveraging_started, liquidation_bonus) = calculate_autodeleverage_bonus(
            autodeleverage_reserve,
            slots_since_deleveraging_started,
            &user_ltv,
        )
        .unwrap();

        xmsg!("Auto-deleveraging LTV threshold crossed: {user_ltv}/{autodeleverage_ltv_threshold}, LTV reduction: {ltv_reduction_bps}, slots: {slots_since_deleveraging_started} ({days_since_deleveraging_started} days), liquidation bonus: {liquidation_bonus}", );
        Some(LiquidationParams {
            user_ltv,
            liquidation_bonus_rate: liquidation_bonus,
        })
    } else {
        xmsg!("LTV is below the current auto-deleverage threshold: {user_ltv}/{autodeleverage_ltv_threshold}, slots since deleveraging started: {slots_since_deleveraging_started}, LTV reduction: {ltv_reduction_bps}", );
        None
    }
}

fn get_slots_since_autodeleverage_obligation_collateral_deposit_limit_crossed(
    collateral_reserve: &Reserve,
    slot: Slot,
) -> Option<u64> {
    if collateral_reserve.deposit_limit_crossed().unwrap() {
        if collateral_reserve.liquidity.deposit_limit_crossed_slot == 0 {
            xmsg!("Reserve deposit limit crossed but timestamp not set - need to call refresh reserve?");
            None
        } else {
            xmsg!("Reserve is eligible for collateral auto-deleveraging");
            slot.checked_sub(collateral_reserve.liquidity.deposit_limit_crossed_slot)
                .filter(|slots_since_deleveraging_started| {
                    has_margin_call_period_expired(
                        collateral_reserve,
                        *slots_since_deleveraging_started,
                    )
                })
        }
    } else {
        xmsg!("Reserve deposit limit not crossed");
        None
    }
}

fn get_slots_since_autodeleverage_obligation_debt_borrow_limit_crossed(
    debt_reserve: &Reserve,
    slot: Slot,
) -> Option<u64> {
    if debt_reserve.borrow_limit_crossed().unwrap() {
        if debt_reserve.liquidity.borrow_limit_crossed_slot == 0 {
            xmsg!("Reserve borrow limit crossed but timestamp not set - need to call refresh reserve?");
            None
        } else {
            xmsg!("Reserve is eligible for debt auto-deleveraging");
            slot.checked_sub(debt_reserve.liquidity.borrow_limit_crossed_slot)
                .filter(|slots_since_deleveraging_started| {
                    has_margin_call_period_expired(debt_reserve, *slots_since_deleveraging_started)
                })
        }
    } else {
        xmsg!("Reserve borrow limit not crossed");
        None
    }
}

fn has_margin_call_period_expired(
    reserve: &Reserve,
    slots_since_deleveraging_started: u64,
) -> bool {
    let secs_since_deleveraging_started = slots::to_secs(slots_since_deleveraging_started);
    let deleveraging_margin_call_period_secs = reserve.config.deleveraging_margin_call_period_secs;
    if secs_since_deleveraging_started < deleveraging_margin_call_period_secs {
        xmsg!("Reserve is eligible for auto-deleveraging, but margin call period not expired ({secs_since_deleveraging_started}/{deleveraging_margin_call_period_secs} seconds)");
        false
    } else {
        true
    }
}

fn calculate_autodeleverage_threshold(
    lending_market: &LendingMarket,
    autodeleverage_reserve: &Reserve,
    slots_since_deleveraging_started: u64,
    obligation_elevation_group: u8,
) -> Result<(Fraction, Fraction)> {
    const ONE_BPS: Fraction = bps_u128_to_fraction(1_u128);
    let ltv_reduction_bps = ONE_BPS * u128::from(slots_since_deleveraging_started)
        / u128::from(
            autodeleverage_reserve
                .config
                .deleveraging_threshold_slots_per_bps,
        );

    let (_, liquidation_threshold_pct) = get_max_ltv_and_liquidation_threshold(
        lending_market,
        autodeleverage_reserve,
        obligation_elevation_group,
    )?;

    let liquidation_ltv = Fraction::from_percent(liquidation_threshold_pct);
    let autodeleverage_ltv_threshold = liquidation_ltv.saturating_sub(ltv_reduction_bps);
    Ok((ltv_reduction_bps, autodeleverage_ltv_threshold))
}

fn calculate_autodeleverage_bonus(
    autodeleverage_reserve: &Reserve,
    slots_since_deleveraging_started: u64,
    user_ltv: &Fraction,
) -> LendingResult<(Fraction, Fraction)> {
    let days_since_deleveraging_started =
        slots::to_days_fractional(slots_since_deleveraging_started);
    let ltv_rate = user_ltv / 100;

    let liquidation_bonus = Fraction::from_bps(MIN_AUTODELEVERAGE_BONUS_BPS)
        + (ltv_rate * days_since_deleveraging_started);

    let liquidation_bonus = min(
        liquidation_bonus,
        Fraction::from_bps(autodeleverage_reserve.config.max_liquidation_bonus_bps),
    );
    Ok((days_since_deleveraging_started, liquidation_bonus))
}

pub fn calculate_protocol_liquidation_fee(
    amount_liquidated: u64,
    liquidation_bonus: Fraction,
    protocol_liquidation_fee_pct: u8,
) -> LendingResult<u64> {
    let protocol_liquidation_fee_rate = Fraction::from_percent(protocol_liquidation_fee_pct);
    let amount_liquidated = Fraction::from(amount_liquidated);

    let bonus_rate = liquidation_bonus + Fraction::ONE;

    let bonus = amount_liquidated - (amount_liquidated / bonus_rate);

    let protocol_fee = bonus * protocol_liquidation_fee_rate;
    let protocol_fee: u64 = protocol_fee.to_ceil();

    Ok(max(protocol_fee, 1))
}
