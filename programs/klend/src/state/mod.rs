pub mod global_config;
pub mod last_update;
pub mod lending_market;
pub mod liquidation_operations;
pub mod nested_accounts;
pub mod obligation;
pub mod order_operations;
pub mod referral;
pub mod reserve;
pub mod token_info;
pub mod types;

use anchor_lang::prelude::*;
pub use global_config::*;
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
    UpdateFeesOriginationFee = 6,
    UpdateFeesFlashLoanFee = 7,
    DeprecatedUpdateFeesReferralFeeBps = 8,
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
    DeprecatedUpdateDebtWithdrawalCapCurrentTotal = 28,
    DeprecatedUpdateDepositWithdrawalCapCurrentTotal = 29,
    UpdateBadDebtLiquidationBonusBps = 30,
    UpdateMinLiquidationBonusBps = 31,
    UpdateDeleveragingMarginCallPeriod = 32,
    UpdateBorrowFactor = 33,
    UpdateAssetTier = 34,
    UpdateElevationGroup = 35,
    UpdateDeleveragingThresholdDecreaseBpsPerDay = 36,
    DeprecatedUpdateMultiplierSideBoost = 37,
    DeprecatedUpdateMultiplierTagBoost = 38,
    UpdateReserveStatus = 39,
    UpdateFarmCollateral = 40,
    UpdateFarmDebt = 41,
    UpdateDisableUsageAsCollateralOutsideEmode = 42,
    UpdateBlockBorrowingAboveUtilizationPct = 43,
    UpdateBlockPriceUsage = 44,
    UpdateBorrowLimitOutsideElevationGroup = 45,
    UpdateBorrowLimitsInElevationGroupAgainstThisReserve = 46,
    UpdateHostFixedInterestRateBps = 47,
    UpdateAutodeleverageEnabled = 48,
    UpdateDeleveragingBonusIncreaseBpsPerDay = 49,
    UpdateProtocolOrderExecutionFee = 50,
    UpdateProposerAuthorityLock = 51,
    UpdateMinDeleveragingBonusBps = 52,
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
    Name([u8; 32]),
}

impl UpdateLendingMarketConfigValue {
    pub fn to_bytes(&self) -> [u8; VALUE_BYTE_MAX_ARRAY_LEN_MARKET_UPDATE] {
        let mut val = [0; VALUE_BYTE_MAX_ARRAY_LEN_MARKET_UPDATE];
        match self {
            UpdateLendingMarketConfigValue::Bool(v) => {
                val[0] = *v as u8;
            }
            UpdateLendingMarketConfigValue::U8(v) => {
                val[0] = *v;
            }
            UpdateLendingMarketConfigValue::U16(v) => {
                val[..2].copy_from_slice(&v.to_le_bytes());
            }
            UpdateLendingMarketConfigValue::U64(v) => {
                val[..8].copy_from_slice(&v.to_le_bytes());
            }
            UpdateLendingMarketConfigValue::U128(v) => {
                val[..16].copy_from_slice(&v.to_le_bytes());
            }
            UpdateLendingMarketConfigValue::Pubkey(v) => {
                val[..32].copy_from_slice(v.as_ref());
            }
            UpdateLendingMarketConfigValue::ElevationGroup(v) => {
                val[..72].copy_from_slice(v.try_to_vec().unwrap().as_slice());
            }
            UpdateLendingMarketConfigValue::U8Array(value) => {
                val[..8].copy_from_slice(value);
            }
            UpdateLendingMarketConfigValue::Name(v) => {
                val[..v.len()].copy_from_slice(v);
            }
        }
        val
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
#[cfg_attr(feature = "serde", derive(EnumIter))]
#[repr(u64)]
pub enum UpdateLendingMarketMode {
    UpdateOwner = 0,
    UpdateEmergencyMode = 1,
    UpdateLiquidationCloseFactor = 2,
    UpdateLiquidationMaxValue = 3,
    DeprecatedUpdateGlobalUnhealthyBorrow = 4,
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
    UpdateMinValueLtvSkipPriorityLiqCheck = 16,
    UpdateMinValueBfSkipPriorityLiqCheck = 17,
    UpdatePaddingFields = 18,
    UpdateName = 19,
    UpdateIndividualAutodeleverageMarginCallPeriodSecs = 20,
    UpdateInitialDepositAmount = 21,
    UpdateObligationOrderExecutionEnabled = 22,
    UpdateImmutableFlag = 23,
    UpdateObligationOrderCreationEnabled = 24,
    UpdateProposerAuthority = 25,
    UpdatePriceTriggeredLiquidationDisabled = 26,
}

#[cfg(feature = "serde")]
pub mod serde_iter {
    use strum::IntoEnumIterator;

    use super::*;
    impl UpdateLendingMarketMode {
        pub fn iter_without_deprecated() -> impl Iterator<Item = Self> {
            Self::iter().filter(|mode| !mode.is_deprecated())
        }

        pub fn is_deprecated(&self) -> bool {
            matches!(
                *self,
                UpdateLendingMarketMode::DeprecatedUpdateMultiplierPoints
                    | UpdateLendingMarketMode::DeprecatedUpdateGlobalUnhealthyBorrow
            )
        }
    }
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

