use anchor_lang::prelude::*;
use derivative::Derivative;
use solana_program::pubkey::Pubkey;

use crate::utils::WITHDRAW_TICKET_SIZE;

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


    #[derivative(Debug = "ignore")]
    pub alignment_padding: [u8; 7],


    #[derivative(Debug = "ignore")]
    pub end_padding: [u64; 48],
}

impl WithdrawTicket {

    pub fn is_valid(&self) -> bool {
        self.invalid == false as u8
    }
}
