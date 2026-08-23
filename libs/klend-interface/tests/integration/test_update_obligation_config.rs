use klend_interface::{state::Obligation, types::UpdateObligationConfigMode};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};

use super::setup::{self, build_obligation_info, build_reserve_info};

fn send(env: &mut setup::TestEnv, ixs: &[solana_sdk::instruction::Instruction], user: &Keypair) {
    let tx = Transaction::new_signed_with_payer(
        ixs,
        Some(&user.pubkey()),
        &[user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();
}

fn rollover_window_days(env: &setup::TestEnv, obligation: &Pubkey) -> u8 {
    let account = env.svm.get_account(obligation).unwrap();
    let obligation =
        klend_interface::state::from_account_data::<Obligation>(&account.data).unwrap();
    obligation.borrows[0]
        .fixed_term_borrow_rollover_config
        .fixed_term_rollover_window_duration_days
}

/// Drives `UpdateObligationConfigMode::FixedTermRolloverWindowDurationDays` (borsh
/// discriminant 5) through the on-chain program and reads the result back, confirming the
/// program decodes the variant this crate encodes and applies it to the intended field.
#[test]
fn test_update_obligation_config_sets_rollover_window_duration_days() {
    let mut env = setup::setup_full_env();
    let (user, obligation) = setup::create_user_and_obligation(&mut env);
    let reserve_info = build_reserve_info(&env);

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
    let ixs = klend_interface::helpers::deposit_to_obligation(
        user.pubkey(),
        &reserve_info,
        &build_obligation_info(&obligation, &env.reserve.pubkey(), false, false),
        &[reserve_info.clone()],
        user_ta,
        deposit_amount,
        None,
    );
    send(&mut env, &ixs, &user);
    setup::advance_clock_by_slots(&mut env.svm, 1);

    // The rollover config lives on a borrow position, so the obligation needs one.
    let ixs = klend_interface::helpers::borrow(
        user.pubkey(),
        &reserve_info,
        &build_obligation_info(&obligation, &env.reserve.pubkey(), true, false),
        &[reserve_info.clone()],
        user_ta,
        100_000u64,
        None,
    );
    send(&mut env, &ixs, &user);

    assert_eq!(
        rollover_window_days(&env, &obligation),
        0,
        "field should start unset"
    );

    const WINDOW_DAYS: u8 = 7;
    let ix = klend_interface::helpers::update_obligation_config(
        user.pubkey(),
        obligation,
        env.lending_market.pubkey(),
        Some(env.reserve.pubkey()),
        None,
        UpdateObligationConfigMode::FixedTermRolloverWindowDurationDays,
        vec![WINDOW_DAYS],
    );
    send(&mut env, &[ix], &user);

    assert_eq!(
        rollover_window_days(&env, &obligation),
        WINDOW_DAYS,
        "program should have applied the new mode to the borrow's rollover config"
    );
}
