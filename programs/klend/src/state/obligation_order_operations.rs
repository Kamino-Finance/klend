use std::{
    cmp::min,
    fmt::Display,
    ops::{Range, RangeInclusive},
};

use anchor_lang::{err, error, prelude::Pubkey, require_gte, Result};
use fixed::prelude::ToFixed;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::{
    fraction,
    fraction::FractionExtra,
    utils::{accounts::is_default_array, Fraction},
    xmsg, CollateralExchangeRate, LendingError, LendingMarket, Obligation, ObligationCollateral,
    ObligationLiquidity, ObligationOrder, Reserve,
};


const VALID_DEBT_COLL_PRICE_RATIO_RANGE: RangeInclusive<Fraction> =
    fraction!(0.000000000000001)..=fraction!(1000000000000000);





const VALID_CONDITION_LTV_RANGE: Range<Fraction> = fraction!(0.01)..fraction!(1.0);


const VALID_DIFF_TO_LIQUIDATION_LTV_RANGE: Range<Fraction> = VALID_CONDITION_LTV_RANGE;




const VALID_TARGET_LTV_RANGE: RangeInclusive<Fraction> = fraction!(0.0)..=fraction!(1.0);


const EXECUTION_BONUS_SANITY_LIMIT: Fraction = fraction!(0.1);















#[repr(u8)]
#[derive(PartialEq, Eq, Debug, Clone, Copy, TryFromPrimitive, IntoPrimitive)]
pub enum ConditionType {



    Never = 0,


    UserLtvAbove = 1,


    UserLtvBelow = 2,




    DebtCollPriceRatioAbove = 3,




    DebtCollPriceRatioBelow = 4,





    Always = 5,









    LiquidationLtvCloserThan = 6,
}


#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum OrderCondition {
    Never,
    UserLtvAbove(Fraction),
    UserLtvBelow(Fraction),
    DebtCollPriceRatioAbove(Fraction),
    DebtCollPriceRatioBelow(Fraction),
    Always,
    LiquidationLtvCloserThan(Fraction),
}













#[repr(u8)]
#[derive(PartialEq, Eq, Debug, Clone, Copy, TryFromPrimitive, IntoPrimitive)]
pub enum OpportunityType {













    DeleverageDebtAmount = 0,

















    LeverUpDebtAmount = 1,











    DeleverageToTargetLtv = 2,













    LeverUpToTargetLtv = 3,
}


#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum OrderOpportunity {
    Deleverage(OrderSize),
    LeverUp(OrderSize),
}

#[derive(PartialEq, Eq, Debug, Clone, Copy)]
pub enum OrderSize {
    DebtAmount(Fraction),
    ToTargetLtv(Fraction),
}

pub enum DebtMovementDirection {
    Increase {
        bonus_factor: Fraction,
    },
    Decrease {
        bonus_factor: Fraction,
        penalty_factor: Fraction,
    },
}


impl Display for OrderCondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderCondition::Never => f.write_str("<inactive>"),
            OrderCondition::UserLtvAbove(threshold) => {
                write!(f, "LTV > {}", threshold.to_display())
            }
            OrderCondition::UserLtvBelow(threshold) => {
                write!(f, "LTV < {}", threshold.to_display())
            }
            OrderCondition::DebtCollPriceRatioAbove(threshold) => write!(
                f,
                "ratio of (debt token price / collateral token price) > {}",
                threshold.to_display()
            ),
            OrderCondition::DebtCollPriceRatioBelow(threshold) => write!(
                f,
                "ratio of (debt token price / collateral token price) < {}",
                threshold.to_display()
            ),
            OrderCondition::Always => f.write_str("<unconditional>"),
            OrderCondition::LiquidationLtvCloserThan(threshold) => write!(
                f,
                "LTV closer than {} to liquidation",
                threshold.to_display()
            ),
        }
    }
}


impl Display for OrderOpportunity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OrderOpportunity::Deleverage(OrderSize::DebtAmount(amount)) => {
                write!(f, "repay {} of debt amount", amount.to_display())
            }
            OrderOpportunity::Deleverage(OrderSize::ToTargetLtv(ltv)) => {
                write!(f, "repay down to LTV {}", ltv.to_display())
            }
            OrderOpportunity::LeverUp(OrderSize::DebtAmount(amount)) => {
                write!(f, "borrow {} of debt amount", amount.to_display())
            }
            OrderOpportunity::LeverUp(OrderSize::ToTargetLtv(ltv)) => {
                write!(f, "borrow up to LTV {}", ltv.to_display())
            }
        }
    }
}


#[derive(PartialEq, Eq, Debug)]
pub struct ConditionHit {














    pub normalized_distance_from_threshold: Option<Fraction>,
}


impl Display for ConditionHit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.normalized_distance_from_threshold {
            None => f.write_str("<undefined distance from threshold>"),
            Some(normalized_distance_from_threshold) => write!(
                f,
                "distance from threshold = {}",
                normalized_distance_from_threshold.to_display()
            ),
        }
    }
}









