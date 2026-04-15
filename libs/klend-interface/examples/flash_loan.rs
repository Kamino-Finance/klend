//! Flash-borrow and repay within a single transaction.
//!
//! A flash loan borrows tokens at the start of the transaction and repays them
//! (plus a fee) at the end. You can insert arbitrary instructions between the
//! borrow and repay — e.g. arbitrage, liquidation, or collateral swap.
//!
//! ```text
//! cargo run --example flash_loan
//! ```

use std::str::FromStr;

use klend_interface::{helpers, ReserveInfo};
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

    // --- 2. Derive token account ---------------------------------------------------------

    let user_token_account = get_associated_token_address(&owner, &reserve.liquidity_mint);

    // --- 3. Build flash loan instructions ------------------------------------------------

    // `borrow_instruction_index` is the position of the borrow instruction in
    // the final transaction. It starts at 0 here because we place it first.
    let (borrow_ix, repay_ix) = helpers::flash::flash_loan(
        owner,
        &reserve,
        user_token_account, // source for repayment
        user_token_account, // destination for borrowed funds
        1_000_000,          // 1 USDC (6 decimals)
        0,                  // borrow instruction index in the transaction
        None,               // no referrer
    );

    // --- 4. Compose the full transaction -------------------------------------------------

    // Insert your custom instructions between borrow and repay
    let your_custom_instructions: Vec<solana_instruction::Instruction> =
        vec![/* your arbitrage / swap / liquidation logic here */];

    let mut all_instructions = vec![borrow_ix];
    all_instructions.extend(your_custom_instructions);
    all_instructions.push(repay_ix);

    let message = solana_sdk::message::Message::new(&all_instructions, Some(&owner));
    let recent_blockhash = rpc_client.get_latest_blockhash()?;
    let tx = solana_sdk::transaction::Transaction::new(&[&signer], message, recent_blockhash);
    let signature = rpc_client.send_and_confirm_transaction(&tx)?;
    println!("Flash loan successful! Signature: {signature}");

    Ok(())
}
