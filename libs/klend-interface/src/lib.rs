#![doc = include_str!("../README.md")]
//!
//! ## Example source code
//!
//! <details><summary><code>deposit_lending</code> — Deposit liquidity and receive cTokens directly</summary>
//!
//! ```rust,ignore
#![doc = include_str!("../examples/deposit_lending.rs")]
//! ```
//! </details>
//!
//! <details><summary><code>deposit_borrowing</code> — Deposit as collateral into an obligation</summary>
//!
//! ```rust,ignore
#![doc = include_str!("../examples/deposit_borrowing.rs")]
//! ```
//! </details>
//!
//! <details><summary><code>borrow</code> — Borrow liquidity against an obligation</summary>
//!
//! ```rust,ignore
#![doc = include_str!("../examples/borrow.rs")]
//! ```
//! </details>
//!
//! <details><summary><code>repay</code> — Repay borrowed liquidity</summary>
//!
//! ```rust,ignore
#![doc = include_str!("../examples/repay.rs")]
//! ```
//! </details>
//!
//! <details><summary><code>withdraw_obligation</code> — Withdraw collateral from an obligation and redeem for liquidity</summary>
//!
//! ```rust,ignore
#![doc = include_str!("../examples/withdraw_obligation.rs")]
//! ```
//! </details>
//!
//! <details><summary><code>redeem_ctokens</code> — Redeem cTokens for the underlying liquidity (no obligation)</summary>
//!
//! ```rust,ignore
#![doc = include_str!("../examples/redeem_ctokens.rs")]
//! ```
//! </details>
//!
//! <details><summary><code>flash_loan</code> — Flash-borrow and repay in a single transaction</summary>
//!
//! ```rust,ignore
#![doc = include_str!("../examples/flash_loan.rs")]
//! ```
//! </details>
//!
//! <details><summary><code>market_data</code> — Fetch and inspect lending market and reserve data</summary>
//!
//! ```rust,ignore
#![doc = include_str!("../examples/market_data.rs")]
//! ```
//! </details>
//!
//! <details><summary><code>user_position</code> — Read an obligation and display positions</summary>
//!
//! ```rust,ignore
#![doc = include_str!("../examples/user_position.rs")]
//! ```
//! </details>
//!
//! <details><summary><code>cpi_deposit_and_borrow</code> — CPI from an Anchor program: deposit collateral and borrow</summary>
//!
//! ```rust,ignore
#![doc = include_str!("../docs/cpi_deposit_and_borrow.rs")]
//! ```
//! </details>

pub mod discriminators;
pub mod errors;
pub mod fraction;
pub mod helpers;
pub mod instructions;
pub mod pda;
pub mod state;
pub mod types;
pub mod util;

// Convenience re-exports for the most commonly used types.
pub use errors::LendingError;
pub use fraction::{Fraction, FRACTION_ONE_SCALED};
pub use helpers::{
    CallbackAccounts, FarmsAccounts, ObligationContext, ObligationContextError, ObligationInfo,
    ReserveInfo,
};
use solana_pubkey::{pubkey, Pubkey};
pub use state::{from_account_data, AccountDataError};

/// Sentinel value that tells the program to use the maximum available amount
/// (e.g. repay all debt, withdraw all collateral).
pub const MAX_AMOUNT: u64 = u64::MAX;

/// Kamino Lending (mainnet) program ID.
pub const KLEND_PROGRAM_ID: Pubkey = pubkey!("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");

/// Kamino Lending (staging / devnet) program ID.
pub const KLEND_STAGING_PROGRAM_ID: Pubkey = pubkey!("SLendK7ySfcEzyaFqy93gDnD3RtrpXJcnRwb6zFHJSh");

/// Kamino Farms program ID.
pub const FARMS_PROGRAM_ID: Pubkey = pubkey!("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");

/// Kamino Vault (mainnet) program ID — used as the progress callback program
/// for `KlendQueueAccountingHandlerOnKvault`.
pub const KVAULT_PROGRAM_ID: Pubkey = pubkey!("KvauGMspG5k6rtzrqqn7WNn3oZdyKqLKwK2XWQ8FLjd");

// Well-known program and sysvar IDs.
pub const TOKEN_PROGRAM_ID: Pubkey = pubkey!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
pub const ASSOCIATED_TOKEN_PROGRAM_ID: Pubkey =
    pubkey!("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
pub const SYSTEM_PROGRAM_ID: Pubkey = pubkey!("11111111111111111111111111111111");
pub const SYSVAR_RENT_ID: Pubkey = pubkey!("SysvarRent111111111111111111111111111111111");
pub const SYSVAR_INSTRUCTIONS_ID: Pubkey = pubkey!("Sysvar1nstructions1111111111111111111111111");
