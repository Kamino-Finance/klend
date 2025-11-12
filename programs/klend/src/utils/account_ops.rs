use super::{accounts, spltoken};
use anchor_lang::{prelude::*, AccountsClose, Owner, Result, ToAccountInfo, ZeroCopy};
use anchor_spl::{
    token_2022::spl_token_2022::extension::ExtensionType,
    token_interface::{Mint, TokenInterface},
};
use solana_program::msg;

pub fn close_account_loader<'info, T: ZeroCopy + Owner>(
    close_account: bool,
    owner: &Signer<'info>,
    account_to_be_closed: &AccountLoader<'info, T>,
) -> Result<()> {
    if close_account {
        msg!("Closing account");
        account_to_be_closed.close(owner.to_account_info().clone())?;
    }

    Ok(())
}



pub fn initialize_pda_token_account<'info>(
    payer: &AccountInfo<'info>,
    token_account: &AccountInfo<'info>,
    token_mint: &InterfaceAccount<'info, Mint>,
    token_account_authority: &AccountInfo<'info>,
    token_program: &Interface<'info, TokenInterface>,
    system_program: &AccountInfo<'info>,
    seeds: &[&[&[u8]]],
) -> Result<()> {
    let is_token_2022 = token_program.key() == anchor_spl::token_2022::spl_token_2022::ID;

   
   
   
   
    let space = anchor_spl::token_2022::get_account_data_size(
        CpiContext::new(
            token_program.to_account_info(),
            anchor_spl::token_2022::GetAccountDataSize {
                mint: token_mint.to_account_info(),
            },
        ),
       
       
       
       
        if is_token_2022 {
            &[ExtensionType::ImmutableOwner]
        } else {
            &[]
        },
    )?;

    let lamports = Rent::get()?.minimum_balance(space.try_into().unwrap());

   
    accounts::create_pda_account(
        system_program.to_account_info(),
        payer.to_account_info(),
        token_account.to_account_info(),
        &token_program.key(),
        lamports,
        space,
        seeds,
    )?;

    if is_token_2022 {
        spltoken::initialize_immutable_owner(
            token_program.to_account_info(),
            token_account.to_account_info(),
        )?;
    }

    spltoken::initialize_token_account(
        token_program.to_account_info(),
        token_mint.to_account_info(),
        token_account.to_account_info(),
        token_account_authority.to_account_info(),
    )
    .map_err(Into::into)
}
