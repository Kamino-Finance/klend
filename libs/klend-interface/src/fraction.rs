//! Fixed-point fraction type matching the on-chain representation.
//!
//! All on-chain fields ending in `_sf` (e.g. `market_price_sf`, `borrowed_amount_sf`)
//! store values as [`Fraction`] — a 128-bit unsigned fixed-point number with 68 integer
//! bits and 60 fractional bits (`U68F60`).
//!
//! # Converting from on-chain `_sf` fields
//!
//! The `_sf` fields are stored as `u128` (or [`PodU128`](crate::state::PodU128))
//! on-chain. To interpret them as a [`Fraction`]:
//!
//! ```rust
//! use klend_interface::Fraction;
//!
//! let raw_sf: u128 = 1_152_921_504_606_846_976; // 1.0 as _sf
//! let value = Fraction::from_bits(raw_sf);
//! assert_eq!(value, Fraction::ONE);
//!
//! // Convert to f64
//! let float_value: f64 = value.to_num();
//! assert!((float_value - 1.0).abs() < 1e-10);
//! ```

/// Fixed-point fraction type: 128-bit unsigned, 68 integer bits, 60 fractional bits.
///
/// This is the same type used internally by the klend program for all `_sf` fields.
/// Use [`Fraction::from_bits`] to convert raw on-chain `u128` values.
pub use fixed::types::U68F60 as Fraction;

/// Scale factor for `_sf` fields: `1.0 == 2^60`.
///
/// This is equivalent to `Fraction::ONE.to_bits()`.
pub const FRACTION_ONE_SCALED: u128 = 1 << 60;
