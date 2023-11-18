pub mod utils {
    use std::convert::TryInto;

    #[cfg(target_arch = "bpf")]
    use anchor_lang::prelude::msg;

    use crate::{dbg_msg, LendingError, WithdrawalCaps};

    #[derive(PartialEq)]
    enum WithdrawalCapAction {
        Add,
        Remove,
    }

    enum WithdrawalCapOverflowAction {
        SaturatingOverflow,
        ErrorOnOverflow,
    }

    #[derive(PartialEq, Eq)]
    pub enum WithdrawalCapAccumulatorAction {
        KeepAccumulator,
        ResetAccumulator,
    }

    impl From<WithdrawalCapAccumulatorAction> for bool {
        fn from(a: WithdrawalCapAccumulatorAction) -> bool {
            match a {
                WithdrawalCapAccumulatorAction::KeepAccumulator => false,
                WithdrawalCapAccumulatorAction::ResetAccumulator => true,
            }
        }
    }

    impl From<bool> for WithdrawalCapAccumulatorAction {
        fn from(b: bool) -> WithdrawalCapAccumulatorAction {
            match b {
                true => WithdrawalCapAccumulatorAction::ResetAccumulator,
                false => WithdrawalCapAccumulatorAction::KeepAccumulator,
            }
        }
    }

    fn update_counter(
        caps: &mut WithdrawalCaps,
        requested_amount: u64,
        action: WithdrawalCapAction,
        overflow_action: WithdrawalCapOverflowAction,
    ) -> Result<(), LendingError> {
        match action {
            WithdrawalCapAction::Add => match overflow_action {
                WithdrawalCapOverflowAction::SaturatingOverflow => {
                    caps.current_total = caps.current_total.saturating_add(
                        requested_amount
                            .try_into()
                            .map_err(|_| dbg_msg!(LendingError::MathOverflow))?,
                    );
                    Ok(())
                }
                WithdrawalCapOverflowAction::ErrorOnOverflow => {
                    caps.current_total = caps
                        .current_total
                        .checked_add(
                            requested_amount
                                .try_into()
                                .map_err(|_| dbg_msg!(LendingError::IntegerOverflow))?,
                        )
                        .ok_or_else(|| {
                            dbg_msg!("MathOverflow adding {:?} and {:?}", *caps, requested_amount);
                            LendingError::MathOverflow
                        })?;
                    Ok(())
                }
            },
            WithdrawalCapAction::Remove => match overflow_action {
                WithdrawalCapOverflowAction::SaturatingOverflow => {
                    caps.current_total = caps.current_total.saturating_sub(
                        requested_amount
                            .try_into()
                            .map_err(|_| dbg_msg!(LendingError::MathOverflow))?,
                    );
                    Ok(())
                }
                WithdrawalCapOverflowAction::ErrorOnOverflow => {
                    caps.current_total = caps
                        .current_total
                        .checked_sub(
                            requested_amount
                                .try_into()
                                .map_err(|_| dbg_msg!(LendingError::IntegerOverflow))?,
                        )
                        .ok_or_else(|| {
                            dbg_msg!(
                                "MathOverflow subbing {:?} and {:?}",
                                *caps,
                                requested_amount
                            );
                            LendingError::MathOverflow
                        })?;
                    Ok(())
                }
            },
        }
    }

    fn check_capacity_allows_withdrawals(
        caps: &mut WithdrawalCaps,
        requested_amount: u64,
    ) -> Result<(), LendingError> {
        if caps.config_capacity < 0 {
            return Err(LendingError::WithdrawalCapReached);
        }
        if caps
            .current_total
            .checked_add(
                requested_amount
                    .try_into()
                    .map_err(|_| dbg_msg!(LendingError::MathOverflow))?,
            )
            .ok_or_else(|| dbg_msg!(LendingError::MathOverflow))?
            > caps.config_capacity
        {
            return Err(LendingError::WithdrawalCapReached);
        }
        Ok(())
    }

    fn check_last_interval_elapsed(
        caps: &mut WithdrawalCaps,
        curr_timestamp: u64,
    ) -> Result<bool, LendingError> {
        if caps.last_interval_start_timestamp > curr_timestamp {
            return Err(LendingError::LastTimestampGreaterThanCurrent);
        }
        Ok(caps.config_interval_length_seconds
            <= curr_timestamp.saturating_sub(caps.last_interval_start_timestamp))
    }

    fn reset_current_interval_and_counter(caps: &mut WithdrawalCaps, curr_timestamp: u64) {
        caps.current_total = 0;
        caps.last_interval_start_timestamp = curr_timestamp;
    }

    pub fn add_to_withdrawal_accum(
        caps: &mut WithdrawalCaps,
        requested_amount: u64,
        curr_timestamp: u64,
    ) -> Result<(), LendingError> {
        check_and_update_withdrawal_caps(
            caps,
            requested_amount,
            curr_timestamp,
            WithdrawalCapAction::Add,
        )
    }

    pub fn sub_from_withdrawal_accum(
        caps: &mut WithdrawalCaps,
        requested_amount: u64,
        curr_timestamp: u64,
    ) -> Result<(), LendingError> {
        check_and_update_withdrawal_caps(
            caps,
            requested_amount,
            curr_timestamp,
            WithdrawalCapAction::Remove,
        )
    }

    fn check_and_update_withdrawal_caps(
        caps: &mut WithdrawalCaps,
        requested_amount: u64,
        curr_timestamp: u64,
        action: WithdrawalCapAction,
    ) -> Result<(), LendingError> {
        if caps.config_interval_length_seconds != 0 {
            if check_last_interval_elapsed(caps, curr_timestamp)? {
                reset_current_interval_and_counter(caps, curr_timestamp);
            }
            if action == WithdrawalCapAction::Add {
                check_capacity_allows_withdrawals(caps, requested_amount)?;
            }
            update_counter(
                caps,
                requested_amount,
                action,
                WithdrawalCapOverflowAction::ErrorOnOverflow,
            )
        } else {
            update_counter(
                caps,
                requested_amount,
                action,
                WithdrawalCapOverflowAction::SaturatingOverflow,
            )
        }
    }
}
