use std::fmt::Formatter;

use anchor_lang::{prelude::*, AnchorDeserialize, AnchorSerialize};
#[cfg(feature = "serde")]
use serde;

#[cfg(feature = "serde")]
use super::serde_string;
use crate::{utils::NULL_PUBKEY, LendingError};

#[derive(AnchorSerialize, AnchorDeserialize, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[zero_copy]
#[repr(C)]
pub struct TokenInfo {
    #[cfg_attr(feature = "serde", serde(with = "serde_name"))]
    pub name: [u8; 32],

    pub heuristic: PriceHeuristic,

    pub max_twap_divergence_bps: u64,

    pub max_age_price_seconds: u64,
    pub max_age_twap_seconds: u64,

    #[cfg_attr(feature = "serde", serde(default))]
    pub scope_configuration: ScopeConfiguration,

    #[cfg_attr(feature = "serde", serde(default))]
    pub switchboard_configuration: SwitchboardConfiguration,

    #[cfg_attr(feature = "serde", serde(default))]
    pub pyth_configuration: PythConfiguration,

    #[cfg_attr(feature = "serde", serde(skip_serializing, default))]
    pub _padding: [u64; 20],
}

impl std::fmt::Debug for TokenInfo {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        let name = std::str::from_utf8(&self.name).unwrap_or("InvalidTokenName");
        f.debug_struct("TokenInfo")
            .field("name", &name)
            .field("heuristic", &self.heuristic)
            .field("max_twap_divergence_bps", &self.max_twap_divergence_bps)
            .field("max_age_price_seconds", &self.max_age_price_seconds)
            .field("max_age_twap_seconds", &self.max_age_twap_seconds)
            .field("scope_configuration", &self.scope_configuration)
            .field("switchboard_configuration", &self.switchboard_configuration)
            .field("pyth_configuration", &self.pyth_configuration)
            .finish()
    }
}

impl TokenInfo {
    pub fn validate_token_info_config(
        &self,
        pyth_info: &Option<AccountInfo>,
        switchboard_price_info: &Option<AccountInfo>,
        switchboard_twap_info: &Option<AccountInfo>,
        scope_prices_info: &Option<AccountInfo>,
    ) -> Result<()> {
        require!(self.is_valid(), LendingError::InvalidOracleConfig);
        require!(self.is_twap_config_valid(), LendingError::InvalidTwapConfig);
        require!(
            self.check_pyth_acc_matches(pyth_info),
            LendingError::InvalidPythPriceAccount
        );
        require!(
            self.check_switchboard_acc_matches(switchboard_price_info, switchboard_twap_info),
            LendingError::InvalidSwitchboardAccount
        );
        require!(
            self.check_scope_acc_matches(scope_prices_info),
            LendingError::InvalidScopePriceAccount
        );
        Ok(())
    }

    #[inline]
    pub fn is_valid(&self) -> bool {
        self.scope_configuration.is_valid()
            && (self.scope_configuration.is_enabled()
                || self.switchboard_configuration.is_enabled()
                || self.pyth_configuration.is_enabled())
    }

    #[inline]
    pub fn is_twap_enabled(&self) -> bool {
        self.max_twap_divergence_bps > 0
    }

    #[inline]
    pub fn is_twap_config_valid(&self) -> bool {
        if !self.is_twap_enabled() {
            return true;
        }

        if self.max_age_twap_seconds == 0 {
            return false;
        }

        if self.scope_configuration.is_enabled() && !self.scope_configuration.has_twap() {
            return false;
        }

        if self.switchboard_configuration.is_enabled() && !self.switchboard_configuration.has_twap()
        {
            return false;
        }

        true
    }

    #[inline]
    pub fn check_pyth_acc_matches(&self, pyth_info: &Option<AccountInfo>) -> bool {
        if self.pyth_configuration.is_enabled() {
            matches!(pyth_info, Some(a) if *a.key == self.pyth_configuration.price)
        } else {
            pyth_info.is_none()
        }
    }

