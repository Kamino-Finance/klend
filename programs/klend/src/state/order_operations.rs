use std::{
    fmt::Display,
    ops::{Range, RangeInclusive},
};

use anchor_lang::{err, Result};
use fixed::prelude::ToFixed;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use solana_program::msg;

use crate::{
    fraction,
    fraction::FractionExtra,
    utils::{accounts::is_default_array, Fraction},
    xmsg, LendingError, LendingMarket, Obligation, ObligationOrder, Reserve,
};


const VALID_DEBT_COLL_PRICE_RATIO_RANGE: RangeInclusive<Fraction> =
    fraction!(0.000000000000001)..=fraction!(1000000000000000);





const VALID_USER_LTV_RANGE: Range<Fraction> = fraction!(0.01)..fraction!(1.0);


const VALID_DIFF_TO_LIQUIDATION_LTV_RANGE: Range<Fraction> = VALID_USER_LTV_RANGE;


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



#[repr(u8)]
#[derive(PartialEq, Eq, Debug, Clone, Copy, TryFromPrimitive, IntoPrimitive)]
pub enum OpportunityType {




    DeleverageSingleDebtAmount = 0,




    DeleverageAllDebt = 1,
}


pub type ApplicableObligationOrder = (usize, ConditionHit);


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
                normalized_distance_from_threshold
            ),
        }
    }
}





pub fn find_applicable_obligation_order(
    collateral_reserve: &Reserve,
    debt_reserve: &Reserve,
    obligation: &Obligation,
    price_triggered_liquidation_disabled: bool,
) -> Option<ApplicableObligationOrder> {
    for (order_index, order) in obligation.orders.iter().enumerate() {
        if let Some(condition_hit) =
            evaluate_order_condition(collateral_reserve, debt_reserve, obligation, order)
        {
            if order.condition_type().is_price_triggered() && price_triggered_liquidation_disabled {
                xmsg!(
                    "Obligation's order {}. condition {} is hit with {}, but price-triggered liquidations are disabled",
                    order_index,
                    order.condition_to_display(),
                    condition_hit
                );
                continue;
            }
            return Some((order_index, condition_hit));
        }
    }
    None
}








pub fn check_orders_supported_after_user_operation(obligation: &mut Obligation) -> Result<()> {
    let has_unsupported_orders = obligation
        .orders
        .iter()
        .any(|order| !order.is_supported_by(obligation));
    if has_unsupported_orders {
       
        let unsupported_orders = obligation
            .orders
            .iter()
            .filter(|order| !order.is_supported_by(obligation))
            .collect::<Vec<_>>();
        msg!(
            "The obligation has orders which have to be cancelled before the operation: {:?}",
            unsupported_orders
        );
        return err!(LendingError::OperationNotPermittedWithCurrentObligationOrders);
    }
    Ok(())
}






