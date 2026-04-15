use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use super::info::ReserveInfo;
use crate::{
    instructions::flash::{
        flash_borrow_reserve_liquidity, flash_repay_reserve_liquidity,
        FlashBorrowReserveLiquidityAccounts, FlashRepayReserveLiquidityAccounts,
    },
    pda::{self, ReservePdas},
    KLEND_PROGRAM_ID,
};

/// Build instructions for a flash loan (borrow + repay pair).
///
/// The returned instructions must be placed in a single transaction. The caller
/// can insert arbitrary instructions between the borrow and repay.
///
/// `borrow_instruction_index` is the index of the flash-borrow instruction
/// within the final transaction (typically 0 when no other instructions precede it).
///
/// Returns: `(borrow_ix, repay_ix)` — a `flash_borrow` instruction and a
/// `flash_repay` instruction to append after user logic.
pub fn flash_loan(
    user: Pubkey,
    reserve: &ReserveInfo,
    user_source_liquidity: Pubkey,
    user_destination_liquidity: Pubkey,
    liquidity_amount: u64,
    borrow_instruction_index: u8,
    referrer: Option<Pubkey>,
) -> (Instruction, Instruction) {
    let pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &reserve.address);
    let (lma, _) = pda::lending_market_authority(&KLEND_PROGRAM_ID, &reserve.lending_market);

    let referrer_token_state =
        referrer.map(|r| pda::referrer_token_state(&KLEND_PROGRAM_ID, &r, &reserve.address).0);

    let borrow_ix = flash_borrow_reserve_liquidity(
        FlashBorrowReserveLiquidityAccounts {
            user_transfer_authority: user,
            lending_market_authority: lma,
            lending_market: reserve.lending_market,
            reserve: reserve.address,
            reserve_liquidity_mint: reserve.liquidity_mint,
            reserve_source_liquidity: pdas.liquidity_supply_vault,
            user_destination_liquidity,
            reserve_liquidity_fee_receiver: pdas.fee_vault,
            referrer_token_state,
            referrer_account: referrer,
            token_program: reserve.liquidity_token_program,
        },
        liquidity_amount,
    );

    let repay_ix = flash_repay_reserve_liquidity(
        FlashRepayReserveLiquidityAccounts {
            user_transfer_authority: user,
            lending_market_authority: lma,
            lending_market: reserve.lending_market,
            reserve: reserve.address,
            reserve_liquidity_mint: reserve.liquidity_mint,
            reserve_destination_liquidity: pdas.liquidity_supply_vault,
            user_source_liquidity,
            reserve_liquidity_fee_receiver: pdas.fee_vault,
            referrer_token_state,
            referrer_account: referrer,
            token_program: reserve.liquidity_token_program,
        },
        liquidity_amount,
        borrow_instruction_index,
    );

    (borrow_ix, repay_ix)
}
