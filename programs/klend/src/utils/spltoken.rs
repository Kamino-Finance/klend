use anchor_lang::{
    prelude::{AccountInfo, CpiContext},
    Result,
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
