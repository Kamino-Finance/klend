use anchor_lang::prelude::*;
use borsh::{BorshDeserialize, BorshSerialize};
use derivative::Derivative;

#[cfg(feature = "serde")]
use super::{serde_bool_u8, serde_string, serde_utf_string};
use crate::{
    utils::{
        CLOSE_TO_INSOLVENCY_RISKY_LTV, ELEVATION_GROUP_NONE, GLOBAL_ALLOWED_BORROW_VALUE,
        GLOBAL_UNHEALTHY_BORROW_VALUE, LENDING_MARKET_SIZE, LIQUIDATION_CLOSE_FACTOR,
        LIQUIDATION_CLOSE_VALUE, MAX_LIQUIDATABLE_VALUE_AT_ONCE, MIN_NET_VALUE_IN_OBLIGATION,
        PROGRAM_VERSION,
    },
    LendingError,
};
#[cfg(feature = "serde")]
use serde_values::*;

static_assertions::const_assert_eq!(LENDING_MARKET_SIZE, std::mem::size_of::<LendingMarket>());
static_assertions::const_assert_eq!(0, std::mem::size_of::<LendingMarket>() % 8);
#[derive(PartialEq, Eq, Derivative)]
#[derivative(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[account(zero_copy)]
#[repr(C)]
pub struct LendingMarket {
    pub version: u64,
    pub bump_seed: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub lending_market_owner: Pubkey,
    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub lending_market_owner_cached: Pubkey,
    #[cfg_attr(feature = "serde", serde(with = "serde_utf_string", default))]
    pub quote_currency: [u8; 32],

    pub referral_fee_bps: u16,

    #[cfg_attr(feature = "serde", serde(with = "serde_bool_u8"))]
    pub emergency_mode: u8,

    #[cfg_attr(feature = "serde", serde(with = "serde_bool_u8"))]
    pub autodeleverage_enabled: u8,

    #[cfg_attr(feature = "serde", serde(with = "serde_bool_u8"))]
    pub borrow_disabled: u8,

    pub price_refresh_trigger_to_max_age_pct: u8,

    pub liquidation_max_debt_close_factor_pct: u8,
    pub insolvency_risk_unhealthy_ltv_pct: u8,
    pub min_full_liquidation_value_threshold: u64,

    pub max_liquidatable_debt_market_value_at_once: u64,
    pub global_unhealthy_borrow_value: u64,
    pub global_allowed_borrow_value: u64,
    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub risk_council: Pubkey,

    pub multiplier_points_tag_boost: [u8; 8],

    pub elevation_groups: [ElevationGroup; 32],
    #[cfg_attr(
        feature = "serde",
        serde(skip_deserializing, skip_serializing, default = "default_padding_90")
    )]
    pub elevation_group_padding: [u64; 90],

    #[cfg_attr(
        feature = "serde",
        serde(
            serialize_with = "serialize_min_net_value",
            deserialize_with = "deserialize_min_net_value"
        )
    )]
    pub min_net_value_in_obligation_sf: u128,

    #[cfg_attr(
        feature = "serde",
        serde(skip_deserializing, skip_serializing, default = "default_padding_178")
    )]
    #[derivative(Debug = "ignore")]
    pub padding1: [u64; 178],
}

#[cfg(feature = "serde")]
fn default_padding_178() -> [u64; 178] {
    [0; 178]
}

#[cfg(feature = "serde")]
fn default_padding_90() -> [u64; 90] {
    [0; 90]
}

