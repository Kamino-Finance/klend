use klend_interface::{pda, KLEND_PROGRAM_ID};
use solana_sdk::{signer::Signer, transaction::Transaction};

use super::setup::{self, build_reserve_info};

#[test]
fn test_standalone_deposit() {
    let mut env = setup::setup_full_env();

    let user = solana_sdk::signature::Keypair::new();
    env.svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let deposit_amount = 1_000_000u64;

    // Create user's source token account and mint tokens
    let user_source =
        setup::create_token_account(&mut env.svm, &user, &env.liquidity_mint, &user.pubkey());
    setup::mint_to(
        &mut env.svm,
        &env.admin,
        &env.liquidity_mint,
        &user_source,
        deposit_amount,
    );

    // Create a cToken destination account
    let (coll_mint, _) = pda::reserve_collateral_mint(&KLEND_PROGRAM_ID, &env.reserve.pubkey());
    let user_dest_coll =
        setup::create_token_account(&mut env.svm, &user, &coll_mint, &user.pubkey());

    let reserve_info = build_reserve_info(&env);

    // Use standalone deposit helper (no obligation)
    let ixs = klend_interface::helpers::deposit(
        user.pubkey(),
        &reserve_info,
        user_source,
        user_dest_coll,
        deposit_amount,
    );

    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    // Verify the user received cTokens
    let coll_balance = setup::token_balance(&mut env.svm, &user_dest_coll);
    assert!(
        coll_balance > 0,
        "Should receive cTokens from standalone deposit"
    );

    // Verify source balance is zero
    let source_balance = setup::token_balance(&mut env.svm, &user_source);
    assert_eq!(source_balance, 0);
}

#[test]
fn test_reserve_info_from_account_data() {
    let env = setup::setup_full_env();

    // Read the raw reserve account data
    let reserve_account = env.svm.get_account(&env.reserve.pubkey()).unwrap();

    // Build ReserveInfo from raw bytes
    let reserve_info = klend_interface::helpers::ReserveInfo::from_account_data(
        env.reserve.pubkey(),
        &reserve_account.data,
    )
    .expect("Should deserialize reserve account");

    // Verify fields match manual construction
    let expected = build_reserve_info(&env);
    assert_eq!(reserve_info.address, expected.address);
    assert_eq!(reserve_info.lending_market, expected.lending_market);
    assert_eq!(reserve_info.liquidity_mint, expected.liquidity_mint);
    assert_eq!(reserve_info.pyth_oracle, expected.pyth_oracle);
}

#[test]
fn test_reserve_accessors() {
    let env = setup::setup_full_env();

    let reserve_account = env.svm.get_account(&env.reserve.pubkey()).unwrap();
    let reserve = klend_interface::state::from_account_data::<klend_interface::state::Reserve>(
        &reserve_account.data,
    )
    .expect("Should deserialize reserve");

    // After setup, reserve should be active with initial deposit
    assert_eq!(reserve.status(), 0, "Reserve should be active");
    assert_eq!(reserve.loan_to_value_pct(), 75);
    assert_eq!(reserve.liquidation_threshold_pct(), 80);
    assert_eq!(reserve.borrow_factor_pct(), 100);
    assert!(
        reserve.available_liquidity() > 0,
        "Should have initial liquidity"
    );
}

#[test]
fn test_obligation_info_from_account_data() {
    let mut env = setup::setup_full_env();
    let (user, obligation_pda) = setup::create_user_and_obligation(&mut env);

    // Deposit so the obligation has a position
    let deposit_amount = 1_000_000u64;
    let user_source =
        setup::create_token_account(&mut env.svm, &user, &env.liquidity_mint, &user.pubkey());
    setup::mint_to(
        &mut env.svm,
        &env.admin,
        &env.liquidity_mint,
        &user_source,
        deposit_amount,
    );

    let reserve_info = build_reserve_info(&env);
    let obligation_info =
        setup::build_obligation_info(&obligation_pda, &env.reserve.pubkey(), false, false);

    let ixs = klend_interface::helpers::deposit_to_obligation(
        user.pubkey(),
        &reserve_info,
        &obligation_info,
        &[reserve_info.clone()],
        user_source,
        deposit_amount,
        None,
    );
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    // Now read the obligation from raw account data
    let obligation_account = env.svm.get_account(&obligation_pda).unwrap();
    let obligation_info = klend_interface::helpers::ObligationInfo::from_account_data(
        obligation_pda,
        &obligation_account.data,
    )
    .expect("Should deserialize obligation");

    assert_eq!(obligation_info.address, obligation_pda);
    assert_eq!(obligation_info.deposit_reserves.len(), 1);
    assert_eq!(obligation_info.deposit_reserves[0], env.reserve.pubkey());
    assert!(obligation_info.borrow_reserves.is_empty());

    // Also test obligation accessors
    let obligation =
        klend_interface::state::from_account_data::<klend_interface::state::Obligation>(
            &obligation_account.data,
        )
        .unwrap();
    assert_eq!(obligation.num_deposits(), 1);
    assert_eq!(obligation.num_borrows(), 0);
    assert!(!obligation.is_liquidatable());
}

