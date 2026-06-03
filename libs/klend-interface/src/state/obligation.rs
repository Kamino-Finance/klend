use bytemuck::{Pod, Zeroable};
use solana_pubkey::Pubkey;
use spl_discriminator::SplDiscriminate;

use super::{
    common::{BigFractionBytes, LastUpdate},
    pod::PodU128,
};

// ---------------------------------------------------------------------------
// Obligation
// ---------------------------------------------------------------------------

/// Lending market obligation state.
#[derive(Debug, Clone, Copy, Pod, Zeroable, SplDiscriminate)]
#[discriminator_hash_input("account:Obligation")]
#[repr(C)]
pub struct Obligation {
    pub tag: u64,
    pub last_update: LastUpdate,
    pub lending_market: Pubkey,
    pub owner: Pubkey,
    /// Deposited collateral (up to 8 positions).
    pub deposits: [ObligationCollateral; 8],
    pub lowest_reserve_deposit_liquidation_ltv: u64,
    /// Market value of deposits (scaled fraction).
    pub deposited_value_sf: PodU128,
    /// Borrowed liquidity (up to 5 positions).
    pub borrows: [ObligationLiquidity; 5],
    /// Risk-adjusted debt value (scaled fraction).
    pub borrow_factor_adjusted_debt_value_sf: PodU128,
    /// Market value of borrows (scaled fraction).
    pub borrowed_assets_market_value_sf: PodU128,
    /// Max borrow value at weighted-avg LTV (scaled fraction).
    pub allowed_borrow_value_sf: PodU128,
    /// Dangerous borrow value at liquidation threshold (scaled fraction).
    pub unhealthy_borrow_value_sf: PodU128,
    pub padding_deprecated_asset_tiers: [u8; 13],
    pub elevation_group: u8,
    pub num_of_obsolete_deposit_reserves: u8,
    /// 1 if borrows array is non-empty.
    pub has_debt: u8,
    pub referrer: Pubkey,
    pub borrowing_disabled: u8,
    pub autodeleverage_target_ltv_pct: u8,
    pub lowest_reserve_deposit_max_ltv_pct: u8,
    pub num_of_obsolete_borrow_reserves: u8,
    /// State of the ownership transfer process (see `OwnershipTransferState` in klend).
    pub ownership_transfer_state: u8,
    pub reserved: [u8; 3],
    pub highest_borrow_factor_pct: u64,
    pub autodeleverage_margin_call_started_timestamp: u64,
    pub obligation_orders: [ObligationOrder; 2],
    pub borrow_order: BorrowOrder,
    /// Pending owner during ownership transfer process.
    /// `Pubkey::default()` means no pending owner.
    pub pending_owner: Pubkey,
    pub padding_3: [u64; 69],
}

const _: () = assert!(core::mem::size_of::<Obligation>() == 3336);

impl Obligation {
    /// Number of active deposit positions.
    pub fn num_deposits(&self) -> usize {
        self.deposits
            .iter()
            .filter(|d| d.deposit_reserve != Pubkey::default())
            .count()
    }

    /// Number of active borrow positions.
    pub fn num_borrows(&self) -> usize {
        self.borrows
            .iter()
            .filter(|b| b.borrow_reserve != Pubkey::default())
            .count()
    }

    /// Total deposited value as a raw u128 scaled fraction.
    pub fn deposited_value(&self) -> u128 {
        u128::from(self.deposited_value_sf)
    }

    /// Allowed borrow value (weighted-avg LTV) as a raw u128 scaled fraction.
    pub fn allowed_borrow_value(&self) -> u128 {
        u128::from(self.allowed_borrow_value_sf)
    }

    /// Unhealthy borrow value (liquidation threshold) as a raw u128 scaled fraction.
    pub fn unhealthy_borrow_value(&self) -> u128 {
        u128::from(self.unhealthy_borrow_value_sf)
    }

    /// Borrow-factor-adjusted debt value as a raw u128 scaled fraction.
    pub fn borrow_factor_adjusted_debt_value(&self) -> u128 {
        u128::from(self.borrow_factor_adjusted_debt_value_sf)
    }

    /// Market value of borrowed assets as a raw u128 scaled fraction.
    pub fn borrowed_assets_market_value(&self) -> u128 {
        u128::from(self.borrowed_assets_market_value_sf)
    }

