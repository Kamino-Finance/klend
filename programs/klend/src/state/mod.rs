pub mod last_update;
pub mod lending_market;
pub mod liquidation_operations;
pub mod nested_accounts;
pub mod obligation;
pub mod referral;
pub mod reserve;
pub mod token_info;
pub mod types;

use anchor_lang::prelude::*;
pub use last_update::*;
pub use lending_market::*;
pub use nested_accounts::*;
use num_enum::TryFromPrimitive;
pub use obligation::*;
pub use referral::*;
pub use reserve::*;
#[cfg(feature = "serde")]
use strum::EnumIter;
use strum::EnumString;
pub use token_info::*;
pub use types::*;

use crate::utils::{borrow_rate_curve::BorrowRateCurve, RESERVE_CONFIG_SIZE};

pub const VALUE_BYTE_ARRAY_LEN_RESERVE: usize = RESERVE_CONFIG_SIZE;
pub const VALUE_BYTE_ARRAY_LEN_SHORT_UPDATE: usize = 32;

pub const VALUE_BYTE_MAX_ARRAY_LEN_MARKET_UPDATE: usize = 72;
#[derive(PartialEq, Eq, Clone, Debug)]
pub enum UpdateReserveConfigValue {
    Bool(bool),
    U8(u8),
    U8Tuple(u8, u8),
    U16(u16),
    U64(u64),
    Pubkey(Pubkey),
    ScopeChain([u16; 4]),
    Name([u8; 32]),
    BorrowRateCurve(BorrowRateCurve),
    Full(Box<ReserveConfig>),
    WithdrawalCap(u64, u64),
    ElevationGroups([u8; 20]),
    ElevationGroupBorrowLimits([u64; 32]),
}

impl UpdateReserveConfigValue {
    pub fn to_raw_bytes(&self) -> Vec<u8> {
        match self {
            UpdateReserveConfigValue::Bool(v) => {
                vec![*v as u8]
            }
            UpdateReserveConfigValue::U8(v) => {
                vec![*v]
            }
            UpdateReserveConfigValue::U16(v) => v.to_le_bytes().to_vec(),
            UpdateReserveConfigValue::U64(v) => v.to_le_bytes().to_vec(),
            UpdateReserveConfigValue::Pubkey(v) => v.as_ref().to_vec(),
            UpdateReserveConfigValue::ScopeChain(chain) => chain.map(|x| x.to_le_bytes()).concat(),
            UpdateReserveConfigValue::Name(v) => v.to_vec(),
            UpdateReserveConfigValue::Full(config) => config.try_to_vec().unwrap(),
            UpdateReserveConfigValue::BorrowRateCurve(curve) => curve.try_to_vec().unwrap(),
            UpdateReserveConfigValue::WithdrawalCap(cap, interval) => {
                (*cap, *interval).try_to_vec().unwrap()
            }
            UpdateReserveConfigValue::ElevationGroups(groups) => groups.to_vec(),
            UpdateReserveConfigValue::U8Tuple(mode, value) => (*mode, *value).try_to_vec().unwrap(),
            UpdateReserveConfigValue::ElevationGroupBorrowLimits(e) => e.try_to_vec().unwrap(),
        }
    }
}

#[derive(
    AnchorSerialize,
    AnchorDeserialize,
    TryFromPrimitive,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Debug,
    EnumString,
)]
#[cfg_attr(feature = "serde", derive(EnumIter))]
#[repr(u64)]
pub enum UpdateConfigMode {
    UpdateLoanToValuePct = 1,
    UpdateMaxLiquidationBonusBps = 2,
    UpdateLiquidationThresholdPct = 3,
    UpdateProtocolLiquidationFee = 4,
    UpdateProtocolTakeRate = 5,
    UpdateFeesBorrowFee = 6,
    UpdateFeesFlashLoanFee = 7,
    UpdateFeesReferralFeeBps = 8,
    UpdateDepositLimit = 9,
    UpdateBorrowLimit = 10,
    UpdateTokenInfoLowerHeuristic = 11,
    UpdateTokenInfoUpperHeuristic = 12,
    UpdateTokenInfoExpHeuristic = 13,
    UpdateTokenInfoTwapDivergence = 14,
    UpdateTokenInfoScopeTwap = 15,
    UpdateTokenInfoScopeChain = 16,
    UpdateTokenInfoName = 17,
    UpdateTokenInfoPriceMaxAge = 18,
    UpdateTokenInfoTwapMaxAge = 19,
    UpdateScopePriceFeed = 20,
    UpdatePythPrice = 21,
    UpdateSwitchboardFeed = 22,
    UpdateSwitchboardTwapFeed = 23,
    UpdateBorrowRateCurve = 24,
    UpdateEntireReserveConfig = 25,
    UpdateDebtWithdrawalCap = 26,
    UpdateDepositWithdrawalCap = 27,
    UpdateDebtWithdrawalCapCurrentTotal = 28,
    UpdateDepositWithdrawalCapCurrentTotal = 29,
    UpdateBadDebtLiquidationBonusBps = 30,
    UpdateMinLiquidationBonusBps = 31,
    DeleveragingMarginCallPeriod = 32,
    UpdateBorrowFactor = 33,
    UpdateAssetTier = 34,
    UpdateElevationGroup = 35,
    DeleveragingThresholdSlotsPerBps = 36,
    DeprecatedUpdateMultiplierSideBoost = 37,
    DeprecatedUpdateMultiplierTagBoost = 38,
    UpdateReserveStatus = 39,
    UpdateFarmCollateral = 40,
    UpdateFarmDebt = 41,
    UpdateDisableUsageAsCollateralOutsideEmode = 42,
    UpdateBlockBorrowingAboveUtilization = 43,
    UpdateBlockPriceUsage = 44,
    UpdateBorrowLimitOutsideElevationGroup = 45,
    UpdateBorrowLimitsInElevationGroupAgainstThisReserve = 46,
    UpdateHostFixedInterestRateBps = 47,
}