impl Default for LendingMarket {
    fn default() -> Self {
        Self {
            version: 0,
            bump_seed: 0,
            lending_market_owner: Pubkey::default(),
            risk_council: Pubkey::default(),
            quote_currency: [0; 32],
            lending_market_owner_cached: Pubkey::default(),
            emergency_mode: 0,
            autodeleverage_enabled: 0,
            liquidation_max_debt_close_factor_pct: LIQUIDATION_CLOSE_FACTOR,
            insolvency_risk_unhealthy_ltv_pct: CLOSE_TO_INSOLVENCY_RISKY_LTV,
            max_liquidatable_debt_market_value_at_once: MAX_LIQUIDATABLE_VALUE_AT_ONCE,
            global_allowed_borrow_value: GLOBAL_ALLOWED_BORROW_VALUE,
            global_unhealthy_borrow_value: GLOBAL_UNHEALTHY_BORROW_VALUE,
            min_full_liquidation_value_threshold: LIQUIDATION_CLOSE_VALUE,
            multiplier_points_tag_boost: [1; 8],
            referral_fee_bps: 0,
            price_refresh_trigger_to_max_age_pct: 0,
            elevation_groups: [ElevationGroup::default(); 32],
            borrow_disabled: 0,
            elevation_group_padding: [0; 90],
            min_net_value_in_obligation_sf: MIN_NET_VALUE_IN_OBLIGATION.to_bits(),
            padding1: [0; 178],
        }
    }
}

impl LendingMarket {
    pub fn init(&mut self, params: InitLendingMarketParams) {
        *self = Self::default();
        self.version = PROGRAM_VERSION as u64;
        self.bump_seed = params.bump_seed as u64;
        self.lending_market_owner = params.lending_market_owner;
        self.quote_currency = params.quote_currency;
        self.multiplier_points_tag_boost = [1; 8];
    }

    pub fn get_elevation_group(
        &self,
        index: u8,
    ) -> std::result::Result<ElevationGroup, LendingError> {
        if index == ELEVATION_GROUP_NONE {
            Ok(ElevationGroup::default())
        } else {
            Ok(*self
                .elevation_groups
                .get(index as usize - 1)
                .ok_or(LendingError::InvalidElevationGroup)?)
        }
    }

    pub fn set_elevation_group(&mut self, elevation_group: ElevationGroup) -> Result<()> {
        if elevation_group.id == ELEVATION_GROUP_NONE {
            return err!(LendingError::InvalidElevationGroupConfig);
        }

        self.elevation_groups[elevation_group.id as usize - 1] = elevation_group;

        Ok(())
    }

    pub fn is_borrowing_disabled(&self) -> bool {
        self.borrow_disabled != false as u8
    }
}

pub struct InitLendingMarketParams {
    pub bump_seed: u8,
    pub lending_market_owner: Pubkey,
    pub quote_currency: [u8; 32],
}

#[derive(BorshSerialize, BorshDeserialize, Default, Derivative, PartialEq, Eq)]
#[derivative(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[zero_copy]
#[repr(C)]
pub struct ElevationGroup {
    pub max_liquidation_bonus_bps: u16,
    pub id: u8,
    pub ltv_pct: u8,
    pub liquidation_threshold_pct: u8,
    pub allow_new_loans: u8,
    #[derivative(Debug = "ignore")]
    #[cfg_attr(
        feature = "serde",
        serde(skip_deserializing, skip_serializing, default)
    )]
    #[derivative(Debug = "ignore")]
    pub reserved: [u8; 2],
    #[derivative(Debug = "ignore")]
    #[cfg_attr(
        feature = "serde",
        serde(skip_deserializing, skip_serializing, default)
    )]
    pub padding: [u64; 8],
}

impl ElevationGroup {
    pub fn new_loans_disabled(&self) -> bool {
        self.allow_new_loans == 0
    }
}

#[cfg(feature = "serde")]
mod serde_values {
    use std::result::Result;

    use serde::{
        de::{self, Deserialize, Deserializer},
        Serializer,
    };

    use crate::fraction::Fraction;

    pub fn serialize_min_net_value<S>(value: &u128, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let min_net_action_value_f = Fraction::from_bits(*value);
        serializer.serialize_str(&min_net_action_value_f.to_string())
    }

    pub fn deserialize_min_net_value<'de, D>(deserializer: D) -> Result<u128, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let net_value_action_f = Fraction::from_str(&s)
            .map_err(|_| de::Error::custom("min_net_value must be a fraction"))?;

        Ok(net_value_action_f.to_bits())
    }
}
