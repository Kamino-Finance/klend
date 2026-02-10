use anchor_lang::{prelude::error, require, require_eq, Key, Result, ToAccountInfo};
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use solana_program::{
    account_info::AccountInfo, instruction::AccountMeta, program, pubkey::Pubkey,
    system_instruction,
};
use spl_associated_token_account::instruction::create_associated_token_account;

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

pub fn filled_array<T: Copy, const N: usize>(fill: T) -> [T; N] {
    [fill; N]
}






pub fn is_default_array<T: Default + PartialEq>(array: &[T]) -> bool {
    let default_value = T::default();
    array.iter().all(|element| *element == default_value)
}


pub fn has_ata_address(
    account: &impl Key,
    owner: &Pubkey,
    mint: &Pubkey,
    token_program: &Pubkey,
) -> bool {
    account.key() == get_associated_token_address_with_program_id(owner, mint, token_program)
}


#[allow(clippy::too_many_arguments)]
pub fn create_ata<'a>(
    account: AccountInfo<'a>,
    owner: AccountInfo<'a>,
    mint: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    associated_token_program: AccountInfo<'a>,
    system_program: AccountInfo<'a>,
    payer: AccountInfo<'a>,
    remaining_accounts: &[AccountInfo<'a>],
) -> Result<()> {
   
   
   
   
   
   
   
    let mut ix = create_associated_token_account(payer.key, owner.key, mint.key, token_program.key);
    ix.accounts.extend(
        remaining_accounts
            .iter()
            .map(|account| AccountMeta::new_readonly(account.key(), false)),
    );
    let mut account_infos = vec![
        payer,
        account,
        associated_token_program,
        owner,
        mint,
        system_program,
        token_program,
    ];
    account_infos.extend_from_slice(remaining_accounts);
    solana_program::program::invoke(&ix, &account_infos)?;
    Ok(())
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