#[test]
fn test_reserve_info_from_trait() {
    let env = setup::setup_full_env();

    let reserve_account = env.svm.get_account(&env.reserve.pubkey()).unwrap();
    let reserve = klend_interface::state::from_account_data::<klend_interface::state::Reserve>(
        &reserve_account.data,
    )
    .unwrap();

    let from_method =
        klend_interface::helpers::ReserveInfo::from_reserve(env.reserve.pubkey(), reserve);
    let from_trait: klend_interface::helpers::ReserveInfo = (env.reserve.pubkey(), reserve).into();

    assert_eq!(from_method, from_trait);
}

#[test]
fn test_obligation_info_from_trait() {
    let mut env = setup::setup_full_env();
    let (user, obligation_pda) = setup::create_user_and_obligation(&mut env);

    // Deposit so the obligation has a position
    let deposit_amount = 1_000_000u64;
    let user_source =
        setup::create_token_account(&mut env.svm, &user, &env.liquidity_mint, &user.pubkey());
    setup::mint_to(
        &mut env.svm,
        &env.admin,
        &env.liquidity_mint,
        &user_source,
        deposit_amount,
    );

    let reserve_info = build_reserve_info(&env);
    let obligation_info =
        setup::build_obligation_info(&obligation_pda, &env.reserve.pubkey(), false, false);

    let ixs = klend_interface::helpers::deposit_to_obligation(
        user.pubkey(),
        &reserve_info,
        &obligation_info,
        &[reserve_info.clone()],
        user_source,
        deposit_amount,
        None,
    );
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    let obligation_account = env.svm.get_account(&obligation_pda).unwrap();
    let obligation =
        klend_interface::state::from_account_data::<klend_interface::state::Obligation>(
            &obligation_account.data,
        )
        .unwrap();

    let from_method =
        klend_interface::helpers::ObligationInfo::from_obligation(obligation_pda, obligation);
    let from_trait: klend_interface::helpers::ObligationInfo = (obligation_pda, obligation).into();

    assert_eq!(from_method, from_trait);
}

#[test]
fn test_reserve_info_try_from_bytes() {
    let env = setup::setup_full_env();

    let reserve_account = env.svm.get_account(&env.reserve.pubkey()).unwrap();

    let from_method = klend_interface::helpers::ReserveInfo::from_account_data(
        env.reserve.pubkey(),
        &reserve_account.data,
    )
    .unwrap();

    let from_trait: klend_interface::helpers::ReserveInfo =
        (env.reserve.pubkey(), reserve_account.data.as_slice())
            .try_into()
            .unwrap();

    assert_eq!(from_method, from_trait);
}

#[test]
fn test_obligation_info_try_from_bytes() {
    let mut env = setup::setup_full_env();
    let (user, obligation_pda) = setup::create_user_and_obligation(&mut env);

    let deposit_amount = 1_000_000u64;
    let user_source =
        setup::create_token_account(&mut env.svm, &user, &env.liquidity_mint, &user.pubkey());
    setup::mint_to(
        &mut env.svm,
        &env.admin,
        &env.liquidity_mint,
        &user_source,
        deposit_amount,
    );

    let reserve_info = build_reserve_info(&env);
    let obligation_info =
        setup::build_obligation_info(&obligation_pda, &env.reserve.pubkey(), false, false);

    let ixs = klend_interface::helpers::deposit_to_obligation(
        user.pubkey(),
        &reserve_info,
        &obligation_info,
        &[reserve_info.clone()],
        user_source,
        deposit_amount,
        None,
    );
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    let obligation_account = env.svm.get_account(&obligation_pda).unwrap();

    let from_method = klend_interface::helpers::ObligationInfo::from_account_data(
        obligation_pda,
        &obligation_account.data,
    )
    .unwrap();

    let from_trait: klend_interface::helpers::ObligationInfo =
        (obligation_pda, obligation_account.data.as_slice())
            .try_into()
            .unwrap();

    assert_eq!(from_method, from_trait);
}

#[test]
fn test_try_from_invalid_data() {
    let bogus_data = vec![0u8; 32];
    let key = solana_sdk::pubkey::Pubkey::new_unique();

    let result: Result<klend_interface::helpers::ReserveInfo, _> =
        (key, bogus_data.as_slice()).try_into();
    assert!(result.is_err());

    let result: Result<klend_interface::helpers::ObligationInfo, _> =
        (key, bogus_data.as_slice()).try_into();
    assert!(result.is_err());
}
