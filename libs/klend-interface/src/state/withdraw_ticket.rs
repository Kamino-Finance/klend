use bytemuck::{Pod, Zeroable};
use solana_pubkey::Pubkey;
use spl_discriminator::SplDiscriminate;

/// A ticket representing a depositor's place in a reserve's withdraw queue.
#[derive(Debug, Clone, Copy, Zeroable, Pod, SplDiscriminate)]
#[discriminator_hash_input("account:WithdrawTicket")]
#[repr(C)]
pub struct WithdrawTicket {
    pub sequence_number: u64,
    pub owner: Pubkey,
    pub reserve: Pubkey,
    pub user_destination_liquidity_ta: Pubkey,
    pub queued_collateral_amount: u64,
    pub created_at_timestamp: u64,
    /// 0 = valid, 1 = invalid.
    pub invalid: u8,
    /// `ProgressCallbackType` representation.
    pub progress_callback_type: u8,
    pub alignment_padding: [u8; 6],
    pub progress_callback_custom_accounts: [Pubkey; 2],
    pub end_padding: [u64; 40],
}

const _: () = assert!(core::mem::size_of::<WithdrawTicket>() == 512);

impl WithdrawTicket {
    /// Whether this ticket is valid (eligible for withdrawal).
    pub fn is_valid(&self) -> bool {
        self.invalid == 0
    }

    /// Whether this ticket has been fully cancelled (tombstoned).
    pub fn is_fully_cancelled(&self) -> bool {
        self.queued_collateral_amount == 0
    }
}
