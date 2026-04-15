use borsh::BorshSerialize;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use crate::{
    discriminators, util::*, FARMS_PROGRAM_ID, KLEND_PROGRAM_ID, SYSVAR_INSTRUCTIONS_ID,
    TOKEN_PROGRAM_ID,
};

// ---------------------------------------------------------------------------
// deposit_reserve_liquidity
// ---------------------------------------------------------------------------

pub struct DepositReserveLiquidityAccounts {
    pub owner: Pubkey,
    pub reserve: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub reserve_liquidity_mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub user_source_liquidity: Pubkey,
    pub user_destination_collateral: Pubkey,
    pub liquidity_token_program: Pubkey,
}

pub fn deposit_reserve_liquidity(
    accounts: DepositReserveLiquidityAccounts,
    liquidity_amount: u64,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        liquidity_amount: u64,
    }

    let args = Args { liquidity_amount };
    let mut data = discriminators::DEPOSIT_RESERVE_LIQUIDITY.to_vec();
    args.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.owner),
            writable(accounts.reserve),
            readonly(accounts.lending_market),
            readonly(accounts.lending_market_authority),
            readonly(accounts.reserve_liquidity_mint),
            writable(accounts.reserve_liquidity_supply),
            writable(accounts.reserve_collateral_mint),
            writable(accounts.user_source_liquidity),
            writable(accounts.user_destination_collateral),
            readonly(TOKEN_PROGRAM_ID),
            readonly(accounts.liquidity_token_program),
            readonly(SYSVAR_INSTRUCTIONS_ID),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// deposit_obligation_collateral_v2
// ---------------------------------------------------------------------------

pub struct DepositObligationCollateralV2Accounts {
    // V1 accounts
    pub owner: Pubkey,
    pub obligation: Pubkey,
    pub lending_market: Pubkey,
    pub deposit_reserve: Pubkey,
    pub reserve_destination_collateral: Pubkey,
    pub user_source_collateral: Pubkey,
    // V2 additions
    pub lending_market_authority: Pubkey,
    pub obligation_farm_user_state: Option<Pubkey>,
    pub reserve_farm_state: Option<Pubkey>,
}

pub fn deposit_obligation_collateral_v2(
    accounts: DepositObligationCollateralV2Accounts,
    collateral_amount: u64,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        collateral_amount: u64,
    }

    let args = Args { collateral_amount };
    let mut data = discriminators::DEPOSIT_OBLIGATION_COLLATERAL_V2.to_vec();
    args.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            // V1 accounts
            signer(accounts.owner),
            writable(accounts.obligation),
            readonly(accounts.lending_market),
            writable(accounts.deposit_reserve),
            writable(accounts.reserve_destination_collateral),
            writable(accounts.user_source_collateral),
            readonly(TOKEN_PROGRAM_ID),
            readonly(SYSVAR_INSTRUCTIONS_ID),
            // V2 additions
            readonly(accounts.lending_market_authority),
            optional_account(&KLEND_PROGRAM_ID, accounts.obligation_farm_user_state, true),
            optional_account(&KLEND_PROGRAM_ID, accounts.reserve_farm_state, true),
            readonly(FARMS_PROGRAM_ID),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// deposit_reserve_liquidity_and_obligation_collateral_v2
// ---------------------------------------------------------------------------

pub struct DepositReserveLiquidityAndObligationCollateralV2Accounts {
    // V1 accounts
    pub owner: Pubkey,
    pub obligation: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub reserve: Pubkey,
    pub reserve_liquidity_mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub reserve_destination_deposit_collateral: Pubkey,
    pub user_source_liquidity: Pubkey,
    pub placeholder_user_destination_collateral: Option<Pubkey>,
    pub liquidity_token_program: Pubkey,
    // V2 additions
    pub obligation_farm_user_state: Option<Pubkey>,
    pub reserve_farm_state: Option<Pubkey>,
}

pub fn deposit_reserve_liquidity_and_obligation_collateral_v2(
    accounts: DepositReserveLiquidityAndObligationCollateralV2Accounts,
    liquidity_amount: u64,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        liquidity_amount: u64,
    }

    let args = Args { liquidity_amount };
    let mut data = discriminators::DEPOSIT_RESERVE_LIQUIDITY_AND_OBLIGATION_COLLATERAL_V2.to_vec();
    args.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            // V1 accounts
            signer_writable(accounts.owner),
            writable(accounts.obligation),
            readonly(accounts.lending_market),
            readonly(accounts.lending_market_authority),
            writable(accounts.reserve),
            readonly(accounts.reserve_liquidity_mint),
            writable(accounts.reserve_liquidity_supply),
            writable(accounts.reserve_collateral_mint),
            writable(accounts.reserve_destination_deposit_collateral),
            writable(accounts.user_source_liquidity),
            optional_account(
                &KLEND_PROGRAM_ID,
                accounts.placeholder_user_destination_collateral,
                false,
            ),
            readonly(TOKEN_PROGRAM_ID),
            readonly(accounts.liquidity_token_program),
            readonly(SYSVAR_INSTRUCTIONS_ID),
            // V2 additions
            optional_account(&KLEND_PROGRAM_ID, accounts.obligation_farm_user_state, true),
            optional_account(&KLEND_PROGRAM_ID, accounts.reserve_farm_state, true),
            readonly(FARMS_PROGRAM_ID),
        ],
        data,
    }
}