pub fn check_orders_supported_after_user_operation(obligation: &mut Obligation) -> Result<()> {
    let has_unsupported_orders = obligation
        .obligation_orders
        .iter()
        .any(|order| !order.is_supported_by(obligation));
    if has_unsupported_orders {
       
        let unsupported_orders = obligation
            .obligation_orders
            .iter()
            .filter(|order| !order.is_supported_by(obligation))
            .collect::<Vec<_>>();
        xmsg!(
            "The obligation has orders which have to be cancelled before the operation: {:?}",
            unsupported_orders
        );
        return err!(LendingError::OperationNotPermittedWithCurrentObligationOrders);
    }
    Ok(())
}










pub fn remove_all_orders(obligation: &mut Obligation) -> bool {
    let mut had_orders = false;
    for order in obligation.obligation_orders.iter_mut() {
       
        if order != &ObligationOrder::default() {
            *order = ObligationOrder::default();
            had_orders = true;
        }
    }
    had_orders
}




pub fn set_order_on_obligation(
    lending_market: &LendingMarket,
    obligation: &mut Obligation,
    index: u8,
    order: ObligationOrder,
    min_expected_current_opportunity_parameter: Fraction,
) -> Result<()> {
    validate_order(&order)?;
    if !order.is_supported_by(obligation) {
        xmsg!("Order {:?} not supported by obligation", order);
        return err!(LendingError::OrderConfigurationNotSupportedByObligation);
    }

    let index = usize::from(index);
    if index >= obligation.obligation_orders.len() {
        xmsg!(
            "Obligation may have at most {} orders; got index {}",
            obligation.obligation_orders.len(),
            index
        );
        return err!(LendingError::OrderIndexOutOfBounds);
    }

    let previous_order = &mut obligation.obligation_orders[index];
    if !previous_order.is_active()
        && order.is_active()
        && !lending_market.is_obligation_order_creation_enabled()
    {
        xmsg!("Creation of new obligation orders is disabled by the market's configuration");
        return err!(LendingError::OrderCreationDisabled);
    }

   
    require_gte!(
        previous_order.opportunity_parameter(),
        min_expected_current_opportunity_parameter,
        LendingError::ExpectationNotMet,
    );

    xmsg!(
        "Setting obligation order[{}]; previous: {:?}; new: {:?}",
        index,
        previous_order,
        order
    );
    *previous_order = order;

    Ok(())
}



impl ConditionType {
    pub fn is_supported_by(&self, obligation: &Obligation) -> bool {
        match self {
            Self::Never => true,                            
            Self::UserLtvAbove | Self::UserLtvBelow => true,
            Self::DebtCollPriceRatioAbove | Self::DebtCollPriceRatioBelow => {
                obligation.is_single_debt_single_coll()
            }
            Self::Always => true,
            Self::LiquidationLtvCloserThan => true,
        }
    }


    pub fn iter_active() -> impl Iterator<Item = Self> {
        (1..=u8::MAX)
            .map(ConditionType::try_from)
            .take_while(|condition_type| condition_type.is_ok())
            .map(|condition_type| condition_type.expect("above we take only while no error"))
    }
}

impl OpportunityType {
   
    pub fn is_supported_by(&self, _obligation: &Obligation) -> bool {
        match self {
            Self::DeleverageDebtAmount => true,
            Self::LeverUpDebtAmount => true,
            Self::DeleverageToTargetLtv | Self::LeverUpToTargetLtv => true,
        }
    }
}