    #[inline]
    pub fn check_switchboard_acc_matches(
        &self,
        switchboard_price_info: &Option<AccountInfo>,
        switchboard_twap_info: &Option<AccountInfo>,
    ) -> bool {
        if self.switchboard_configuration.is_enabled() {
            matches!(
                switchboard_price_info,
                Some(a) if *a.key == self.switchboard_configuration.price_aggregator)
                && (!self.is_twap_enabled()
                    || matches!(
                        switchboard_twap_info,
                        Some(a) if *a.key == self.switchboard_configuration.twap_aggregator
                    ))
        } else {
            switchboard_price_info.is_none() && switchboard_twap_info.is_none()
        }
    }

    #[inline]
    pub fn check_scope_acc_matches(&self, scope_prices_info: &Option<AccountInfo>) -> bool {
        if self.scope_configuration.is_enabled() {
            matches!(scope_prices_info, Some(a) if *a.key == self.scope_configuration.price_feed)
        } else {
            scope_prices_info.is_none()
        }
    }

    pub fn symbol(&self) -> &str {
        std::str::from_utf8(&self.name)
            .unwrap_or("InvalidTokenName")
            .trim_end_matches('\0')
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[zero_copy]
#[repr(C)]
pub struct PriceHeuristic {
    pub lower: u64,
    pub upper: u64,
    pub exp: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[zero_copy]
#[repr(C)]
pub struct ScopeConfiguration {
    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub price_feed: Pubkey,
    #[cfg_attr(feature = "serde", serde(with = "serde_scope_chain"))]
    pub price_chain: [u16; 4],
    #[cfg_attr(feature = "serde", serde(with = "serde_scope_chain"))]
    pub twap_chain: [u16; 4],
}

impl Default for ScopeConfiguration {
    #[inline]
    fn default() -> ScopeConfiguration {
        ScopeConfiguration {
            price_feed: NULL_PUBKEY,
            price_chain: [u16::MAX; 4],
            twap_chain: [u16::MAX; 4],
        }
    }
}

impl ScopeConfiguration {
    pub fn is_enabled(&self) -> bool {
        self.price_feed != Pubkey::default() && self.price_feed != NULL_PUBKEY
    }

    pub fn is_valid(&self) -> bool {
        !self.is_enabled() || (self.price_chain != [u16::MAX; 4] && self.price_chain != [0; 4])
    }

    pub fn has_twap(&self) -> bool {
        self.twap_chain != [u16::MAX; 4] && self.twap_chain != [0; 4]
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[zero_copy]
#[repr(C)]
pub struct SwitchboardConfiguration {
    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub price_aggregator: Pubkey,
    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub twap_aggregator: Pubkey,
}

impl SwitchboardConfiguration {
    pub fn is_enabled(&self) -> bool {
        self.price_aggregator != Pubkey::default() && self.price_aggregator != NULL_PUBKEY
    }

    pub fn has_twap(&self) -> bool {
        self.twap_aggregator != Pubkey::default() && self.twap_aggregator != NULL_PUBKEY
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[zero_copy]
#[repr(transparent)]
pub struct PythConfiguration {
    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub price: Pubkey,
}

impl PythConfiguration {
    pub fn is_enabled(&self) -> bool {
        self.price != Pubkey::default() && self.price != NULL_PUBKEY
    }
}

#[cfg(feature = "serde")]
mod serde_name {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(name: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let s = std::str::from_utf8(name)
            .unwrap_or("InvalidTokenName")
            .trim_end_matches('\0');
        serializer.serialize_str(s)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 32], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        let mut name = [0u8; 32];
        let name_bytes = s.as_bytes();
        name[..name_bytes.len()].copy_from_slice(name_bytes);
        Ok(name)
    }
}

#[cfg(feature = "serde")]
mod serde_scope_chain {
    use serde::{ser::SerializeSeq, Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(chain: &[u16; 4], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut seq = serializer.serialize_seq(None)?;
        for element in chain.iter().take_while(|&&e| e != u16::MAX) {
            seq.serialize_element(&element)?;
        }
        seq.end()
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u16; 4], D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = Vec::<u16>::deserialize(deserializer)?;
        let mut chain = [u16::MAX; 4];
        for (source, chain_elem) in s.iter().zip(chain.iter_mut()) {
            *chain_elem = *source;
        }
        Ok(chain)
    }
}
