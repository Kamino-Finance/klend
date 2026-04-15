use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use super::{
    common::{build_refresh_all_obligation_reserves, build_refresh_obligation},
    info::{ObligationInfo, ReserveInfo},
};
use crate::{pda, types::UpdateObligationConfigMode, util::readonly, KLEND_PROGRAM_ID};

/// Build instructions to request an elevation group change for an obligation.
///
/// The remaining_accounts for `request_elevation_group` must include all
/// deposit and borrow reserves on the obligation. When the obligation has a
/// referrer, a `ReferrerTokenState` PDA per borrow reserve is appended.
///
/// `obligation_reserves` should contain [`ReserveInfo`] for every deposit and
/// borrow reserve on the obligation.
///
/// Returns: `[refresh_all_reserves..., refresh_obligation, request_elevation_group]`
pub fn request_elevation_group(
    owner: Pubkey,
    lending_market: Pubkey,
    obligation: &ObligationInfo,
    obligation_reserves: &[ReserveInfo],
    elevation_group: u8,
) -> Vec<Instruction> {
    let mut remaining =
        Vec::with_capacity(obligation.deposit_reserves.len() + obligation.borrow_reserves.len());
    for r in &obligation.deposit_reserves {
        remaining.push(readonly(*r));
    }
    for r in &obligation.borrow_reserves {
        remaining.push(readonly(*r));
    }
    if let Some(referrer) = obligation.referrer {
        for borrow_reserve in &obligation.borrow_reserves {
            let (rts, _) = pda::referrer_token_state(&KLEND_PROGRAM_ID, &referrer, borrow_reserve);
            remaining.push(readonly(rts));
        }
    }

    let mut ixs = build_refresh_all_obligation_reserves(obligation, obligation_reserves, &[]);
    ixs.push(build_refresh_obligation(&lending_market, obligation));
    ixs.push(crate::instructions::obligation::request_elevation_group(
        crate::instructions::obligation::RequestElevationGroupAccounts {
            owner,
            obligation: obligation.address,
            lending_market,
        },
        elevation_group,
        remaining,
    ));

    ixs
}

/// Build an instruction to update an obligation's configuration (e.g. rollover settings).
///
/// No refresh is needed — this instruction only modifies configuration flags.
pub fn update_obligation_config(
    owner: Pubkey,
    obligation: Pubkey,
    lending_market: Pubkey,
    borrow_reserve: Option<Pubkey>,
    deposit_reserve: Option<Pubkey>,
    mode: UpdateObligationConfigMode,
    value: Vec<u8>,
) -> Instruction {
    crate::instructions::obligation::update_obligation_config(
        crate::instructions::obligation::UpdateObligationConfigAccounts {
            owner,
            obligation,
            borrow_reserve,
            deposit_reserve,
            lending_market,
        },
        mode,
        value,
    )
}