pub(crate) fn validate_order(order: &ObligationOrder) -> Result<()> {
    match ConditionType::try_from(order.condition_type) {
        Ok(ConditionType::DebtCollPriceRatioAbove | ConditionType::DebtCollPriceRatioBelow) => {
            if !VALID_DEBT_COLL_PRICE_RATIO_RANGE.contains(&order.condition_threshold()) {
                xmsg!(
                    "Invalid price ratio threshold {}; should be in range [{}; {}]",
                    order.condition_threshold().to_display(),
                    VALID_DEBT_COLL_PRICE_RATIO_RANGE.start().to_display(),
                    VALID_DEBT_COLL_PRICE_RATIO_RANGE.end().to_display(),
                );
                return err!(LendingError::InvalidOrderConfiguration);
            }
        }
        Ok(ConditionType::UserLtvAbove | ConditionType::UserLtvBelow) => {
            if !VALID_CONDITION_LTV_RANGE.contains(&order.condition_threshold()) {
                xmsg!(
                    "Invalid LTV threshold {}; should be in range [{}; {})",
                    order.condition_threshold().to_display(),
                    VALID_CONDITION_LTV_RANGE.start.to_display(),
                    VALID_CONDITION_LTV_RANGE.end.to_display(),
                );
                return err!(LendingError::InvalidOrderConfiguration);
            }
        }
        Ok(ConditionType::Always) => {
            if order.condition_threshold() != Fraction::default() {
                xmsg!(
                    "An unconditional order should use zeroed condition threshold; got {}",
                    order.condition_threshold().to_display()
                );
                return err!(LendingError::InvalidOrderConfiguration);
            }
            let bonus_range = order.execution_bonus_rate_range();
            if bonus_range.start() != bonus_range.end() {
                xmsg!(
                    "An unconditional order should define a constant bonus; got range [{}; {}]",
                    bonus_range.start().to_display(),
                    bonus_range.end().to_display(),
                );
                return err!(LendingError::InvalidOrderConfiguration);
            }
        }
        Ok(ConditionType::Never) => {
            if order != &ObligationOrder::default() {
                xmsg!("A void order should be entirely zeroed; got {:?}", order);
                return err!(LendingError::InvalidOrderConfiguration);
            }
           
            return Ok(());
        }
        Ok(ConditionType::LiquidationLtvCloserThan) => {
            if !VALID_DIFF_TO_LIQUIDATION_LTV_RANGE.contains(&order.condition_threshold()) {
                xmsg!(
                    "Invalid difference to liquidation LTV {}; should be in range [{}; {})",
                    order.condition_threshold().to_display(),
                    VALID_DIFF_TO_LIQUIDATION_LTV_RANGE.start.to_display(),
                    VALID_DIFF_TO_LIQUIDATION_LTV_RANGE.end.to_display(),
                );
                return err!(LendingError::InvalidOrderConfiguration);
            }
        }
        Err(error) => {
            xmsg!(
                "Invalid order condition type {}: {:?}",
                order.condition_type,
                error
            );
            return err!(LendingError::InvalidOrderConfiguration);
        }
    }
    match OpportunityType::try_from(order.opportunity_type) {
        Ok(OpportunityType::DeleverageDebtAmount) => {
            if order.opportunity_parameter().is_zero() {
                xmsg!("Debt-amount deleveraging opportunity amount cannot be 0");
                return err!(LendingError::InvalidOrderConfiguration);
            }
            if order.opportunity_parameter() == Fraction::MAX {
                xmsg!("Debt-amount deleveraging opportunity amount must be finite (use DeleverageToTargetLtv with target 0 for repaying all debt)");
                return err!(LendingError::InvalidOrderConfiguration);
            }
            if order.debt_mint_address == Pubkey::default() {
                xmsg!("Debt-amount deleveraging opportunity must specify a debt mint");
                return err!(LendingError::InvalidOrderConfiguration);
            }
            if order.max_borrow_rate_bps != 0 || order.min_debt_term_seconds != 0 {
                xmsg!("Deleveraging debt amount does not involve borrowing (debt reserve constraints must be zeroed)");
                return err!(LendingError::InvalidOrderConfiguration);
            }
        }
        Ok(OpportunityType::LeverUpDebtAmount) => {
            if order.opportunity_parameter().is_zero() {
                xmsg!("Debt-amount lever-up opportunity amount cannot be 0");
                return err!(LendingError::InvalidOrderConfiguration);
            }
            if order.opportunity_parameter() == Fraction::MAX {
                xmsg!("Debt-amount lever-up opportunity amount must be finite");
                return err!(LendingError::InvalidOrderConfiguration);
            }
            if order.debt_mint_address == Pubkey::default() {
                xmsg!("Debt-amount lever-up opportunity must specify a debt mint");
                return err!(LendingError::InvalidOrderConfiguration);
            }
            if order.collateral_mint_address == Pubkey::default() {
                xmsg!("Debt-amount lever-up opportunity must specify a collateral mint");
                return err!(LendingError::InvalidOrderConfiguration);
            }
            if order.max_borrow_rate_bps == 0 {
               
                xmsg!("Debt-amount lever-up opportunity must specify a max borrow rate");
                return err!(LendingError::InvalidOrderConfiguration);
            }
            check_lever_up_condition_not_risk_increasing(order)?;
        }
        Ok(OpportunityType::DeleverageToTargetLtv) => {
            if !VALID_TARGET_LTV_RANGE.contains(&order.opportunity_parameter()) {
                xmsg!(
                    "Invalid deleverage target LTV {}; should be in range [{}; {}]",
                    order.opportunity_parameter().to_display(),
                    VALID_TARGET_LTV_RANGE.start().to_display(),
                    VALID_TARGET_LTV_RANGE.end().to_display(),
                );
                return err!(LendingError::InvalidOrderConfiguration);
            }
            if order.max_borrow_rate_bps != 0 || order.min_debt_term_seconds != 0 {
                xmsg!("Target-LTV deleveraging does not involve borrowing (debt reserve constraints must be zeroed)");
                return err!(LendingError::InvalidOrderConfiguration);
            }
            check_target_ltv_against_ltv_condition(order, OpportunityType::DeleverageToTargetLtv)?;
        }
        Ok(OpportunityType::LeverUpToTargetLtv) => {
            if !VALID_TARGET_LTV_RANGE.contains(&order.opportunity_parameter()) {
                xmsg!(
                    "Invalid lever-up target LTV {}; should be in range [{}; {}]",
                    order.opportunity_parameter().to_display(),
                    VALID_TARGET_LTV_RANGE.start().to_display(),
                    VALID_TARGET_LTV_RANGE.end().to_display(),
                );
                return err!(LendingError::InvalidOrderConfiguration);
            }
            if order.debt_mint_address == Pubkey::default() {
                xmsg!("Target-LTV lever-up opportunity must specify a debt mint");
                return err!(LendingError::InvalidOrderConfiguration);
            }
            if order.collateral_mint_address == Pubkey::default() {
                xmsg!("Target-LTV lever-up opportunity must specify a collateral mint");
                return err!(LendingError::InvalidOrderConfiguration);
            }
            if order.max_borrow_rate_bps == 0 {
               
                xmsg!("Target-LTV lever-up opportunity must specify a max borrow rate");
                return err!(LendingError::InvalidOrderConfiguration);
            }
            check_lever_up_condition_not_risk_increasing(order)?;
            check_target_ltv_against_ltv_condition(order, OpportunityType::LeverUpToTargetLtv)?;
        }
        Err(error) => {
            xmsg!(
                "Invalid order opportunity type {}: {:?}",
                order.opportunity_type,
                error
            );
            return err!(LendingError::InvalidOrderConfiguration);
        }
    }
    let execution_bonus_rate_range = order.execution_bonus_rate_range();
    if execution_bonus_rate_range.start() > execution_bonus_rate_range.end() {
        xmsg!(
            "Minimum execution bonus {} higher than maximum {}",
            execution_bonus_rate_range.start().to_display(),
            execution_bonus_rate_range.end().to_display(),
        );
        return err!(LendingError::InvalidOrderConfiguration);
    }
    if execution_bonus_rate_range.end() > &EXECUTION_BONUS_SANITY_LIMIT {
        xmsg!(
            "Maximum execution bonus {} higher than sanity limit {}",
            execution_bonus_rate_range.end().to_display(),
            EXECUTION_BONUS_SANITY_LIMIT.to_display()
        );
        return err!(LendingError::InvalidOrderConfiguration);
    }
    if !is_default_array(&order.padding1) || !is_default_array(&order.padding2) {
        xmsg!("Padding fields must be zeroed");
        return err!(LendingError::InvalidOrderConfiguration);
    }
    Ok(())
}



