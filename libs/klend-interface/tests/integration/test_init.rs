use klend_interface::{pda, KLEND_PROGRAM_ID};
use solana_sdk::{pubkey::Pubkey, signature::Keypair, signer::Signer, transaction::Transaction};

use super::setup;

#[test]
fn test_init_user_succeeds() {
    let mut env = setup::setup_full_env();

    let user = Keypair::new();
    env.svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let ixs = klend_interface::helpers::init_user(
        user.pubkey(),
        user.pubkey(),
        Pubkey::new_unique(),
        None,
    );
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    let (user_metadata_pda, _) = pda::user_metadata(&KLEND_PROGRAM_ID, &user.pubkey());
    assert!(env.svm.get_account(&user_metadata_pda).is_some());
}

#[test]
fn test_init_user_with_referrer() {
    let mut env = setup::setup_full_env();

    // Create referrer first
    let referrer = Keypair::new();
    env.svm.airdrop(&referrer.pubkey(), 10_000_000_000).unwrap();

    let ref_ixs = klend_interface::helpers::init_user(
        referrer.pubkey(),
        referrer.pubkey(),
        Pubkey::new_unique(),
        None,
    );
    let tx = Transaction::new_signed_with_payer(
        &ref_ixs,
        Some(&referrer.pubkey()),
        &[&referrer],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    let (referrer_metadata, _) = pda::user_metadata(&KLEND_PROGRAM_ID, &referrer.pubkey());

    // Create user with referrer
    let user = Keypair::new();
    env.svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let ixs = klend_interface::helpers::init_user(
        user.pubkey(),
        user.pubkey(),
        Pubkey::new_unique(),
        Some(referrer_metadata),
    );
    let tx = Transaction::new_signed_with_payer(
        &ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    let (user_metadata_pda, _) = pda::user_metadata(&KLEND_PROGRAM_ID, &user.pubkey());
    assert!(env.svm.get_account(&user_metadata_pda).is_some());
}

#[test]
fn test_init_obligation_vanilla() {
    let mut env = setup::setup_full_env();
    let (_, obligation) = setup::create_user_and_obligation(&mut env);
    assert!(env.svm.get_account(&obligation).is_some());
}
