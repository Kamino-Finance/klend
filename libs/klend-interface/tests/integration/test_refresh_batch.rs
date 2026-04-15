use solana_sdk::{signature::Keypair, signer::Signer, transaction::Transaction};

use super::setup::{self, build_reserve_info};

/// Execute `refresh_reserves_batch` against the real klend program via LiteSVM.
///
/// This catches account layout mismatches (e.g. missing lending_market) that
/// unit-level account-count checks cannot detect.
#[test]
fn test_refresh_reserves_batch_executes_on_chain() {
    let mut env = setup::setup_full_env();

    let user = Keypair::new();
    env.svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let reserve_info = build_reserve_info(&env);
    let ix = klend_interface::helpers::refresh_reserves_batch(&[reserve_info], false);

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm
        .send_transaction(tx)
        .expect("refresh_reserves_batch should succeed on-chain");
}
