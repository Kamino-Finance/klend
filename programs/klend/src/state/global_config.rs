use anchor_lang::prelude::*;
use bytemuck::Zeroable;
use derivative::Derivative;
use num_enum::TryFromPrimitive;
#[cfg(feature = "serde")]
use strum::EnumIter;
use strum::EnumString;

#[cfg(feature = "serde")]
use super::serde_string;
#[cfg(feature = "serde")]
use crate::utils::accounts::default_array;
use crate::{lending_market::config_items, utils::GLOBAL_CONFIG_SIZE};

static_assertions::const_assert_eq!(GLOBAL_CONFIG_SIZE, std::mem::size_of::<GlobalConfig>());
static_assertions::const_assert_eq!(0, std::mem::size_of::<GlobalConfig>() % 8);
#[derive(PartialEq, Eq, Derivative)]
#[derivative(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[account(zero_copy)]
#[repr(C)]
pub struct GlobalConfig {
    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub global_admin: Pubkey,
    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub pending_admin: Pubkey,

    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub fee_collector: Pubkey,

    #[cfg_attr(
        feature = "serde",
        serde(skip_deserializing, skip_serializing, default = "default_array")
    )]
    pub padding: [u8; 928],
}

impl Default for GlobalConfig {
    fn default() -> Self {
        Self::zeroed()
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
#[repr(u8)]
pub enum UpdateGlobalConfigMode {
    PendingAdmin = 0,
    FeeCollector = 1,
}

impl GlobalConfig {
    pub fn init(&mut self, initial_admin: Pubkey) {
        self.global_admin = initial_admin;
        self.pending_admin = initial_admin;
        self.fee_collector = initial_admin;
    }

    pub fn update_value(&mut self, mode: UpdateGlobalConfigMode, value: &[u8]) -> Result<()> {
        let global_config = self;
        match mode {
            UpdateGlobalConfigMode::PendingAdmin => {
                config_items::for_named_field!(&mut global_config.pending_admin).set(value)?;
            }
            UpdateGlobalConfigMode::FeeCollector => {
                config_items::for_named_field!(&mut global_config.fee_collector).set(value)?;
            }
        }
        Ok(())
    }

    #[inline(always)]
    pub fn apply_pending_admin(&mut self) -> Result<()> {
        self.global_admin = self.pending_admin;
        Ok(())
    }
}
