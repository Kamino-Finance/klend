//! Withdraw collateral from an obligation and redeem it for the underlying liquidity.
//!
//! This is the standard withdraw flow for borrowers — it removes collateral from
//! the obligation and redeems the cTokens for the underlying tokens in one step.
//! Your position must remain healthy (within LTV limits) after the withdrawal.
//!
//! Pass [`klend_interface::MAX_AMOUNT`] as the amount to withdraw everything.
//!
//! ```text
//! cargo run --example withdraw_obligation
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

    let (obligation_pubkey, _) = pda::obligation(
        &KLEND_PROGRAM_ID,
        0,
        0,
        &owner,
        &lending_market,
        &Pubkey::default(),
        &Pubkey::default(),
    );

    let obligation_data = rpc_client.get_account(&obligation_pubkey)?;
    let reserve_addrs = ObligationContext::reserve_addresses_for_obligation(&obligation_data.data)?;
    let reserve_accounts = rpc_client.get_multiple_accounts(&reserve_addrs)?;

    let reserves: Vec<(Pubkey, &[u8])> = reserve_addrs
        .iter()
        .zip(reserve_accounts.iter())
        .filter_map(|(addr, acc)| acc.as_ref().map(|a| (*addr, a.data.as_slice())))
        .collect();
    let ctx =
        ObligationContext::from_account_data(obligation_pubkey, &obligation_data.data, &reserves)?;

    // --- 2. Build withdraw instructions --------------------------------------------------

    let liquidity_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")?;
    let user_dest_liquidity = get_associated_token_address(&owner, &liquidity_mint);

    let instructions = ctx.withdraw(
        owner,
        &reserve_pubkey,
        user_dest_liquidity,
        1_000_000, // 1 USDC (6 decimals)
    )?;
    // Returns: [refresh_reserves..., refresh_obligation, withdraw_and_redeem]

    // --- 3. Send transaction -------------------------------------------------------------

    let message = solana_sdk::message::Message::new(&instructions, Some(&owner));
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let tx = solana_sdk::transaction::Transaction::new(&[&signer], message, recent_blockhash);
    let signature = rpc_client.send_and_confirm_transaction(&tx)?;
    println!("Withdrawal successful! Signature: {signature}");

    Ok(())
}
