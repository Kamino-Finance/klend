use borsh::BorshSerialize;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use crate::{
    discriminators, util::*, ASSOCIATED_TOKEN_PROGRAM_ID, KLEND_PROGRAM_ID, SYSTEM_PROGRAM_ID,
    SYSVAR_INSTRUCTIONS_ID, TOKEN_PROGRAM_ID,
};

// ---------------------------------------------------------------------------
// enqueue_to_withdraw
// ---------------------------------------------------------------------------

pub struct EnqueueToWithdrawAccounts {
    pub owner: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub reserve: Pubkey,
    pub user_source_collateral_ta: Pubkey,
    pub user_destination_liquidity_ta: Pubkey,
    pub reserve_liquidity_mint: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub withdraw_ticket: Pubkey,
    pub owner_queued_collateral_vault: Pubkey,
    pub progress_callback_custom_account_0: Option<Pubkey>,
    pub progress_callback_custom_account_1: Option<Pubkey>,
}

pub fn enqueue_to_withdraw(
    accounts: EnqueueToWithdrawAccounts,
    collateral_amount: u64,
    progress_callback_type: crate::types::ProgressCallbackType,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        collateral_amount: u64,
        progress_callback_type: crate::types::ProgressCallbackType,
    }

    let mut data = discriminators::ENQUEUE_TO_WITHDRAW.to_vec();
    Args {
        collateral_amount,
        progress_callback_type,
    }
    .serialize(&mut data)
    .unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer_writable(accounts.owner),
            readonly(accounts.lending_market),
            readonly(accounts.lending_market_authority),
            writable(accounts.reserve),
            writable(accounts.user_source_collateral_ta),
            readonly(accounts.user_destination_liquidity_ta),
            readonly(accounts.reserve_liquidity_mint),
            readonly(accounts.reserve_collateral_mint),
            readonly(TOKEN_PROGRAM_ID),
            writable(accounts.withdraw_ticket),
            writable(accounts.owner_queued_collateral_vault),
            readonly(SYSTEM_PROGRAM_ID),
            optional_account(
                &KLEND_PROGRAM_ID,
                accounts.progress_callback_custom_account_0,
                false,
            ),
            optional_account(
                &KLEND_PROGRAM_ID,
                accounts.progress_callback_custom_account_1,
                false,
            ),
            readonly(SYSVAR_INSTRUCTIONS_ID),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// withdraw_queued_liquidity
// ---------------------------------------------------------------------------

pub struct WithdrawQueuedLiquidityAccounts {
    pub payer: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub reserve: Pubkey,
    pub reserve_liquidity_mint: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    pub owner_queued_collateral_vault: Pubkey,
    pub user_destination_liquidity: Pubkey,
    pub liquidity_token_program: Pubkey,
    pub withdraw_ticket: Pubkey,
    pub withdraw_ticket_owner: Pubkey,
    pub progress_callback_program: Option<Pubkey>,
    pub progress_callback_custom_account_0: Option<Pubkey>,
    pub progress_callback_custom_account_1: Option<Pubkey>,
}

pub fn withdraw_queued_liquidity(accounts: WithdrawQueuedLiquidityAccounts) -> Instruction {
    let data = discriminators::WITHDRAW_QUEUED_LIQUIDITY.to_vec();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer_writable(accounts.payer),
            readonly(accounts.lending_market),
            readonly(accounts.lending_market_authority),
            writable(accounts.reserve),
            readonly(accounts.reserve_liquidity_mint),
            writable(accounts.reserve_collateral_mint),
            writable(accounts.reserve_liquidity_supply),
            writable(accounts.owner_queued_collateral_vault),
            writable(accounts.user_destination_liquidity),
            readonly(TOKEN_PROGRAM_ID),
            readonly(accounts.liquidity_token_program),
            writable(accounts.withdraw_ticket),
            writable(accounts.withdraw_ticket_owner),
            readonly(ASSOCIATED_TOKEN_PROGRAM_ID),
            readonly(SYSTEM_PROGRAM_ID),
            optional_account(&KLEND_PROGRAM_ID, accounts.progress_callback_program, false),
            optional_account(
                &KLEND_PROGRAM_ID,
                accounts.progress_callback_custom_account_0,
                true, // writable — kvault callback requires vault state to be writable
            ),
            optional_account(
                &KLEND_PROGRAM_ID,
                accounts.progress_callback_custom_account_1,
                false,
            ),
            readonly(SYSVAR_INSTRUCTIONS_ID),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// recover_invalid_ticket_collateral
// ---------------------------------------------------------------------------

pub struct RecoverInvalidTicketCollateralAccounts {
    pub payer: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub reserve: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub owner_queued_collateral_vault: Pubkey,
    pub user_source_collateral: Pubkey,
    pub withdraw_ticket: Pubkey,
    pub withdraw_ticket_owner: Pubkey,
}

pub fn recover_invalid_ticket_collateral(
    accounts: RecoverInvalidTicketCollateralAccounts,
    ticket_sequence_number: u64,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        ticket_sequence_number: u64,
    }

    let mut data = discriminators::RECOVER_INVALID_TICKET_COLLATERAL.to_vec();
    Args {
        ticket_sequence_number,
    }
    .serialize(&mut data)
    .unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.payer),
            readonly(accounts.lending_market),
            readonly(accounts.lending_market_authority),
            readonly(accounts.reserve),
            readonly(accounts.reserve_collateral_mint),
            writable(accounts.owner_queued_collateral_vault),
            writable(accounts.user_source_collateral),
            readonly(TOKEN_PROGRAM_ID),
            writable(accounts.withdraw_ticket),
            writable(accounts.withdraw_ticket_owner),
            readonly(SYSVAR_INSTRUCTIONS_ID),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// cancel_withdraw_ticket
// ---------------------------------------------------------------------------

pub struct CancelWithdrawTicketAccounts {
    pub owner: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub reserve: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub owner_queued_collateral_vault: Pubkey,
    pub user_destination_collateral: Pubkey,
    pub collateral_token_program: Pubkey,
    pub withdraw_ticket: Pubkey,
}

pub fn cancel_withdraw_ticket(
    accounts: CancelWithdrawTicketAccounts,
    ticket_sequence_number: u64,
    collateral_amount_to_cancel: u64,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        ticket_sequence_number: u64,
        collateral_amount_to_cancel: u64,
    }

    let mut data = discriminators::CANCEL_WITHDRAW_TICKET.to_vec();
    Args {
        ticket_sequence_number,
        collateral_amount_to_cancel,
    }
    .serialize(&mut data)
    .unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.owner),
            readonly(accounts.lending_market),
            readonly(accounts.lending_market_authority),
            writable(accounts.reserve),
            readonly(accounts.reserve_collateral_mint),
            writable(accounts.owner_queued_collateral_vault),
            writable(accounts.user_destination_collateral),
            readonly(accounts.collateral_token_program),
            writable(accounts.withdraw_ticket),
        ],
        data,
    }
}
