//! Repay borrowed liquidity on an obligation.
//!
//! Pass [`klend_interface::MAX_AMOUNT`] as the amount to repay the full debt.
//!
//! ```text
//! cargo run --example repay
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
    let repay_reserve_pubkey = Pubkey::from_str("FBSyPnxtHKLBZ4UeeUyAnbtFuAmTHLtso9YtsqRDRWpM")?;

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

    // --- 2. Build repay instructions -----------------------------------------------------

    let repay_liquidity_mint = Pubkey::from_str("Es9vMFrzaCERmJfrF4H2FYD4KCoNkY11McCe8BenwNYB")?;
    let user_source_liquidity = get_associated_token_address(&owner, &repay_liquidity_mint);

    let instructions = ctx.repay(
        owner,
        &repay_reserve_pubkey,
        user_source_liquidity,
        1_000_000, // 1 USDT (6 decimals)
    )?;
    // Returns: [refresh_reserves..., refresh_obligation, repay_obligation_liquidity_v2]

    // --- 3. Send transaction -------------------------------------------------------------

    let message = solana_sdk::message::Message::new(&instructions, Some(&owner));
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let tx = solana_sdk::transaction::Transaction::new(&[&signer], message, recent_blockhash);
    let signature = rpc_client.send_and_confirm_transaction(&tx)?;
    println!("Repay successful! Signature: {signature}");

    Ok(())
}
