use solana_sdk::{signer::Signer, transaction::Transaction};

use super::setup::{self, build_obligation_info, build_reserve_info};

fn deposit_first(
    env: &mut setup::TestEnv,
    user: &solana_sdk::signature::Keypair,
    obligation: &solana_sdk::pubkey::Pubkey,
) -> solana_sdk::pubkey::Pubkey {
    let deposit_amount = 10_000_000u64;
    let user_source =
        setup::create_token_account(&mut env.svm, user, &env.liquidity_mint, &user.pubkey());
    setup::mint_to(
        &mut env.svm,
        &env.admin,
        &env.liquidity_mint,
        &user_source,
        deposit_amount,
    );

    let reserve_info = build_reserve_info(env);
    let obligation_info = build_obligation_info(obligation, &env.reserve.pubkey(), false, false);

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
        &[user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();
    user_source
}

#[test]
fn test_borrow_after_deposit() {
    let mut env = setup::setup_full_env();
    let (user, obligation) = setup::create_user_and_obligation(&mut env);
    deposit_first(&mut env, &user, &obligation);
    setup::advance_clock_by_slots(&mut env.svm, 1);

    let borrow_amount = 100_000u64;
    let user_dest =
        setup::create_token_account(&mut env.svm, &user, &env.liquidity_mint, &user.pubkey());

    let reserve_info = build_reserve_info(&env);
    let obligation_info = build_obligation_info(
        &obligation,
        &env.reserve.pubkey(),
        true,  // has deposit
        false, // no existing borrow
    );

    let ixs = klend_interface::helpers::borrow(
        user.pubkey(),
        &reserve_info,
        &obligation_info,
        &[reserve_info.clone()],
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
    assert!(balance > 0, "Borrow should produce tokens in user dest");
}

#[test]
fn test_repay_after_borrow() {
    let mut env = setup::setup_full_env();
    let (user, obligation) = setup::create_user_and_obligation(&mut env);
    deposit_first(&mut env, &user, &obligation);
    setup::advance_clock_by_slots(&mut env.svm, 1);

    // Borrow first
    let borrow_amount = 100_000u64;
    let user_ta =
        setup::create_token_account(&mut env.svm, &user, &env.liquidity_mint, &user.pubkey());

    let reserve_info = build_reserve_info(&env);
    let obligation_info = build_obligation_info(&obligation, &env.reserve.pubkey(), true, false);

    let ixs = klend_interface::helpers::borrow(
        user.pubkey(),
        &reserve_info,
        &obligation_info,
        &[reserve_info.clone()],
        user_ta,
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

    let borrowed_balance = setup::token_balance(&mut env.svm, &user_ta);
    assert!(borrowed_balance > 0);
    setup::advance_clock_by_slots(&mut env.svm, 1);

    // Repay
    let obligation_info_with_borrow = build_obligation_info(
        &obligation,
        &env.reserve.pubkey(),
        true,
        true, // now has borrow
    );

    let repay_amount = borrowed_balance / 2;
    let ixs = klend_interface::helpers::repay(
        user.pubkey(),
        &reserve_info,
        &obligation_info_with_borrow,
        &[reserve_info.clone()],
        user_ta,
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

    let after_repay = setup::token_balance(&mut env.svm, &user_ta);
    assert!(
        after_repay < borrowed_balance,
        "Balance should decrease after repay"
    );
}
