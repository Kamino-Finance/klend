#[macro_export]
macro_rules! gen_signer_seeds {
    (
    $key: expr, $bump: expr
) => {
        &[
            $crate::utils::seeds::LENDING_MARKET_AUTH as &[u8],
            $key.as_ref(),
            &[$bump],
        ]
    };
}

#[macro_export]
macro_rules! try_block {
    ($($expr:expr)*) => {
        match $($expr)* {
            Ok(val) => val,
            Err(err) => {
                use $crate::LendingError;
                use ::anchor_lang::error;
                return $($expr)*.map_err(|_| error!(LendingError::MathOverflow));
            }
        }
    };
}

#[macro_export]
macro_rules! check_cpi {
    ($ctx:expr) => {{
        $crate::utils::check_cpi_call(&$ctx.accounts.instruction_sysvar_account)?;
    }};
}

#[macro_export]
macro_rules! check_refresh_ixs {
    ($ctx_accounts:expr, $reserve:expr, $mode:expr) => {{
        let _reserve = $reserve.load()?;
        $crate::utils::check_refresh(
            &$ctx_accounts.instruction_sysvar_account,
            &[($reserve.key(), &_reserve)],
            &$ctx_accounts.obligation.to_account_info().key(),
            &[$mode],
        )?;
    }};
    ($ctx_accounts:expr, $reserve_one:expr, $reserve_two:expr, $mode_one:expr, $mode_two:expr) => {{
        let _reserve_one = $reserve_one.load()?;
        let _reserve_two = $reserve_two.load()?;

        if $reserve_one.key() == $reserve_two.key() {
            $crate::utils::check_refresh(
                &$ctx_accounts.instruction_sysvar_account,
                &[
                    ($reserve_one.key(), &_reserve_one),
                    ($reserve_one.key(), &_reserve_one),
                ],
                &$ctx_accounts.obligation.to_account_info().key(),
                &[$mode_one, $mode_two],
            )?;
        } else {
            $crate::utils::check_refresh(
                &$ctx_accounts.instruction_sysvar_account,
                &[
                    ($reserve_one.key(), &_reserve_one),
                    ($reserve_two.key(), &_reserve_two),
                ],
                &$ctx_accounts.obligation.to_account_info().key(),
                &[$mode_one, $mode_two],
            )?;
        }
    }};
}

#[cfg(target_arch = "bpf")]
#[macro_export]
macro_rules! dbg_msg {
                () => {
        msg!("[{}:{}]", file!(), line!())
    };
    ($val:expr $(,)?) => {
                      match $val {
            tmp => {
                msg!("[{}:{}] {} = {:#?}",
                    file!(), line!(), stringify!($val), &tmp);
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg_msg!($val)),+,)
    };
}

#[cfg(not(target_arch = "bpf"))]
#[macro_export]
macro_rules! dbg_msg {
                () => {
        println!("[{}:{}]", file!(), line!())
    };
    ($val:expr $(,)?) => {
                      match $val {
            tmp => {
                println!("[{}:{}] {} = {:#?}",
                    file!(), line!(), stringify!($val), &tmp);
                tmp
            }
        }
    };
    ($($val:expr),+ $(,)?) => {
        ($($crate::dbg_msg!($val)),+,)
    };
}

#[cfg(target_arch = "bpf")]
#[macro_export]
macro_rules! xmsg {
    ($($arg:tt)*) => (::anchor_lang::prelude::msg!($($arg)*));
}

#[cfg(all(not(target_arch = "bpf"), not(feature = "tracing")))]
#[macro_export]
macro_rules! xmsg {
    ($($arg:tt)*) => (println!($($arg)*));
}

#[cfg(all(not(target_arch = "bpf"), feature = "tracing"))]
#[macro_export]
macro_rules! xmsg {
    ($($arg:tt)*) => (tracing::info!($($arg)*));
}

#[cfg(not(target_arch = "bpf"))]
#[macro_export]
macro_rules! cu_log {
    () => {};
}

#[cfg(target_arch = "bpf")]
#[macro_export]
macro_rules! cu_log {
    () => {
        ::anchor_lang::solana_program::log::sol_log(concat!("CU at: ", file!(), ":", line!()));
        ::anchor_lang::solana_program::log::sol_log_compute_units();
    };
}

#[macro_export]
macro_rules! assert_fuzzy_eq_f {
    ($actual:expr, $expected:expr, $epsilon:expr) => {
        let eps = $epsilon as Fraction;
        let act = $actual as Fraction;
        let exp = $expected as Fraction;
        let diff = if act > exp { act - exp } else { exp - act };
        if diff > eps {
            panic!(
                "Actual {} Expected {} diff {} Epsilon {}",
                $actual, $expected, diff, eps
            );
        }
    };
}

