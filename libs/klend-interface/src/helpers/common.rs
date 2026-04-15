use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use super::info::{ObligationInfo, ReserveInfo};
use crate::{
    instructions::refresh::{RefreshObligationAccounts, RefreshReserveAccounts},
    pda,
    util::writable,
    KLEND_PROGRAM_ID,
};

pub(super) fn build_refresh_reserve(reserve: &ReserveInfo) -> Instruction {
    crate::instructions::refresh::refresh_reserve(RefreshReserveAccounts {
        reserve: reserve.address,
        lending_market: reserve.lending_market,
        pyth_oracle: reserve.pyth_oracle,
        switchboard_price_oracle: reserve.switchboard_price_oracle,
        switchboard_twap_oracle: reserve.switchboard_twap_oracle,
        scope_prices: reserve.scope_prices,
    })
}

pub(super) fn build_refresh_obligation_remaining_accounts(
    obligation: &ObligationInfo,
) -> Vec<AccountMeta> {
    let referrer_count = if obligation.referrer.is_some() {
        obligation.borrow_reserves.len()
    } else {
        0
    };
    let mut remaining = Vec::with_capacity(
        obligation.deposit_reserves.len() + obligation.borrow_reserves.len() + referrer_count,
    );

    for r in &obligation.deposit_reserves {
        remaining.push(writable(*r));
    }
    for r in &obligation.borrow_reserves {
        remaining.push(writable(*r));
    }
    if let Some(referrer) = obligation.referrer {
        for borrow_reserve in &obligation.borrow_reserves {
            let (rts, _) = pda::referrer_token_state(&KLEND_PROGRAM_ID, &referrer, borrow_reserve);
            remaining.push(writable(rts));
        }
    }

    remaining
}

pub(super) fn build_refresh_obligation(
    lending_market: &Pubkey,
    obligation: &ObligationInfo,
) -> Instruction {
    let remaining = build_refresh_obligation_remaining_accounts(obligation);
    crate::instructions::refresh::refresh_obligation(
        RefreshObligationAccounts {
            lending_market: *lending_market,
            obligation: obligation.address,
        },
        remaining,
    )
}

pub(super) fn build_deposit_reserves_remaining(obligation: &ObligationInfo) -> Vec<AccountMeta> {
    obligation
        .deposit_reserves
        .iter()
        .map(|r| writable(*r))
        .collect()
}

/// Build refresh instructions for all unique obligation reserves, skipping any
/// already-refreshed ones.
///
/// This ensures that `refresh_obligation` won't fail due to stale reserves in
/// multi-position obligations. Reserves not found in `obligation_reserves` are
/// silently skipped (the caller is responsible for providing all required
/// reserves via `obligation_reserves`).
pub(super) fn build_refresh_all_obligation_reserves(
    obligation: &ObligationInfo,
    obligation_reserves: &[ReserveInfo],
    already_refreshed: &[Pubkey],
) -> Vec<Instruction> {
    let mut seen =
        Vec::with_capacity(obligation.deposit_reserves.len() + obligation.borrow_reserves.len());
    let mut ixs = Vec::new();

    for key in obligation
        .deposit_reserves
        .iter()
        .chain(obligation.borrow_reserves.iter())
    {
        if already_refreshed.contains(key) || seen.contains(key) {
            continue;
        }
        seen.push(*key);

        if let Some(info) = obligation_reserves.iter().find(|r| r.address == *key) {
            ixs.push(build_refresh_reserve(info));
        }
    }

    ixs
}
