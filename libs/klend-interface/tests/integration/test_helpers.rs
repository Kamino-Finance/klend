use klend_interface::{
    discriminators::{self, KlendInstruction},
    helpers::{ObligationInfo, ReserveInfo},
    KLEND_PROGRAM_ID, TOKEN_PROGRAM_ID,
};
use solana_sdk::pubkey::Pubkey;

#[test]
fn test_identify_instruction() {
    // Build a deposit instruction and verify identification
    let reserve_info = ReserveInfo {
        address: Pubkey::new_unique(),
        lending_market: Pubkey::new_unique(),
        liquidity_mint: Pubkey::new_unique(),
        liquidity_token_program: TOKEN_PROGRAM_ID,
        pyth_oracle: None,
        switchboard_price_oracle: None,
        switchboard_twap_oracle: None,
        scope_prices: None,
    };

    let ixs = klend_interface::helpers::deposit(
        Pubkey::new_unique(),
        &reserve_info,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        1_000_000,
    );

    // First instruction is refresh_reserve
    let id = discriminators::identify_instruction(&ixs[0].data);
    assert_eq!(id, Some(KlendInstruction::RefreshReserve));

    // Second instruction is deposit_reserve_liquidity
    let id = discriminators::identify_instruction(&ixs[1].data);
    assert_eq!(id, Some(KlendInstruction::DepositReserveLiquidity));

    // Unknown data returns None
    assert_eq!(discriminators::identify_instruction(&[0u8; 8]), None);

    // Short data returns None
    assert_eq!(discriminators::identify_instruction(&[0u8; 4]), None);
}

#[test]
fn test_refresh_all_for_obligation_deduplicates() {
    let market = Pubkey::new_unique();
    let reserve_a = Pubkey::new_unique();
    let reserve_b = Pubkey::new_unique();

    let reserve_info_a = ReserveInfo {
        address: reserve_a,
        lending_market: market,
        liquidity_mint: Pubkey::new_unique(),
        liquidity_token_program: TOKEN_PROGRAM_ID,
        pyth_oracle: None,
        switchboard_price_oracle: None,
        switchboard_twap_oracle: None,
        scope_prices: None,
    };
    let reserve_info_b = ReserveInfo {
        address: reserve_b,
        lending_market: market,
        liquidity_mint: Pubkey::new_unique(),
        liquidity_token_program: TOKEN_PROGRAM_ID,
        pyth_oracle: None,
        switchboard_price_oracle: None,
        switchboard_twap_oracle: None,
        scope_prices: None,
    };

    // Obligation with reserve_a as both deposit and borrow (overlap)
    let obligation = ObligationInfo {
        address: Pubkey::new_unique(),
        deposit_reserves: vec![reserve_a, reserve_b],
        borrow_reserves: vec![reserve_a], // overlaps with deposit
        referrer: None,
    };

    let lookup = |key: &Pubkey| -> Option<ReserveInfo> {
        if *key == reserve_a {
            Some(reserve_info_a.clone())
        } else if *key == reserve_b {
            Some(reserve_info_b.clone())
        } else {
            None
        }
    };

    let ixs = klend_interface::helpers::refresh_all_for_obligation(&market, &obligation, &lookup)
        .expect("all reserves should be resolvable");

    // Should be: refresh_reserve_a + refresh_reserve_b + refresh_obligation = 3
    // (reserve_a is NOT duplicated even though it's in both deposits and borrows)
    assert_eq!(ixs.len(), 3);

    // First two are refresh_reserve
    assert_eq!(
        discriminators::identify_instruction(&ixs[0].data),
        Some(KlendInstruction::RefreshReserve)
    );
    assert_eq!(
        discriminators::identify_instruction(&ixs[1].data),
        Some(KlendInstruction::RefreshReserve)
    );
    // Last is refresh_obligation
    assert_eq!(
        discriminators::identify_instruction(&ixs[2].data),
        Some(KlendInstruction::RefreshObligation)
    );
}

