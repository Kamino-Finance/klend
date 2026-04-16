use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use super::{
    common::build_refresh_reserve,
    info::{CallbackAccounts, ReserveInfo},
};
use crate::{
    instructions::withdraw_queue::{
        CancelWithdrawTicketAccounts, EnqueueToWithdrawAccounts, WithdrawQueuedLiquidityAccounts,
    },
    pda::{self, ReservePdas},
    types, KLEND_PROGRAM_ID, TOKEN_PROGRAM_ID,
};

/// Build instructions to enqueue collateral for delayed withdrawal.
///
/// Used for reserves with withdrawal queues. The `ticket_sequence_number` is
/// the next sequence number from the reserve's withdraw queue state.
///
/// Returns: `[refresh_reserve, enqueue_to_withdraw]`
pub fn enqueue_to_withdraw(
    owner: Pubkey,
    reserve: &ReserveInfo,
    user_source_collateral: Pubkey,
    user_destination_liquidity: Pubkey,
    collateral_amount: u64,
    ticket_sequence_number: u64,
    callback: Option<&CallbackAccounts>,
) -> Vec<Instruction> {
    let pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &reserve.address);
    let (lma, _) = pda::lending_market_authority(&KLEND_PROGRAM_ID, &reserve.lending_market);
    let (ticket, _) =
        pda::withdraw_ticket(&KLEND_PROGRAM_ID, &reserve.address, ticket_sequence_number);
    let (queued_vault, _) =
        pda::owner_queued_collateral_vault(&KLEND_PROGRAM_ID, &reserve.address, &owner);

    let progress_callback_type = callback
        .map(|c| c.progress_callback_type)
        .unwrap_or(types::ProgressCallbackType::None);

    vec![
        build_refresh_reserve(reserve),
        crate::instructions::withdraw_queue::enqueue_to_withdraw(
            EnqueueToWithdrawAccounts {
                owner,
                lending_market: reserve.lending_market,
                lending_market_authority: lma,
                reserve: reserve.address,
                user_source_collateral_ta: user_source_collateral,
                user_destination_liquidity_ta: user_destination_liquidity,
                reserve_liquidity_mint: reserve.liquidity_mint,
                reserve_collateral_mint: pdas.collateral_mint,
                withdraw_ticket: ticket,
                owner_queued_collateral_vault: queued_vault,
                progress_callback_custom_account_0: callback.and_then(|c| c.custom_account_0),
                progress_callback_custom_account_1: callback.and_then(|c| c.custom_account_1),
            },
            collateral_amount,
            progress_callback_type,
        ),
    ]
}

/// Build instructions to execute a queued withdrawal (redeem a withdraw ticket).
///
/// The `payer` cranks the withdrawal; the ticket owner receives the liquidity.
///
/// Returns: `[refresh_reserve, withdraw_queued_liquidity]`
pub fn withdraw_queued_liquidity(
    payer: Pubkey,
    reserve: &ReserveInfo,
    ticket_owner: Pubkey,
    user_destination_liquidity: Pubkey,
    ticket_sequence_number: u64,
    callback: Option<&CallbackAccounts>,
) -> Vec<Instruction> {
    let pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &reserve.address);
    let (lma, _) = pda::lending_market_authority(&KLEND_PROGRAM_ID, &reserve.lending_market);
    let (ticket, _) =
        pda::withdraw_ticket(&KLEND_PROGRAM_ID, &reserve.address, ticket_sequence_number);
    let (queued_vault, _) =
        pda::owner_queued_collateral_vault(&KLEND_PROGRAM_ID, &reserve.address, &ticket_owner);

    let progress_callback_program = callback.and_then(|c| {
        if c.progress_callback_type != types::ProgressCallbackType::None {
            Some(c.progress_callback_type.program_address())
        } else {
            None
        }
    });

    vec![
        build_refresh_reserve(reserve),
        crate::instructions::withdraw_queue::withdraw_queued_liquidity(
            WithdrawQueuedLiquidityAccounts {
                payer,
                lending_market: reserve.lending_market,
                lending_market_authority: lma,
                reserve: reserve.address,
                reserve_liquidity_mint: reserve.liquidity_mint,
                reserve_collateral_mint: pdas.collateral_mint,
                reserve_liquidity_supply: pdas.liquidity_supply_vault,
                owner_queued_collateral_vault: queued_vault,
                user_destination_liquidity,
                liquidity_token_program: reserve.liquidity_token_program,
                withdraw_ticket: ticket,
                withdraw_ticket_owner: ticket_owner,
                progress_callback_program,
                progress_callback_custom_account_0: callback.and_then(|c| c.custom_account_0),
                progress_callback_custom_account_1: callback.and_then(|c| c.custom_account_1),
            },
        ),
    ]
}

/// Build instructions to cancel a queued withdrawal and recover ctokens.
///
/// Returns: `[refresh_reserve, cancel_withdraw_ticket]`
pub fn cancel_withdraw_ticket(
    owner: Pubkey,
    reserve: &ReserveInfo,
    user_destination_collateral: Pubkey,
    ticket_sequence_number: u64,
    collateral_amount_to_cancel: u64,
) -> Vec<Instruction> {
    let pdas = ReservePdas::derive(&KLEND_PROGRAM_ID, &reserve.address);
    let (lma, _) = pda::lending_market_authority(&KLEND_PROGRAM_ID, &reserve.lending_market);
    let (ticket, _) =
        pda::withdraw_ticket(&KLEND_PROGRAM_ID, &reserve.address, ticket_sequence_number);
    let (queued_vault, _) =
        pda::owner_queued_collateral_vault(&KLEND_PROGRAM_ID, &reserve.address, &owner);

    vec![
        build_refresh_reserve(reserve),
        crate::instructions::withdraw_queue::cancel_withdraw_ticket(
            CancelWithdrawTicketAccounts {
                owner,
                lending_market: reserve.lending_market,
                lending_market_authority: lma,
                reserve: reserve.address,
                reserve_collateral_mint: pdas.collateral_mint,
                owner_queued_collateral_vault: queued_vault,
                user_destination_collateral,
                collateral_token_program: TOKEN_PROGRAM_ID,
                withdraw_ticket: ticket,
            },
            ticket_sequence_number,
            collateral_amount_to_cancel,
        ),
    ]
}
