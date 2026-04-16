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
    instructions::repay::{
        repay_and_withdraw_and_redeem, repay_obligation_liquidity_v2,
        RepayAndWithdrawAndRedeemAccounts, RepayObligationLiquidityV2Accounts,
    },
    pda::{self, ReservePdas},
    KLEND_PROGRAM_ID,
};

/// Build instructions to repay borrowed liquidity.
///
/// `obligation_reserves` should contain [`ReserveInfo`] for every deposit and
/// borrow reserve on the obligation.
///
/// Returns: `[refresh_other_reserves..., refresh_repay_reserve, refresh_obligation, repay_obligation_liquidity_v2]`
pub fn repay(
    owner: Pubkey,
    repay_reserve: &ReserveInfo,
    obligation: &ObligationInfo,
    obligation_reserves: &[ReserveInfo],
    user_source_liquidity: Pubkey,
    liquidity_amount: u64,
    farms: Option<&FarmsAccounts>,
) -> Vec<Instruction> {
    let pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &repay_reserve.address);
    let (lma, _) = pda::lending_market_authority(&KLEND_PROGRAM_ID, &repay_reserve.lending_market);

    let remaining = build_deposit_reserves_remaining(obligation);

    let mut ixs = build_refresh_all_obligation_reserves(
        obligation,
        obligation_reserves,
        &[repay_reserve.address],
    );
    ixs.push(build_refresh_reserve(repay_reserve));
    ixs.push(build_refresh_obligation(
        &repay_reserve.lending_market,
        obligation,
    ));
    ixs.push(repay_obligation_liquidity_v2(
        RepayObligationLiquidityV2Accounts {
            owner,
            obligation: obligation.address,
            lending_market: repay_reserve.lending_market,
            repay_reserve: repay_reserve.address,
            reserve_liquidity_mint: repay_reserve.liquidity_mint,
            reserve_destination_liquidity: pdas.liquidity_supply_vault,
            user_source_liquidity,
            token_program: repay_reserve.liquidity_token_program,
            obligation_farm_user_state: farms.map(|f| f.obligation_farm_user_state),
            reserve_farm_state: farms.map(|f| f.reserve_farm_state),
            lending_market_authority: lma,
        },
        liquidity_amount,
        remaining,
    ));

    ixs
}

/// Build instructions to atomically repay a borrow, withdraw collateral, and
/// redeem it for the underlying liquidity — all in a single instruction.
///
/// This is the "close position" flow: repay debt on `repay_reserve`, then
/// withdraw + redeem collateral from `withdraw_reserve`.
///
/// `obligation_reserves` should contain [`ReserveInfo`] for every deposit and
/// borrow reserve on the obligation.
///
/// Returns: `[refresh_other_reserves..., refresh_repay_reserve, refresh_withdraw_reserve,
///            refresh_obligation, repay_and_withdraw_and_redeem]`
#[allow(clippy::too_many_arguments)]
pub fn repay_and_withdraw(
    owner: Pubkey,
    repay_reserve: &ReserveInfo,
    withdraw_reserve: &ReserveInfo,
    obligation: &ObligationInfo,
    obligation_reserves: &[ReserveInfo],
    user_source_liquidity: Pubkey,
    user_destination_liquidity: Pubkey,
    repay_amount: u64,
    withdraw_collateral_amount: u64,
    collateral_farms: Option<&FarmsAccounts>,
    debt_farms: Option<&FarmsAccounts>,
) -> Vec<Instruction> {
    let (lma, _) = pda::lending_market_authority(&KLEND_PROGRAM_ID, &repay_reserve.lending_market);
    let repay_pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &repay_reserve.address);
    let withdraw_pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &withdraw_reserve.address);

    let remaining = build_deposit_reserves_remaining(obligation);

    let mut ixs = build_refresh_all_obligation_reserves(
        obligation,
        obligation_reserves,
        &[repay_reserve.address, withdraw_reserve.address],
    );
    ixs.push(build_refresh_reserve(repay_reserve));
    ixs.push(build_refresh_reserve(withdraw_reserve));
    ixs.push(build_refresh_obligation(
        &repay_reserve.lending_market,
        obligation,
    ));
    ixs.push(repay_and_withdraw_and_redeem(
        RepayAndWithdrawAndRedeemAccounts {
            owner,
            obligation: obligation.address,
            lending_market: repay_reserve.lending_market,
            repay_reserve: repay_reserve.address,
            reserve_liquidity_mint: repay_reserve.liquidity_mint,
            reserve_destination_liquidity: repay_pdas.liquidity_supply_vault,
            user_source_liquidity,
            token_program: repay_reserve.liquidity_token_program,
            lending_market_authority: lma,
            withdraw_reserve: withdraw_reserve.address,
            withdraw_reserve_liquidity_mint: withdraw_reserve.liquidity_mint,
            withdraw_reserve_source_collateral: withdraw_pdas.collateral_supply_vault,
            withdraw_reserve_collateral_mint: withdraw_pdas.collateral_mint,
            withdraw_reserve_liquidity_supply: withdraw_pdas.liquidity_supply_vault,
            user_destination_liquidity,
            placeholder_user_destination_collateral: None,
            withdraw_liquidity_token_program: withdraw_reserve.liquidity_token_program,
            collateral_obligation_farm_user_state: collateral_farms
                .map(|f| f.obligation_farm_user_state),
            collateral_reserve_farm_state: collateral_farms.map(|f| f.reserve_farm_state),
            debt_obligation_farm_user_state: debt_farms.map(|f| f.obligation_farm_user_state),
            debt_reserve_farm_state: debt_farms.map(|f| f.reserve_farm_state),
        },
        repay_amount,
        withdraw_collateral_amount,
        remaining,
    ));

    ixs
}