#[test]
fn test_refresh_reserves_batch_helper() {
    let reserve_infos = vec![
        ReserveInfo {
            address: Pubkey::new_unique(),
            lending_market: Pubkey::new_unique(),
            liquidity_mint: Pubkey::new_unique(),
            liquidity_token_program: TOKEN_PROGRAM_ID,
            pyth_oracle: Some(Pubkey::new_unique()),
            switchboard_price_oracle: None,
            switchboard_twap_oracle: None,
            scope_prices: None,
        },
        ReserveInfo {
            address: Pubkey::new_unique(),
            lending_market: Pubkey::new_unique(),
            liquidity_mint: Pubkey::new_unique(),
            liquidity_token_program: TOKEN_PROGRAM_ID,
            pyth_oracle: None,
            switchboard_price_oracle: Some(Pubkey::new_unique()),
            switchboard_twap_oracle: None,
            scope_prices: None,
        },
    ];

    let ix = klend_interface::helpers::refresh_reserves_batch(&reserve_infos, false);

    assert_eq!(
        discriminators::identify_instruction(&ix.data),
        Some(KlendInstruction::RefreshReservesBatch)
    );
    assert_eq!(ix.program_id, KLEND_PROGRAM_ID);
    // Each reserve contributes 6 accounts (reserve + lending_market + 4 oracles)
    assert_eq!(ix.accounts.len(), 12);
}

#[test]
fn test_borrow_multi_reserve_instruction_count() {
    let market = Pubkey::new_unique();
    let reserve_a = Pubkey::new_unique();
    let reserve_b = Pubkey::new_unique();

    let reserve_info_a = ReserveInfo {
        address: reserve_a,
        lending_market: market,
        liquidity_mint: Pubkey::new_unique(),
        liquidity_token_program: TOKEN_PROGRAM_ID,
        pyth_oracle: None,
        switchboard_price_oracle: None,
        switchboard_twap_oracle: None,
        scope_prices: None,
    };
    let reserve_info_b = ReserveInfo {
        address: reserve_b,
        lending_market: market,
        liquidity_mint: Pubkey::new_unique(),
        liquidity_token_program: TOKEN_PROGRAM_ID,
        pyth_oracle: None,
        switchboard_price_oracle: None,
        switchboard_twap_oracle: None,
        scope_prices: None,
    };

    // Obligation with deposit in A, borrow from B
    let obligation = ObligationInfo {
        address: Pubkey::new_unique(),
        deposit_reserves: vec![reserve_a],
        borrow_reserves: vec![reserve_b],
        referrer: None,
    };

    let ixs = klend_interface::helpers::borrow(
        Pubkey::new_unique(),
        &reserve_info_b,
        &obligation,
        &[reserve_info_a, reserve_info_b.clone()],
        Pubkey::new_unique(),
        1_000_000,
        None,
    );

    // Should be: refresh_A, refresh_B, refresh_obligation, borrow = 4
    assert_eq!(
        ixs.len(),
        4,
        "multi-reserve borrow should produce 4 instructions"
    );
    assert_eq!(
        discriminators::identify_instruction(&ixs[0].data),
        Some(KlendInstruction::RefreshReserve)
    );
    assert_eq!(
        discriminators::identify_instruction(&ixs[1].data),
        Some(KlendInstruction::RefreshReserve)
    );
    assert_eq!(
        discriminators::identify_instruction(&ixs[2].data),
        Some(KlendInstruction::RefreshObligation)
    );
    assert_eq!(
        discriminators::identify_instruction(&ixs[3].data),
        Some(KlendInstruction::BorrowObligationLiquidityV2)
    );
}

#[test]
fn test_flash_loan_helper() {
    let reserve_info = ReserveInfo {
        address: Pubkey::new_unique(),
        lending_market: Pubkey::new_unique(),
        liquidity_mint: Pubkey::new_unique(),
        liquidity_token_program: TOKEN_PROGRAM_ID,
        pyth_oracle: None,
        switchboard_price_oracle: None,
        switchboard_twap_oracle: None,
        scope_prices: None,
    };

    let (borrow_ix, repay_ix) = klend_interface::helpers::flash_loan(
        Pubkey::new_unique(),
        &reserve_info,
        Pubkey::new_unique(),
        Pubkey::new_unique(),
        1_000_000,
        0, // borrow instruction index
        None,
    );

    assert_eq!(
        discriminators::identify_instruction(&borrow_ix.data),
        Some(KlendInstruction::FlashBorrowReserveLiquidity)
    );
    assert_eq!(
        discriminators::identify_instruction(&repay_ix.data),
        Some(KlendInstruction::FlashRepayReserveLiquidity)
    );
}