fn check_lever_up_condition_not_risk_increasing(order: &ObligationOrder) -> Result<()> {
    match order.condition_type() {
        ConditionType::UserLtvAbove
        | ConditionType::LiquidationLtvCloserThan
        | ConditionType::DebtCollPriceRatioAbove => {
            xmsg!("A risk-increasing condition cannot trigger leveraging-up");
            err!(LendingError::InvalidOrderConfiguration)
        }
        ConditionType::Always
        | ConditionType::Never
        | ConditionType::UserLtvBelow
        | ConditionType::DebtCollPriceRatioBelow => Ok(()),
    }
}






fn check_target_ltv_against_ltv_condition(
    order: &ObligationOrder,
    opportunity_type: OpportunityType,
) -> Result<()> {
    let target_ltv = order.opportunity_parameter();
    let threshold = order.condition_threshold();
    let valid = match (opportunity_type, order.condition_type()) {
        (OpportunityType::LeverUpToTargetLtv, ConditionType::UserLtvBelow) => {
            target_ltv >= threshold
        }
        (OpportunityType::DeleverageToTargetLtv, ConditionType::UserLtvAbove) => {
            target_ltv <= threshold
        }
        (OpportunityType::DeleverageToTargetLtv, ConditionType::UserLtvBelow) => {
            target_ltv < threshold
        }
        _ => true,
    };
    if !valid {
        xmsg!(
            "Target LTV {} can never be meaningfully executed under the condition {}",
            target_ltv.to_display(),
            order.condition(),
        );
        return err!(LendingError::InvalidOrderConfiguration);
    }
    Ok(())
}

impl OrderCondition {



