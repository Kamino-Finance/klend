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





    pub fn remaining_withdrawal_caps_amount(caps: &WithdrawalCaps, timestamp: u64) -> u64 {
        if caps.config_interval_length_seconds == 0 {
            return u64::MAX;
        }
        if caps.config_capacity < 0 {
            return 0;
        }
        let time_spent_within_current_interval_seconds =
            timestamp.saturating_sub(caps.last_interval_start_timestamp);
        let interval_elapsed =
            time_spent_within_current_interval_seconds >= caps.config_interval_length_seconds;
        let current_total = if interval_elapsed {
            0
        } else {
            caps.current_total
        };
        if current_total <= caps.config_capacity {
            let remaining_amount = i128::from(caps.config_capacity) - i128::from(current_total);
            u64::try_from(remaining_amount).expect("difference between i64s always fits in u64")
        } else {
            0
        }
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
            if action == WithdrawalCapAction::Add
                && requested_amount > remaining_withdrawal_caps_amount(caps, curr_timestamp)
            {
                return Err(LendingError::WithdrawalCapReached);
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
