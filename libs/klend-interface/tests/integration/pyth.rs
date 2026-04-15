use litesvm::LiteSVM;
use solana_sdk::{account::Account, pubkey, pubkey::Pubkey};

/// Pyth push-oracle program ID used in Klend tests.
pub const PYTH_PROGRAM_ID: Pubkey = pubkey!("Pyth111111111111111111111111111111111111111");

/// Create a mock Pyth `PriceUpdateV2` account and inject it into the SVM.
/// Returns the account's pubkey.
pub fn create_pyth_price_account(svm: &mut LiteSVM, price: f64) -> Pubkey {
    let key = Pubkey::new_unique();

    // Discriminator: sha256("account:PriceUpdateV2")[..8]
    let disc = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(b"account:PriceUpdateV2");
        let hash = h.finalize();
        let mut d = [0u8; 8];
        d.copy_from_slice(&hash[..8]);
        d
    };

    let price_i64 = (price * 1e8) as i64;

    let mut data = Vec::with_capacity(256);
    // 8-byte discriminator
    data.extend_from_slice(&disc);
    // write_authority: Pubkey (32 bytes)
    data.extend_from_slice(Pubkey::new_unique().as_ref());
    // verification_level: Full = 1 (borsh enum tag u8 + variant index)
    // In borsh, an enum with C-like variants is serialized as u8 index.
    // Full = variant index 1 (Partial=0, Full=1)
    data.push(1u8);
    // price_message (PriceFeedMessage):
    //   feed_id: [u8; 32]
    data.extend_from_slice(&[13u8; 32]);
    //   price: i64
    data.extend_from_slice(&price_i64.to_le_bytes());
    //   conf: u64
    data.extend_from_slice(&0u64.to_le_bytes());
    //   exponent: i32
    data.extend_from_slice(&(-8i32).to_le_bytes());
    //   publish_time: i64
    data.extend_from_slice(&i64::MAX.to_le_bytes());
    //   prev_publish_time: i64
    data.extend_from_slice(&i64::MAX.to_le_bytes());
    //   ema_price: i64
    data.extend_from_slice(&price_i64.to_le_bytes());
    //   ema_conf: u64
    data.extend_from_slice(&0u64.to_le_bytes());
    // posted_slot: u64
    data.extend_from_slice(&0u64.to_le_bytes());

    let account = Account {
        lamports: u32::MAX as u64,
        data: data.clone(),
        owner: PYTH_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };

    svm.set_account(key, account).unwrap();
    key
}