    pub fn evaluate(
        &self,
        collateral_reserve: &Reserve,
        debt_reserve: &Reserve,
        obligation: &Obligation,
    ) -> Result<ConditionHit> {
        match self {
            OrderCondition::Always => Some(ConditionHit::without_distance()),
            OrderCondition::Never => None,
            OrderCondition::UserLtvAbove(threshold) => evaluate_stop_loss(
                obligation.loan_to_value(),
                *threshold,
                obligation.unhealthy_loan_to_value(),
            ),
            OrderCondition::UserLtvBelow(threshold) => {
                evaluate_take_profit(obligation.loan_to_value(), *threshold)
            }
            OrderCondition::DebtCollPriceRatioAbove(threshold) => {
                let price_ratio = calculate_price_ratio(debt_reserve, collateral_reserve);
                evaluate_stop_loss(
                    price_ratio,
                    *threshold,
                   
                   
                   
                    price_ratio * obligation.unhealthy_loan_to_value() / obligation.loan_to_value(),
                )
            }
            OrderCondition::DebtCollPriceRatioBelow(threshold) => evaluate_take_profit(
                calculate_price_ratio(debt_reserve, collateral_reserve),
                *threshold,
            ),
            OrderCondition::LiquidationLtvCloserThan(threshold) => {
                let unhealthy_ltv = obligation.unhealthy_loan_to_value();
                evaluate_stop_loss(
                    obligation.loan_to_value(),
                    unhealthy_ltv.saturating_sub(*threshold),
                    unhealthy_ltv,
                )
            }
        }
        .ok_or_else(|| error!(LendingError::ObligationOrderConditionNotMet))
    }
}

fn evaluate_stop_loss(
    current_value: Fraction,
    condition_threshold: Fraction,
    liquidation_threshold: Fraction,
) -> Option<ConditionHit> {
    if current_value <= condition_threshold {
        return None;
    }
    let normalized_distance_towards_liquidation = if condition_threshold >= liquidation_threshold {
       
       
       
       
        Fraction::ONE
    } else {
       
        let current_distance = current_value - condition_threshold;
        let maximum_distance = liquidation_threshold - condition_threshold;
       
       
        min(current_distance / maximum_distance, Fraction::ONE)
    };
    Some(ConditionHit::with_distance(
        normalized_distance_towards_liquidation,
    ))
}

fn evaluate_take_profit(
    current_value: Fraction,
    condition_threshold: Fraction,
) -> Option<ConditionHit> {
    if current_value >= condition_threshold {
        return None;
    }
    let distance_towards_0 = condition_threshold - current_value;
    Some(ConditionHit::with_distance(
        distance_towards_0 / condition_threshold,
    ))
}

fn calculate_price_ratio(numerator_reserve: &Reserve, denominator_reserve: &Reserve) -> Fraction {
    let numerator_price = numerator_reserve.liquidity.get_market_price();
    let denominator_price = denominator_reserve.liquidity.get_market_price();
    numerator_price / denominator_price
}

impl ConditionHit {

    pub fn with_distance(normalized_distance_from_threshold: impl ToFixed) -> Self {
        Self {
            normalized_distance_from_threshold: Some(Fraction::from_num(
                normalized_distance_from_threshold,
            )),
        }
    }



    pub fn without_distance() -> Self {
        Self {
            normalized_distance_from_threshold: None,
        }
    }
}

















pub(crate) fn calculate_order_execution_bonus_rate(
    order: &ObligationOrder,
    condition_hit: &ConditionHit,
    user_no_bf_ltv: Fraction,
    early_repay_penalty_rate: Fraction,
) -> Fraction {
    let theoretic_bonus_rate = match condition_hit.normalized_distance_from_threshold {
        Some(normalized_distance_from_threshold) => interpolate_bonus_rate(
            normalized_distance_from_threshold,
            order.execution_bonus_rate_range(),
        ),
        None => get_constant_bonus_rate(order),
    };
   
   
   
    let diff_to_bad_debt = Fraction::ONE.saturating_sub(user_no_bf_ltv);
    let penalty_factor = Fraction::ONE + early_repay_penalty_rate;
    let max_bonus_rate = diff_to_bad_debt.saturating_sub(early_repay_penalty_rate) / penalty_factor;
    if theoretic_bonus_rate > max_bonus_rate {
        xmsg!(
            "At user_no_bf_ltv = {} and early_repay_penalty_rate = {}, the calculated order execution bonus {} is capped at {}",
            user_no_bf_ltv,
            early_repay_penalty_rate,
            theoretic_bonus_rate,
            max_bonus_rate
        );
        max_bonus_rate
    } else {
        theoretic_bonus_rate
    }
}

fn interpolate_bonus_rate(
    normalized_distance_from_threshold: Fraction,
    bonus_rate_range: RangeInclusive<Fraction>,
) -> Fraction {
    bonus_rate_range.start()
        + normalized_distance_from_threshold * (bonus_rate_range.end() - bonus_rate_range.start())
}

fn get_constant_bonus_rate(order: &ObligationOrder) -> Fraction {
    let range = order.execution_bonus_rate_range();
    if range.end() != range.start() {
        panic!(
            "The order validation should not have allowed non-constant bonus range when condition is {}; got: [{}; {}]",
            order.condition(),
            range.start(),
            range.end()
        );
    }
    *range.start()
}



