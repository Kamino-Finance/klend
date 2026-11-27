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
    /// Unused by the program now. Pass the reserve fee vault (works with every program
    /// version) or `KLEND_PROGRAM_ID` as the `None` placeholder (read-only meta).
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
    min_received_liquidity_amount: u64,
    max_allowed_ltv_override_percent: u64,
    remaining_accounts: Vec<AccountMeta>,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        liquidity_amount: u64,
        min_received_liquidity_amount: u64,
        max_allowed_ltv_override_percent: u64,
    }

    let args = Args {
        liquidity_amount,
        min_received_liquidity_amount,
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
        // `KLEND_PROGRAM_ID` is the `None` placeholder: read-only, otherwise a CPI caller would hit
        // `PrivilegeEscalation`. The fee vault stays writable for older program versions.
        optional_account(
            &KLEND_PROGRAM_ID,
            Some(accounts.withdraw_reserve_liquidity_fee_receiver)
                .filter(|key| *key != KLEND_PROGRAM_ID),
            true,
        ),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn fee_receiver_meta(fee_receiver: Pubkey) -> AccountMeta {
        let u = Pubkey::new_unique;
        let ix = liquidate_obligation_and_redeem_reserve_collateral_v2(
            LiquidateObligationAndRedeemReserveCollateralV2Accounts {
                liquidator: u(),
                obligation: u(),
                lending_market: u(),
                lending_market_authority: u(),
                repay_reserve: u(),
                repay_reserve_liquidity_mint: u(),
                repay_reserve_liquidity_supply: u(),
                withdraw_reserve: u(),
                withdraw_reserve_liquidity_mint: u(),
                withdraw_reserve_collateral_mint: u(),
                withdraw_reserve_collateral_supply: u(),
                withdraw_reserve_liquidity_supply: u(),
                withdraw_reserve_liquidity_fee_receiver: fee_receiver,
                user_source_liquidity: u(),
                user_destination_collateral: u(),
                user_destination_liquidity: u(),
                repay_liquidity_token_program: u(),
                withdraw_liquidity_token_program: u(),
                collateral_obligation_farm_user_state: None,
                collateral_reserve_farm_state: None,
                debt_obligation_farm_user_state: None,
                debt_reserve_farm_state: None,
            },
            0,
            0,
            0,
            vec![],
        );
        ix.accounts[12].clone()
    }

    #[test]
    fn fee_receiver_writability() {
        let none_placeholder = fee_receiver_meta(KLEND_PROGRAM_ID);
        assert_eq!(none_placeholder.pubkey, KLEND_PROGRAM_ID);
        assert!(!none_placeholder.is_writable);

        let fee_vault = Pubkey::new_unique();
        let vault_meta = fee_receiver_meta(fee_vault);
        assert_eq!(vault_meta.pubkey, fee_vault);
        assert!(vault_meta.is_writable);
    }
}
