use bytemuck::{Pod, Zeroable};
use solana_pubkey::Pubkey;
use spl_discriminator::SplDiscriminate;

use super::pod::PodU128;

// ---------------------------------------------------------------------------
// LendingMarket
// ---------------------------------------------------------------------------

/// Lending market account state.
#[derive(Debug, Clone, Copy, Pod, Zeroable, SplDiscriminate)]
#[discriminator_hash_input("account:LendingMarket")]
#[repr(C)]
pub struct LendingMarket {
    pub version: u64,
    pub bump_seed: u64,
    pub lending_market_owner: Pubkey,
    pub lending_market_owner_cached: Pubkey,
    pub quote_currency: [u8; 32],
    pub referral_fee_bps: u16,
    pub emergency_mode: u8,
    pub autodeleverage_enabled: u8,
    pub borrow_disabled: u8,
    pub price_refresh_trigger_to_max_age_pct: u8,
    pub liquidation_max_debt_close_factor_pct: u8,
    pub insolvency_risk_unhealthy_ltv_pct: u8,
    pub min_full_liquidation_value_threshold: u64,
    pub max_liquidatable_debt_market_value_at_once: u64,
    pub reserved0: [u8; 8],
    pub global_allowed_borrow_value: u64,
    pub emergency_council: Pubkey,
    pub reserved1: [u8; 8],
    pub elevation_groups: [ElevationGroup; 32],
    pub elevation_group_padding: [u64; 90],
    pub min_net_value_in_obligation_sf: PodU128,
    pub min_value_skip_liquidation_ltv_checks: u64,
    pub name: [u8; 32],
    pub min_value_skip_liquidation_bf_checks: u64,
    pub individual_autodeleverage_margin_call_period_secs: u64,
    pub min_initial_deposit_amount: u64,
    pub obligation_order_execution_enabled: u8,
    pub immutable: u8,
    pub obligation_order_creation_enabled: u8,
    pub price_triggered_liquidation_disabled: u8,
    pub mature_reserve_debt_liquidation_enabled: u8,
    pub obligation_borrow_debt_term_liquidation_enabled: u8,
    pub borrow_order_creation_enabled: u8,
    pub borrow_order_execution_enabled: u8,
    pub proposer_authority: Pubkey,
    pub min_borrow_order_fill_value: u64,
    pub withdraw_ticket_issuance_enabled: u8,
    pub withdraw_ticket_redemption_enabled: u8,
    pub obligation_borrow_rollover_configuration_enabled: u8,
    pub obligation_borrow_migration_to_fixed_execution_enabled: u8,
    pub withdraw_ticket_cancellation_enabled: u8,
    pub padding2: [u8; 1],
    /// Cap (in basis points; `FULL_BPS = 10_000` = 100%) on reserve rewards distribution APR
    pub reserve_rewards_max_apr_bps: u16,
    pub min_withdraw_queued_liquidity_value: u64,
    pub fixed_term_rollover_window_duration_seconds: u64,
    pub open_term_rollover_window_duration_seconds: u64,
    pub min_partial_rollover_value: u64,
    pub term_based_full_liquidation_duration_secs: u64,
    pub permissioning_authority: Pubkey,
    pub permissioned_ops: u64,
    pub padding1: [u64; 153],
}

const _: () = assert!(core::mem::size_of::<LendingMarket>() == 4656);

impl LendingMarket {
    /// Whether the market is in emergency mode.
    pub fn is_emergency_mode(&self) -> bool {
        self.emergency_mode != 0
    }

    /// Whether borrowing is globally disabled.
    pub fn is_borrow_disabled(&self) -> bool {
        self.borrow_disabled != 0
    }

    /// Whether autodeleverage is enabled.
    pub fn is_autodeleverage_enabled(&self) -> bool {
        self.autodeleverage_enabled != 0
    }

    /// Max percentage of debt that can be closed in a single liquidation.
    pub fn liquidation_max_debt_close_factor_pct(&self) -> u8 {
        self.liquidation_max_debt_close_factor_pct
    }

    /// Minimum value threshold for full liquidation.
    pub fn min_full_liquidation_value_threshold(&self) -> u64 {
        self.min_full_liquidation_value_threshold
    }

    /// Maximum liquidatable debt market value at once.
    pub fn max_liquidatable_debt_market_value_at_once(&self) -> u64 {
        self.max_liquidatable_debt_market_value_at_once
    }

    /// Referral fee in basis points.
    pub fn referral_fee_bps(&self) -> u16 {
        self.referral_fee_bps
    }

    /// Get an elevation group by index (0-31). Returns `None` if out of bounds.
    pub fn elevation_group(&self, index: usize) -> Option<&ElevationGroup> {
        self.elevation_groups.get(index)
    }

    /// Whether the market is immutable (no more config changes).
    pub fn is_immutable(&self) -> bool {
        self.immutable != 0
    }

    /// Whether withdraw ticket cancellation is enabled.
    pub fn is_withdraw_ticket_cancellation_enabled(&self) -> bool {
        self.withdraw_ticket_cancellation_enabled != 0
    }

    /// Whether migration-to-fixed rollover execution is enabled.
    pub fn is_obligation_borrow_migration_to_fixed_execution_enabled(&self) -> bool {
        self.obligation_borrow_migration_to_fixed_execution_enabled != 0
    }
}

// ---------------------------------------------------------------------------
// ElevationGroup
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct ElevationGroup {
    pub max_liquidation_bonus_bps: u16,
    pub id: u8,
    pub ltv_pct: u8,
    pub liquidation_threshold_pct: u8,
    pub allow_new_loans: u8,
    pub max_reserves_as_collateral: u8,
    pub padding_0: u8,
    pub debt_reserve: Pubkey,
    pub padding_1: [u64; 4],
}
