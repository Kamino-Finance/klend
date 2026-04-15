use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::{
    discriminators, util::*, FARMS_PROGRAM_ID, KLEND_PROGRAM_ID, SYSVAR_INSTRUCTIONS_ID,
    TOKEN_PROGRAM_ID,
};

// ---------------------------------------------------------------------------
// repay_obligation_liquidity_v2
// ---------------------------------------------------------------------------

pub struct RepayObligationLiquidityV2Accounts {
    pub owner: Pubkey,
    pub obligation: Pubkey,
    pub lending_market: Pubkey,
    pub repay_reserve: Pubkey,
    pub reserve_liquidity_mint: Pubkey,
    pub reserve_destination_liquidity: Pubkey,
    pub user_source_liquidity: Pubkey,
    pub token_program: Pubkey,
    // V2 farms
    pub obligation_farm_user_state: Option<Pubkey>,
    pub reserve_farm_state: Option<Pubkey>,
    pub lending_market_authority: Pubkey,
}

pub fn repay_obligation_liquidity_v2(
    accounts: RepayObligationLiquidityV2Accounts,
    liquidity_amount: u64,
    remaining_accounts: Vec<AccountMeta>,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        liquidity_amount: u64,
    }

    let mut data = discriminators::REPAY_OBLIGATION_LIQUIDITY_V2.to_vec();
    Args { liquidity_amount }.serialize(&mut data).unwrap();

    // V1 accounts
    let mut account_metas = vec![
        signer(accounts.owner),
        writable(accounts.obligation),
        readonly(accounts.lending_market),
        writable(accounts.repay_reserve),
        readonly(accounts.reserve_liquidity_mint),
        writable(accounts.reserve_destination_liquidity),
        writable(accounts.user_source_liquidity),
        readonly(accounts.token_program),
        readonly(SYSVAR_INSTRUCTIONS_ID),
    ];

    // V2: farms accounts, then lending_market_authority, then farms_program
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
    account_metas.push(readonly(accounts.lending_market_authority));
    account_metas.push(readonly(FARMS_PROGRAM_ID));

    account_metas.extend(remaining_accounts);

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: account_metas,
        data,
    }
}

// ---------------------------------------------------------------------------
// repay_and_withdraw_and_redeem
// ---------------------------------------------------------------------------

pub struct RepayAndWithdrawAndRedeemAccounts {
    // RepayObligationLiquidity
    pub owner: Pubkey,
    pub obligation: Pubkey,
    pub lending_market: Pubkey,
    pub repay_reserve: Pubkey,
    pub reserve_liquidity_mint: Pubkey,
    pub reserve_destination_liquidity: Pubkey,
    pub user_source_liquidity: Pubkey,
    pub token_program: Pubkey,

    // WithdrawObligationCollateralAndRedeemReserveCollateral
    pub lending_market_authority: Pubkey,
    pub withdraw_reserve: Pubkey,
    pub withdraw_reserve_liquidity_mint: Pubkey,
    pub withdraw_reserve_source_collateral: Pubkey,
    pub withdraw_reserve_collateral_mint: Pubkey,
    pub withdraw_reserve_liquidity_supply: Pubkey,
    pub user_destination_liquidity: Pubkey,
    pub placeholder_user_destination_collateral: Option<Pubkey>,
    pub withdraw_liquidity_token_program: Pubkey,

    // Farms
    pub collateral_obligation_farm_user_state: Option<Pubkey>,
    pub collateral_reserve_farm_state: Option<Pubkey>,
    pub debt_obligation_farm_user_state: Option<Pubkey>,
    pub debt_reserve_farm_state: Option<Pubkey>,
}

pub fn repay_and_withdraw_and_redeem(
    accounts: RepayAndWithdrawAndRedeemAccounts,
    repay_amount: u64,
    withdraw_collateral_amount: u64,
    remaining_accounts: Vec<AccountMeta>,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        repay_amount: u64,
        withdraw_collateral_amount: u64,
    }

    let mut data = discriminators::REPAY_AND_WITHDRAW_AND_REDEEM.to_vec();
    Args {
        repay_amount,
        withdraw_collateral_amount,
    }
    .serialize(&mut data)
    .unwrap();

    // RepayObligationLiquidity accounts
    let mut account_metas = vec![
        signer(accounts.owner),
        writable(accounts.obligation),
        readonly(accounts.lending_market),
        writable(accounts.repay_reserve),
        readonly(accounts.reserve_liquidity_mint),
        writable(accounts.reserve_destination_liquidity),
        writable(accounts.user_source_liquidity),
        readonly(accounts.token_program),
        readonly(SYSVAR_INSTRUCTIONS_ID),
    ];

    // WithdrawObligationCollateralAndRedeemReserveCollateral accounts
    // (owner, obligation, lending_market are duplicated in the wire format)
    account_metas.push(signer_writable(accounts.owner));
    account_metas.push(writable(accounts.obligation));
    account_metas.push(readonly(accounts.lending_market));
    account_metas.push(readonly(accounts.lending_market_authority));
    account_metas.push(writable(accounts.withdraw_reserve));
    account_metas.push(readonly(accounts.withdraw_reserve_liquidity_mint));
    account_metas.push(writable(accounts.withdraw_reserve_source_collateral));
    account_metas.push(writable(accounts.withdraw_reserve_collateral_mint));
    account_metas.push(writable(accounts.withdraw_reserve_liquidity_supply));
    account_metas.push(writable(accounts.user_destination_liquidity));
    account_metas.push(optional_account(
        &KLEND_PROGRAM_ID,
        accounts.placeholder_user_destination_collateral,
        false,
    ));
    account_metas.push(readonly(TOKEN_PROGRAM_ID));
    account_metas.push(readonly(accounts.withdraw_liquidity_token_program));
    account_metas.push(readonly(SYSVAR_INSTRUCTIONS_ID));

    // Farms accounts
    account_metas.push(optional_account(
        &KLEND_PROGRAM_ID,
        accounts.collateral_obligation_farm_user_state,
        true,
    ));
    account_metas.push(optional_account(
        &KLEND_PROGRAM_ID,
        accounts.collateral_reserve_farm_state,
        true,
    ));
    account_metas.push(optional_account(
        &KLEND_PROGRAM_ID,
        accounts.debt_obligation_farm_user_state,
        true,
    ));
    account_metas.push(optional_account(
        &KLEND_PROGRAM_ID,
        accounts.debt_reserve_farm_state,
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
