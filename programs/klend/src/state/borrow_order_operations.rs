use std::fmt::Debug;

use anchor_lang::{err, Result};
use solana_program::{clock::Clock, msg};

use crate::{
    lending_market::utils::calculate_market_value_from_liquidity_amount,
    utils::{accounts::default_array, EventEmitter, Fraction, FractionExtra},
    BorrowOrder, BorrowOrderCancelEvent, BorrowOrderConfig, BorrowOrderFullFillEvent,
    BorrowOrderPartialFillEvent, BorrowOrderPlaceEvent, BorrowOrderUpdateEvent, LendingError,
    LendingMarket, Reserve,
};






pub fn set_borrow_order(
    lending_market: &LendingMarket,
    borrow_order: &mut BorrowOrder,
    order_config: Option<BorrowOrderConfig>,
    clock: &Clock,
    event_emitter: impl EventEmitter,
) -> Result<()> {
   
    let Some(order_config) = order_config else {
        if borrow_order == &Default::default() {
            msg!("Ignored a no-op cancellation of the borrow order");
        } else {
            event_emitter.emit(BorrowOrderCancelEvent {
                before: *borrow_order,
            })?;
            *borrow_order = BorrowOrder::default();
        }
        return Ok(());
    };

    let timestamp = clock.unix_timestamp.try_into().expect("negative timestamp");

   
    check_order_config_valid(&order_config, timestamp)?;

   
    borrow_order.clear_if_past_fillable_timestamp(timestamp);

   
    if borrow_order == &Default::default() {
       
        check_borrow_order_creation_enabled(lending_market)?;
        initialize_borrow_order(borrow_order, order_config, timestamp)?;
        event_emitter.emit(BorrowOrderPlaceEvent {
            after: *borrow_order,
        })?;
    } else {
        let borrow_order_initial_state = *borrow_order;
        update_borrow_order_config(borrow_order, order_config, timestamp)?;
        event_emitter.emit(BorrowOrderUpdateEvent {
            before: borrow_order_initial_state,
            after: *borrow_order,
        })?;
    }

    Ok(())
}





pub fn fill_borrow_order(
    lending_market: &LendingMarket,
    reserve: &Reserve,
    borrow_order: &mut BorrowOrder,
    clock: &Clock,
    amount: u64,
    event_emitter: impl EventEmitter,
) -> Result<()> {
    check_borrow_order_execution_enabled(lending_market)?;

   
    let reserve_max_borrow_rate_bps = reserve.config.max_borrow_rate_bps();
    if reserve_max_borrow_rate_bps > borrow_order.max_borrow_rate_bps {
        msg!(
            "Cannot use reserve with max borrow rate of {} bps on an order requesting max {} bps",
            reserve_max_borrow_rate_bps,
            borrow_order.max_borrow_rate_bps
        );
        return err!(LendingError::BorrowOrderMaxBorrowRateExceeded);
    }

   
    if !is_term_satisfied(
        borrow_order.get_min_debt_term_seconds(),
        reserve.config.get_debt_term_seconds(),
    ) {
        msg!(
            "Cannot use reserve with debt term of {} seconds on an order requesting min {} seconds",
            reserve.config.debt_term_seconds,
            borrow_order.min_debt_term_seconds
        );
        return err!(LendingError::BorrowOrderMinDebtTermInsufficient);
    }

    let current_timestamp: u64 = clock.unix_timestamp.try_into().expect("negative timestamp");
   
   
   
    let seconds_until_reserve_debt_maturity =
        reserve
            .config
            .get_debt_maturity_timestamp()
            .map(|reserve_debt_maturity_timestamp| {
                reserve_debt_maturity_timestamp.saturating_sub(current_timestamp)
            });

   
    if !is_term_satisfied(
        borrow_order.get_min_debt_term_seconds(),
        seconds_until_reserve_debt_maturity,
    ) {
        msg!(
            "Cannot use reserve with debt maturity timestamp {} (i.e. in {:?} seconds) on an order requesting min {} seconds",
            reserve.config.debt_maturity_timestamp,
            seconds_until_reserve_debt_maturity,
            borrow_order.min_debt_term_seconds,
        );
        return err!(LendingError::BorrowOrderMinDebtTermInsufficient);
    }

   
    if current_timestamp > borrow_order.fillable_until_timestamp {
        msg!(
            "At current timestamp {} it is no longer possible to fill an order fillable until {}",
            current_timestamp,
            borrow_order.fillable_until_timestamp
        );
        return err!(LendingError::BorrowOrderFillTimeLimitExceeded);
    }

   
    if amount < borrow_order.remaining_debt_amount {
       
        let fill_value =
            calculate_market_value_from_liquidity_amount(reserve, Fraction::from_num(amount));
        if fill_value < lending_market.min_borrow_order_fill_value {
            msg!(
                "Filled amount {} would have value {}, lower than the configured minimum {}",
                amount,
                fill_value.to_display(),
                lending_market.min_borrow_order_fill_value
            );
            return err!(LendingError::BorrowOrderFillValueTooSmall);
        }
    }

    let borrow_order_initial_state = *borrow_order;

   
    borrow_order.remaining_debt_amount -= amount;

    if borrow_order.remaining_debt_amount == 0 {
        event_emitter.emit(BorrowOrderFullFillEvent {
            before: borrow_order_initial_state,
        })?;
        *borrow_order = BorrowOrder::default();
    } else {
        event_emitter.emit(BorrowOrderPartialFillEvent {
            before: borrow_order_initial_state,
            after: *borrow_order,
        })?;
    }

    Ok(())
}



