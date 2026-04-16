use bytemuck::{Pod, Zeroable};
use solana_pubkey::Pubkey;
use spl_discriminator::SplDiscriminate;

/// Global program configuration account.
#[derive(Debug, Clone, Copy, Zeroable, Pod, SplDiscriminate)]
#[discriminator_hash_input("account:GlobalConfig")]
#[repr(C)]
pub struct GlobalConfig {
    pub global_admin: Pubkey,
    pub pending_admin: Pubkey,
    pub fee_collector: Pubkey,
    pub padding: [u8; 928],
}

const _: () = assert!(core::mem::size_of::<GlobalConfig>() == 1024);
