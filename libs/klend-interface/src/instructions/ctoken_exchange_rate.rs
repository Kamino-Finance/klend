use borsh::{BorshDeserialize, BorshSerialize};
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use crate::{discriminators, util::*, KLEND_PROGRAM_ID};

// ---------------------------------------------------------------------------
// calculate_ctoken_exchange_rate
// ---------------------------------------------------------------------------

pub struct CalculateCTokenExchangeRateAccounts {
    pub reserve: Pubkey,
}

/// Reads an already-refreshed `reserve` and returns its cToken exchange rate (as CPI return
/// data). It does not refresh: the reserve must already be fresh in the current slot, or the
/// instruction reverts. Callers normally guarantee that by refreshing it earlier in the same
/// transaction (e.g. via [`refresh_reserves_batch`](super::refresh::refresh_reserves_batch)
/// skipping prices).
pub fn calculate_ctoken_exchange_rate(
    accounts: CalculateCTokenExchangeRateAccounts,
) -> Instruction {
    let data = discriminators::CALCULATE_CTOKEN_EXCHANGE_RATE.to_vec();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![readonly(accounts.reserve)],
        data,
    }
}

/// Mirror of the on-chain `ExchangeRateWithDecimals` returned (as CPI return data) by
/// [calculate_ctoken_exchange_rate]. See the on-chain handler for the meaning of the fields;
/// the whole-token rate is `exchange_rate_sf / 10^mint_decimals` (with `exchange_rate_sf`
/// interpreted as `Fraction` bits — see the field doc below).
///
/// The on-chain handler is fallible: on any error it reverts,
/// so this type is only ever produced on the success path.
#[derive(BorshSerialize, BorshDeserialize, Clone, Debug, PartialEq, Eq)]
pub struct ExchangeRateWithDecimals {
    /// The exchange rate as the raw bits of a `Fraction`, i.e. the `fixed` crate's `U68F60`: a
    /// `u128` fixed-point value with 60 fractional bits.
    pub exchange_rate_sf: u128,
    pub mint_decimals: u8,
}
