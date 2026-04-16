use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use super::{
    common::{build_refresh_obligation, build_refresh_reserve},
    info::{ObligationInfo, ReserveInfo},
};
use crate::{util::writable, KLEND_PROGRAM_ID};

/// Build a single `refresh_reserve` instruction.
///
/// Returns: `refresh_reserve`
pub fn refresh_reserve(reserve: &ReserveInfo) -> Instruction {
    build_refresh_reserve(reserve)
}

/// Build a single `refresh_obligation` instruction with all required
/// remaining accounts (deposit reserves, borrow reserves, referrer token
/// states) derived from the obligation info.
///
/// Returns: `refresh_obligation`
pub fn refresh_obligation(lending_market: &Pubkey, obligation: &ObligationInfo) -> Instruction {
    build_refresh_obligation(lending_market, obligation)
}

/// Build a `refresh_reserves_batch` instruction from a slice of reserve infos.
///
/// The remaining_accounts list is built automatically: for each reserve, the
/// reserve account (writable) is followed by its lending market (readonly) and
/// oracle accounts (readonly).
///
/// Returns: `refresh_reserves_batch`
pub fn refresh_reserves_batch(reserves: &[ReserveInfo], skip_price_updates: bool) -> Instruction {
    let mut remaining = Vec::with_capacity(reserves.len() * 6);
    for r in reserves {
        remaining.push(writable(r.address));
        remaining.push(crate::util::readonly(r.lending_market));
        remaining.push(crate::util::optional_account(
            &KLEND_PROGRAM_ID,
            r.pyth_oracle,
            false,
        ));
        remaining.push(crate::util::optional_account(
            &KLEND_PROGRAM_ID,
            r.switchboard_price_oracle,
            false,
        ));
        remaining.push(crate::util::optional_account(
            &KLEND_PROGRAM_ID,
            r.switchboard_twap_oracle,
            false,
        ));
        remaining.push(crate::util::optional_account(
            &KLEND_PROGRAM_ID,
            r.scope_prices,
            false,
        ));
    }
    crate::instructions::refresh::refresh_reserves_batch(skip_price_updates, remaining)
}

/// Build all refresh instructions needed for an obligation: refresh each
/// unique reserve (deposits + borrows), then refresh the obligation itself.
///
/// This is useful for bots that need to bring an entire obligation up-to-date
/// before scanning health or executing liquidations.
///
/// `reserve_infos` is a lookup function that returns `ReserveInfo` for a given
/// reserve address. All deposit and borrow reserves on the obligation must be
/// resolvable through this function.
///
/// Returns: `Ok([refresh_reserve * N, refresh_obligation])`
///
/// # Errors
///
/// Returns an error if any deposit or borrow reserve on the obligation cannot
/// be resolved through `reserve_infos`.
pub fn refresh_all_for_obligation(
    lending_market: &Pubkey,
    obligation: &ObligationInfo,
    reserve_infos: &dyn Fn(&Pubkey) -> Option<ReserveInfo>,
) -> Result<Vec<Instruction>, RefreshError> {
    // Collect unique reserves (deposits + borrows may overlap)
    let mut seen =
        Vec::with_capacity(obligation.deposit_reserves.len() + obligation.borrow_reserves.len());
    let mut ixs = Vec::new();

    for r in obligation
        .deposit_reserves
        .iter()
        .chain(obligation.borrow_reserves.iter())
    {
        if !seen.contains(r) {
            seen.push(*r);
            let info = reserve_infos(r).ok_or(RefreshError::ReserveNotFound(*r))?;
            ixs.push(build_refresh_reserve(&info));
        }
    }

    ixs.push(build_refresh_obligation(lending_market, obligation));
    Ok(ixs)
}

/// Error type for refresh operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefreshError {
    /// A reserve referenced by the obligation could not be resolved.
    ReserveNotFound(Pubkey),
}

impl std::fmt::Display for RefreshError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RefreshError::ReserveNotFound(key) => {
                write!(f, "reserve not found: {key}")
            }
        }
    }
}

impl std::error::Error for RefreshError {}
