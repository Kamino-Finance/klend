use std::slice::SliceIndex;

use anchor_lang::{
    err, prelude::error, require, require_eq, Key, Owner, Result, ToAccountInfo, ZeroCopy,
};
use anchor_spl::associated_token::get_associated_token_address_with_program_id;
use solana_program::{
    account_info::AccountInfo, instruction::AccountMeta, program, pubkey::Pubkey,
    system_instruction,
};
use spl_associated_token_account::instruction::create_associated_token_account;

use crate::{
    state::obligation::Obligation, utils::FatAccountLoader, xmsg, LendingError, ReferrerTokenState,
    Reserve,
};

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







pub struct ObligationRemainingAccounts<'a, 'info> {
    deposit_count: usize,
    borrow_count: usize,
    remaining_accounts: &'a [AccountInfo<'info>],
}

impl<'a, 'info> ObligationRemainingAccounts<'a, 'info> {
    pub fn parse(obligation: &Obligation, remaining: &'a [AccountInfo<'info>]) -> Result<Self> {
        let deposit_count = obligation.active_deposits_count();
        let borrow_count = obligation.active_borrows_count();
        let reserves_count = deposit_count + borrow_count;
        let expected = if obligation.has_referrer() {
            reserves_count + borrow_count
        } else {
            reserves_count
        };
        if remaining.len() != expected {
            xmsg!(
                "expected_remaining_accounts={}, actual_remaining_accounts={} obligation.has_referrer()={} reserves_count={} borrow_count={}",
                expected,
                remaining.len(),
                obligation.has_referrer(),
                reserves_count,
                borrow_count,
            );
            return err!(LendingError::InvalidAccountInput);
        }
        Ok(Self {
            deposit_count,
            borrow_count,
            remaining_accounts: remaining,
        })
    }

    pub fn deposit_reserves(
        &self,
    ) -> impl Iterator<Item = FatAccountLoader<'info, Reserve>> + Clone + 'a {
        self.slice(..self.deposit_count)
    }

    pub fn borrow_reserves(
        &self,
    ) -> impl Iterator<Item = FatAccountLoader<'info, Reserve>> + Clone + 'a {
        self.slice(self.deposit_count..self.deposit_count + self.borrow_count)
    }

    pub fn all_reserves(
        &self,
    ) -> impl Iterator<Item = FatAccountLoader<'info, Reserve>> + Clone + 'a {
        self.slice(..self.deposit_count + self.borrow_count)
    }

    pub fn referrer_token_states(
        &self,
    ) -> impl Iterator<Item = FatAccountLoader<'info, ReferrerTokenState>> + Clone + 'a {
        self.slice(self.deposit_count + self.borrow_count..)
    }

    fn slice<T: ZeroCopy + Owner>(
        &self,
        range: impl SliceIndex<[AccountInfo<'info>], Output = [AccountInfo<'info>]>,
    ) -> impl Iterator<Item = FatAccountLoader<'info, T>> + Clone + 'a {
        self.remaining_accounts[range].iter().map(|account| {
            FatAccountLoader::<T>::try_from(account).unwrap_or_else(|err| {
                panic!(
                    "Remaining account is not a valid {}: {:?}",
                    std::any::type_name::<T>(),
                    err
                )
            })
        })
    }
}
