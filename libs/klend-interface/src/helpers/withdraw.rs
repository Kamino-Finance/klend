use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use super::{
    common::{
        build_refresh_all_obligation_reserves, build_refresh_obligation, build_refresh_reserve,
    },
    info::{FarmsAccounts, ObligationInfo, ReserveInfo},
};
use crate::{
    instructions::withdraw::{
        redeem_reserve_collateral, withdraw_obligation_collateral_and_redeem_reserve_collateral_v2,
        withdraw_obligation_collateral_v2, RedeemReserveCollateralAccounts,
        WithdrawObligationCollateralAndRedeemReserveCollateralV2Accounts,
        WithdrawObligationCollateralV2Accounts,
    },
    pda::{self, ReservePdas},
    KLEND_PROGRAM_ID,
};

/// Build instructions to withdraw collateral from an obligation **and** redeem
/// it for the underlying liquidity tokens.
///
/// `obligation_reserves` should contain [`ReserveInfo`] for every deposit and
/// borrow reserve on the obligation. Any reserves not already refreshed by this
/// helper are refreshed automatically so that `refresh_obligation` succeeds.
///
/// Returns: `[refresh_other_reserves..., refresh_withdraw_reserve, refresh_obligation, withdraw_and_redeem]`
pub fn withdraw(
    owner: Pubkey,
    withdraw_reserve: &ReserveInfo,
    obligation: &ObligationInfo,
    obligation_reserves: &[ReserveInfo],
    user_destination_liquidity: Pubkey,
    collateral_amount: u64,
    farms: Option<&FarmsAccounts>,
) -> Vec<Instruction> {
    let pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &withdraw_reserve.address);
    let (lma, _) =
        pda::lending_market_authority(&KLEND_PROGRAM_ID, &withdraw_reserve.lending_market);

    let mut ixs = build_refresh_all_obligation_reserves(
        obligation,
        obligation_reserves,
        &[withdraw_reserve.address],
    );
    ixs.push(build_refresh_reserve(withdraw_reserve));
    ixs.push(build_refresh_obligation(
        &withdraw_reserve.lending_market,
        obligation,
    ));
    ixs.push(
        withdraw_obligation_collateral_and_redeem_reserve_collateral_v2(
            WithdrawObligationCollateralAndRedeemReserveCollateralV2Accounts {
                owner,
                obligation: obligation.address,
                lending_market: withdraw_reserve.lending_market,
                lending_market_authority: lma,
                withdraw_reserve: withdraw_reserve.address,
                reserve_liquidity_mint: withdraw_reserve.liquidity_mint,
                reserve_source_collateral: pdas.collateral_supply_vault,
                reserve_collateral_mint: pdas.collateral_mint,
                reserve_liquidity_supply: pdas.liquidity_supply_vault,
                user_destination_liquidity,
                placeholder_user_destination_collateral: None,
                liquidity_token_program: withdraw_reserve.liquidity_token_program,
                obligation_farm_user_state: farms.map(|f| f.obligation_farm_user_state),
                reserve_farm_state: farms.map(|f| f.reserve_farm_state),
            },
            collateral_amount,
        ),
    );

    ixs
}

/// Build instructions to withdraw collateral (cTokens) from an obligation
/// **without** redeeming them for liquidity.
///
/// `obligation_reserves` should contain [`ReserveInfo`] for every deposit and
/// borrow reserve on the obligation.
///
/// Returns: `[refresh_other_reserves..., refresh_withdraw_reserve, refresh_obligation, withdraw_obligation_collateral_v2]`
pub fn withdraw_collateral(
    owner: Pubkey,
    withdraw_reserve: &ReserveInfo,
    obligation: &ObligationInfo,
    obligation_reserves: &[ReserveInfo],
    user_destination_collateral: Pubkey,
    collateral_amount: u64,
    farms: Option<&FarmsAccounts>,
) -> Vec<Instruction> {
    let pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &withdraw_reserve.address);
    let (lma, _) =
        pda::lending_market_authority(&KLEND_PROGRAM_ID, &withdraw_reserve.lending_market);

    let mut ixs = build_refresh_all_obligation_reserves(
        obligation,
        obligation_reserves,
        &[withdraw_reserve.address],
    );
    ixs.push(build_refresh_reserve(withdraw_reserve));
    ixs.push(build_refresh_obligation(
        &withdraw_reserve.lending_market,
        obligation,
    ));
    ixs.push(withdraw_obligation_collateral_v2(
        WithdrawObligationCollateralV2Accounts {
            owner,
            obligation: obligation.address,
            lending_market: withdraw_reserve.lending_market,
            lending_market_authority: lma,
            withdraw_reserve: withdraw_reserve.address,
            reserve_source_collateral: pdas.collateral_supply_vault,
            user_destination_collateral,
            obligation_farm_user_state: farms.map(|f| f.obligation_farm_user_state),
            reserve_farm_state: farms.map(|f| f.reserve_farm_state),
        },
        collateral_amount,
    ));

    ixs
}

/// Build instructions to redeem cTokens for underlying liquidity (no obligation).
///
/// Returns: `[refresh_reserve, redeem_reserve_collateral]`
pub fn redeem(
    owner: Pubkey,
    reserve: &ReserveInfo,
    user_source_collateral: Pubkey,
    user_destination_liquidity: Pubkey,
    collateral_amount: u64,
) -> Vec<Instruction> {
    let pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &reserve.address);
    let (lma, _) = pda::lending_market_authority(&KLEND_PROGRAM_ID, &reserve.lending_market);

    vec![
        build_refresh_reserve(reserve),
        redeem_reserve_collateral(
            RedeemReserveCollateralAccounts {
                owner,
                lending_market: reserve.lending_market,
                reserve: reserve.address,
                lending_market_authority: lma,
                reserve_liquidity_mint: reserve.liquidity_mint,
                reserve_collateral_mint: pdas.collateral_mint,
                reserve_liquidity_supply: pdas.liquidity_supply_vault,
                user_source_collateral,
                user_destination_liquidity,
                liquidity_token_program: reserve.liquidity_token_program,
            },
            collateral_amount,
        ),
    ]
}
