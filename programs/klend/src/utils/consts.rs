use anchor_lang::solana_program;

use crate::utils::fraction::{fraction, Fraction};

pub const SLOTS_PER_SECOND: u64 = 2;
pub const SLOTS_PER_MINUTE: u64 = SLOTS_PER_SECOND * 60;
pub const SLOTS_PER_HOUR: u64 = SLOTS_PER_MINUTE * 60;
pub const SLOTS_PER_DAY: u64 = SLOTS_PER_HOUR * 24;
pub const SLOTS_PER_YEAR: u64 = SLOTS_PER_DAY * 365;

pub const PROGRAM_VERSION: u8 = 1;

pub const FULL_BPS: u16 = 10_000;

pub const UNINITIALIZED_VERSION: u8 = 0;

pub const INITIAL_COLLATERAL_RATIO: u64 = 1;
pub const INITIAL_COLLATERAL_RATE: Fraction = fraction!(1);

pub const LIQUIDATION_CLOSE_FACTOR: u8 = 20;

pub const LIQUIDATION_CLOSE_AMOUNT: u64 = 2;

pub const MAX_LIQUIDATABLE_VALUE_AT_ONCE: u64 = 500_000;

pub const MIN_AUTODELEVERAGE_BONUS_BPS: u64 = 50;

pub const MAX_OBLIGATION_RESERVES: u64 = 20;

pub const CLOSE_TO_INSOLVENCY_RISKY_LTV: u8 = 95;

pub const NULL_PUBKEY: solana_program::pubkey::Pubkey =
    solana_program::pubkey::Pubkey::new_from_array([
        11, 193, 238, 216, 208, 116, 241, 195, 55, 212, 76, 22, 75, 202, 40, 216, 76, 206, 27, 169,
        138, 64, 177, 28, 19, 90, 156, 0, 0, 0, 0, 0,
    ]);

pub const LENDING_MARKET_SIZE: usize = 4656;
pub const RESERVE_SIZE: usize = 8616;
pub const OBLIGATION_SIZE: usize = 3336;
pub const RESERVE_CONFIG_SIZE: usize = 648;
pub const REFERRER_TOKEN_STATE_SIZE: usize = 352;
pub const USER_METADATA_SIZE: usize = 1024;
pub const REFERRER_STATE_SIZE: usize = 32;
pub const SHORT_URL_SIZE: usize = 68;
pub const GLOBAL_UNHEALTHY_BORROW_VALUE: u64 = 50_000_000;

pub const GLOBAL_ALLOWED_BORROW_VALUE: u64 = 45_000_000;

pub const DEFAULT_BORROW_FACTOR_PCT: u64 = 100;

pub const ELEVATION_GROUP_NONE: u8 = 0;

pub const MAX_NUM_ELEVATION_GROUPS: u8 = 10;
