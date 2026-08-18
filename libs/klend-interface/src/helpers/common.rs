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
    obligation_reserves: &[ReserveInfo],
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
        // `refresh_obligation` consumes one referrer token state per borrow that accrues
        // referral fees, i.e. whose reserve takes protocol fees (in borrow order, and only when
        // the market's referral fee is on); the token states of the remaining borrows are
        // appended after them, only padding the expected account count. Borrow reserves not
        // found in `obligation_reserves` are assumed to take protocol fees.
        let mut padding = Vec::new();
        for borrow_reserve in &obligation.borrow_reserves {
            let (rts, _) = pda::referrer_token_state(&KLEND_PROGRAM_ID, &referrer, borrow_reserve);
            let takes_protocol_fees = obligation_reserves
                .iter()
                .find(|r| r.address == *borrow_reserve)
                .map_or(true, |r| r.protocol_take_rate_pct > 0);
            if takes_protocol_fees {
                remaining.push(writable(rts));
            } else {
                padding.push(writable(rts));
            }
        }
        remaining.extend(padding);
    }

    remaining
}

pub(super) fn build_refresh_obligation(
    lending_market: &Pubkey,
    obligation: &ObligationInfo,
    obligation_reserves: &[ReserveInfo],
) -> Instruction {
    let remaining = build_refresh_obligation_remaining_accounts(obligation, obligation_reserves);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn reserve_info(address: Pubkey, protocol_take_rate_pct: u8) -> ReserveInfo {
        ReserveInfo {
            address,
            lending_market: Pubkey::new_unique(),
            liquidity_mint: Pubkey::new_unique(),
            liquidity_token_program: Pubkey::new_unique(),
            pyth_oracle: None,
            switchboard_price_oracle: None,
            switchboard_twap_oracle: None,
            scope_prices: None,
            protocol_take_rate_pct,
        }
    }

    fn rts(referrer: &Pubkey, reserve: &Pubkey) -> Pubkey {
        pda::referrer_token_state(&KLEND_PROGRAM_ID, referrer, reserve).0
    }

    fn tail_of(remaining: &[AccountMeta], obligation: &ObligationInfo) -> Vec<Pubkey> {
        let reserves_count = obligation.deposit_reserves.len() + obligation.borrow_reserves.len();
        remaining[reserves_count..]
            .iter()
            .map(|m| m.pubkey)
            .collect()
    }

    #[test]
    fn test_no_referrer_no_token_state_tail() {
        let obligation = ObligationInfo {
            address: Pubkey::new_unique(),
            deposit_reserves: vec![Pubkey::new_unique()],
            borrow_reserves: vec![Pubkey::new_unique()],
            referrer: None,
        };
        let remaining = build_refresh_obligation_remaining_accounts(&obligation, &[]);
        assert_eq!(remaining.len(), 2);
    }

    #[test]
    fn test_referrer_token_states_one_per_borrow_in_order() {
        let referrer = Pubkey::new_unique();
        let borrows = vec![Pubkey::new_unique(), Pubkey::new_unique()];
        let obligation = ObligationInfo {
            address: Pubkey::new_unique(),
            deposit_reserves: vec![Pubkey::new_unique()],
            borrow_reserves: borrows.clone(),
            referrer: Some(referrer),
        };
        let reserves = [reserve_info(borrows[0], 10), reserve_info(borrows[1], 10)];
        let remaining = build_refresh_obligation_remaining_accounts(&obligation, &reserves);
        assert_eq!(
            tail_of(&remaining, &obligation),
            vec![rts(&referrer, &borrows[0]), rts(&referrer, &borrows[1])]
        );
    }

    #[test]
    fn test_referrer_token_states_zero_take_rate_reserves_padded_last() {
        let referrer = Pubkey::new_unique();
        let borrows = vec![
            Pubkey::new_unique(), // zero take rate -> padding
            Pubkey::new_unique(), // takes fees -> consumed
            Pubkey::new_unique(), // not in `obligation_reserves` -> assumed to take fees
        ];
        let obligation = ObligationInfo {
            address: Pubkey::new_unique(),
            deposit_reserves: vec![],
            borrow_reserves: borrows.clone(),
            referrer: Some(referrer),
        };
        let reserves = [reserve_info(borrows[0], 0), reserve_info(borrows[1], 10)];
        let remaining = build_refresh_obligation_remaining_accounts(&obligation, &reserves);
        assert_eq!(
            tail_of(&remaining, &obligation),
            vec![
                rts(&referrer, &borrows[1]),
                rts(&referrer, &borrows[2]),
                rts(&referrer, &borrows[0]),
            ]
        );
    }
}
