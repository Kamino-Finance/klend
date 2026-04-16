use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::{discriminators, util::*, FARMS_PROGRAM_ID, KLEND_PROGRAM_ID, SYSVAR_INSTRUCTIONS_ID};

// ---------------------------------------------------------------------------
// set_obligation_order
// ---------------------------------------------------------------------------

pub struct SetObligationOrderAccounts {
    pub owner: Pubkey,
    pub obligation: Pubkey,
    pub lending_market: Pubkey,
}

pub fn set_obligation_order(
    accounts: SetObligationOrderAccounts,
    index: u8,
    order: crate::types::ObligationOrder,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        index: u8,
        order: crate::types::ObligationOrder,
    }

    let mut data = discriminators::SET_OBLIGATION_ORDER.to_vec();
    Args { index, order }.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.owner),
            writable(accounts.obligation),
            readonly(accounts.lending_market),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// set_borrow_order
// ---------------------------------------------------------------------------

pub struct SetBorrowOrderAccounts {
    pub owner: Pubkey,
    pub obligation: Pubkey,
    pub lending_market: Pubkey,
    pub reserve: Pubkey,
    pub filled_debt_destination: Pubkey,
    pub debt_liquidity_mint: Pubkey,
}

pub fn set_borrow_order(
    accounts: SetBorrowOrderAccounts,
    order_config: crate::types::BorrowOrderConfigArgs,
    min_expected_current_remaining_debt_amount: u64,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        order_config: crate::types::BorrowOrderConfigArgs,
        min_expected_current_remaining_debt_amount: u64,
    }

    let mut data = discriminators::SET_BORROW_ORDER.to_vec();
    Args {
        order_config,
        min_expected_current_remaining_debt_amount,
    }
    .serialize(&mut data)
    .unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.owner),
            writable(accounts.obligation),
            readonly(accounts.lending_market),
            readonly(accounts.reserve),
            readonly(accounts.filled_debt_destination),
            readonly(accounts.debt_liquidity_mint),
            readonly(SYSVAR_INSTRUCTIONS_ID),
            readonly(crate::pda::event_authority(&KLEND_PROGRAM_ID).0),
            readonly(KLEND_PROGRAM_ID),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// fill_borrow_order
// ---------------------------------------------------------------------------

pub struct FillBorrowOrderAccounts {
    pub payer: Pubkey,
    pub obligation: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub borrow_reserve: Pubkey,
    pub borrow_reserve_liquidity_mint: Pubkey,
    pub reserve_source_liquidity: Pubkey,
    pub borrow_reserve_liquidity_fee_receiver: Pubkey,
    pub user_destination_liquidity: Pubkey,
    pub referrer_token_state: Option<Pubkey>,
    pub token_program: Pubkey,
    // Optional farms accounts
    pub obligation_farm_user_state: Option<Pubkey>,
    pub reserve_farm_state: Option<Pubkey>,
}

pub fn fill_borrow_order(
    accounts: FillBorrowOrderAccounts,
    remaining_accounts: Vec<AccountMeta>,
) -> Instruction {
    let data = discriminators::FILL_BORROW_ORDER.to_vec();

    let mut account_metas = vec![
        signer(accounts.payer),
        writable(accounts.obligation),
        readonly(accounts.lending_market),
        readonly(accounts.lending_market_authority),
        writable(accounts.borrow_reserve),
        readonly(accounts.borrow_reserve_liquidity_mint),
        writable(accounts.reserve_source_liquidity),
        writable(accounts.borrow_reserve_liquidity_fee_receiver),
        writable(accounts.user_destination_liquidity),
        optional_account(&KLEND_PROGRAM_ID, accounts.referrer_token_state, true),
        readonly(accounts.token_program),
        readonly(SYSVAR_INSTRUCTIONS_ID),
        // Optional farms accounts
        optional_account(&KLEND_PROGRAM_ID, accounts.obligation_farm_user_state, true),
        optional_account(&KLEND_PROGRAM_ID, accounts.reserve_farm_state, true),
        readonly(FARMS_PROGRAM_ID),
        // event_cpi accounts
        readonly(crate::pda::event_authority(&KLEND_PROGRAM_ID).0),
        readonly(KLEND_PROGRAM_ID),
    ];

    account_metas.extend(remaining_accounts);

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: account_metas,
        data,
    }
}
