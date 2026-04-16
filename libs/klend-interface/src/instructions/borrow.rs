use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::{discriminators, util::*, FARMS_PROGRAM_ID, KLEND_PROGRAM_ID, SYSVAR_INSTRUCTIONS_ID};

// ---------------------------------------------------------------------------
// borrow_obligation_liquidity_v2
// ---------------------------------------------------------------------------

pub struct BorrowObligationLiquidityV2Accounts {
    pub owner: Pubkey,
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
    // V2 farms
    pub obligation_farm_user_state: Option<Pubkey>,
    pub reserve_farm_state: Option<Pubkey>,
}

pub fn borrow_obligation_liquidity_v2(
    accounts: BorrowObligationLiquidityV2Accounts,
    liquidity_amount: u64,
    remaining_accounts: Vec<AccountMeta>,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        liquidity_amount: u64,
    }

    let mut data = discriminators::BORROW_OBLIGATION_LIQUIDITY_V2.to_vec();
    Args { liquidity_amount }.serialize(&mut data).unwrap();

    // V1 accounts
    let mut account_metas = vec![
        signer(accounts.owner),
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
    ];

    // V2 farms accounts
    account_metas.push(optional_account(
        &KLEND_PROGRAM_ID,
        accounts.obligation_farm_user_state,
        true,
    ));
    account_metas.push(optional_account(
        &KLEND_PROGRAM_ID,
        accounts.reserve_farm_state,
        true,
    ));
    account_metas.push(readonly(FARMS_PROGRAM_ID));

    account_metas.extend(remaining_accounts);

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: account_metas,
        data,
    }
}

// ---------------------------------------------------------------------------
// rollover_fixed_term_borrow
// ---------------------------------------------------------------------------

pub struct RolloverFixedTermBorrowAccounts {
    pub payer: Pubkey,
    pub obligation: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub source_borrow_reserve: Pubkey,
    pub target_borrow_reserve: Pubkey,
    pub liquidity_mint: Pubkey,
    pub source_borrow_reserve_liquidity: Pubkey,
    pub target_borrow_reserve_liquidity: Pubkey,
    pub token_program: Pubkey,
    // Source farms (optional)
    pub source_obligation_farm_user_state: Option<Pubkey>,
    pub source_reserve_farm_state: Option<Pubkey>,
    // Target farms (optional)
    pub target_obligation_farm_user_state: Option<Pubkey>,
    pub target_reserve_farm_state: Option<Pubkey>,
}

pub fn rollover_fixed_term_borrow(accounts: RolloverFixedTermBorrowAccounts) -> Instruction {
    let data = discriminators::ROLLOVER_FIXED_TERM_BORROW.to_vec();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            // RolloverAccounts
            signer(accounts.payer),
            writable(accounts.obligation),
            readonly(accounts.lending_market),
            readonly(accounts.lending_market_authority),
            writable(accounts.source_borrow_reserve),
            writable(accounts.target_borrow_reserve),
            readonly(accounts.liquidity_mint),
            writable(accounts.source_borrow_reserve_liquidity),
            writable(accounts.target_borrow_reserve_liquidity),
            readonly(accounts.token_program),
            // Source farms
            optional_account(
                &KLEND_PROGRAM_ID,
                accounts.source_obligation_farm_user_state,
                true,
            ),
            optional_account(&KLEND_PROGRAM_ID, accounts.source_reserve_farm_state, true),
            // Target farms
            optional_account(
                &KLEND_PROGRAM_ID,
                accounts.target_obligation_farm_user_state,
                true,
            ),
            optional_account(&KLEND_PROGRAM_ID, accounts.target_reserve_farm_state, true),
            // Farms program
            readonly(FARMS_PROGRAM_ID),
        ],
        data,
    }
}