fn check_borrow_order_creation_enabled(lending_market: &LendingMarket) -> Result<()> {
    if !lending_market.is_borrow_order_creation_enabled() {
        msg!("Creation of new borrow orders is disabled by the market's configuration");
        return err!(LendingError::OrderCreationDisabled);
    }
    Ok(())
}

fn check_borrow_order_execution_enabled(lending_market: &LendingMarket) -> Result<()> {
    if !lending_market.is_borrow_order_execution_enabled() {
        msg!("Execution of borrow orders is disabled by the market's configuration");
        return err!(LendingError::BorrowOrderExecutionDisabled);
    }
    Ok(())
}

fn check_order_config_valid(order_config: &BorrowOrderConfig, timestamp: u64) -> Result<()> {
    let BorrowOrderConfig {
        debt_liquidity_mint: _,
        remaining_debt_amount,
        filled_debt_destination: _,
        max_borrow_rate_bps,
        min_debt_term_seconds: _,
        fillable_until_timestamp,
    } = order_config;

   
    if *max_borrow_rate_bps == 0 {
        msg!("Borrow order must specify max borrow rate");
        return err!(LendingError::InvalidOrderConfiguration);
    }

   
    if *remaining_debt_amount == 0 {
        msg!("Borrow order must request non-0 debt",);
        return err!(LendingError::InvalidOrderConfiguration);
    }

   
    if *fillable_until_timestamp < timestamp {
        msg!(
            "Fillable until timestamp {} cannot be in the past (at {})",
            fillable_until_timestamp,
            timestamp
        );
        return err!(LendingError::InvalidOrderConfiguration);
    }

    Ok(())
}

fn initialize_borrow_order(
    borrow_order: &mut BorrowOrder,
    initial_order_config: BorrowOrderConfig,
    timestamp: u64,
) -> Result<()> {
   
    let BorrowOrderConfig {
        debt_liquidity_mint,
        remaining_debt_amount,
        filled_debt_destination,
        max_borrow_rate_bps,
        min_debt_term_seconds,
        fillable_until_timestamp,
    } = initial_order_config;

   
    *borrow_order = BorrowOrder {
        active: true as u8,
        debt_liquidity_mint,
        remaining_debt_amount,
        filled_debt_destination,
        min_debt_term_seconds,
        fillable_until_timestamp,
        max_borrow_rate_bps,
        placed_at_timestamp: timestamp,
        last_updated_at_timestamp: timestamp,
        requested_debt_amount: remaining_debt_amount,
        padding1: default_array(),
        end_padding: default_array(),
    };

    Ok(())
}

fn update_borrow_order_config(
    borrow_order: &mut BorrowOrder,
    new_order_config: BorrowOrderConfig,
    timestamp: u64,
) -> Result<()> {
   
   
   
    let BorrowOrder {
        active: _,
        debt_liquidity_mint: current_debt_liquidity_mint,
        remaining_debt_amount: current_remaining_debt_amount,
        filled_debt_destination: current_filled_debt_destination,
        max_borrow_rate_bps: current_max_borrow_rate_bps,
        min_debt_term_seconds: current_min_debt_term_seconds,
        fillable_until_timestamp: current_fillable_until_timestamp,
        placed_at_timestamp: _,
        last_updated_at_timestamp,
        requested_debt_amount, 
        padding1: _,           
        end_padding: _,        
    } = borrow_order;

   
    let BorrowOrderConfig {
        debt_liquidity_mint: new_debt_liquidity_mint,
        remaining_debt_amount: new_remaining_debt_amount,
        filled_debt_destination: new_filled_debt_destination,
        max_borrow_rate_bps: new_max_borrow_rate_bps,
        min_debt_term_seconds: new_min_debt_term_seconds,
        fillable_until_timestamp: new_fillable_until_timestamp,
    } = new_order_config;

   
    *last_updated_at_timestamp = timestamp;
    if new_remaining_debt_amount != *current_remaining_debt_amount {
        *requested_debt_amount = new_remaining_debt_amount;
    }

   
    *current_remaining_debt_amount = new_remaining_debt_amount;
    *current_max_borrow_rate_bps = new_max_borrow_rate_bps;
    *current_min_debt_term_seconds = new_min_debt_term_seconds;
    *current_fillable_until_timestamp = new_fillable_until_timestamp;

   
   
    check_not_updated(
        "debt liquidity mint",
        current_debt_liquidity_mint,
        new_debt_liquidity_mint,
    )?;
    check_not_updated(
        "filled debt destination",
        current_filled_debt_destination,
        new_filled_debt_destination,
    )?;

    Ok(())
}

fn check_not_updated<T: PartialEq + Debug>(name: &str, current: &T, new: T) -> Result<()> {
    if new != *current {
        msg!(
            "Cannot update the borrow order's {} (currently {:?}, requested {:?})",
            name,
            current,
            new
        );
        return err!(LendingError::NonUpdatableOrderConfiguration);
    }
    Ok(())
}



fn is_term_satisfied(
    order_requested_seconds: Option<u64>,
    reserve_offered_seconds: Option<u64>,
) -> bool {
    let Some(reserve_offered_seconds) = reserve_offered_seconds else {
        return true;
    };
    let Some(order_requested_seconds) = order_requested_seconds else {
        return false;
    };
    order_requested_seconds <= reserve_offered_seconds
}

