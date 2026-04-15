//! Deposit liquidity as collateral into an obligation using `ObligationContext`.
//!
//! Unlike the simple `deposit_lending` example, this deposits tokens into a
//! reserve **and** credits the resulting cTokens as collateral on an obligation,
//! enabling the user to borrow against them.
//!
//! ```text
//! cargo run --example deposit_borrowing
//! ```

use std::str::FromStr;

use klend_interface::{pda, ObligationContext, KLEND_PROGRAM_ID};
use solana_client::rpc_client::RpcClient;
use solana_pubkey::Pubkey;
use solana_sdk::signer::{keypair::read_keypair_file, Signer};
use spl_associated_token_account::get_associated_token_address;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rpc_client = RpcClient::new("https://api.mainnet-beta.solana.com");
    let signer = read_keypair_file("/path/to/your/keypair.json")?;
    let owner = signer.pubkey();

    // --- 1. Build ObligationContext ------------------------------------------------------

    let lending_market = Pubkey::from_str("7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF")?;
    let reserve_pubkey = Pubkey::from_str("D6q6wuQSrifJKZYpR1M8R4YawnLDtDsMmWM1NbBmgJ59")?;

    // Derive the obligation PDA
    let (obligation_pubkey, _) = pda::obligation(
        &KLEND_PROGRAM_ID,
        0,
        0,
        &owner,
        &lending_market,
        &Pubkey::default(),
        &Pubkey::default(),
    );

    // Fetch the obligation account
    let obligation_data = rpc_client.get_account(&obligation_pubkey)?;

    // Discover which reserves the obligation references
    let reserve_addrs = ObligationContext::reserve_addresses_for_obligation(&obligation_data.data)?;

    // Fetch all reserve accounts in one RPC call
    let reserve_accounts = rpc_client.get_multiple_accounts(&reserve_addrs)?;

    // Build the context
    let reserves: Vec<(Pubkey, &[u8])> = reserve_addrs
        .iter()
        .zip(reserve_accounts.iter())
        .filter_map(|(addr, acc)| acc.as_ref().map(|a| (*addr, a.data.as_slice())))
        .collect();
    let ctx =
        ObligationContext::from_account_data(obligation_pubkey, &obligation_data.data, &reserves)?;

    // --- 2. Build deposit instructions ---------------------------------------------------

    let liquidity_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?;
    let user_source_liquidity = get_associated_token_address(&owner, &liquidity_mint);

    let instructions = ctx.deposit(
        owner,
        &reserve_pubkey,
        user_source_liquidity,
        3_000_000, // 3 USDC (6 decimals)
    )?;
    // Returns: [refresh_reserves..., refresh_obligation, deposit_and_collateral_v2]

    // --- 3. Send transaction -------------------------------------------------------------

    let message = solana_sdk::message::Message::new(&instructions, Some(&owner));
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let tx = solana_sdk::transaction::Transaction::new(&[&signer], message, recent_blockhash);
    let signature = rpc_client.send_and_confirm_transaction(&tx)?;
    println!("Deposit successful! Signature: {signature}");

    Ok(())
}
