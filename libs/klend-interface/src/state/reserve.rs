use bytemuck::{Pod, Zeroable};
use solana_pubkey::Pubkey;
use spl_discriminator::SplDiscriminate;

use super::{
    common::{BigFractionBytes, LastUpdate},
    pod::PodU128,
};

// ---------------------------------------------------------------------------
// Reserve
// ---------------------------------------------------------------------------

/// Lending reserve account state.
#[derive(Debug, Clone, Copy, Pod, Zeroable, SplDiscriminate)]
#[discriminator_hash_input("account:Reserve")]
#[repr(C)]
pub struct Reserve {
    pub version: u64,
    pub last_update: LastUpdate,
    pub lending_market: Pubkey,
    pub farm_collateral: Pubkey,
    pub farm_debt: Pubkey,
    pub liquidity: ReserveLiquidity,
    pub reserve_liquidity_padding: [u64; 150],
    pub collateral: ReserveCollateral,
    pub reserve_collateral_padding: [u64; 150],
    pub config: ReserveConfig,
    pub config_padding: [u64; 112],
    pub borrowed_amount_outside_elevation_group: u64,
    pub borrowed_amounts_against_this_reserve_in_elevation_groups: [u64; 32],
    pub withdraw_queue: WithdrawQueue,
    pub padding: [u64; 204],
}

const _: () = assert!(core::mem::size_of::<Reserve>() == 8616);

impl Reserve {
    /// Total available (unborrowed) liquidity in native token units.
    pub fn available_liquidity(&self) -> u64 {
        self.liquidity.total_available_amount
    }

    /// Total borrowed amount as a raw u128 scaled fraction.
    pub fn borrowed_amount(&self) -> u128 {
        u128::from(self.liquidity.borrowed_amount_sf)
    }

    /// Market price of the reserve token as a raw u128 scaled fraction.
    pub fn market_price(&self) -> u128 {
        u128::from(self.liquidity.market_price_sf)
    }

    /// Decimals of the liquidity mint.
    pub fn mint_decimals(&self) -> u64 {
        self.liquidity.mint_decimals
    }

    /// Total supply of collateral tokens (cTokens).
    pub fn collateral_total_supply(&self) -> u64 {
        self.collateral.mint_total_supply
    }

    /// Accumulated protocol fees as a raw u128 scaled fraction.
    pub fn accumulated_protocol_fees(&self) -> u128 {
        u128::from(self.liquidity.accumulated_protocol_fees_sf)
    }

    /// Accumulated referrer fees as a raw u128 scaled fraction.
    pub fn accumulated_referrer_fees(&self) -> u128 {
        u128::from(self.liquidity.accumulated_referrer_fees_sf)
    }

    /// Reserve status: 0 = Active, 1 = Obsolete, 2 = Hidden.
    pub fn status(&self) -> u8 {
        self.config.status
    }

    /// Whether the reserve is in emergency mode.
    pub fn is_emergency_mode(&self) -> bool {
        self.config.emergency_mode != 0
    }

    /// Loan-to-value percentage.
    pub fn loan_to_value_pct(&self) -> u8 {
        self.config.loan_to_value_pct
    }

    /// Liquidation threshold percentage.
    pub fn liquidation_threshold_pct(&self) -> u8 {
        self.config.liquidation_threshold_pct
    }

    /// Borrow factor percentage (100 = 1x).
    pub fn borrow_factor_pct(&self) -> u64 {
        self.config.borrow_factor_pct
    }

    /// Deposit limit in native token units.
    pub fn deposit_limit(&self) -> u64 {
        self.config.deposit_limit
    }

    /// Borrow limit in native token units.
    pub fn borrow_limit(&self) -> u64 {
        self.config.borrow_limit
    }

    /// Next sequence number for withdraw tickets.
    pub fn next_withdraw_ticket_sequence_number(&self) -> u64 {
        self.withdraw_queue.next_issued_ticket_sequence_number
    }
}

// ---------------------------------------------------------------------------
// ReserveLiquidity
// ---------------------------------------------------------------------------

/// Reserve liquidity state.
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct ReserveLiquidity {
    pub mint_pubkey: Pubkey,
    pub supply_vault: Pubkey,
    pub fee_vault: Pubkey,
    pub total_available_amount: u64,
    /// Total borrowed (scaled fraction).
    pub borrowed_amount_sf: PodU128,
    /// Market price in quote currency (scaled fraction).
    pub market_price_sf: PodU128,
    pub market_price_last_updated_ts: u64,
    pub mint_decimals: u64,
    pub deposit_limit_crossed_timestamp: u64,
    pub borrow_limit_crossed_timestamp: u64,
    /// Cumulative borrow rate (big scaled fraction).
    pub cumulative_borrow_rate_bsf: BigFractionBytes,
    /// Protocol fees (scaled fraction).
    pub accumulated_protocol_fees_sf: PodU128,
    /// Referrer fees (scaled fraction).
    pub accumulated_referrer_fees_sf: PodU128,
    /// Pending referrer fees (scaled fraction).
    pub pending_referrer_fees_sf: PodU128,
    /// Referral rate (scaled fraction).
    pub absolute_referral_rate_sf: PodU128,
    pub token_program: Pubkey,
    /// Reserve rewards budget remaining for distribution
    pub rewards_amount_available: u64,
    pub padding2: [u64; 50],
    pub padding3: [PodU128; 32],
}