#[derive(AnchorSerialize, AnchorDeserialize, PartialEq, Eq, Clone, Debug)]
pub enum UpdateLendingMarketConfigValue {
    Bool(bool),
    U8(u8),
    U8Array([u8; 8]),
    U16(u16),
    U64(u64),
    U128(u128),
    Pubkey(Pubkey),
    ElevationGroup(ElevationGroup),
}

impl UpdateLendingMarketConfigValue {
    pub fn to_bytes(&self) -> [u8; VALUE_BYTE_MAX_ARRAY_LEN_MARKET_UPDATE] {
        let mut val = [0; VALUE_BYTE_MAX_ARRAY_LEN_MARKET_UPDATE];
        match self {
            UpdateLendingMarketConfigValue::Bool(v) => {
                val[0] = *v as u8;
                val
            }
            UpdateLendingMarketConfigValue::U8(v) => {
                val[0] = *v;
                val
            }
            UpdateLendingMarketConfigValue::U16(v) => {
                val[..2].copy_from_slice(&v.to_le_bytes());
                val
            }
            UpdateLendingMarketConfigValue::U64(v) => {
                val[..8].copy_from_slice(&v.to_le_bytes());
                val
            }
            UpdateLendingMarketConfigValue::U128(v) => {
                val[..16].copy_from_slice(&v.to_le_bytes());
                val
            }
            UpdateLendingMarketConfigValue::Pubkey(v) => {
                val[..32].copy_from_slice(v.as_ref());
                val
            }
            UpdateLendingMarketConfigValue::ElevationGroup(v) => {
                val[..72].copy_from_slice(v.try_to_vec().unwrap().as_slice());
                val
            }
            UpdateLendingMarketConfigValue::U8Array(value) => {
                val[..8].copy_from_slice(value);
                val
            }
        }
    }
}

#[derive(
    TryFromPrimitive,
    AnchorSerialize,
    AnchorDeserialize,
    EnumString,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Debug,
)]
#[repr(u64)]
pub enum UpdateLendingMarketMode {
    UpdateOwner = 0,
    UpdateEmergencyMode = 1,
    UpdateLiquidationCloseFactor = 2,
    UpdateLiquidationMaxValue = 3,
    UpdateGlobalUnhealthyBorrow = 4,
    UpdateGlobalAllowedBorrow = 5,
    UpdateRiskCouncil = 6,
    UpdateMinFullLiquidationThreshold = 7,
    UpdateInsolvencyRiskLtv = 8,
    UpdateElevationGroup = 9,
    UpdateReferralFeeBps = 10,
    DeprecatedUpdateMultiplierPoints = 11,
    UpdatePriceRefreshTriggerToMaxAgePct = 12,
    UpdateAutodeleverageEnabled = 13,
    UpdateBorrowingDisabled = 14,
    UpdateMinNetValueObligationPostAction = 15,
    UpdateMinValueSkipPriorityLiqCheck = 16,
    UpdatePaddingFields = 17,
}

#[cfg(feature = "serde")]
pub mod serde_string {
    use std::{fmt::Display, str::FromStr};

    use serde::{de, Deserialize, Deserializer, Serializer};

    pub fn serialize<T, S>(value: &T, serializer: S) -> Result<S::Ok, S::Error>
    where
        T: Display,
        S: Serializer,
    {
        serializer.collect_str(value)
    }

    pub fn deserialize<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where
        T: FromStr,
        T::Err: Display,
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer)?
            .parse()
            .map_err(de::Error::custom)
    }
}

#[cfg(feature = "serde")]
pub mod serde_utf_string {
    pub fn serialize<S>(field: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let trimmed_field = String::from_utf8_lossy(field)
            .trim_end_matches('\0')
            .to_string();

        serializer.serialize_str(&trimmed_field)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: String = serde::Deserialize::deserialize(deserializer)?;
        let mut bytes = [0u8; 32];
        bytes[..s.len()].copy_from_slice(s.as_bytes());
        Ok(bytes)
    }
}

#[cfg(feature = "serde")]
pub mod serde_bool_u8 {
    pub fn serialize<S>(field: &u8, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bool(*field != 0)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u8, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s: bool = serde::Deserialize::deserialize(deserializer)?;
        Ok(s as u8)
    }
}
