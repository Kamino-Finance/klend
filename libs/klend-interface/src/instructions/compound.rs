use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::{
    discriminators, util::*, FARMS_PROGRAM_ID, KLEND_PROGRAM_ID, SYSVAR_INSTRUCTIONS_ID,
    TOKEN_PROGRAM_ID,
};

// ---------------------------------------------------------------------------
// deposit_and_withdraw
// ---------------------------------------------------------------------------

pub struct DepositAndWithdrawAccounts {
    // DepositReserveLiquidityAndObligationCollateral accounts
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

    // WithdrawObligationCollateralAndRedeemReserveCollateral accounts
    pub withdraw_owner: Pubkey,
    pub withdraw_obligation: Pubkey,
    pub withdraw_lending_market: Pubkey,
    pub withdraw_lending_market_authority: Pubkey,
    pub withdraw_reserve: Pubkey,
    pub withdraw_reserve_liquidity_mint: Pubkey,
    pub withdraw_reserve_source_collateral: Pubkey,
    pub withdraw_reserve_collateral_mint: Pubkey,
    pub withdraw_reserve_liquidity_supply: Pubkey,
    pub withdraw_user_destination_liquidity: Pubkey,
    pub withdraw_placeholder_user_destination_collateral: Option<Pubkey>,
    pub withdraw_liquidity_token_program: Pubkey,

    // Farms accounts
    pub deposit_obligation_farm_user_state: Option<Pubkey>,
    pub deposit_reserve_farm_state: Option<Pubkey>,
    pub withdraw_obligation_farm_user_state: Option<Pubkey>,
    pub withdraw_reserve_farm_state: Option<Pubkey>,
}

pub fn deposit_and_withdraw(
    accounts: DepositAndWithdrawAccounts,
    liquidity_amount: u64,
    withdraw_collateral_amount: u64,
    remaining_accounts: Vec<AccountMeta>,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        liquidity_amount: u64,
        withdraw_collateral_amount: u64,
    }

    let args = Args {
        liquidity_amount,
        withdraw_collateral_amount,
    };
    let mut data = discriminators::DEPOSIT_AND_WITHDRAW.to_vec();
    args.serialize(&mut data).unwrap();

    let mut account_metas = vec![
        // DepositReserveLiquidityAndObligationCollateral
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
        // WithdrawObligationCollateralAndRedeemReserveCollateral
        signer_writable(accounts.withdraw_owner),
        writable(accounts.withdraw_obligation),
        readonly(accounts.withdraw_lending_market),
        readonly(accounts.withdraw_lending_market_authority),
        writable(accounts.withdraw_reserve),
        readonly(accounts.withdraw_reserve_liquidity_mint),
        writable(accounts.withdraw_reserve_source_collateral),
        writable(accounts.withdraw_reserve_collateral_mint),
        writable(accounts.withdraw_reserve_liquidity_supply),
        writable(accounts.withdraw_user_destination_liquidity),
        optional_account(
            &KLEND_PROGRAM_ID,
            accounts.withdraw_placeholder_user_destination_collateral,
            false,
        ),
        readonly(TOKEN_PROGRAM_ID),
        readonly(accounts.withdraw_liquidity_token_program),
        readonly(SYSVAR_INSTRUCTIONS_ID),
        // Farms
        optional_account(
            &KLEND_PROGRAM_ID,
            accounts.deposit_obligation_farm_user_state,
            true,
        ),
        optional_account(&KLEND_PROGRAM_ID, accounts.deposit_reserve_farm_state, true),
        optional_account(
            &KLEND_PROGRAM_ID,
            accounts.withdraw_obligation_farm_user_state,
            true,
        ),
        optional_account(
            &KLEND_PROGRAM_ID,
            accounts.withdraw_reserve_farm_state,
            true,
        ),
        readonly(FARMS_PROGRAM_ID),
    ];

    account_metas.extend(remaining_accounts);

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: account_metas,
        data,
    }
}
