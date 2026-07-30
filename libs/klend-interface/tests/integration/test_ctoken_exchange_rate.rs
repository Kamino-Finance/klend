use borsh::BorshDeserialize;
use klend_interface::{
    instructions::{
        ctoken_exchange_rate::{
            calculate_ctoken_exchange_rate, CalculateCTokenExchangeRateAccounts,
            ExchangeRateWithDecimals,
        },
        refresh::refresh_reserves_batch,
    },
    state::{from_account_data, Reserve},
    Fraction,
};
use litesvm::types::FailedTransactionMetadata;
use solana_sdk::{
    instruction::AccountMeta, signature::Keypair, signer::Signer, transaction::Transaction,
};

use super::setup::{self, build_reserve_info};

/// Builds the transaction the Scope program issues: a price-skipping `refresh_reserves_batch`
/// (which the `calculate_ctoken_exchange_rate` ix requires, since it does not refresh itself),
/// followed by `calculate_ctoken_exchange_rate`.
fn build_calculate_ctoken_exchange_rate_tx(env: &setup::TestEnv, user: &Keypair) -> Transaction {
    let reserve_info = build_reserve_info(env);

    let refresh_ix = refresh_reserves_batch(
        /* skip_price_updates */ true,
        vec![
            AccountMeta::new(reserve_info.address, false),
            AccountMeta::new_readonly(reserve_info.lending_market, false),
        ],
    );
    let calculate_ix = calculate_ctoken_exchange_rate(CalculateCTokenExchangeRateAccounts {
        reserve: reserve_info.address,
    });

    Transaction::new_signed_with_payer(
        &[refresh_ix, calculate_ix],
        Some(&user.pubkey()),
        &[user],
        env.svm.latest_blockhash(),
    )
}

fn execute_calculate_ctoken_exchange_rate(
    env: &mut setup::TestEnv,
    user: &Keypair,
) -> ExchangeRateWithDecimals {
    let tx = build_calculate_ctoken_exchange_rate_tx(env, user);
    let meta = env
        .svm
        .send_transaction(tx)
        .expect("transaction should succeed on-chain");

    ExchangeRateWithDecimals::try_from_slice(&meta.return_data.data)
        .expect("return data must deserialize as ExchangeRateWithDecimals")
}

/// Execute `calculate_ctoken_exchange_rate` (preceded by a refresh) against the real klend
/// program via LiteSVM and deserialize the CPI return data with the interface's mirror
/// type — guards against on-chain/interface drift in the layout of `ExchangeRateWithDecimals`.
#[test]
fn test_calculate_ctoken_exchange_rate_round_trip() {
    let mut env = setup::setup_full_env();

    let user = Keypair::new();
    env.svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let reserve_info = build_reserve_info(&env);
    let rate = execute_calculate_ctoken_exchange_rate(&mut env, &user);

    let reserve_account = env
        .svm
        .get_account(&reserve_info.address)
        .expect("reserve account must exist");
    let reserve: &Reserve = from_account_data(&reserve_account.data).unwrap();

    // A freshly-initialized reserve mints 1:1, so 1 cToken (10^6 cToken lamports)
    // redeems for exactly 10^6 underlying lamports - i.e. Fraction::ONE * 10^6.
    assert_eq!(
        rate.exchange_rate_sf,
        Fraction::from_num(1_000_000).to_bits(),
        "initial exchange rate must be 1:1"
    );
    assert_eq!(
        u64::from(rate.mint_decimals),
        reserve.liquidity.mint_decimals,
        "mint_decimals must match the underlying reserve"
    );
}

/// The flow is fallible: a deprecated reserve makes the preceding `refresh_reserves_batch` revert,
/// so the whole transaction fails on-chain.
#[test]
fn test_calculate_ctoken_exchange_rate_reverts_on_error() {
    let mut env = setup::setup_full_env();

    let user = Keypair::new();
    env.svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    let reserve_info = build_reserve_info(&env);
    let mut reserve_account = env
        .svm
        .get_account(&reserve_info.address)
        .expect("reserve account must exist");

    // Mark the reserve deprecated.
    let mut reserve = *from_account_data::<Reserve>(&reserve_account.data).unwrap();
    reserve.version = 0;
    reserve_account.data[8..].copy_from_slice(bytemuck::bytes_of(&reserve));
    env.svm
        .set_account(reserve_info.address, reserve_account)
        .unwrap();

    let tx = build_calculate_ctoken_exchange_rate_tx(&env, &user);
    let result: Result<_, FailedTransactionMetadata> = env.svm.send_transaction(tx);

    assert!(
        result.is_err(),
        "a deprecated reserve must make the transaction revert",
    );
}