pub(crate) fn calculate_debt_amount_to_reach_target_ltv(
    obligation: &Obligation,
    debt_reserve: &Reserve,
    target_ltv: Fraction,
    debt_movement_direction: DebtMovementDirection,
) -> Fraction {
    let borrow_factor = debt_reserve.borrow_factor_f(obligation.elevation_group().is_some());

   
    let (ltv_distance, collateral_side_ltv_speed, borrow_side_ltv_speed) =
        match debt_movement_direction {
           
            DebtMovementDirection::Increase { bonus_factor } => {
               
                let fee_factor = Fraction::ONE + debt_reserve.config.fees.origination_fee_rate();
                (
                    target_ltv.saturating_sub(obligation.loan_to_value()),
                    target_ltv / bonus_factor,
                    borrow_factor * fee_factor,
                )
            }
           
            DebtMovementDirection::Decrease {
                bonus_factor,
                penalty_factor,
            } => {
                if target_ltv.is_zero() {
                    return Fraction::MAX;
                }
                (
                    obligation.loan_to_value().saturating_sub(target_ltv),
                    target_ltv * bonus_factor * penalty_factor,
                    borrow_factor,
                )
            }
        };

   
    if ltv_distance.is_zero() {
        return Fraction::ZERO;
    }

   
    let ltv_speed = borrow_side_ltv_speed.saturating_sub(collateral_side_ltv_speed);
    if ltv_speed.is_zero() {
       
        return Fraction::MAX;
    }

   
    let Some(debt_value) = obligation
        .deposited_value()
        .try_full_mul_int_ratio(ltv_distance.to_bits(), ltv_speed.to_bits())
    else {
        return Fraction::MAX;
    };
    debt_reserve
        .liquidity
        .try_market_value_to_liquidity_amount(debt_value)
        .unwrap_or(Fraction::MAX)
}


#[allow(clippy::too_many_arguments)]
pub(crate) fn resolve_deleverage_order_execution(
    order_size: OrderSize,
    collateral_reserve: &Reserve,
    debt_reserve: &Reserve,
    collateral_reserve_pubkey: Pubkey,
    debt_reserve_pubkey: Pubkey,
    obligation: &Obligation,
    max_given_repay_amount: u64,
    min_received_liquidity_amount: u64,
    bonus_factor: Fraction,
    penalty_factor: Fraction,
) -> Result<DeleverageExecution> {
    let liquidity = obligation.find_liquidity_in_borrows(debt_reserve_pubkey)?.0;

   
    let ordered_debt_amount = resolve_ordered_debt_amount(
        order_size,
        DebtMovementDirection::Decrease {
            bonus_factor,
            penalty_factor,
        },
        obligation,
        debt_reserve,
    );

   
    let exchange_rate = collateral_reserve.collateral_exchange_rate();
    let DeleverageAmounts {
        repay_amount,
        bonus_priced_withdraw_collateral_amount,
        bonus_amount,
        consumed_entire_order,
    } = calculate_deleverage_execution_amounts(
        Fraction::from_num(max_given_repay_amount),
        ordered_debt_amount,
        penalty_factor,
        bonus_factor,
        exchange_rate,
        liquidity,
        obligation.find_collateral_in_deposits(collateral_reserve_pubkey)?,
    )?;

   
    let protocol_fee =
        calculate_protocol_obligation_order_execution_fee(bonus_amount, collateral_reserve);

   
    let withdraw_collateral_amount = if min_received_liquidity_amount == 0 {
       
        bonus_priced_withdraw_collateral_amount
    } else {
       
        let min_withdraw_liquidity_amount = min_received_liquidity_amount + protocol_fee;
        min(
            exchange_rate.liquidity_to_collateral_ceil(min_withdraw_liquidity_amount),
            bonus_priced_withdraw_collateral_amount,
        )
    };

    Ok(DeleverageExecution {
        repay_amount,
        withdraw_collateral_amount,
        protocol_fee,
        consumed_entire_order,
    })
}





pub(crate) fn resolve_lever_up_order_execution(
    order_size: OrderSize,
    collateral_reserve: &Reserve,
    debt_reserve: &Reserve,
    obligation: &Obligation,
    max_given_deposit_amount: u64,
    bonus_factor: Fraction,
) -> Result<LeverUpExecution> {
   
    let ordered_debt_amount = resolve_ordered_debt_amount(
        order_size,
        DebtMovementDirection::Increase { bonus_factor },
        obligation,
        debt_reserve,
    );

   
    let LeverUpAmounts {
        bonus_priced_borrow_amount,
        deposit_liquidity_amount,
        bonus_amount,
        consumed_entire_order,
    } = calculate_lever_up_execution_amounts(
        max_given_deposit_amount,
        ordered_debt_amount,
        bonus_factor,
        debt_reserve,
        collateral_reserve,
    )?;

    Ok(LeverUpExecution {
        borrow_liquidity_amount: bonus_priced_borrow_amount,
        deposit_liquidity_amount,
        protocol_fee: calculate_protocol_obligation_order_execution_fee(bonus_amount, debt_reserve),
        consumed_entire_order,
    })
}



