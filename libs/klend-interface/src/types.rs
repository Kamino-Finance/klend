use borsh::{BorshDeserialize, BorshSerialize};
use solana_pubkey::Pubkey;

use crate::KVAULT_PROGRAM_ID;

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct InitObligationArgs {
    pub tag: u8,
    pub id: u8,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct BorrowOrderConfigArgs {
    pub remaining_debt_amount: u64,
    pub max_borrow_rate_bps: u32,
    pub min_debt_term_seconds: u64,
    pub fillable_until_timestamp: u64,
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct ObligationOrder {
    pub condition_threshold_sf: u128,
    pub opportunity_parameter_sf: u128,
    pub min_execution_bonus_bps: u16,
    pub max_execution_bonus_bps: u16,
    pub condition_type: u8,
    pub opportunity_type: u8,
    pub padding1: [u8; 10],
    pub padding2: [u128; 5],
}

#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ProgressCallbackType {
    #[default]
    None = 0,
    KlendQueueAccountingHandlerOnKvault = 1,
}

impl ProgressCallbackType {
    /// The target program to be called-back, matching the on-chain validation.
    pub fn program_address(&self) -> Pubkey {
        match self {
            ProgressCallbackType::None => Pubkey::default(),
            ProgressCallbackType::KlendQueueAccountingHandlerOnKvault => KVAULT_PROGRAM_ID,
        }
    }
}

/// Optional customizations applied after cloning a reserve config.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct ReserveConfigCustomizationArgs {
    /// Gate flag for `fixed_borrow_rate_bps` (0 = don't override).
    pub override_fixed_rate_bps: u8,
    /// Fixed borrow rate to apply (only effective when `override_fixed_rate_bps != 0`).
    pub fixed_borrow_rate_bps: u32,
    /// Gate flag for `debt_term_seconds` (0 = don't override).
    pub override_debt_term_seconds: u8,
    /// Debt term override value (only effective when `override_debt_term_seconds != 0`).
    pub debt_term_seconds: u64,
    /// Whether to clear elevation groups from the cloned config.
    pub clear_elevation_groups: u8,
}

/// Mode for the `update_obligation_config` instruction.
#[derive(BorshSerialize, BorshDeserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum UpdateObligationConfigMode {
    FixedTermRolloverEnabled = 0,
    FixedTermRolloverMaxBorrowRateBps = 1,
    FixedTermRolloverMinDebtTermSeconds = 2,
    FixedTermRolloverOpenTermAllowed = 3,
    MigrationToFixedEnabled = 4,
}
