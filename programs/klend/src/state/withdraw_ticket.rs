use anchor_lang::prelude::*;
use derivative::Derivative;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use solana_program::pubkey::Pubkey;

use crate::utils::{CORRESPONDING_KAMINO_VAULT_PROGRAM_ID, WITHDRAW_TICKET_SIZE};

static_assertions::const_assert_eq!(WITHDRAW_TICKET_SIZE, std::mem::size_of::<WithdrawTicket>());
static_assertions::const_assert_eq!(0, std::mem::size_of::<WithdrawTicket>() % 8);


















#[derive(PartialEq, Derivative)]
#[derivative(Debug)]
#[account(zero_copy)]
#[repr(C)]
pub struct WithdrawTicket {

    pub sequence_number: u64,


    pub owner: Pubkey,


    pub reserve: Pubkey,



    pub user_destination_liquidity_ta: Pubkey,


    pub queued_collateral_amount: u64,




    pub created_at_timestamp: u64,







    pub invalid: u8,


    pub progress_callback_type: u8,


    #[derivative(Debug = "ignore")]
    pub alignment_padding: [u8; 6],


    pub progress_callback_custom_accounts: [Pubkey; 2],


    #[derivative(Debug = "ignore")]
    pub end_padding: [u64; 40],
}

impl WithdrawTicket {

    pub fn progress_callback_type(&self) -> ProgressCallbackType {
        ProgressCallbackType::try_from_primitive(self.progress_callback_type)
            .expect("validated when configuring the callback")
    }
}










#[repr(u8)]
#[derive(
    PartialEq,
    Eq,
    Debug,
    Clone,
    Copy,
    AnchorSerialize,
    AnchorDeserialize,
    TryFromPrimitive,
    IntoPrimitive,
)]
pub enum ProgressCallbackType {

    None = 0,







    KlendQueueAccountingHandlerOnKvault = 1,
}

impl ProgressCallbackType {

    pub fn program_address(&self) -> Pubkey {
        match self {
            ProgressCallbackType::None => Pubkey::default(),
            ProgressCallbackType::KlendQueueAccountingHandlerOnKvault => {
                CORRESPONDING_KAMINO_VAULT_PROGRAM_ID
            }
        }
    }
}

#[repr(u8)]
#[derive(
    PartialEq,
    Eq,
    Debug,
    Clone,
    Copy,
    AnchorSerialize,
    AnchorDeserialize,
    TryFromPrimitive,
    IntoPrimitive,
)]
pub enum WithdrawTicketProgressEvent {














    QueuedLiquidityWithdrawn = 0,
}

impl WithdrawTicket {

    pub fn is_valid(&self) -> bool {
        self.invalid == false as u8
    }
}
