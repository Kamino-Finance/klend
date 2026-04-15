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
    pda::{self, ReservePdas},
    KLEND_PROGRAM_ID,
};

/// Build instructions to atomically deposit to one reserve and withdraw from
/// another within the same obligation (rebalancing).
///
/// `obligation_reserves` should contain [`ReserveInfo`] for every deposit and
/// borrow reserve on the obligation.
///
/// Returns: `[refresh_other_reserves..., refresh_deposit_reserve, refresh_withdraw_reserve,
///            refresh_obligation, deposit_and_withdraw]`
#[allow(clippy::too_many_arguments)]
pub fn deposit_and_withdraw(
    owner: Pubkey,
    deposit_reserve: &ReserveInfo,
    withdraw_reserve: &ReserveInfo,
    obligation: &ObligationInfo,
    obligation_reserves: &[ReserveInfo],
    user_source_liquidity: Pubkey,
    user_destination_liquidity: Pubkey,
    deposit_liquidity_amount: u64,
    withdraw_collateral_amount: u64,
    deposit_farms: Option<&FarmsAccounts>,
    withdraw_farms: Option<&FarmsAccounts>,
) -> Vec<Instruction> {
    let (lma, _) =
        pda::lending_market_authority(&KLEND_PROGRAM_ID, &deposit_reserve.lending_market);
    let deposit_pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &deposit_reserve.address);
    let withdraw_pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &withdraw_reserve.address);
    let (withdraw_lma, _) =
        pda::lending_market_authority(&KLEND_PROGRAM_ID, &withdraw_reserve.lending_market);

    let remaining = build_deposit_reserves_remaining(obligation);

    let mut ixs = build_refresh_all_obligation_reserves(
        obligation,
        obligation_reserves,
        &[deposit_reserve.address, withdraw_reserve.address],
    );
    ixs.push(build_refresh_reserve(deposit_reserve));
    ixs.push(build_refresh_reserve(withdraw_reserve));
    ixs.push(build_refresh_obligation(
        &deposit_reserve.lending_market,
        obligation,
    ));
    ixs.push(crate::instructions::compound::deposit_and_withdraw(
        crate::instructions::compound::DepositAndWithdrawAccounts {
            owner,
            obligation: obligation.address,
            lending_market: deposit_reserve.lending_market,
            lending_market_authority: lma,
            reserve: deposit_reserve.address,
            reserve_liquidity_mint: deposit_reserve.liquidity_mint,
            reserve_liquidity_supply: deposit_pdas.liquidity_supply_vault,
            reserve_collateral_mint: deposit_pdas.collateral_mint,
            reserve_destination_deposit_collateral: deposit_pdas.collateral_supply_vault,
            user_source_liquidity,
            placeholder_user_destination_collateral: None,
            liquidity_token_program: deposit_reserve.liquidity_token_program,
            withdraw_owner: owner,
            withdraw_obligation: obligation.address,
            withdraw_lending_market: withdraw_reserve.lending_market,
            withdraw_lending_market_authority: withdraw_lma,
            withdraw_reserve: withdraw_reserve.address,
            withdraw_reserve_liquidity_mint: withdraw_reserve.liquidity_mint,
            withdraw_reserve_source_collateral: withdraw_pdas.collateral_supply_vault,
            withdraw_reserve_collateral_mint: withdraw_pdas.collateral_mint,
            withdraw_reserve_liquidity_supply: withdraw_pdas.liquidity_supply_vault,
            withdraw_user_destination_liquidity: user_destination_liquidity,
            withdraw_placeholder_user_destination_collateral: None,
            withdraw_liquidity_token_program: withdraw_reserve.liquidity_token_program,
            deposit_obligation_farm_user_state: deposit_farms.map(|f| f.obligation_farm_user_state),
            deposit_reserve_farm_state: deposit_farms.map(|f| f.reserve_farm_state),
            withdraw_obligation_farm_user_state: withdraw_farms
                .map(|f| f.obligation_farm_user_state),
            withdraw_reserve_farm_state: withdraw_farms.map(|f| f.reserve_farm_state),
        },
        deposit_liquidity_amount,
        withdraw_collateral_amount,
        remaining,
    ));

    ixs
}
