use solana_sdk::{signer::Signer, transaction::Transaction};

use super::setup::{self, build_obligation_info, build_reserve_info};

/// Helper: build an ObligationContext using manual ReserveInfo (matches test env setup).
fn build_context(
    env: &setup::TestEnv,
    obligation: &solana_sdk::pubkey::Pubkey,
    has_deposit: bool,
    has_borrow: bool,
) -> klend_interface::helpers::ObligationContext {
    let reserve_info = build_reserve_info(env);
    let obligation_info =
        build_obligation_info(obligation, &env.reserve.pubkey(), has_deposit, has_borrow);
    klend_interface::helpers::ObligationContext::from_infos(
        env.lending_market.pubkey(),
        obligation_info,
        &[reserve_info],
    )
}

/// Helper: set up env, create user+obligation, deposit via ObligationContext.
fn setup_with_deposit() -> (
    setup::TestEnv,
    solana_sdk::signature::Keypair,
    solana_sdk::pubkey::Pubkey, // obligation
    solana_sdk::pubkey::Pubkey, // user token account
) {
    let mut env = setup::setup_full_env();
    let (user, obligation) = setup::create_user_and_obligation(&mut env);

    let deposit_amount = 10_000_000u64;
    let user_ta =
        setup::create_token_account(&mut env.svm, &user, &env.liquidity_mint, &user.pubkey());
    setup::mint_to(
        &mut env.svm,
        &env.admin,
        &env.liquidity_mint,
        &user_ta,
        deposit_amount,
    );

    let ctx = build_context(&env, &obligation, false, false);

    let ixs = ctx
        .deposit(
            user.pubkey(),
            &env.reserve.pubkey(),
            user_ta,
            deposit_amount,
        )
        .unwrap();
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    (env, user, obligation, user_ta)
}

#[test]
fn test_context_deposit_and_borrow() {
    let (mut env, user, obligation, _user_ta) = setup_with_deposit();
    setup::advance_clock_by_slots(&mut env.svm, 1);

    let user_dest =
        setup::create_token_account(&mut env.svm, &user, &env.liquidity_mint, &user.pubkey());

    let ctx = build_context(&env, &obligation, true, false);

    let borrow_amount = 100_000u64;
    let ixs = ctx
        .borrow(
            user.pubkey(),
            &env.reserve.pubkey(),
            user_dest,
            borrow_amount,
        )
        .unwrap();
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    let balance = setup::token_balance(&mut env.svm, &user_dest);
    assert!(balance > 0, "Borrow should produce tokens");
}

#[test]
fn test_context_repay() {
    let (mut env, user, obligation, _user_ta) = setup_with_deposit();
    setup::advance_clock_by_slots(&mut env.svm, 1);

    // Borrow first using context
    let user_ta2 =
        setup::create_token_account(&mut env.svm, &user, &env.liquidity_mint, &user.pubkey());
    let ctx = build_context(&env, &obligation, true, false);
    let ixs = ctx
        .borrow(user.pubkey(), &env.reserve.pubkey(), user_ta2, 100_000)
        .unwrap();
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();
    setup::advance_clock_by_slots(&mut env.svm, 1);

    let borrowed = setup::token_balance(&mut env.svm, &user_ta2);

    // Repay using ObligationContext
    let ctx = build_context(&env, &obligation, true, true);
    let repay_amount = borrowed / 2;
    let ixs = ctx
        .repay(user.pubkey(), &env.reserve.pubkey(), user_ta2, repay_amount)
        .unwrap();
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    let after = setup::token_balance(&mut env.svm, &user_ta2);
    assert!(after < borrowed, "Balance should decrease after repay");
}

#[test]
fn test_context_withdraw() {
    let (mut env, user, obligation, _user_ta) = setup_with_deposit();
    setup::advance_clock_by_slots(&mut env.svm, 1);

    let user_dest =
        setup::create_token_account(&mut env.svm, &user, &env.liquidity_mint, &user.pubkey());

    let ctx = build_context(&env, &obligation, true, false);

    let ixs = ctx
        .withdraw(user.pubkey(), &env.reserve.pubkey(), user_dest, 1_000_000)
        .unwrap();
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    let balance = setup::token_balance(&mut env.svm, &user_dest);
    assert!(balance > 0, "Withdraw should produce liquidity tokens");
}

#[test]
fn test_context_reserve_not_found() {
    let mut env = setup::setup_full_env();
    let (_user, obligation) = setup::create_user_and_obligation(&mut env);

    let ctx = build_context(&env, &obligation, false, false);

    let fake_reserve = solana_sdk::pubkey::Pubkey::new_unique();
    let result = ctx.borrow(
        solana_sdk::pubkey::Pubkey::new_unique(),
        &fake_reserve,
        solana_sdk::pubkey::Pubkey::new_unique(),
        100,
    );
    assert_eq!(
        result.unwrap_err(),
        klend_interface::helpers::ObligationContextError::ReserveNotFound(fake_reserve)
    );
}

#[test]
fn test_context_accessors() {
    let mut env = setup::setup_full_env();
    let (_user, obligation) = setup::create_user_and_obligation(&mut env);

    let ctx = build_context(&env, &obligation, false, false);

    assert_eq!(ctx.obligation().address, obligation);
    assert!(ctx.reserve_info(&env.reserve.pubkey()).is_some());
    assert!(ctx
        .reserve_info(&solana_sdk::pubkey::Pubkey::new_unique())
        .is_none());
}

#[test]
fn test_context_from_account_data() {
    let mut env = setup::setup_full_env();
    let (user, obligation) = setup::create_user_and_obligation(&mut env);

    // Deposit so the obligation has a reserve reference
    let deposit_amount = 1_000_000u64;
    let user_ta =
        setup::create_token_account(&mut env.svm, &user, &env.liquidity_mint, &user.pubkey());
    setup::mint_to(
        &mut env.svm,
        &env.admin,
        &env.liquidity_mint,
        &user_ta,
        deposit_amount,
    );
    let reserve_info = build_reserve_info(&env);
    let obligation_info = build_obligation_info(&obligation, &env.reserve.pubkey(), false, false);
    let ixs = klend_interface::helpers::deposit_to_obligation(
        user.pubkey(),
        &reserve_info,
        &obligation_info,
        &[reserve_info.clone()],
        user_ta,
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

    // Step 1: fetch obligation, discover reserve addresses
    let obligation_account = env.svm.get_account(&obligation).unwrap();
    let needed_reserves =
        klend_interface::helpers::ObligationContext::reserve_addresses_for_obligation(
            &obligation_account.data,
        )
        .unwrap();
    assert_eq!(needed_reserves.len(), 1);
    assert_eq!(needed_reserves[0], env.reserve.pubkey());

    // Step 2: fetch reserves, build context
    let reserve_account = env.svm.get_account(&needed_reserves[0]).unwrap();
    let ctx = klend_interface::helpers::ObligationContext::from_account_data(
        obligation,
        &obligation_account.data,
        &[(needed_reserves[0], &reserve_account.data)],
    )
    .unwrap();

    assert_eq!(ctx.obligation().address, obligation);
    assert_eq!(ctx.obligation().deposit_reserves.len(), 1);
    assert!(ctx.reserve_info(&env.reserve.pubkey()).is_some());
    assert_eq!(
        ctx.reserve_info(&env.reserve.pubkey())
            .unwrap()
            .lending_market,
        env.lending_market.pubkey()
    );
}
