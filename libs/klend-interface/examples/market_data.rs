//! Fetch and inspect lending market and reserve data.
//!
//! Demonstrates how to:
//! - Parse a `LendingMarket` account
//! - Discover all reserves for a market using `getProgramAccounts` with memcmp
//! - Read reserve metrics (available liquidity, borrow amount, price, config)
//!
//! Fields suffixed with `_sf` are fixed-point fractions — use
//! [`klend_interface::Fraction::from_bits`] to interpret them.
//!
//! ```text
//! cargo run --example market_data
//! ```

use std::str::FromStr;

use klend_interface::{
    state::{LendingMarket, Reserve},
    Fraction, KLEND_PROGRAM_ID,
};
use solana_account::ReadableAccount;
use solana_client::{
    rpc_client::RpcClient,
    rpc_config::RpcProgramAccountsConfig,
    rpc_filter::{Memcmp, RpcFilterType},
};
use solana_pubkey::Pubkey;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rpc_client = RpcClient::new("https://api.mainnet-beta.solana.com");
    let market_pubkey = Pubkey::from_str("7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF")?;

    // --- 1. Fetch and parse the lending market -------------------------------------------

    let market_account = rpc_client.get_account(&market_pubkey)?;
    let market = klend_interface::from_account_data::<LendingMarket>(&market_account.data)?;
    println!("Market authority: {}", market.lending_market_owner);

    // --- 2. Fetch all reserves for this market -------------------------------------------

    // The `lending_market` field is at byte offset 32 in the Reserve account
    // (after the 8-byte discriminator and 8-byte tag and 16-byte LastUpdate).
    let filters = vec![
        RpcFilterType::Memcmp(Memcmp::new_raw_bytes(32, market_pubkey.to_bytes().to_vec())),
        RpcFilterType::DataSize(8616), // Reserve account size
    ];
    let accounts = rpc_client.get_program_accounts_with_config(
        &KLEND_PROGRAM_ID,
        RpcProgramAccountsConfig {
            filters: Some(filters),
            ..Default::default()
        },
    )?;

    // --- 3. Parse each reserve and print key metrics -------------------------------------

    for (pubkey, account) in &accounts {
        let reserve = klend_interface::from_account_data::<Reserve>(account.data())?;

        let borrowed: f64 =
            Fraction::from_bits(u128::from(reserve.liquidity.borrowed_amount_sf)).to_num();
        let price: f64 =
            Fraction::from_bits(u128::from(reserve.liquidity.market_price_sf)).to_num();

        println!("Reserve: {pubkey}");
        println!(
            "  Available liquidity: {}",
            reserve.liquidity.total_available_amount
        );
        println!("  Borrowed amount: {borrowed:.6}");
        println!("  Market price: {price:.6}");
        println!("  Mint decimals: {}", reserve.liquidity.mint_decimals);
        println!("  LTV: {}%", reserve.config.loan_to_value_pct);
        println!("  Deposit limit: {}", reserve.config.deposit_limit);
        println!("  Borrow limit: {}", reserve.config.borrow_limit);
    }

    Ok(())
}
