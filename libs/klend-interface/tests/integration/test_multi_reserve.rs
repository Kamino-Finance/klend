use solana_sdk::{signature::Keypair, signer::Signer, transaction::Transaction};

use super::{
    pyth,
    setup::{self, build_obligation_info, build_reserve_info, build_reserve_info_for},
};

/// Create a second reserve (reserve B) in the same lending market as the test env.
/// Returns (reserve_keypair, liquidity_mint, pyth_oracle).
fn add_second_reserve(
    env: &mut setup::TestEnv,
) -> (
    Keypair,
    solana_sdk::pubkey::Pubkey,
    solana_sdk::pubkey::Pubkey,
) {
    let liquidity_mint_b = setup::create_mint(&mut env.svm, &env.admin, 6);
    let pyth_oracle_b = pyth::create_pyth_price_account(&mut env.svm, 1.0);
    let reserve_b = Keypair::new();

    setup::init_reserve(
        &mut env.svm,
        &env.admin,
        &env.lending_market,
        &reserve_b,
        &liquidity_mint_b,
    );
    setup::configure_reserve(
        &mut env.svm,
        &env.admin,
        &env.lending_market.pubkey(),
        &reserve_b.pubkey(),
        &pyth_oracle_b,
    );

    (reserve_b, liquidity_mint_b, pyth_oracle_b)
}

/// Deposit into reserve A via the obligation.
fn deposit_to_reserve_a(
    env: &mut setup::TestEnv,
    user: &Keypair,
    obligation: &solana_sdk::pubkey::Pubkey,
    amount: u64,
) {
    let user_source =
        setup::create_token_account(&mut env.svm, user, &env.liquidity_mint, &user.pubkey());
    setup::mint_to(
        &mut env.svm,
        &env.admin,
        &env.liquidity_mint,
        &user_source,
        amount,
    );

    let reserve_info = build_reserve_info(env);
    let obligation_info = build_obligation_info(obligation, &env.reserve.pubkey(), false, false);

    let ixs = klend_interface::helpers::deposit_to_obligation(
        user.pubkey(),
        &reserve_info,
        &obligation_info,
        &[reserve_info.clone()],
        user_source,
        amount,
        None,
    );
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();
}

#[test]
fn test_borrow_multi_reserve() {
    let mut env = setup::setup_full_env();
    let (reserve_b, liquidity_mint_b, pyth_oracle_b) = add_second_reserve(&mut env);
    let (user, obligation) = setup::create_user_and_obligation(&mut env);

    // Deposit into reserve A
    deposit_to_reserve_a(&mut env, &user, &obligation, 10_000_000);
    setup::advance_clock_by_slots(&mut env.svm, 1);

    // Borrow from reserve B — obligation has deposit in A, borrow from B
    let user_dest =
        setup::create_token_account(&mut env.svm, &user, &liquidity_mint_b, &user.pubkey());

    let reserve_info_a = build_reserve_info(&env);
    let reserve_info_b = build_reserve_info_for(
        &reserve_b.pubkey(),
        &env.lending_market.pubkey(),
        &liquidity_mint_b,
        &pyth_oracle_b,
    );

    let obligation_info = klend_interface::helpers::ObligationInfo {
        address: obligation,
        deposit_reserves: vec![env.reserve.pubkey()],
        borrow_reserves: vec![],
        referrer: None,
    };

    let borrow_amount = 100_000u64;
    let ixs = klend_interface::helpers::borrow(
        user.pubkey(),
        &reserve_info_b,
        &obligation_info,
        &[reserve_info_a, reserve_info_b.clone()],
        user_dest,
        borrow_amount,
        None,
    );
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    let balance = setup::token_balance(&mut env.svm, &user_dest);
    assert!(balance > 0, "Borrow from reserve B should produce tokens");
}

