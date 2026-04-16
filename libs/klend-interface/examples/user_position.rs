//! Read an obligation and display deposit/borrow positions and health metrics.
//!
//! Demonstrates how to:
//! - Derive an obligation PDA for a given user and market
//! - Parse the obligation account
//! - Iterate over active deposits and borrows
//! - Compute health metrics from scaled fraction fields
//!
//! ```text
//! cargo run --example user_position
//! ```

use std::str::FromStr;

use klend_interface::{pda, state::Obligation, Fraction, KLEND_PROGRAM_ID};
use solana_client::rpc_client::RpcClient;
use solana_pubkey::Pubkey;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rpc_client = RpcClient::new("https://api.mainnet-beta.solana.com");
    let market_pubkey = Pubkey::from_str("7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF")?;
    let user_pubkey = Pubkey::from_str("EZC9wzVCvihCsCHEMGADYdsRhcpdRYWzSCZAVegSCfqY")?;

    // --- 1. Derive obligation PDA --------------------------------------------------------

    // tag=0, id=0 is the default "Vanilla" obligation
    let (obligation_pubkey, _bump) = pda::obligation(
        &KLEND_PROGRAM_ID,
        0,
        0,
        &user_pubkey,
        &market_pubkey,
        &Pubkey::default(),
        &Pubkey::default(),
    );

    // --- 2. Fetch and parse ---------------------------------------------------------------

    let obligation_account = rpc_client.get_account(&obligation_pubkey)?;
    let obligation = klend_interface::from_account_data::<Obligation>(&obligation_account.data)?;
    println!("Obligation owner: {}", obligation.owner);

    // --- 3. Active deposits --------------------------------------------------------------

    println!("\nDeposits (Collateral):");
    for deposit in &obligation.deposits {
        if deposit.deposit_reserve == Pubkey::default() {
            continue;
        }
        println!("  Reserve: {}", deposit.deposit_reserve);
        println!(
            "    Deposited amount (cTokens): {}",
            deposit.deposited_amount
        );
    }

    // --- 4. Active borrows ---------------------------------------------------------------

    println!("\nBorrows:");
    for borrow in &obligation.borrows {
        if borrow.borrow_reserve == Pubkey::default() {
            continue;
        }
        println!("  Reserve: {}", borrow.borrow_reserve);
        let borrowed: f64 = Fraction::from_bits(borrow.borrowed_amount()).to_num();
        println!("    Borrowed amount: {borrowed:.6}");
    }

    // --- 5. Health metrics ---------------------------------------------------------------

    let sf = |v: u128| -> f64 { Fraction::from_bits(v).to_num() };

    let deposited_value = sf(obligation.deposited_value());
    let allowed_borrow = sf(obligation.allowed_borrow_value());
    let unhealthy_borrow = sf(obligation.unhealthy_borrow_value());
    let borrowed_value = sf(obligation.borrow_factor_adjusted_debt_value());

    println!("\nMetrics:");
    println!("  Deposited value: ${deposited_value:.2}");
    println!("  Allowed borrow value: ${allowed_borrow:.2}");
    println!("  Unhealthy borrow value: ${unhealthy_borrow:.2}");
    println!("  Current borrow value: ${borrowed_value:.2}");
    println!("  Liquidatable: {}", obligation.is_liquidatable());
    println!(
        "  Borrowing disabled: {}",
        obligation.borrowing_disabled > 0
    );

    Ok(())
}
