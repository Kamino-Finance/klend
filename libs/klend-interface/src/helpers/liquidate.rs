use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use super::{
    common::{
        build_refresh_all_obligation_reserves, build_refresh_obligation,
        build_refresh_obligation_remaining_accounts, build_refresh_reserve,
    },
    info::{FarmsAccounts, ObligationInfo, ReserveInfo},
};
use crate::{
    instructions::liquidate::{
        liquidate_obligation_and_redeem_reserve_collateral_v2,
        LiquidateObligationAndRedeemReserveCollateralV2Accounts,
    },
    pda::{self, ReservePdas},
    KLEND_PROGRAM_ID,
};

/// Build instructions to liquidate an undercollateralized obligation.
///
/// The liquidator repays debt on `repay_reserve` and receives collateral
/// from `withdraw_reserve`. Both reserves and the obligation are refreshed
/// automatically.
///
/// `obligation_reserves` should contain [`ReserveInfo`] for every deposit and
/// borrow reserve on the obligation.
///
/// Returns: `[refresh_other_reserves..., refresh_repay_reserve, refresh_withdraw_reserve,
///            refresh_obligation, liquidate_and_redeem_v2]`
#[allow(clippy::too_many_arguments)]
pub fn liquidate(
    liquidator: Pubkey,
    repay_reserve: &ReserveInfo,
    withdraw_reserve: &ReserveInfo,
    obligation: &ObligationInfo,
    obligation_reserves: &[ReserveInfo],
    user_source_liquidity: Pubkey,
    user_destination_collateral: Pubkey,
    user_destination_liquidity: Pubkey,
    liquidity_amount: u64,
    min_acceptable_received_liquidity_amount: u64,
    max_allowed_ltv_override_percent: u64,
    collateral_farms: Option<&FarmsAccounts>,
    debt_farms: Option<&FarmsAccounts>,
) -> Vec<Instruction> {
    let (lma, _) = pda::lending_market_authority(&KLEND_PROGRAM_ID, &repay_reserve.lending_market);
    let repay_pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &repay_reserve.address);
    let withdraw_pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &withdraw_reserve.address);

    let remaining = build_refresh_obligation_remaining_accounts(obligation);

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
    ixs.push(liquidate_obligation_and_redeem_reserve_collateral_v2(
        LiquidateObligationAndRedeemReserveCollateralV2Accounts {
            liquidator,
            obligation: obligation.address,
            lending_market: repay_reserve.lending_market,
            lending_market_authority: lma,
            repay_reserve: repay_reserve.address,
            repay_reserve_liquidity_mint: repay_reserve.liquidity_mint,
            repay_reserve_liquidity_supply: repay_pdas.liquidity_supply_vault,
            withdraw_reserve: withdraw_reserve.address,
            withdraw_reserve_liquidity_mint: withdraw_reserve.liquidity_mint,
            withdraw_reserve_collateral_mint: withdraw_pdas.collateral_mint,
            withdraw_reserve_collateral_supply: withdraw_pdas.collateral_supply_vault,
            withdraw_reserve_liquidity_supply: withdraw_pdas.liquidity_supply_vault,
            withdraw_reserve_liquidity_fee_receiver: withdraw_pdas.fee_vault,
            user_source_liquidity,
            user_destination_collateral,
            user_destination_liquidity,
            repay_liquidity_token_program: repay_reserve.liquidity_token_program,
            withdraw_liquidity_token_program: withdraw_reserve.liquidity_token_program,
            collateral_obligation_farm_user_state: collateral_farms
                .map(|f| f.obligation_farm_user_state),
            collateral_reserve_farm_state: collateral_farms.map(|f| f.reserve_farm_state),
            debt_obligation_farm_user_state: debt_farms.map(|f| f.obligation_farm_user_state),
            debt_reserve_farm_state: debt_farms.map(|f| f.reserve_farm_state),
        },
        liquidity_amount,
        min_acceptable_received_liquidity_amount,
        max_allowed_ltv_override_percent,
        remaining,
    ));

    ixs
}
