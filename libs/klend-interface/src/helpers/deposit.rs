use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use super::{
    common::{
        build_refresh_all_obligation_reserves, build_refresh_obligation, build_refresh_reserve,
    },
    info::{FarmsAccounts, ObligationInfo, ReserveInfo},
};
use crate::{
    instructions::deposit::{
        deposit_reserve_liquidity, deposit_reserve_liquidity_and_obligation_collateral_v2,
        DepositReserveLiquidityAccounts, DepositReserveLiquidityAndObligationCollateralV2Accounts,
    },
    pda::{self, ReservePdas},
    KLEND_PROGRAM_ID,
};

/// Build instructions to deposit liquidity into a reserve and receive cTokens.
///
/// No obligation involved — the user receives cTokens directly.
///
/// Returns: `[refresh_reserve, deposit_reserve_liquidity]`
pub fn deposit(
    owner: Pubkey,
    reserve: &ReserveInfo,
    user_source_liquidity: Pubkey,
    user_destination_collateral: Pubkey,
    liquidity_amount: u64,
) -> Vec<Instruction> {
    let pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &reserve.address);
    let (lma, _) = pda::lending_market_authority(&KLEND_PROGRAM_ID, &reserve.lending_market);

    vec![
        build_refresh_reserve(reserve),
        deposit_reserve_liquidity(
            DepositReserveLiquidityAccounts {
                owner,
                reserve: reserve.address,
                lending_market: reserve.lending_market,
                lending_market_authority: lma,
                reserve_liquidity_mint: reserve.liquidity_mint,
                reserve_liquidity_supply: pdas.liquidity_supply_vault,
                reserve_collateral_mint: pdas.collateral_mint,
                user_source_liquidity,
                user_destination_collateral,
                liquidity_token_program: reserve.liquidity_token_program,
            },
            liquidity_amount,
        ),
    ]
}

/// Build instructions to deposit liquidity into a reserve and credit the
/// resulting collateral to an obligation.
///
/// `obligation_reserves` should contain [`ReserveInfo`] for every deposit and
/// borrow reserve on the obligation.
///
/// Returns: `[refresh_other_reserves..., refresh_reserve, refresh_obligation, deposit_and_collateral_v2]`
pub fn deposit_to_obligation(
    owner: Pubkey,
    reserve: &ReserveInfo,
    obligation: &ObligationInfo,
    obligation_reserves: &[ReserveInfo],
    user_source_liquidity: Pubkey,
    liquidity_amount: u64,
    farms: Option<&FarmsAccounts>,
) -> Vec<Instruction> {
    let pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &reserve.address);
    let (lma, _) = pda::lending_market_authority(&KLEND_PROGRAM_ID, &reserve.lending_market);

    let mut ixs =
        build_refresh_all_obligation_reserves(obligation, obligation_reserves, &[reserve.address]);
    ixs.push(build_refresh_reserve(reserve));
    ixs.push(build_refresh_obligation(
        &reserve.lending_market,
        obligation,
    ));
    ixs.push(deposit_reserve_liquidity_and_obligation_collateral_v2(
        DepositReserveLiquidityAndObligationCollateralV2Accounts {
            owner,
            obligation: obligation.address,
            lending_market: reserve.lending_market,
            lending_market_authority: lma,
            reserve: reserve.address,
            reserve_liquidity_mint: reserve.liquidity_mint,
            reserve_liquidity_supply: pdas.liquidity_supply_vault,
            reserve_collateral_mint: pdas.collateral_mint,
            reserve_destination_deposit_collateral: pdas.collateral_supply_vault,
            user_source_liquidity,
            placeholder_user_destination_collateral: None,
            liquidity_token_program: reserve.liquidity_token_program,
            obligation_farm_user_state: farms.map(|f| f.obligation_farm_user_state),
            reserve_farm_state: farms.map(|f| f.reserve_farm_state),
        },
        liquidity_amount,
    ));

    ixs
}
