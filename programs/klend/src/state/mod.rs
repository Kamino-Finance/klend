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
}

impl UpdateReserveConfigValue {
    pub fn to_bytes_single(&self) -> [u8; VALUE_BYTE_ARRAY_LEN_SHORT_UPDATE] {
        let long_bytes = self.to_raw_bytes();

        let mut short_bytes = [0; VALUE_BYTE_ARRAY_LEN_SHORT_UPDATE];
        short_bytes.copy_from_slice(&long_bytes[..VALUE_BYTE_ARRAY_LEN_SHORT_UPDATE]);
        short_bytes
    }

    pub fn to_bytes_entire(&self) -> [u8; VALUE_BYTE_ARRAY_LEN_RESERVE] {
        self.to_raw_bytes()
    }

    pub fn to_raw_bytes(&self) -> [u8; VALUE_BYTE_ARRAY_LEN_RESERVE] {
        let mut val = [0; VALUE_BYTE_ARRAY_LEN_RESERVE];
        match self {
            UpdateReserveConfigValue::Bool(v) => {
                val[0] = *v as u8;
                val
            }
            UpdateReserveConfigValue::U8(v) => {
                val[0] = *v;
                val
            }
            UpdateReserveConfigValue::U16(v) => {
                val[..2].copy_from_slice(&v.to_le_bytes());
                val
            }
            UpdateReserveConfigValue::U64(v) => {
                val[..8].copy_from_slice(&v.to_le_bytes());
                val
            }
            UpdateReserveConfigValue::Pubkey(v) => {
                val[..32].copy_from_slice(v.as_ref());
                val
            }
            UpdateReserveConfigValue::ScopeChain(chain) => {
                let expanded: [u8; 8] = chain.map(|x| x.to_le_bytes()).concat().try_into().unwrap();
                val[..8].clone_from_slice(&expanded);
                val
            }
            UpdateReserveConfigValue::Name(v) => {
                val[..v.len()].copy_from_slice(v);
                val
            }
            UpdateReserveConfigValue::Full(config) => {
                let src = config.try_to_vec().unwrap();
                val[..src.len()].copy_from_slice(&src);
                val
            }
            UpdateReserveConfigValue::BorrowRateCurve(curve) => {
                let curve = curve.try_to_vec().unwrap();
                val[..curve.len()].copy_from_slice(&curve);
                val
            }
            UpdateReserveConfigValue::WithdrawalCap(cap, interval) => {
                let cap = cap.try_to_vec().unwrap();
                let interval = interval.try_to_vec().unwrap();
                val[..8].copy_from_slice(&cap);
                val[8..16].copy_from_slice(&interval);
                val
            }
            UpdateReserveConfigValue::ElevationGroups(groups) => {
                val[..groups.len()].copy_from_slice(groups);
                val
            }
            UpdateReserveConfigValue::U8Tuple(mode, value) => {
                val[0] = *mode;
                val[1] = *value;
                val
            }
        }
    }
}

#[derive(TryFromPrimitive, PartialEq, Eq, Clone, Copy, Debug)]
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
    UpdateMultiplierSideBoost = 37,
    UpdateMultiplierTagBoost = 38,
    UpdateReserveStatus = 39,
}

#[derive(PartialEq, Eq, Clone, Debug)]
pub enum UpdateLendingMarketConfigValue {
    Bool(bool),
    U8(u8),
    U8Array([u8; 8]),
    U16(u16),
    U64(u64),
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

#[derive(TryFromPrimitive, EnumString, PartialEq, Eq, Clone, Copy, Debug)]
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
    UpdateMultiplierPoints = 11,
    UpdatePriceRefreshTriggerToMaxAgePct = 12,
    UpdateAutodeleverageEnabled = 13,
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
