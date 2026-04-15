use borsh::BorshSerialize;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use crate::{discriminators, util::*, KLEND_PROGRAM_ID, SYSVAR_INSTRUCTIONS_ID};

// ---------------------------------------------------------------------------
// flash_borrow_reserve_liquidity
// ---------------------------------------------------------------------------

pub struct FlashBorrowReserveLiquidityAccounts {
    pub user_transfer_authority: Pubkey,
    pub lending_market_authority: Pubkey,
    pub lending_market: Pubkey,
    pub reserve: Pubkey,
    pub reserve_liquidity_mint: Pubkey,
    pub reserve_source_liquidity: Pubkey,
    pub user_destination_liquidity: Pubkey,
    pub reserve_liquidity_fee_receiver: Pubkey,
    pub referrer_token_state: Option<Pubkey>,
    pub referrer_account: Option<Pubkey>,
    pub token_program: Pubkey,
}

pub fn flash_borrow_reserve_liquidity(
    accounts: FlashBorrowReserveLiquidityAccounts,
    liquidity_amount: u64,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        liquidity_amount: u64,
    }

    let args = Args { liquidity_amount };
    let mut data = discriminators::FLASH_BORROW_RESERVE_LIQUIDITY.to_vec();
    args.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.user_transfer_authority),
            readonly(accounts.lending_market_authority),
            readonly(accounts.lending_market),
            writable(accounts.reserve),
            readonly(accounts.reserve_liquidity_mint),
            writable(accounts.reserve_source_liquidity),
            writable(accounts.user_destination_liquidity),
            writable(accounts.reserve_liquidity_fee_receiver),
            optional_account(&KLEND_PROGRAM_ID, accounts.referrer_token_state, true),
            optional_account(&KLEND_PROGRAM_ID, accounts.referrer_account, true),
            readonly(SYSVAR_INSTRUCTIONS_ID),
            readonly(accounts.token_program),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// flash_repay_reserve_liquidity
// ---------------------------------------------------------------------------

pub struct FlashRepayReserveLiquidityAccounts {
    pub user_transfer_authority: Pubkey,
    pub lending_market_authority: Pubkey,
    pub lending_market: Pubkey,
    pub reserve: Pubkey,
    pub reserve_liquidity_mint: Pubkey,
    pub reserve_destination_liquidity: Pubkey,
    pub user_source_liquidity: Pubkey,
    pub reserve_liquidity_fee_receiver: Pubkey,
    pub referrer_token_state: Option<Pubkey>,
    pub referrer_account: Option<Pubkey>,
    pub token_program: Pubkey,
}

pub fn flash_repay_reserve_liquidity(
    accounts: FlashRepayReserveLiquidityAccounts,
    liquidity_amount: u64,
    borrow_instruction_index: u8,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        liquidity_amount: u64,
        borrow_instruction_index: u8,
    }

    let args = Args {
        liquidity_amount,
        borrow_instruction_index,
    };
    let mut data = discriminators::FLASH_REPAY_RESERVE_LIQUIDITY.to_vec();
    args.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.user_transfer_authority),
            readonly(accounts.lending_market_authority),
            readonly(accounts.lending_market),
            writable(accounts.reserve),
            readonly(accounts.reserve_liquidity_mint),
            writable(accounts.reserve_destination_liquidity),
            writable(accounts.user_source_liquidity),
            writable(accounts.reserve_liquidity_fee_receiver),
            optional_account(&KLEND_PROGRAM_ID, accounts.referrer_token_state, true),
            optional_account(&KLEND_PROGRAM_ID, accounts.referrer_account, true),
            readonly(SYSVAR_INSTRUCTIONS_ID),
            readonly(accounts.token_program),
        ],
        data,
    }
}