#[macro_export]
macro_rules! assert_fuzzy_eq {
    ($actual:expr, $expected:expr, $epsilon:expr) => {
        let eps = $epsilon as i128;
        let act = $actual as i128;
        let exp = $expected as i128;
        let diff = (act - exp).abs();
        if diff > eps {
            panic!(
                "Actual {} Expected {} diff {} Epsilon {}",
                $actual, $expected, diff, eps
            );
        }
    };

    ($actual:expr, $expected:expr, $epsilon:expr, $type:ty) => {
        let eps = $epsilon as $type;
        let act = $actual as $type;
        let exp = $expected as $type;
        let diff = if act > exp { act - exp } else { exp - act };
        if diff > eps {
            panic!(
                "Actual {} Expected {} diff {} Epsilon {}",
                $actual, $expected, diff, eps
            );
        }
    };
}

#[macro_export]
macro_rules! assert_fuzzy_eq_percentage {
    ($actual:expr, $expected:expr, $percentage:expr) => {
        let act = $actual as i128;
        let exp = $expected as i128;
        let percentage = $percentage as f64;
        let diff = (act - exp).abs();
        let diff_percentage = match exp {
            0 => f64::MAX,            _ => (100.0 * diff as f64) / (exp as f64),
        };
        if diff > 0 && diff_percentage > percentage {
            panic!("Actual {} Expected {} diff {} and percentage_diff > percentage ({}% > {}%)",
            $actual, $expected, diff, diff_percentage, percentage
        );
    }
    };
    ($actual:expr, $expected:expr, $percentage:expr, $testcase:expr) => {
        let act = $actual as i128;
        let exp = $expected as i128;
        let percentage = $percentage as f64;
        let diff = (act - exp).abs();
        let diff_percentage = match exp {
            0 => f64::MAX,            _ => (100.0 * diff as f64) / (exp as f64),
        };
        if diff > 0 && diff_percentage > percentage {
            panic!("Actual {} Expected {} diff {} and percentage_diff > percentage ({}% > {}%) testcase: {}",
            $actual, $expected, diff, diff_percentage, percentage, $testcase
        );
    }
    };
}

#[macro_export]
macro_rules! assert_almost_eq_fraction {
    ($left:expr, $right:expr $(,)?) => {
        $crate::assert_almost_eq_fraction!($left, $right, 0.0001);
    };
    ($left:expr, $right:expr, $epsilon_rate:expr $(,)?) => {
        let left_val: Fraction = $left;
        let right_val: Fraction = $right;
        let scaler: f64 = $epsilon_rate + 1.0;

        let left_val_upper = left_val * $crate::utils::fraction::Fraction::from_num(scaler);
        let right_val_upper = right_val * $crate::utils::fraction::Fraction::from_num(scaler);

        if left_val_upper < right_val || right_val_upper < left_val {
            panic!(
                "assertion failed: `(left ~= right)` \
                 \n  left: `{}`,\
                 \n right: `{}`\n",
                left_val, right_val
            );
        }
    };
    ($left:expr, $right:expr, $epsilon:expr, $($arg:tt)+) => {
        let left_val: Fraction = $left;
        let right_val: Fraction = $right;
        let scaler: f64 = $epsilon_rate + 1.0;

        let left_val_upper = left_val * $crate::utils::fraction::Fraction::from_num(scaler);
        let right_val_upper = right_val * $crate::utils::fraction::Fraction::from_num(scaler);

        if left_val_upper < right_val || right_val_upper < left_val {
            panic!(
                "assertion failed: `(left ~= right)` \
                 \n  left: `{}`,\
                 \n right: `{}`,\
                 \n reason: `{}`\n",
                left_val, right_val, std::fmt::format(format_args!($($arg)+))
            );
        }
    };
}

#[macro_export]
macro_rules! assert_gt {
    ($left:expr, $right:expr) => {
        if !($left > $right) {
            panic!(
                "Assertion failed: {:?} is not greater than {:?}",
                $left, $right
            );
        }
    };
}

#[macro_export]
macro_rules! assert_gte {
    ($left:expr, $right:expr) => {
        if !($left >= $right) {
            panic!(
                "Assertion failed: {:?} is not greater than {:?}",
                $left, $right
            );
        }
    };
}

#[macro_export]
macro_rules! prop_assert_gt {
    ($left:expr, $right:expr) => {
        let act = $left;
        let exp = $right;
        ::proptest::prop_assert!(
            act > exp,
            "assertion failed: `(Left > Right)` \
             \n   Left: `{:?}`,\
             \n Right: `{:?}`",
            act,
            exp,
        );
    };
}

#[macro_export]
macro_rules! prop_assert_gte {
    ($left:expr, $right:expr) => {
        let act = $left;
        let exp = $right;
        ::proptest::prop_assert!(
            act >= exp,
            "assertion failed: `(Left >= Right)` \
             \n   Left: `{:?}`,\
             \n Right: `{:?}`",
            act,
            exp,
        );
    };
}

#[macro_export]
macro_rules! prop_assert_eq {
    ($left:expr, $right:expr) => {
        let l = $left;
        let r = $right;
        ::proptest::prop_assert!(
            l == r,
            "assertion failed: `(Left == Right)` \
             \n   Left: `{:?}`,\
             \n Right: `{:?}`",
            l,
            r,
        );
    };
}

