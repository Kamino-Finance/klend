use anchor_lang::solana_program;
use solana_program::{pubkey, pubkey::Pubkey};

use crate::{
    utils::fraction::{fraction, Fraction},
    CollateralExchangeRate,
};

pub const SLOTS_PER_SECOND: u64 = 2;
pub const SLOTS_PER_MINUTE: u64 = SLOTS_PER_SECOND * 60;
pub const SLOTS_PER_HOUR: u64 = SLOTS_PER_MINUTE * 60;
pub const SLOTS_PER_DAY: u64 = SLOTS_PER_HOUR * 24;
pub const SLOTS_PER_YEAR: u64 = SLOTS_PER_DAY * 365;

pub const SECONDS_PER_MINUTE: u64 = 60;
pub const SECONDS_PER_HOUR: u64 = SECONDS_PER_MINUTE * 60;
pub const SECONDS_PER_DAY: u64 = SECONDS_PER_HOUR * 24;

pub const PROGRAM_VERSION: u8 = 1;

pub const FULL_BPS: u16 = 10_000;

pub const UNINITIALIZED_VERSION: u8 = 0;

pub const INITIAL_COLLATERAL_RATIO: u64 = 1;
pub const INITIAL_COLLATERAL_RATE: CollateralExchangeRate = CollateralExchangeRate::ONE;

pub const LIQUIDATION_CLOSE_FACTOR: u8 = 20;

pub const LIQUIDATION_CLOSE_VALUE: u64 = 2;

pub const MAX_LIQUIDATABLE_VALUE_AT_ONCE: u64 = 500_000;

pub const MIN_AUTODELEVERAGE_BONUS_BPS: u64 = 50;

pub const NO_DELEVERAGING_MARKER: u8 = u8::MAX;

pub const MAX_OBLIGATION_RESERVES: u64 = 20;

pub const CLOSE_TO_INSOLVENCY_RISKY_LTV: u8 = 95;

pub const MIN_INITIAL_DEPOSIT_AMOUNT: u64 = 1000;

pub const DEFAULT_MIN_DEPOSIT_AMOUNT: u64 = 100_000;

pub const NULL_PUBKEY: solana_program::pubkey::Pubkey =
    solana_program::pubkey::Pubkey::new_from_array([
        11, 193, 238, 216, 208, 116, 241, 195, 55, 212, 76, 22, 75, 202, 40, 216, 76, 206, 27, 169,
        138, 64, 177, 28, 19, 90, 156, 0, 0, 0, 0, 0,
    ]);

pub const LENDING_MARKET_SIZE: usize = 4656;
pub const RESERVE_SIZE: usize = 8616;
pub const OBLIGATION_SIZE: usize = 3336;
pub const RESERVE_CONFIG_SIZE: usize = 920;
pub const REFERRER_TOKEN_STATE_SIZE: usize = 352;
pub const USER_METADATA_SIZE: usize = 1024;
pub const REFERRER_STATE_SIZE: usize = 64;
pub const SHORT_URL_SIZE: usize = 68;
pub const TOKEN_INFO_SIZE: usize = 384;
pub const GLOBAL_CONFIG_SIZE: usize = 1024;

pub const GLOBAL_ALLOWED_BORROW_VALUE: u64 = 45_000_000;

pub const DEFAULT_BORROW_FACTOR_PCT: u64 = 100;

pub const ELEVATION_GROUP_NONE: u8 = 0;

pub const MAX_NUM_ELEVATION_GROUPS: u8 = 32;

pub const USD_DECIMALS: u32 = 6;

pub const MIN_NET_VALUE_IN_OBLIGATION: Fraction = fraction!(0.000001);

pub const DUST_LAMPORT_THRESHOLD: u64 = 1;

pub const MAX_PRICE_DECIMALS_U256: u32 = 36;

pub const TARGET_PRICE_DECIMALS: u32 = MAX_PRICE_DECIMALS_U256 / 2;

pub fn ten_pow(x: usize) -> u64 {
    const POWERS_OF_TEN: [u64; 20] = [
        1,
        10,
        100,
        1_000,
        10_000,
        100_000,
        1_000_000,
        10_000_000,
        100_000_000,
        1_000_000_000,
        10_000_000_000,
        100_000_000_000,
        1_000_000_000_000,
        10_000_000_000_000,
        100_000_000_000_000,
        1_000_000_000_000_000,
        10_000_000_000_000_000,
        100_000_000_000_000_000,
        1_000_000_000_000_000_000,
        10_000_000_000_000_000_000,
    ];

    if x > 19 {
        panic!("The exponent must be between 0 and 19.");
    }

    POWERS_OF_TEN[x]
}

pub const SQUADS_PROGRAM_ID_V3_MAINNET_PROD: Pubkey =
    pubkey!("SMPLecH534NA9acpos4G6x7uf3LWbCAwZQE9e8ZekMu");

pub const SQUADS_PROGRAM_ID_V3_MAINNET_DEV: Pubkey =
    pubkey!("84Ue9gKQUsStFJQCNQpsqvbceo7fKYSSCCMXxMZ5PkiW");

