use anchor_lang::{
    prelude::{AccountInfo, CpiContext},
    Result,
};
use anchor_spl::token_2022::{
    self,
    spl_token_2022::{
        self,
        extension::{BaseStateWithExtensions, ExtensionType, StateWithExtensions},
    },
};

pub fn mint<'info>(
    token_program: AccountInfo<'info>,
    token_mint: AccountInfo<'info>,
    token_mint_authority: AccountInfo<'info>,
    user_token_ata: AccountInfo<'info>,
    authority_signer_seeds: &[&[u8]],
    mint_amount: u64,
) -> Result<()> {
    anchor_spl::token::mint_to(
        CpiContext::new_with_signer(
            token_program,
            anchor_spl::token::MintTo {
                mint: token_mint,
                to: user_token_ata,
                authority: token_mint_authority,
            },
            &[authority_signer_seeds],
        ),
        mint_amount,
    )?;

    Ok(())
}

pub fn burn<'info>(
    token_mint: AccountInfo<'info>,
    user_token_ata: AccountInfo<'info>,
    user: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    burn_amount: u64,
) -> Result<()> {
    anchor_spl::token::burn(
        CpiContext::new(
            token_program,
            anchor_spl::token::Burn {
                mint: token_mint,
                from: user_token_ata,
                authority: user,
            },
        ),
        burn_amount,
    )?;

    Ok(())
}

pub fn burn_with_signer<'info>(
    token_mint: AccountInfo<'info>,
    token_ata: AccountInfo<'info>,
    authority: AccountInfo<'info>,
    token_program: AccountInfo<'info>,
    burn_amount: u64,
    authority_signer_seeds: &[&[&[u8]]],
) -> Result<()> {
    anchor_spl::token::burn(
        CpiContext::new_with_signer(
            token_program,
            anchor_spl::token::Burn {
                mint: token_mint,
                from: token_ata,
                authority,
            },
            authority_signer_seeds,
        ),
        burn_amount,
    )?;

    Ok(())
}


pub fn is_frozen_default_account_state_extension(mint_account_info: &AccountInfo) -> Result<bool> {
    let mint_data = mint_account_info.data.borrow();

   
    if mint_account_info.owner == &anchor_spl::token::spl_token::id() {
        return Ok(false);
    }

    let mint = StateWithExtensions::<spl_token_2022::state::Mint>::unpack(&mint_data)?;

    let extension_types = mint.get_extension_types()?;

   
    if !extension_types.contains(&ExtensionType::DefaultAccountState) {
        return Ok(false);
    }

    let extension = mint
        .get_extension::<spl_token_2022::extension::default_account_state::DefaultAccountState>()?;

    Ok(extension.state == spl_token_2022::state::AccountState::Frozen as u8)
}

pub fn initialize_immutable_owner<'info>(
    token_program: AccountInfo<'info>,
    account: AccountInfo<'info>,
) -> Result<()> {
    let cpi_context = CpiContext::new(
        token_program,
        token_2022::InitializeImmutableOwner { account },
    );

    token_2022::initialize_immutable_owner(cpi_context)?;

    Ok(())
}

pub fn initialize_token_account<'info>(
    token_program: AccountInfo<'info>,
    mint: AccountInfo<'info>,
    token_account: AccountInfo<'info>,
    authority: AccountInfo<'info>,
) -> Result<()> {
    token_2022::initialize_account3(CpiContext::new(
        token_program,
        token_2022::InitializeAccount3 {
            mint,
            account: token_account,
            authority,
        },
    ))
}
