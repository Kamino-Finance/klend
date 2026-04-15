use klend_interface::{pda, KLEND_PROGRAM_ID};
use solana_sdk::{signer::Signer, transaction::Transaction};

use super::setup::{self, build_obligation_info, build_reserve_info};

fn deposit(
    env: &mut setup::TestEnv,
    user: &solana_sdk::signature::Keypair,
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
fn test_withdraw_after_deposit() {
    let mut env = setup::setup_full_env();
    let (user, obligation) = setup::create_user_and_obligation(&mut env);
    let deposit_amount = 1_000_000u64;
    deposit(&mut env, &user, &obligation, deposit_amount);
    setup::advance_clock_by_slots(&mut env.svm, 1);

    // Withdraw (collateral + redeem in one step)
    let user_dest_liq =
        setup::create_token_account(&mut env.svm, &user, &env.liquidity_mint, &user.pubkey());

    let reserve_info = build_reserve_info(&env);
    let obligation_info = build_obligation_info(&obligation, &env.reserve.pubkey(), true, false);

    // Withdraw all collateral (use deposit_amount as collateral amount since 1:1 initial rate)
    let ixs = klend_interface::helpers::withdraw(
        user.pubkey(),
        &reserve_info,
        &obligation_info,
        &[reserve_info.clone()],
        user_dest_liq,
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

    let balance = setup::token_balance(&mut env.svm, &user_dest_liq);
    assert!(balance > 0, "Should receive liquidity after withdraw");
}

#[test]
fn test_withdraw_collateral_after_deposit() {
    let mut env = setup::setup_full_env();
    let (user, obligation) = setup::create_user_and_obligation(&mut env);
    let deposit_amount = 1_000_000u64;
    deposit(&mut env, &user, &obligation, deposit_amount);
    setup::advance_clock_by_slots(&mut env.svm, 1);

    // Get the collateral mint for this reserve
    let (coll_mint, _) = pda::reserve_collateral_mint(&KLEND_PROGRAM_ID, &env.reserve.pubkey());

    // Create a cToken destination account
    let user_dest_coll =
        setup::create_token_account(&mut env.svm, &user, &coll_mint, &user.pubkey());

    let reserve_info = build_reserve_info(&env);
    let obligation_info = build_obligation_info(&obligation, &env.reserve.pubkey(), true, false);

    let ixs = klend_interface::helpers::withdraw_collateral(
        user.pubkey(),
        &reserve_info,
        &obligation_info,
        &[reserve_info.clone()],
        user_dest_coll,
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

    let coll_balance = setup::token_balance(&mut env.svm, &user_dest_coll);
    assert!(
        coll_balance > 0,
        "Should receive cTokens after withdraw_collateral"
    );
}

#[test]
fn test_redeem_ctokens() {
    let mut env = setup::setup_full_env();
    let (user, obligation) = setup::create_user_and_obligation(&mut env);
    let deposit_amount = 1_000_000u64;
    deposit(&mut env, &user, &obligation, deposit_amount);
    setup::advance_clock_by_slots(&mut env.svm, 1);

    let (coll_mint, _) = pda::reserve_collateral_mint(&KLEND_PROGRAM_ID, &env.reserve.pubkey());

    // Step 1: withdraw collateral from obligation to get cTokens
    let user_coll = setup::create_token_account(&mut env.svm, &user, &coll_mint, &user.pubkey());

    let reserve_info = build_reserve_info(&env);
    let obligation_info = build_obligation_info(&obligation, &env.reserve.pubkey(), true, false);

    let ixs = klend_interface::helpers::withdraw_collateral(
        user.pubkey(),
        &reserve_info,
        &obligation_info,
        &[reserve_info.clone()],
        user_coll,
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

    let coll_balance = setup::token_balance(&mut env.svm, &user_coll);
    assert!(coll_balance > 0);
    setup::advance_clock_by_slots(&mut env.svm, 1);

    // Step 2: redeem cTokens for liquidity
    let user_dest_liq =
        setup::create_token_account(&mut env.svm, &user, &env.liquidity_mint, &user.pubkey());

    let ixs = klend_interface::helpers::redeem(
        user.pubkey(),
        &reserve_info,
        user_coll,
        user_dest_liq,
        coll_balance,
    );
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    let liq_balance = setup::token_balance(&mut env.svm, &user_dest_liq);
    assert!(liq_balance > 0, "Should receive liquidity after redeem");
}
