use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::{
    discriminators, util::*, FARMS_PROGRAM_ID, KLEND_PROGRAM_ID, SYSVAR_INSTRUCTIONS_ID,
    TOKEN_PROGRAM_ID,
};

// ---------------------------------------------------------------------------
// liquidate_obligation_and_redeem_reserve_collateral_v2
// ---------------------------------------------------------------------------

pub struct LiquidateObligationAndRedeemReserveCollateralV2Accounts {
    // V1 accounts
    pub liquidator: Pubkey,
    pub obligation: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub repay_reserve: Pubkey,
    pub repay_reserve_liquidity_mint: Pubkey,
    pub repay_reserve_liquidity_supply: Pubkey,
    pub withdraw_reserve: Pubkey,
    pub withdraw_reserve_liquidity_mint: Pubkey,
    pub withdraw_reserve_collateral_mint: Pubkey,
    pub withdraw_reserve_collateral_supply: Pubkey,
    pub withdraw_reserve_liquidity_supply: Pubkey,
    pub withdraw_reserve_liquidity_fee_receiver: Pubkey,
    pub user_source_liquidity: Pubkey,
    pub user_destination_collateral: Pubkey,
    pub user_destination_liquidity: Pubkey,
    pub repay_liquidity_token_program: Pubkey,
    pub withdraw_liquidity_token_program: Pubkey,
    // V2 additions
    pub collateral_obligation_farm_user_state: Option<Pubkey>,
    pub collateral_reserve_farm_state: Option<Pubkey>,
    pub debt_obligation_farm_user_state: Option<Pubkey>,
    pub debt_reserve_farm_state: Option<Pubkey>,
}

pub fn liquidate_obligation_and_redeem_reserve_collateral_v2(
    accounts: LiquidateObligationAndRedeemReserveCollateralV2Accounts,
    liquidity_amount: u64,
    min_acceptable_received_liquidity_amount: u64,
    max_allowed_ltv_override_percent: u64,
    remaining_accounts: Vec<AccountMeta>,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        liquidity_amount: u64,
        min_acceptable_received_liquidity_amount: u64,
        max_allowed_ltv_override_percent: u64,
    }

    let args = Args {
        liquidity_amount,
        min_acceptable_received_liquidity_amount,
        max_allowed_ltv_override_percent,
    };
    let mut data = discriminators::LIQUIDATE_OBLIGATION_AND_REDEEM_RESERVE_COLLATERAL_V2.to_vec();
    args.serialize(&mut data).unwrap();

    let mut account_metas = vec![
        // V1 accounts
        signer(accounts.liquidator),
        writable(accounts.obligation),
        readonly(accounts.lending_market),
        readonly(accounts.lending_market_authority),
        writable(accounts.repay_reserve),
        readonly(accounts.repay_reserve_liquidity_mint),
        writable(accounts.repay_reserve_liquidity_supply),
        writable(accounts.withdraw_reserve),
        readonly(accounts.withdraw_reserve_liquidity_mint),
        writable(accounts.withdraw_reserve_collateral_mint),
        writable(accounts.withdraw_reserve_collateral_supply),
        writable(accounts.withdraw_reserve_liquidity_supply),
        writable(accounts.withdraw_reserve_liquidity_fee_receiver),
        writable(accounts.user_source_liquidity),
        writable(accounts.user_destination_collateral),
        writable(accounts.user_destination_liquidity),
        readonly(TOKEN_PROGRAM_ID),
        readonly(accounts.repay_liquidity_token_program),
        readonly(accounts.withdraw_liquidity_token_program),
        readonly(SYSVAR_INSTRUCTIONS_ID),
        // V2 additions
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