    /// Returns `true` if the obligation can be liquidated
    /// (borrow-factor-adjusted debt exceeds the unhealthy borrow value).
    pub fn is_liquidatable(&self) -> bool {
        let debt = self.borrow_factor_adjusted_debt_value();
        let unhealthy = self.unhealthy_borrow_value();
        debt > unhealthy && unhealthy > 0
    }

    /// Returns `true` if the obligation has no borrow capacity left
    /// (borrow-factor-adjusted debt >= allowed borrow value).
    pub fn is_borrowing_disabled(&self) -> bool {
        self.borrow_factor_adjusted_debt_value() >= self.allowed_borrow_value()
    }
}

impl ObligationCollateral {
    /// Returns `true` if this deposit slot is active (non-default reserve).
    pub fn is_active(&self) -> bool {
        self.deposit_reserve != Pubkey::default()
    }

    /// Market value as a raw u128 scaled fraction.
    pub fn market_value(&self) -> u128 {
        u128::from(self.market_value_sf)
    }
}

impl ObligationLiquidity {
    /// Returns `true` if this borrow slot is active (non-default reserve).
    pub fn is_active(&self) -> bool {
        self.borrow_reserve != Pubkey::default()
    }

    /// Borrowed amount as a raw u128 scaled fraction.
    pub fn borrowed_amount(&self) -> u128 {
        u128::from(self.borrowed_amount_sf)
    }

    /// Market value as a raw u128 scaled fraction.
    pub fn market_value(&self) -> u128 {
        u128::from(self.market_value_sf)
    }

    /// Borrow-factor-adjusted market value as a raw u128 scaled fraction.
    pub fn borrow_factor_adjusted_market_value(&self) -> u128 {
        u128::from(self.borrow_factor_adjusted_market_value_sf)
    }
}

// ---------------------------------------------------------------------------
// Nested types
// ---------------------------------------------------------------------------

/// A single collateral deposit within an obligation.
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct ObligationCollateral {
    pub deposit_reserve: Pubkey,
    pub deposited_amount: u64,
    pub market_value_sf: PodU128,
    pub borrowed_amount_against_this_collateral_in_elevation_group: u64,
    pub padding: [u64; 9],
}

/// A single borrow position within an obligation.
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct ObligationLiquidity {
    pub borrow_reserve: Pubkey,
    pub cumulative_borrow_rate_bsf: BigFractionBytes,
    pub last_borrowed_at_timestamp: u64,
    pub borrowed_amount_sf: PodU128,
    pub market_value_sf: PodU128,
    pub borrow_factor_adjusted_market_value_sf: PodU128,
    pub borrowed_amount_outside_elevation_groups: u64,
    pub fixed_term_borrow_rollover_config: FixedTermBorrowRolloverConfig,
    pub borrowed_amount_at_expiration: u64,
    pub padding2: [u64; 4],
}

/// Fixed-term borrow rollover configuration on an obligation liquidity position.
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct FixedTermBorrowRolloverConfig {
    pub auto_rollover_enabled: u8,
    pub open_term_allowed: u8,
    pub migration_to_fixed_enabled: u8,
    pub alignment_padding: [u8; 1],
    pub max_borrow_rate_bps: u32,
    pub min_debt_term_seconds: u64,
}

/// Owner-defined, permissionlessly-executed repay order.
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct ObligationOrder {
    pub condition_threshold_sf: PodU128,
    pub opportunity_parameter_sf: PodU128,
    pub min_execution_bonus_bps: u16,
    pub max_execution_bonus_bps: u16,
    pub condition_type: u8,
    pub opportunity_type: u8,
    pub padding1: [u8; 10],
    pub padding2: [PodU128; 5],
}

/// Owner-defined, permissionlessly-executed borrow order.
#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct BorrowOrder {
    pub debt_liquidity_mint: Pubkey,
    pub remaining_debt_amount: u64,
    pub filled_debt_destination: Pubkey,
    pub min_debt_term_seconds: u64,
    pub fillable_until_timestamp: u64,
    pub placed_at_timestamp: u64,
    pub last_updated_at_timestamp: u64,
    pub requested_debt_amount: u64,
    pub max_borrow_rate_bps: u32,
    pub active: u8,
    pub enable_auto_rollover_on_filled_borrows: u8,
    pub padding1: [u8; 2],
    pub end_padding: [u64; 5],
}
