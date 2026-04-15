use borsh::BorshSerialize;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use crate::{
    discriminators, util::*, FARMS_PROGRAM_ID, KLEND_PROGRAM_ID, SYSVAR_INSTRUCTIONS_ID,
    TOKEN_PROGRAM_ID,
};

// ---------------------------------------------------------------------------
// redeem_reserve_collateral
// ---------------------------------------------------------------------------

pub struct RedeemReserveCollateralAccounts {
    pub owner: Pubkey,
    pub lending_market: Pubkey,
    pub reserve: Pubkey,
    pub lending_market_authority: Pubkey,
    pub reserve_liquidity_mint: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    pub user_source_collateral: Pubkey,
    pub user_destination_liquidity: Pubkey,
    pub liquidity_token_program: Pubkey,
}

pub fn redeem_reserve_collateral(
    accounts: RedeemReserveCollateralAccounts,
    collateral_amount: u64,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        collateral_amount: u64,
    }

    let args = Args { collateral_amount };
    let mut data = discriminators::REDEEM_RESERVE_COLLATERAL.to_vec();
    args.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.owner),
            readonly(accounts.lending_market),
            writable(accounts.reserve),
            readonly(accounts.lending_market_authority),
            readonly(accounts.reserve_liquidity_mint),
            writable(accounts.reserve_collateral_mint),
            writable(accounts.reserve_liquidity_supply),
            writable(accounts.user_source_collateral),
            writable(accounts.user_destination_liquidity),
            readonly(TOKEN_PROGRAM_ID),
            readonly(accounts.liquidity_token_program),
            readonly(SYSVAR_INSTRUCTIONS_ID),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// withdraw_obligation_collateral_v2
// ---------------------------------------------------------------------------

pub struct WithdrawObligationCollateralV2Accounts {
    // V1 accounts
    pub owner: Pubkey,
    pub obligation: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub withdraw_reserve: Pubkey,
    pub reserve_source_collateral: Pubkey,
    pub user_destination_collateral: Pubkey,
    // V2 additions
    pub obligation_farm_user_state: Option<Pubkey>,
    pub reserve_farm_state: Option<Pubkey>,
}

pub fn withdraw_obligation_collateral_v2(
    accounts: WithdrawObligationCollateralV2Accounts,
    collateral_amount: u64,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        collateral_amount: u64,
    }

    let args = Args { collateral_amount };
    let mut data = discriminators::WITHDRAW_OBLIGATION_COLLATERAL_V2.to_vec();
    args.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            // V1 accounts
            signer_writable(accounts.owner),
            writable(accounts.obligation),
            readonly(accounts.lending_market),
            readonly(accounts.lending_market_authority),
            writable(accounts.withdraw_reserve),
            writable(accounts.reserve_source_collateral),
            writable(accounts.user_destination_collateral),
            readonly(TOKEN_PROGRAM_ID),
            readonly(SYSVAR_INSTRUCTIONS_ID),
            // V2 additions
            optional_account(&KLEND_PROGRAM_ID, accounts.obligation_farm_user_state, true),
            optional_account(&KLEND_PROGRAM_ID, accounts.reserve_farm_state, true),
            readonly(FARMS_PROGRAM_ID),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// withdraw_obligation_collateral_and_redeem_reserve_collateral_v2
// ---------------------------------------------------------------------------

pub struct WithdrawObligationCollateralAndRedeemReserveCollateralV2Accounts {
    // V1 accounts
    pub owner: Pubkey,
    pub obligation: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub withdraw_reserve: Pubkey,
    pub reserve_liquidity_mint: Pubkey,
    pub reserve_source_collateral: Pubkey,
    pub reserve_collateral_mint: Pubkey,
    pub reserve_liquidity_supply: Pubkey,
    pub user_destination_liquidity: Pubkey,
    pub placeholder_user_destination_collateral: Option<Pubkey>,
    pub liquidity_token_program: Pubkey,
    // V2 additions
    pub obligation_farm_user_state: Option<Pubkey>,
    pub reserve_farm_state: Option<Pubkey>,
}

pub fn withdraw_obligation_collateral_and_redeem_reserve_collateral_v2(
    accounts: WithdrawObligationCollateralAndRedeemReserveCollateralV2Accounts,
    collateral_amount: u64,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        collateral_amount: u64,
    }

    let args = Args { collateral_amount };
    let mut data =
        discriminators::WITHDRAW_OBLIGATION_COLLATERAL_AND_REDEEM_RESERVE_COLLATERAL_V2.to_vec();
    args.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            // V1 accounts
            signer_writable(accounts.owner),
            writable(accounts.obligation),
            readonly(accounts.lending_market),
            readonly(accounts.lending_market_authority),
            writable(accounts.withdraw_reserve),
            readonly(accounts.reserve_liquidity_mint),
            writable(accounts.reserve_source_collateral),
            writable(accounts.reserve_collateral_mint),
            writable(accounts.reserve_liquidity_supply),
            writable(accounts.user_destination_liquidity),
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