pub const SQUADS_PROGRAM_ID_V4_MAINNET_PROD: Pubkey =
    pubkey!("SQDS4ep65T869zMMBKyuUq6aD6EgTu8psMjkvj52pCf");

pub const SQUADS_PROGRAM_ID_V4_MAINNET_DEV: Pubkey =
    pubkey!("STAG3xkFMyVK3sRtQhipsKuLpRGbgospDpVdNyJqDpS");

pub const FLEX_LEND_ID_MAINNET_PROD: Pubkey =
    pubkey!("FL3X2pRsQ9zHENpZSKDRREtccwJuei8yg9fwDu9UN69Q");

pub const KAMINO_VAULT_STAGING: Pubkey = pubkey!("STkvh7ostar39Fwr4uZKASs1RNNuYMFMTsE77FiRsL2");

pub const KAMINO_VAULT_MAINNET: Pubkey = pubkey!("kvauTFR8qm1dhniz6pYuBZkuene3Hfrs1VQhVRgCNrr");

pub const DEFI_CARROT_ID_MAINNET: Pubkey = pubkey!("CarrotwivhMpDnm27EHmRLeQ683Z1PufuqEmBZvD282s");

pub const METEORA_DYNAMIC_POOL_ID_MAINNET: Pubkey =
    pubkey!("24Uqj9JCLxUeoC3hGfh5W3s9FM9uCHDS2SG3LYwBpyTi");

pub const SANDGLASS_ID_MAINNET: Pubkey = pubkey!("SANDsy8SBzwUE8Zio2mrYZYqL52Phr2WQb9DDKuXMVK");

pub const BESTLEND_ID_MAINNET: Pubkey = pubkey!("bestdGyQeo7mgaSRNgEYdtjhsryNbP8jgg1Y9qoFbk7");

pub const DIVVY_ID_MAINNET: Pubkey = pubkey!("dvyFwAPniptQNb1ey4eM12L8iLHrzdiDsPPDndd6xAR");

pub const EXPONENT_INTEGRATION_ID_MAINNET: Pubkey =
    pubkey!("XPK1ndTK1xrgRg99ifvdPP1exrx8D1mRXTuxBkkroCx");

pub const EXPONENT_CORE_ID_MAINNET: Pubkey =
    pubkey!("ExponentnaRg3CQbW6dqQNZKXp7gtZ9DGMp1cwC4HAS7");

pub const AGRO_ID_MAINNET: Pubkey = pubkey!("AgroFiE3bX7j4Tvfa7YAoFLqjjb35Bw6eed5BuYukPEn");
pub const AGRO_STAGING_ID_MAINNET: Pubkey = pubkey!("E7jPY6J5s2uAxAjJQX5tqoASkmFr6TYxVoMm97hPLNZ1");

pub const CPI_WHITELISTED_ACCOUNTS: [CpiWhitelistedAccount; 16] = [
    CpiWhitelistedAccount::new(FLEX_LEND_ID_MAINNET_PROD, 1),
    CpiWhitelistedAccount::new(SQUADS_PROGRAM_ID_V3_MAINNET_PROD, 1),
    CpiWhitelistedAccount::new(SQUADS_PROGRAM_ID_V3_MAINNET_DEV, 1),
    CpiWhitelistedAccount::new(SQUADS_PROGRAM_ID_V4_MAINNET_PROD, 1),
    CpiWhitelistedAccount::new(SQUADS_PROGRAM_ID_V4_MAINNET_DEV, 1),
    CpiWhitelistedAccount::new(METEORA_DYNAMIC_POOL_ID_MAINNET, 1),
    CpiWhitelistedAccount::new(DEFI_CARROT_ID_MAINNET, 1),
    CpiWhitelistedAccount::new(SANDGLASS_ID_MAINNET, 1),
    CpiWhitelistedAccount::new(BESTLEND_ID_MAINNET, 1),
    CpiWhitelistedAccount::new(KAMINO_VAULT_STAGING, 1),
    CpiWhitelistedAccount::new(KAMINO_VAULT_MAINNET, 1),
    CpiWhitelistedAccount::new(DIVVY_ID_MAINNET, 1),
    CpiWhitelistedAccount::new(EXPONENT_INTEGRATION_ID_MAINNET, 2),
    CpiWhitelistedAccount::new(EXPONENT_CORE_ID_MAINNET, 3),
    CpiWhitelistedAccount::new(AGRO_ID_MAINNET, 1),
    CpiWhitelistedAccount::new(AGRO_STAGING_ID_MAINNET, 1),
];

pub struct CpiWhitelistedAccount {
    pub program_id: Pubkey,
    pub whitelist_level: usize,
}

impl CpiWhitelistedAccount {
    pub const fn new(program_id: Pubkey, whitelist_level: usize) -> CpiWhitelistedAccount {
        CpiWhitelistedAccount {
            program_id,
            whitelist_level,
        }
    }
}