fn resolve_ordered_debt_amount(
    order_size: OrderSize,
    debt_movement_direction: DebtMovementDirection,
    obligation: &Obligation,
    debt_reserve: &Reserve,
) -> Fraction {
    match order_size {
        OrderSize::DebtAmount(amount) => match debt_movement_direction {
            DebtMovementDirection::Increase { .. } => {
               
                let fee_factor = Fraction::ONE + debt_reserve.config.fees.origination_fee_rate();
                amount / fee_factor
            }
            DebtMovementDirection::Decrease { .. } => amount,
        },
        OrderSize::ToTargetLtv(target_ltv) => calculate_debt_amount_to_reach_target_ltv(
            obligation,
            debt_reserve,
            target_ltv,
            debt_movement_direction,
        ),
    }
}



pub(crate) enum OrderExecution {
    Deleverage(DeleverageExecution),
    LeverUp(LeverUpExecution),
}

impl OrderOpportunity {

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve(
        self,
        collateral_reserve: &Reserve,
        debt_reserve: &Reserve,
        collateral_reserve_pubkey: Pubkey,
        debt_reserve_pubkey: Pubkey,
        obligation: &Obligation,
        max_given_liquidity_amount: u64,
        min_received_liquidity_amount: u64,
        execution_bonus_rate: Fraction,
        early_repay_penalty_rate: Fraction,
    ) -> Result<OrderExecution> {
        let bonus_factor = Fraction::ONE + execution_bonus_rate;
        match self {
            OrderOpportunity::Deleverage(size) => resolve_deleverage_order_execution(
                size,
                collateral_reserve,
                debt_reserve,
                collateral_reserve_pubkey,
                debt_reserve_pubkey,
                obligation,
                max_given_liquidity_amount,
                min_received_liquidity_amount,
                bonus_factor,
                Fraction::ONE + early_repay_penalty_rate,
            )
            .map(OrderExecution::Deleverage),
            OrderOpportunity::LeverUp(size) => resolve_lever_up_order_execution(
                size,
                collateral_reserve,
                debt_reserve,
                obligation,
                max_given_liquidity_amount,
                bonus_factor,
            )
            .map(OrderExecution::LeverUp),
        }
    }
}


pub(crate) struct DeleverageAmounts {
    pub repay_amount: u64,
    pub bonus_priced_withdraw_collateral_amount: u64,
    pub bonus_amount: Fraction,
    pub consumed_entire_order: bool,
}

pub(crate) struct DeleverageExecution {
    pub repay_amount: u64,
    pub withdraw_collateral_amount: u64,
    pub protocol_fee: u64,
    pub consumed_entire_order: bool,
}



fn calculate_bonus_amount(total_amount: impl Into<Fraction>, bonus_factor: Fraction) -> Fraction {
    total_amount.into().full_mul_int_ratio(
        (bonus_factor - Fraction::ONE).to_bits(),
        bonus_factor.to_bits(),
    )
}

pub(crate) fn calculate_deleverage_execution_amounts(
    max_given_repay_amount: Fraction,
    ordered_debt_amount: Fraction,
    penalty_factor: Fraction,
    bonus_factor: Fraction,
    exchange_rate: CollateralExchangeRate,
    liquidity: &ObligationLiquidity,
    collateral: &ObligationCollateral,
) -> Result<DeleverageAmounts> {
    let borrowed_amount = liquidity.borrowed_amount();
    let entire_collateral_value = collateral.market_value();

   
    let premium_factor = penalty_factor * bonus_factor;

   
   
   

    let effective_max_given_repay_amount = max_given_repay_amount / penalty_factor;
    let equivalent_collateral_value = entire_collateral_value / premium_factor;

   
    let collateral_capacity_amount = if equivalent_collateral_value < liquidity.market_value() {
       
        borrowed_amount.full_mul_int_ratio(
            equivalent_collateral_value.to_bits(),
            liquidity.market_value_sf,
        )
    } else {
       
        borrowed_amount
    };

   
    let configured_limit_amount = min(ordered_debt_amount, effective_max_given_repay_amount);

   
    let capacity_limit_amount = min(borrowed_amount, collateral_capacity_amount);

   
    let debt_reduction_amount = min(configured_limit_amount, capacity_limit_amount);

   
    let repay_amount = if capacity_limit_amount < configured_limit_amount {
        min(
            debt_reduction_amount.to_ceil(),
            effective_max_given_repay_amount.to_floor(),
        )
    } else {
        debt_reduction_amount.to_floor()
    };

   
    let settled_amount = min(Fraction::from_num(repay_amount), borrowed_amount);
    let settled_debt_value = liquidity
        .market_value()
        .full_mul_int_ratio(settled_amount.to_bits(), liquidity.borrowed_amount_sf);
    let settled_value = settled_debt_value * premium_factor;
    let seized_value = min(settled_value, entire_collateral_value);
    let bonus_priced_withdraw_collateral_amount = Fraction::from_num(collateral.deposited_amount)
        .full_mul_int_ratio(seized_value.to_bits(), collateral.market_value_sf)
        .to_floor();

   
    let bonus_priced_withdraw_liquidity_amount =
        exchange_rate.collateral_to_liquidity(bonus_priced_withdraw_collateral_amount);

    Ok(DeleverageAmounts {
        repay_amount,
        bonus_priced_withdraw_collateral_amount,
        bonus_amount: calculate_bonus_amount(bonus_priced_withdraw_liquidity_amount, bonus_factor),
        consumed_entire_order: debt_reduction_amount == ordered_debt_amount,
    })
}