#[macro_export]
macro_rules! prop_assert_fuzzy_eq {
    ($actual:expr, $expected:expr, $epsilon:expr) => {
        let eps = $epsilon as i128;
        let act = $actual as i128;
        let exp = $expected as i128;
        let diff = (act - exp).abs();
        ::proptest::prop_assert!(
            diff <= eps,
            "assertion failed: `(Actual == Expected)` \
             \n   Actual: `{:?}`,\
             \n Expected: `{:?}`,\
             \n    Diff: `{:?}`,\
             \n Epsilon: `{:?}`\n",
            act,
            exp,
            diff,
            eps
        );
    };

    ($actual:expr, $expected:expr, $epsilon:expr, $type:ty) => {
        let eps = $epsilon as $type;
        let act = $actual as $type;
        let exp = $expected as $type;
        let diff = if act > exp { act - exp } else { exp - act };
        ::proptest::prop_assert!(
            diff <= eps,
            "assertion failed: `(Actual == Expected)` \
             \n   Actual: `{:?}`,\
             \n Expected: `{:?}`,\
             \n    Diff: `{:?}`,\
             \n Epsilon: `{:?}`\n",
            act,
            exp,
            diff,
            eps
        );
    };
}

#[macro_export]
macro_rules! prop_assert_fuzzy_eq_percentage {
    ($actual: expr, $expected: expr, $epsilon: expr, $percentage: expr) => {
        let act = $actual as i128;
        let exp = $expected as i128;
        let eps = $epsilon as i128;
        let percentage = $percentage as f64;
        let diff = (act - exp).abs();
        let diff_percentage = match exp {
            0 => f64::MAX,
            _ => (100.0 * diff as f64) / (exp as f64),
        };
        ::proptest::prop_assert!(
            !(diff > eps && diff_percentage > percentage),
            "Actual {} Expected {} diff {} and percentage_diff > percentage ({}% > {}%)",
            $actual,
            $expected,
            diff,
            diff_percentage,
            percentage
        );
    };
}

#[macro_export]
macro_rules! prop_assert_fuzzy_bps_diff {
    ($actual: expr, $expected: expr, $bps_diff: expr) => {
        let act = $actual as f64;
        let exp = $expected as f64;
        let bps_diff = $bps_diff as f64;
        ::proptest::prop_assert!(act * 10000.0 <= exp * (10000.0 + bps_diff) && act * 10000.0 >= exp * (10000.0 - bps_diff),
            "{actual_str} = {actual_value} is more than {bps_value} bps away from {expected_str} = {expected_value}",
            actual_str = stringify!($actual),
            actual_value = $actual,
            expected_str = stringify!($expected),
            expected_value = $expected,
            bps_value = $bps_diff
        );
    };
}

#[macro_export]
macro_rules! prop_assert_fuzzy_eq_fraction {
    ($left:expr, $right:expr $(,)?) => {
        $crate::prop_assert_fuzzy_eq_fraction!($left, $right, 0.0001);
    };
    ($left:expr, $right:expr, $epsilon_rate:expr $(,)?) => {
        let left_val: Fraction = $left;
        let right_val: Fraction = $right;
        let diff = left_val.abs_diff(right_val);
        let epsilon_rate: Fraction = $crate::fraction::fraction!($epsilon_rate);
        let epsilon_max_diff_right = right_val * epsilon_rate;
        let epsilon_max_diff_left = left_val * epsilon_rate;

        ::proptest::prop_assert!(diff <= epsilon_max_diff_right && diff <= epsilon_max_diff_left,
            "assertion failed: `(left ~= right)` \
                \n  left: `{}`,\
                \n right: `{}`,\
                \n eps  : `{}`\n",
            left_val, right_val, epsilon_rate
        );
    };
    ($left:expr, $right:expr, $epsilon_rate:expr, $($arg:tt)+) => {
        let left_val: Fraction = $left;
        let right_val: Fraction = $right;
        let diff = left_val.abs_diff(right_val);
        let epsilon_rate: Fraction = $crate::fraction::fraction!($epsilon_rate);
        let epsilon_max_diff_right = right_val * epsilon_rate;
        let epsilon_max_diff_left = left_val * epsilon_rate;

        ::proptest::prop_assert!(diff <= epsilon_max_diff_right && diff <= epsilon_max_diff_left,
            "assertion failed: `(left ~= right)` \
                \n  left: `{}`,\
                \n right: `{}`,\
                \n eps  : `{}`,\
                \n reason: `{}`\n",
            left_val, right_val, epsilon_rate, std::fmt::format(format_args!($($arg)+))
        );
    };
}

#[macro_export]
macro_rules! prop_fail {
    ($($fmt:tt)*) => {{
        let message = format!($($fmt)*);
        let message = format!("{} at {}:{}", message, file!(), line!());
        return ::core::result::Result::Err(
            ::proptest::test_runner::TestCaseError::fail(message));
    }};
}

#[macro_export]
macro_rules! prop_try {
    ($e:expr) => {
        match $e {
            ::core::result::Result::Ok(val) => val,
            ::core::result::Result::Err(err) => {
                let message = format!("{:?} at {}:{}", err, file!(), line!());
                return ::core::result::Result::Err(::proptest::test_runner::TestCaseError::fail(
                    message,
                ));
            }
        }
    };
}
