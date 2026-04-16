use solana_sdk::{signer::Signer, transaction::Transaction};

use super::setup::{self, build_reserve_info};

#[test]
fn test_flash_loan_no_refresh_needed() {
    let mut env = setup::setup_full_env();

    // Deposit liquidity so the reserve has funds to flash-borrow
    let (user, obligation) = setup::create_user_and_obligation(&mut env);
    let deposit_amount = 10_000_000u64;
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
        setup::build_obligation_info(&obligation, &env.reserve.pubkey(), false, false);

    let deposit_ixs = klend_interface::helpers::deposit_to_obligation(
        user.pubkey(),
        &reserve_info,
        &obligation_info,
        &[reserve_info.clone()],
        user_source,
        deposit_amount,
        None,
    );
    let tx = Transaction::new_signed_with_payer(
        &deposit_ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    // Advance clock so the reserve is stale — flash borrow handler refreshes internally
    setup::advance_clock_by_slots(&mut env.svm, 10);

    // Flash loan: borrow and repay in a single transaction, no external refresh
    let flash_amount = 1_000_000u64;
    let user_dest =
        setup::create_token_account(&mut env.svm, &user, &env.liquidity_mint, &user.pubkey());

    let reserve_info = build_reserve_info(&env);
    let (borrow_ix, repay_ix) = klend_interface::helpers::flash_loan(
        user.pubkey(),
        &reserve_info,
        user_dest, // source for repay
        user_dest, // destination for borrow
        flash_amount,
        0, // borrow is first instruction
        None,
    );

    let tx = Transaction::new_signed_with_payer(
        &[borrow_ix, repay_ix],
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    // After flash loan round-trip the user's token account should have 0 balance
    // (borrowed amount was repaid in full; fee is 0 by default config)
    let balance = setup::token_balance(&mut env.svm, &user_dest);
    assert_eq!(
        balance, 0,
        "Flash loan should fully repay, leaving 0 balance"
    );
}
