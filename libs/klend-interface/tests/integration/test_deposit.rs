use solana_sdk::{signer::Signer, transaction::Transaction};

use super::setup::{self, build_obligation_info, build_reserve_info};

#[test]
fn test_deposit_to_obligation() {
    let mut env = setup::setup_full_env();
    let (user, obligation) = setup::create_user_and_obligation(&mut env);

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

    let reserve_info = build_reserve_info(&env);
    let obligation_info = build_obligation_info(
        &obligation,
        &env.reserve.pubkey(),
        false, // no existing deposit
        false,
    );

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

    // Verify the user's source balance decreased
    let balance = setup::token_balance(&mut env.svm, &user_source);
    assert_eq!(balance, 0);
}