// ---------------------------------------------------------------------------
// ReserveCollateral
// ---------------------------------------------------------------------------

/// Reserve collateral state.
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct ReserveCollateral {
    pub mint_pubkey: Pubkey,
    pub mint_total_supply: u64,
    pub supply_vault: Pubkey,
    pub padding1: [PodU128; 32],
    pub padding2: [PodU128; 32],
}

// ---------------------------------------------------------------------------
// ReserveConfig
// ---------------------------------------------------------------------------

/// Reserve configuration parameters.
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct ReserveConfig {
    /// 0 = Active, 1 = Obsolete, 2 = Hidden.
    pub status: u8,
    pub padding_deprecated_asset_tier: u8,
    pub host_fixed_interest_rate_bps: u16,
    pub min_deleveraging_bonus_bps: u16,
    pub block_ctoken_usage: u8,
    pub early_repay_remaining_interest_pct: u8,
    /// Whether the reserve is in emergency mode.
    pub emergency_mode: u8,
    pub reserved_1: [u8; 4],
    pub protocol_order_execution_fee_pct: u8,
    pub protocol_take_rate_pct: u8,
    pub protocol_liquidation_fee_pct: u8,
    pub loan_to_value_pct: u8,
    pub liquidation_threshold_pct: u8,
    pub min_liquidation_bonus_bps: u16,
    pub max_liquidation_bonus_bps: u16,
    pub bad_debt_liquidation_bonus_bps: u16,
    pub deleveraging_margin_call_period_secs: u64,
    pub deleveraging_threshold_decrease_bps_per_day: u64,
    pub fees: ReserveFees,
    pub borrow_rate_curve: BorrowRateCurve,
    pub borrow_factor_pct: u64,
    pub deposit_limit: u64,
    pub borrow_limit: u64,
    pub token_info: TokenInfo,
    pub deposit_withdrawal_cap: WithdrawalCaps,
    pub debt_withdrawal_cap: WithdrawalCaps,
    pub elevation_groups: [u8; 20],
    pub disable_usage_as_coll_outside_emode: u8,
    pub utilization_limit_block_borrowing_above_pct: u8,
    pub autodeleverage_enabled: u8,
    pub proposer_authority_locked: u8,
    pub borrow_limit_outside_elevation_group: u64,
    pub borrow_limit_against_this_collateral_in_elevation_group: [u64; 32],
    pub deleveraging_bonus_increase_bps_per_day: u64,
    pub debt_maturity_timestamp: u64,
    pub debt_term_seconds: u64,
    /// Rewards token amount distributed per slot to depositors
    pub rewards_amount_per_slot: u64,
    pub permissioned_ops: u64,
}

const _: () = assert!(core::mem::size_of::<ReserveConfig>() == 952);

// ---------------------------------------------------------------------------
// ReserveFees
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct ReserveFees {
    /// Borrow origination fee (scaled fraction).
    pub origination_fee_sf: u64,
    /// Flash loan fee (u64::MAX = disabled).
    pub flash_loan_fee_sf: u64,
    pub padding: [u8; 8],
}

// ---------------------------------------------------------------------------
// BorrowRateCurve / CurvePoint
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct BorrowRateCurve {
    pub points: [CurvePoint; 11],
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct CurvePoint {
    pub utilization_rate_bps: u32,
    pub borrow_rate_bps: u32,
}

// ---------------------------------------------------------------------------
// TokenInfo and oracle configs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Pod, Zeroable)]
#[repr(C)]
pub struct TokenInfo {
    pub name: [u8; 32],
    pub heuristic: PriceHeuristic,
    pub max_twap_divergence_bps: u64,
    pub max_age_price_seconds: u64,
    pub max_age_twap_seconds: u64,
    pub scope_configuration: ScopeConfiguration,
    pub switchboard_configuration: SwitchboardConfiguration,
    pub pyth_configuration: PythConfiguration,
    pub block_price_usage: u8,
    pub reserved: [u8; 7],
    pub _padding: [u64; 19],
}

const _: () = assert!(core::mem::size_of::<TokenInfo>() == 384);

impl TokenInfo {
    /// Token name as a UTF-8 string, trimmed of null padding.
    ///
    /// Returns `None` if the name bytes are not valid UTF-8.
    pub fn name_str(&self) -> Option<&str> {
        let end = self
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.name.len());
        core::str::from_utf8(&self.name[..end]).ok()
    }
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct PriceHeuristic {
    pub lower: u64,
    pub upper: u64,
    pub exp: u64,
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct ScopeConfiguration {
    pub price_feed: Pubkey,
    pub price_chain: [u16; 4],
    pub twap_chain: [u16; 4],
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct SwitchboardConfiguration {
    pub price_aggregator: Pubkey,
    pub twap_aggregator: Pubkey,
}

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct PythConfiguration {
    pub price: Pubkey,
}

// ---------------------------------------------------------------------------
// WithdrawalCaps
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct WithdrawalCaps {
    pub config_capacity: i64,
    pub current_total: i64,
    pub last_interval_start_timestamp: u64,
    pub config_interval_length_seconds: u64,
}

// ---------------------------------------------------------------------------
// WithdrawQueue
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, Zeroable, Pod)]
#[repr(C)]
pub struct WithdrawQueue {
    pub queued_collateral_amount: u64,
    pub next_issued_ticket_sequence_number: u64,
    pub next_withdrawable_ticket_sequence_number: u64,
}
