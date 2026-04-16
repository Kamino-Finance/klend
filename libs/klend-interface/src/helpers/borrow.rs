use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use super::{
    common::{
        build_deposit_reserves_remaining, build_refresh_all_obligation_reserves,
        build_refresh_obligation, build_refresh_reserve,
    },
    info::{FarmsAccounts, ObligationInfo, ReserveInfo},
};
use crate::{
    instructions::borrow::{
        borrow_obligation_liquidity_v2, rollover_fixed_term_borrow as rollover_ix,
        BorrowObligationLiquidityV2Accounts, RolloverFixedTermBorrowAccounts,
    },
    pda::{self, ReservePdas},
    KLEND_PROGRAM_ID,
};

/// Build instructions to borrow liquidity from a reserve against an obligation.
///
/// `obligation_reserves` should contain [`ReserveInfo`] for every deposit and
/// borrow reserve on the obligation. Any reserves not already refreshed by this
/// helper are refreshed automatically so that `refresh_obligation` succeeds.
///
/// Returns: `[refresh_other_reserves..., refresh_borrow_reserve, refresh_obligation, borrow_obligation_liquidity_v2]`
pub fn borrow(
    owner: Pubkey,
    borrow_reserve: &ReserveInfo,
    obligation: &ObligationInfo,
    obligation_reserves: &[ReserveInfo],
    user_destination_liquidity: Pubkey,
    liquidity_amount: u64,
    farms: Option<&FarmsAccounts>,
) -> Vec<Instruction> {
    let pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &borrow_reserve.address);
    let (lma, _) = pda::lending_market_authority(&KLEND_PROGRAM_ID, &borrow_reserve.lending_market);

    let referrer_token_state = obligation.referrer.map(|referrer| {
        pda::referrer_token_state(&KLEND_PROGRAM_ID, &referrer, &borrow_reserve.address).0
    });

    let remaining = build_deposit_reserves_remaining(obligation);

    let mut ixs = build_refresh_all_obligation_reserves(
        obligation,
        obligation_reserves,
        &[borrow_reserve.address],
    );
    ixs.push(build_refresh_reserve(borrow_reserve));
    ixs.push(build_refresh_obligation(
        &borrow_reserve.lending_market,
        obligation,
    ));
    ixs.push(borrow_obligation_liquidity_v2(
        BorrowObligationLiquidityV2Accounts {
            owner,
            obligation: obligation.address,
            lending_market: borrow_reserve.lending_market,
            lending_market_authority: lma,
            borrow_reserve: borrow_reserve.address,
            borrow_reserve_liquidity_mint: borrow_reserve.liquidity_mint,
            reserve_source_liquidity: pdas.liquidity_supply_vault,
            borrow_reserve_liquidity_fee_receiver: pdas.fee_vault,
            user_destination_liquidity,
            referrer_token_state,
            token_program: borrow_reserve.liquidity_token_program,
            obligation_farm_user_state: farms.map(|f| f.obligation_farm_user_state),
            reserve_farm_state: farms.map(|f| f.reserve_farm_state),
        },
        liquidity_amount,
        remaining,
    ));

    ixs
}

/// Build instructions to roll over a fixed-term borrow into a new reserve (or same reserve).
///
/// `obligation_reserves` should contain [`ReserveInfo`] for every deposit and
/// borrow reserve on the obligation. The source and target reserves will be
/// refreshed automatically.
///
/// Returns: `[refresh_other_reserves..., refresh_source, refresh_target (if different), refresh_obligation, rollover_fixed_term_borrow]`
pub fn rollover_fixed_term_borrow(
    payer: Pubkey,
    obligation: &ObligationInfo,
    obligation_reserves: &[ReserveInfo],
    source_reserve: &ReserveInfo,
    target_reserve: &ReserveInfo,
    source_farms: Option<&FarmsAccounts>,
    target_farms: Option<&FarmsAccounts>,
) -> Vec<Instruction> {
    let (lma, _) = pda::lending_market_authority(&KLEND_PROGRAM_ID, &source_reserve.lending_market);
    let source_pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &source_reserve.address);
    let target_pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &target_reserve.address);

    let mut already_refreshed = vec![source_reserve.address];
    let same_reserve = source_reserve.address == target_reserve.address;
    if !same_reserve {
        already_refreshed.push(target_reserve.address);
    }

    let mut ixs =
        build_refresh_all_obligation_reserves(obligation, obligation_reserves, &already_refreshed);
    ixs.push(build_refresh_reserve(source_reserve));
    if !same_reserve {
        ixs.push(build_refresh_reserve(target_reserve));
    }
    ixs.push(build_refresh_obligation(
        &source_reserve.lending_market,
        obligation,
    ));

    ixs.push(rollover_ix(RolloverFixedTermBorrowAccounts {
        payer,
        obligation: obligation.address,
        lending_market: source_reserve.lending_market,
        lending_market_authority: lma,
        source_borrow_reserve: source_reserve.address,
        target_borrow_reserve: target_reserve.address,
        liquidity_mint: source_reserve.liquidity_mint,
        source_borrow_reserve_liquidity: source_pdas.liquidity_supply_vault,
        target_borrow_reserve_liquidity: target_pdas.liquidity_supply_vault,
        token_program: source_reserve.liquidity_token_program,
        source_obligation_farm_user_state: source_farms.map(|f| f.obligation_farm_user_state),
        source_reserve_farm_state: source_farms.map(|f| f.reserve_farm_state),
        target_obligation_farm_user_state: target_farms.map(|f| f.obligation_farm_user_state),
        target_reserve_farm_state: target_farms.map(|f| f.reserve_farm_state),
    }));

    ixs
}
