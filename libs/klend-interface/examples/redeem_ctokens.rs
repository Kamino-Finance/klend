//! Redeem cTokens for the underlying liquidity (no obligation).
//!
//! When you deposit liquidity into a reserve (see `deposit_lending`), you receive
//! cTokens representing your share of the pool. This example redeems those cTokens
//! back for the underlying tokens plus any accrued interest.
//!
//! No obligation is involved — this is the reverse of a simple lending deposit.
//!
//! ```text
//! cargo run --example redeem_ctokens
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

    // User's ATA for the underlying token (e.g. USDC)
    let user_destination_liquidity = get_associated_token_address(&owner, &reserve.liquidity_mint);

    // User's ATA for the cToken (collateral token received when depositing)
    let (collateral_mint, _) = pda::reserve_collateral_mint(&KLEND_PROGRAM_ID, &reserve_pubkey);
    let user_source_collateral = get_associated_token_address(&owner, &collateral_mint);

    // --- 3. Build redeem instructions ----------------------------------------------------

    let collateral_amount = 1_000_000; // amount of cTokens to redeem

    let instructions = helpers::withdraw::redeem(
        owner,
        &reserve,
        user_source_collateral,     // cToken account (burned)
        user_destination_liquidity, // receives underlying tokens + interest
        collateral_amount,
    );
    // Returns: [refresh_reserve, redeem_reserve_collateral]

    // --- 4. Send transaction -------------------------------------------------------------

    let message = solana_sdk::message::Message::new(&instructions, Some(&owner));
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let tx = solana_sdk::transaction::Transaction::new(&[&signer], message, recent_blockhash);
    let signature = rpc_client.send_and_confirm_transaction(&tx)?;
    println!("Redeem successful! Signature: {signature}");

    Ok(())
}
