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
    min_expected_current_opportunity_parameter_sf: u128,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        index: u8,
        order: crate::types::ObligationOrder,
        min_expected_current_opportunity_parameter_sf: u128,
    }

    let mut data = discriminators::SET_OBLIGATION_ORDER.to_vec();
    Args {
        index,
        order,
        min_expected_current_opportunity_parameter_sf,
    }
    .serialize(&mut data)
    .unwrap();

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

/// Deprecated pre-multi-BO variant (no `order_idx`); on-chain it operates on the head slot (index 0).
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

pub fn set_borrow_order_v2(
    accounts: SetBorrowOrderAccounts,
    order_idx: u8,
    order_config: crate::types::BorrowOrderConfigArgs,
    min_expected_current_remaining_debt_amount: u64,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        order_idx: u8,
        order_config: crate::types::BorrowOrderConfigArgs,
        min_expected_current_remaining_debt_amount: u64,
    }

    let mut data = discriminators::SET_BORROW_ORDER_V2.to_vec();
    Args {
        order_idx,
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

/// Deprecated pre-multi-BO variant (no `order_idx`); on-chain it fills the head slot (index 0).
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

pub fn fill_borrow_order_v2(
    accounts: FillBorrowOrderAccounts,
    order_idx: u8,
    remaining_accounts: Vec<AccountMeta>,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        order_idx: u8,
    }

    let mut data = discriminators::FILL_BORROW_ORDER_V2.to_vec();
    Args { order_idx }.serialize(&mut data).unwrap();

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

// ---------------------------------------------------------------------------
// execute_obligation_order
// ---------------------------------------------------------------------------

pub struct ExecuteObligationOrderAccounts {
    pub executor: Pubkey,
    pub obligation: Pubkey,
    pub obligation_owner: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub debt_reserve: Pubkey,
    pub debt_reserve_liquidity_mint: Pubkey,
    pub debt_reserve_liquidity_supply: Pubkey,
    pub debt_reserve_liquidity_fee_receiver: Pubkey,
    pub collateral_reserve: Pubkey,
    pub collateral_reserve_liquidity_mint: Pubkey,
    pub collateral_reserve_collateral_mint: Pubkey,
    pub collateral_reserve_collateral_supply: Pubkey,
    pub collateral_reserve_liquidity_supply: Pubkey,
    pub collateral_reserve_liquidity_fee_receiver: Pubkey,
    pub executor_debt_liquidity_ta: Pubkey,
    pub executor_collateral_ctoken_ta: Pubkey,
    pub executor_collateral_liquidity_ta: Pubkey,
    pub collateral_token_program: Pubkey,
    pub debt_liquidity_token_program: Pubkey,
    pub collateral_liquidity_token_program: Pubkey,
    pub referrer_token_state: Option<Pubkey>,
    // Optional farms accounts — collateral side
    pub collateral_obligation_farm_user_state: Option<Pubkey>,
    pub collateral_reserve_farm_state: Option<Pubkey>,
    // Optional farms accounts — debt side
    pub debt_obligation_farm_user_state: Option<Pubkey>,
    pub debt_reserve_farm_state: Option<Pubkey>,
}

pub fn execute_obligation_order(
    accounts: ExecuteObligationOrderAccounts,
    order_index: u8,
    expected_opportunity_type: u8,
    max_given_liquidity_amount: u64,
    min_received_liquidity_amount: u64,
    remaining_accounts: Vec<AccountMeta>,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        order_index: u8,
        expected_opportunity_type: u8,
        max_given_liquidity_amount: u64,
        min_received_liquidity_amount: u64,
    }

    let mut data = discriminators::EXECUTE_OBLIGATION_ORDER.to_vec();
    Args {
        order_index,
        expected_opportunity_type,
        max_given_liquidity_amount,
        min_received_liquidity_amount,
    }
    .serialize(&mut data)
    .unwrap();

    let mut account_metas = vec![
        signer(accounts.executor),
        writable(accounts.obligation),
        writable(accounts.obligation_owner),
        readonly(accounts.lending_market),
        readonly(accounts.lending_market_authority),
        writable(accounts.debt_reserve),
        readonly(accounts.debt_reserve_liquidity_mint),
        writable(accounts.debt_reserve_liquidity_supply),
        writable(accounts.debt_reserve_liquidity_fee_receiver),
        writable(accounts.collateral_reserve),
        readonly(accounts.collateral_reserve_liquidity_mint),
        writable(accounts.collateral_reserve_collateral_mint),
        writable(accounts.collateral_reserve_collateral_supply),
        writable(accounts.collateral_reserve_liquidity_supply),
        writable(accounts.collateral_reserve_liquidity_fee_receiver),
        writable(accounts.executor_debt_liquidity_ta),
        writable(accounts.executor_collateral_ctoken_ta),
        writable(accounts.executor_collateral_liquidity_ta),
        readonly(accounts.collateral_token_program),
        readonly(accounts.debt_liquidity_token_program),
        readonly(accounts.collateral_liquidity_token_program),
        readonly(SYSVAR_INSTRUCTIONS_ID),
        optional_account(&KLEND_PROGRAM_ID, accounts.referrer_token_state, true),
        // Collateral-side farms
        optional_account(
            &KLEND_PROGRAM_ID,
            accounts.collateral_obligation_farm_user_state,
            true,
        ),
        optional_account(
            &KLEND_PROGRAM_ID,
            accounts.collateral_reserve_farm_state,
            true,
        ),
        // Debt-side farms
        optional_account(
            &KLEND_PROGRAM_ID,
            accounts.debt_obligation_farm_user_state,
            true,
        ),
        optional_account(&KLEND_PROGRAM_ID, accounts.debt_reserve_farm_state, true),
        readonly(FARMS_PROGRAM_ID),
    ];

    account_metas.extend(remaining_accounts);

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: account_metas,
        data,
    }
}
