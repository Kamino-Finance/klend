use anchor_lang::{prelude::error, require, require_eq, Key, Result, ToAccountInfo};
use solana_program::{account_info::AccountInfo, program, pubkey::Pubkey, system_instruction};

use crate::LendingError;

#[allow(clippy::derivable_impls)]
impl Default for crate::accounts::OptionalObligationFarmsAccounts {
    fn default() -> Self {
        Self {
            obligation_farm_user_state: None,
            reserve_farm_state: None,
        }
    }
}

impl Clone for crate::accounts::OptionalObligationFarmsAccounts {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for crate::accounts::OptionalObligationFarmsAccounts {}

pub fn default_array<T: Default + Copy, const N: usize>() -> [T; N] {
    [T::default(); N]
}






pub fn is_default_array<T: Default + PartialEq>(array: &[T]) -> bool {
    let default_value = T::default();
    array.iter().all(|element| *element == default_value)
}

pub fn create_pda_account<'info>(
    system_program: AccountInfo<'info>,
    payer: AccountInfo<'info>,
    account: AccountInfo<'info>,
    program_id: &Pubkey,
    minimum_lamports: u64,
    space: u64,
    signers_seeds: &[&[&[u8]]],
) -> Result<()> {
    require!(
        account.owner == &system_program.key(),
        LendingError::InvalidAccountOwner
    );

    let current_lamports = account.lamports();

    if current_lamports > 0 {
       
       
       
        program::invoke_signed(
            &system_instruction::transfer(account.key, payer.key, current_lamports),
            &[account.clone(), payer.clone()],
            signers_seeds,
        )?;
    }

    let lamports_post_transfer = account.lamports();

    require_eq!(lamports_post_transfer, 0);

   
   

   
    program::invoke_signed(
        &system_instruction::create_account(
            payer.key,       
            account.key,     
            minimum_lamports,
            space,           
            program_id,      
        ),
        &[payer.to_account_info(), account],
        signers_seeds,
    )
    .map_err(Into::into)
}