#[test]
fn test_withdraw_multi_reserve() {
    let mut env = setup::setup_full_env();
    let (reserve_b, liquidity_mint_b, pyth_oracle_b) = add_second_reserve(&mut env);
    let (user, obligation) = setup::create_user_and_obligation(&mut env);

    // Deposit into reserve A
    let deposit_amount = 10_000_000u64;
    deposit_to_reserve_a(&mut env, &user, &obligation, deposit_amount);
    setup::advance_clock_by_slots(&mut env.svm, 1);

    // Borrow from reserve B to create a multi-position obligation
    let user_dest_b =
        setup::create_token_account(&mut env.svm, &user, &liquidity_mint_b, &user.pubkey());
    let reserve_info_a = build_reserve_info(&env);
    let reserve_info_b = build_reserve_info_for(
        &reserve_b.pubkey(),
        &env.lending_market.pubkey(),
        &liquidity_mint_b,
        &pyth_oracle_b,
    );

    let obligation_info_deposit = klend_interface::helpers::ObligationInfo {
        address: obligation,
        deposit_reserves: vec![env.reserve.pubkey()],
        borrow_reserves: vec![],
        referrer: None,
    };

    let ixs = klend_interface::helpers::borrow(
        user.pubkey(),
        &reserve_info_b,
        &obligation_info_deposit,
        &[reserve_info_a.clone(), reserve_info_b.clone()],
        user_dest_b,
        100_000,
        None,
    );
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();
    setup::advance_clock_by_slots(&mut env.svm, 1);

    // Now withdraw from reserve A with both reserves on the obligation
    let user_dest_a =
        setup::create_token_account(&mut env.svm, &user, &env.liquidity_mint, &user.pubkey());
    let obligation_info_both = klend_interface::helpers::ObligationInfo {
        address: obligation,
        deposit_reserves: vec![env.reserve.pubkey()],
        borrow_reserves: vec![reserve_b.pubkey()],
        referrer: None,
    };

    // Withdraw a small amount (must stay collateralized)
    let ixs = klend_interface::helpers::withdraw(
        user.pubkey(),
        &reserve_info_a,
        &obligation_info_both,
        &[reserve_info_a.clone(), reserve_info_b],
        user_dest_a,
        1_000_000,
        None,
    );
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    let balance = setup::token_balance(&mut env.svm, &user_dest_a);
    assert!(balance > 0, "Withdraw from reserve A should succeed");
}

#[test]
fn test_repay_multi_reserve() {
    let mut env = setup::setup_full_env();
    let (reserve_b, liquidity_mint_b, pyth_oracle_b) = add_second_reserve(&mut env);
    let (user, obligation) = setup::create_user_and_obligation(&mut env);

    // Deposit into reserve A
    deposit_to_reserve_a(&mut env, &user, &obligation, 10_000_000);
    setup::advance_clock_by_slots(&mut env.svm, 1);

    // Borrow from reserve B
    let user_ta_b =
        setup::create_token_account(&mut env.svm, &user, &liquidity_mint_b, &user.pubkey());
    let reserve_info_a = build_reserve_info(&env);
    let reserve_info_b = build_reserve_info_for(
        &reserve_b.pubkey(),
        &env.lending_market.pubkey(),
        &liquidity_mint_b,
        &pyth_oracle_b,
    );

    let obligation_info_deposit = klend_interface::helpers::ObligationInfo {
        address: obligation,
        deposit_reserves: vec![env.reserve.pubkey()],
        borrow_reserves: vec![],
        referrer: None,
    };

    let borrow_amount = 100_000u64;
    let ixs = klend_interface::helpers::borrow(
        user.pubkey(),
        &reserve_info_b,
        &obligation_info_deposit,
        &[reserve_info_a.clone(), reserve_info_b.clone()],
        user_ta_b,
        borrow_amount,
        None,
    );
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    let borrowed_balance = setup::token_balance(&mut env.svm, &user_ta_b);
    assert!(borrowed_balance > 0);
    setup::advance_clock_by_slots(&mut env.svm, 1);

    // Repay on reserve B with both reserves on the obligation
    let obligation_info_both = klend_interface::helpers::ObligationInfo {
        address: obligation,
        deposit_reserves: vec![env.reserve.pubkey()],
        borrow_reserves: vec![reserve_b.pubkey()],
        referrer: None,
    };

    let repay_amount = borrowed_balance / 2;
    let ixs = klend_interface::helpers::repay(
        user.pubkey(),
        &reserve_info_b,
        &obligation_info_both,
        &[reserve_info_a, reserve_info_b.clone()],
        user_ta_b,
        repay_amount,
        None,
    );
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    let after_repay = setup::token_balance(&mut env.svm, &user_ta_b);
    assert!(
        after_repay < borrowed_balance,
        "Balance should decrease after repay"
    );
}
