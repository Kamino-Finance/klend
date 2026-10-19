use std::fmt::Debug;

use anchor_lang::{err, Result};
use solana_program::{clock::Clock, pubkey::Pubkey};

use crate::{
    lending_market::utils::calculate_market_value_from_liquidity_amount,
    utils::{accounts::default_array, EventEmitter, Fraction, FractionExtra},
    xmsg, BorrowOrder, BorrowOrderCancelEvent, BorrowOrderConfig, BorrowOrderFullFillEvent,
    BorrowOrderPartialFillEvent, BorrowOrderPlaceEvent, BorrowOrderUpdateEvent,
    FixedTermBorrowRolloverConfig, LendingError, LendingMarket, Obligation, Reserve,
};






pub fn set_borrow_order(
    lending_market: &LendingMarket,
    reserve: &Reserve,
    borrow_order: &mut BorrowOrder,
    order_config: Option<BorrowOrderConfig>,
    clock: &Clock,
    event_emitter: impl EventEmitter,
) -> Result<()> {
   
    let Some(order_config) = order_config else {
        if borrow_order == &Default::default() {
            xmsg!("Ignored a no-op cancellation of the borrow order");
        } else {
            event_emitter.emit(BorrowOrderCancelEvent {
                before: *borrow_order,
            })?;
            *borrow_order = BorrowOrder::default();
        }
        return Ok(());
    };

    let timestamp = clock.unix_timestamp.try_into().expect("negative timestamp");

   
    check_order_config_valid(&order_config, lending_market, reserve, timestamp)?;

   
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

   
    let current_timestamp = clock.unix_timestamp.try_into().expect("negative timestamp");
    borrow_order
        .reserve_constraint()
        .check_satisfying(&reserve.config, current_timestamp)?;

   
    if current_timestamp > borrow_order.fillable_until_timestamp {
        xmsg!(
            "At current timestamp {} it is no longer possible to fill an order fillable until {}",
            current_timestamp,
            borrow_order.fillable_until_timestamp
        );
        return err!(LendingError::BorrowOrderFillTimeLimitExceeded);
    }

   
    let new_remaining_debt_amount = borrow_order.remaining_debt_amount - amount;
    if new_remaining_debt_amount > 0 {
       
        let fill_value =
            calculate_market_value_from_liquidity_amount(reserve, Fraction::from_num(amount));
        if fill_value < lending_market.min_borrow_order_fill_value {
            xmsg!(
                "Filled amount {} would have value {}, lower than the configured minimum {}",
                amount,
                fill_value.to_display(),
                lending_market.min_borrow_order_fill_value
            );
            return err!(LendingError::BorrowOrderFillValueTooSmall);
        }

       
        check_order_remaining_debt_value(new_remaining_debt_amount, lending_market, reserve)?;
    }

    let borrow_order_initial_state = *borrow_order;

   
    borrow_order.remaining_debt_amount = new_remaining_debt_amount;

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









pub fn propagate_rollover_config_to_borrow(
    lending_market: &LendingMarket,
    obligation: &mut Obligation,
    reserve_address: Pubkey,
    rollover_config: FixedTermBorrowRolloverConfig,
    already_borrowed_from_same_reserve: bool,
) -> Result<()> {
    if !lending_market.is_obligation_borrow_rollover_configuration_enabled() {
        xmsg!("Borrow order is supposed to enable auto-rollover on its borrows, but the feature is disabled on market level");
        return err!(LendingError::BorrowRolloverConfigurationDisabled);
    }
    let (borrow, index) = obligation.find_liquidity_in_borrows_mut(reserve_address)?;

    if already_borrowed_from_same_reserve {
        xmsg!(
            "Filled pre-existing borrow's[{}] previous rollover config: {:?}",
            index,
            borrow.fixed_term_borrow_rollover_config
        );
        if !borrow
            .fixed_term_borrow_rollover_config
            .is_compatible_with(&rollover_config)
        {
            xmsg!("Borrow order's rollover config does not match the pre-existing borrow slot's config");
            return err!(LendingError::ObligationBorrowRolloverConfigMismatch);
        }
        return Ok(());
    }

    borrow.fixed_term_borrow_rollover_config = rollover_config;
    xmsg!(
        "Borrow order propagated new rollover config: {:?}",
        borrow.fixed_term_borrow_rollover_config
    );
    Ok(())
}



fn check_borrow_order_creation_enabled(lending_market: &LendingMarket) -> Result<()> {
    if !lending_market.is_borrow_order_creation_enabled() {
        xmsg!("Creation of new borrow orders is disabled by the market's configuration");
        return err!(LendingError::OrderCreationDisabled);
    }
    Ok(())
}

fn check_borrow_order_execution_enabled(lending_market: &LendingMarket) -> Result<()> {
    if !lending_market.is_borrow_order_execution_enabled() {
        xmsg!("Execution of borrow orders is disabled by the market's configuration");
        return err!(LendingError::BorrowOrderExecutionDisabled);
    }
    Ok(())
}

fn check_order_config_valid(
    order_config: &BorrowOrderConfig,
    lending_market: &LendingMarket,
    reserve: &Reserve,
    timestamp: u64,
) -> Result<()> {
    let BorrowOrderConfig {
        debt_liquidity_mint: _,
        remaining_debt_amount,
        filled_debt_destination: _,
        max_borrow_rate_bps,
        min_debt_term_seconds: _,
        fillable_until_timestamp,
        enable_auto_rollover_on_filled_borrows: _,
    } = order_config;

   
    if *max_borrow_rate_bps == 0 {
        xmsg!("Borrow order must specify max borrow rate");
        return err!(LendingError::InvalidOrderConfiguration);
    }

   
    if *remaining_debt_amount == 0 {
        xmsg!("Borrow order must request non-0 debt",);
        return err!(LendingError::InvalidOrderConfiguration);
    }

   
    if *fillable_until_timestamp < timestamp {
        xmsg!(
            "Fillable until timestamp {} cannot be in the past (at {})",
            fillable_until_timestamp,
            timestamp
        );
        return err!(LendingError::InvalidOrderConfiguration);
    }

   
    check_order_remaining_debt_value(*remaining_debt_amount, lending_market, reserve)?;

    Ok(())
}

fn check_order_remaining_debt_value(
    remaining_debt_amount: u64,
    lending_market: &LendingMarket,
    reserve: &Reserve,
) -> Result<()> {
    let order_value = calculate_market_value_from_liquidity_amount(
        reserve,
        Fraction::from_num(remaining_debt_amount),
    );
    if order_value < lending_market.min_borrow_order_fill_value {
        xmsg!(
            "Borrow order's remaining debt {} would have value {}, below the configured minimum {}",
            remaining_debt_amount,
            order_value.to_display(),
            lending_market.min_borrow_order_fill_value
        );
        return err!(LendingError::BorrowOrderValueTooSmall);
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
        enable_auto_rollover_on_filled_borrows,
    } = initial_order_config;

   
    *borrow_order = BorrowOrder {
        active: true as u8,
        debt_liquidity_mint,
        remaining_debt_amount,
        filled_debt_destination,
        min_debt_term_seconds,
        fillable_until_timestamp,
        max_borrow_rate_bps,
        enable_auto_rollover_on_filled_borrows,
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
        enable_auto_rollover_on_filled_borrows: current_enable_auto_rollover_on_filled_borrows,
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
        enable_auto_rollover_on_filled_borrows: new_enable_auto_rollover_on_filled_borrows,
    } = new_order_config;

   
    *last_updated_at_timestamp = timestamp;
    if new_remaining_debt_amount != *current_remaining_debt_amount {
        *requested_debt_amount = new_remaining_debt_amount;
    }

   
    *current_remaining_debt_amount = new_remaining_debt_amount;
    *current_max_borrow_rate_bps = new_max_borrow_rate_bps;
    *current_min_debt_term_seconds = new_min_debt_term_seconds;
    *current_fillable_until_timestamp = new_fillable_until_timestamp;
    *current_enable_auto_rollover_on_filled_borrows = new_enable_auto_rollover_on_filled_borrows;

   
   
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
        xmsg!(
            "Cannot update the borrow order's {} (currently {:?}, requested {:?})",
            name,
            current,
            new
        );
        return err!(LendingError::NonUpdatableOrderConfiguration);
    }
    Ok(())
}