pub(crate) struct LeverUpAmounts {
    pub bonus_priced_borrow_amount: u64,
    pub deposit_liquidity_amount: u64,
    pub bonus_amount: Fraction,
    pub consumed_entire_order: bool,
}

pub(crate) struct LeverUpExecution {
    pub borrow_liquidity_amount: u64,
    pub deposit_liquidity_amount: u64,
    pub protocol_fee: u64,
    pub consumed_entire_order: bool,
}

pub(crate) fn calculate_lever_up_execution_amounts(
    max_given_deposit_amount: u64,
    order_size: Fraction,
    bonus_factor: Fraction,
    debt_reserve: &Reserve,
    collateral_reserve: &Reserve,
) -> Result<LeverUpAmounts> {
    let exchange_rate = collateral_reserve.collateral_exchange_rate();

    let deposit_collateral_amount = if max_given_deposit_amount == u64::MAX {
       
        let order_raw_value = debt_reserve
            .liquidity
            .liquidity_amount_to_market_value(order_size);
        let required_deposit_value = order_raw_value / bonus_factor;
        let required_deposit_liquidity = collateral_reserve
            .liquidity
            .market_value_to_liquidity_amount(required_deposit_value);
        exchange_rate
            .fraction_liquidity_to_collateral_ceil(required_deposit_liquidity)
            .to_ceil()
    } else {
       
        exchange_rate.liquidity_to_collateral(max_given_deposit_amount)
    };

   
    let deposit_liquidity_amount =
        exchange_rate.collateral_to_liquidity_ceil(deposit_collateral_amount);

   
    let deposit_liquidity_value = collateral_reserve
        .liquidity
        .liquidity_amount_to_market_value(Fraction::from_num(deposit_liquidity_amount));
    let equivalent_borrow_liquidity_amount = debt_reserve
        .liquidity
        .market_value_to_liquidity_amount(deposit_liquidity_value);
    let bonus_borrow_liquidity_amount = equivalent_borrow_liquidity_amount * bonus_factor;

   
    let bonus_priced_borrow_amount = min(bonus_borrow_liquidity_amount, order_size).to_floor();

    Ok(LeverUpAmounts {
        bonus_priced_borrow_amount,
        deposit_liquidity_amount,
        bonus_amount: calculate_bonus_amount(bonus_priced_borrow_amount, bonus_factor),
        consumed_entire_order: bonus_borrow_liquidity_amount >= order_size,
    })
}

pub fn calculate_protocol_obligation_order_execution_fee(
    bonus_amount: Fraction,
    reserve: &Reserve,
) -> u64 {
   
    (bonus_amount * reserve.config.protocol_order_execution_fee_rate()).to_ceil()
}

pub(crate) fn check_obligation_order_min_execution_value(
    cleared_borrow_or_deposit: bool,
    lending_market: &LendingMarket,
    debt_reserve: &Reserve,
    executed_debt_amount: u64,
    remaining_ordered_debt_amount: Option<Fraction>,
) -> Result<()> {
   
    if cleared_borrow_or_deposit {
        return Ok(());
    }

   
    if remaining_ordered_debt_amount == Some(Fraction::ZERO) {
        return Ok(());
    }

   
    let executed_value = debt_reserve
        .liquidity
        .liquidity_amount_to_market_value(Fraction::from_num(executed_debt_amount));
    if executed_value < lending_market.min_obligation_order_execution_value {
        xmsg!(
            "Executed amount {} would have value {}, lower than the configured minimum {}",
            executed_debt_amount,
            executed_value.to_display(),
            lending_market.min_obligation_order_execution_value
        );
        return err!(LendingError::ObligationOrderExecutionValueTooSmall);
    }

   
    if let Some(remaining_ordered_debt_amount) = remaining_ordered_debt_amount {
        let remaining_value = debt_reserve
            .liquidity
            .liquidity_amount_to_market_value(remaining_ordered_debt_amount);
        if remaining_value < lending_market.min_obligation_order_execution_value {
            xmsg!(
                "Order's remaining size {} would have value {}, below the configured minimum {}",
                remaining_ordered_debt_amount.to_display(),
                remaining_value.to_display(),
                lending_market.min_obligation_order_execution_value
            );
            return err!(LendingError::ObligationOrderRemainingValueTooSmall);
        }
    }

    Ok(())
}




