//! Deposit liquidity into a reserve and receive cTokens directly (no obligation).
//!
//! This is the simplest deposit flow — you supply tokens to a reserve and receive
//! collateral tokens (cTokens) in return. No obligation is involved, so the
//! deposit cannot be used as collateral for borrowing.
//!
//! ```text
//! cargo run --example deposit_lending
//! ```

use std::str::FromStr;

use klend_interface::{helpers, pda, ReserveInfo, KLEND_PROGRAM_ID};
use solana_client::rpc_client::RpcClient;
use solana_pubkey::Pubkey;
use solana_sdk::signer::{keypair::read_keypair_file, Signer};
use spl_associated_token_account::get_associated_token_address;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let rpc_client = RpcClient::new("https://api.mainnet-beta.solana.com");
    let signer = read_keypair_file("/path/to/your/keypair.json")?;
    let owner = signer.pubkey();

    // --- 1. Fetch reserve data -----------------------------------------------------------

    let reserve_pubkey = Pubkey::from_str("D6q6wuQSrifJKZYpR1M8R4YawnLDtDsMmWM1NbBmgJ59")?;

    let reserve_data = rpc_client.get_account(&reserve_pubkey)?;
    let reserve = ReserveInfo::from_account_data(reserve_pubkey, &reserve_data.data)?;

    // --- 2. Derive token accounts --------------------------------------------------------

    // User's ATA for the deposit token (e.g. USDC)
    let user_source_liquidity = get_associated_token_address(&owner, &reserve.liquidity_mint);

    // Derive the collateral mint PDA, then the user's ATA for it
    let (collateral_mint, _) = pda::reserve_collateral_mint(&KLEND_PROGRAM_ID, &reserve_pubkey);
    let user_destination_collateral = get_associated_token_address(&owner, &collateral_mint);

    // --- 3. Build instructions -----------------------------------------------------------

    let instructions = helpers::deposit::deposit(
        owner,
        &reserve,
        user_source_liquidity,       // user's USDC token account
        user_destination_collateral, // user's cToken account
        1_000_000,                   // 1 USDC (6 decimals)
    );
    // Returns: [refresh_reserve, deposit_reserve_liquidity]

    // --- 4. Send transaction -------------------------------------------------------------

    let message = solana_sdk::message::Message::new(&instructions, Some(&owner));
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let tx = solana_sdk::transaction::Transaction::new(&[&signer], message, recent_blockhash);
    let signature = rpc_client.send_and_confirm_transaction(&tx)?;
    println!("Deposit successful! Signature: {signature}");

    Ok(())
}