pub fn remove_all_orders(obligation: &mut Obligation) -> bool {
    let mut had_orders = false;
    for order in obligation.orders.iter_mut() {
       
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
) -> Result<()> {
    validate_order(order)?;
    if !order.is_supported_by(obligation) {
        msg!("Order {:?} not supported by obligation", order);
        return err!(LendingError::OrderConfigurationNotSupportedByObligation);
    }

    let index = usize::from(index);
    if index >= obligation.orders.len() {
        msg!(
            "Obligation may have at most {} orders; got index {}",
            obligation.orders.len(),
            index
        );
        return err!(LendingError::OrderIndexOutOfBounds);
    }

    let previous_order = &mut obligation.orders[index];
    if !previous_order.is_active()
        && order.is_active()
        && !lending_market.is_obligation_order_creation_enabled()
    {
        msg!("Creation of new orders is disabled by the market's configuration");
        return err!(LendingError::OrderCreationDisabled);
    }

    msg!(
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





    pub fn is_price_triggered(&self) -> bool {
        match self {
            ConditionType::Never | ConditionType::Always => false,
            ConditionType::UserLtvAbove
            | ConditionType::UserLtvBelow
            | ConditionType::DebtCollPriceRatioAbove
            | ConditionType::DebtCollPriceRatioBelow
            | ConditionType::LiquidationLtvCloserThan => true,
        }
    }
}

impl OpportunityType {
    pub fn is_supported_by(&self, obligation: &Obligation) -> bool {
        match self {
            Self::DeleverageSingleDebtAmount => obligation.single_debt().is_some(),
            Self::DeleverageAllDebt => true,
        }
    }
}

fn validate_order(order: ObligationOrder) -> Result<()> {
    match ConditionType::try_from(order.condition_type) {
        Ok(ConditionType::DebtCollPriceRatioAbove | ConditionType::DebtCollPriceRatioBelow) => {
            if !VALID_DEBT_COLL_PRICE_RATIO_RANGE.contains(&order.condition_threshold()) {
                msg!(
                    "Invalid price ratio threshold {}; should be in range [{}; {}]",
                    order.condition_threshold().to_display(),
                    VALID_DEBT_COLL_PRICE_RATIO_RANGE.start().to_display(),
                    VALID_DEBT_COLL_PRICE_RATIO_RANGE.end().to_display(),
                );
                return err!(LendingError::InvalidOrderConfiguration);
            }
        }
        Ok(ConditionType::UserLtvAbove | ConditionType::UserLtvBelow) => {
            if !VALID_USER_LTV_RANGE.contains(&order.condition_threshold()) {
                msg!(
                    "Invalid LTV threshold {}; should be in range [{}; {})",
                    order.condition_threshold().to_display(),
                    VALID_USER_LTV_RANGE.start.to_display(),
                    VALID_USER_LTV_RANGE.end.to_display(),
                );
                return err!(LendingError::InvalidOrderConfiguration);
            }
        }
        Ok(ConditionType::Always) => {
            if order.condition_threshold() != Fraction::default() {
                msg!(
                    "An unconditional order should use zeroed condition threshold; got {}",
                    order.condition_threshold()
                );
                return err!(LendingError::InvalidOrderConfiguration);
            }
            let bonus_range = order.execution_bonus_rate_range();
            if bonus_range.start() != bonus_range.end() {
                msg!(
                    "An unconditional order should define a constant bonus; got range [{}; {}]",
                    bonus_range.start(),
                    bonus_range.end()
                );
                return err!(LendingError::InvalidOrderConfiguration);
            }
        }
        Ok(ConditionType::Never) => {
            if order != ObligationOrder::default() {
                msg!("A void order should be entirely zeroed; got {:?}", order);
                return err!(LendingError::InvalidOrderConfiguration);
            }
           
            return Ok(());
        }
        Ok(ConditionType::LiquidationLtvCloserThan) => {
            if !VALID_DIFF_TO_LIQUIDATION_LTV_RANGE.contains(&order.condition_threshold()) {
                msg!(
                    "Invalid difference to liquidation LTV {}; should be in range [{}; {})",
                    order.condition_threshold(),
                    VALID_USER_LTV_RANGE.start,
                    VALID_USER_LTV_RANGE.end,
                );
                return err!(LendingError::InvalidOrderConfiguration);
            }
        }
        Err(error) => {
            msg!(
                "Invalid order condition type {}: {:?}",
                order.condition_type,
                error
            );
            return err!(LendingError::InvalidOrderConfiguration);
        }
    }
    match OpportunityType::try_from(order.opportunity_type) {
        Ok(OpportunityType::DeleverageSingleDebtAmount) => {
            if order.opportunity_parameter().is_zero() {
                msg!("Single debt deleveraging opportunity amount cannot be 0");
                return err!(LendingError::InvalidOrderConfiguration);
            }
            if order.opportunity_parameter() == Fraction::MAX {
                msg!("Single debt deleveraging opportunity amount must be finite (use DeleverageAllDebt for repaying all debt)");
                return err!(LendingError::InvalidOrderConfiguration);
            }
        }
        Ok(OpportunityType::DeleverageAllDebt) => {
            if order.opportunity_parameter() != Fraction::MAX {
                msg!("Deleveraging all debt opportunity must allow repaying the entire amount (Fraction::MAX)");
                return err!(LendingError::InvalidOrderConfiguration);
            }
        }
        Err(error) => {
            msg!(
                "Invalid order opportunity type {}: {:?}",
                order.opportunity_type,
                error
            );
            return err!(LendingError::InvalidOrderConfiguration);
        }
    }
    let execution_bonus_rate_range = order.execution_bonus_rate_range();
    if execution_bonus_rate_range.start() > execution_bonus_rate_range.end() {
        msg!(
            "Minimum execution bonus {} higher than maximum {}",
            execution_bonus_rate_range.start().to_display(),
            execution_bonus_rate_range.end().to_display(),
        );
        return err!(LendingError::InvalidOrderConfiguration);
    }
    if execution_bonus_rate_range.end() > &EXECUTION_BONUS_SANITY_LIMIT {
        msg!(
            "Maximum execution bonus {} higher than sanity limit {}",
            execution_bonus_rate_range.end().to_display(),
            EXECUTION_BONUS_SANITY_LIMIT.to_display()
        );
        return err!(LendingError::InvalidOrderConfiguration);
    }
    if !is_default_array(&order.padding1) || !is_default_array(&order.padding2) {
        msg!("Padding fields must be zeroed");
        return err!(LendingError::InvalidOrderConfiguration);
    }
    Ok(())
}

fn evaluate_order_condition(
    collateral_reserve: &Reserve,
    debt_reserve: &Reserve,
    obligation: &Obligation,
    order: &ObligationOrder,
) -> Option<ConditionHit> {
    match order.condition_type() {
        ConditionType::Always => Some(ConditionHit::without_distance()),
        ConditionType::Never => None,
        ConditionType::UserLtvAbove => evaluate_stop_loss(
            obligation.loan_to_value(),
            order.condition_threshold(),
            obligation.unhealthy_loan_to_value(),
        ),
        ConditionType::UserLtvBelow => {
            evaluate_take_profit(obligation.loan_to_value(), order.condition_threshold())
        }
        ConditionType::DebtCollPriceRatioAbove => {
            let price_ratio = calculate_price_ratio(debt_reserve, collateral_reserve);
            evaluate_stop_loss(
                price_ratio,
                order.condition_threshold(),
               
               
               
                price_ratio * obligation.unhealthy_loan_to_value() / obligation.loan_to_value(),
            )
        }
        ConditionType::DebtCollPriceRatioBelow => evaluate_take_profit(
            calculate_price_ratio(debt_reserve, collateral_reserve),
            order.condition_threshold(),
        ),
        ConditionType::LiquidationLtvCloserThan => {
            let unhealthy_ltv = obligation.unhealthy_loan_to_value();
            evaluate_stop_loss(
                obligation.loan_to_value(),
                unhealthy_ltv.saturating_sub(order.condition_threshold()),
                unhealthy_ltv,
            )
        }
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
    let normalized_distance_towards_liquidation = if condition_threshold > liquidation_threshold {
       
       
       
       
        Fraction::ONE
    } else {
       
        let current_distance = current_value - condition_threshold;
        let maximum_distance = liquidation_threshold - condition_threshold;
        current_distance / maximum_distance
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

