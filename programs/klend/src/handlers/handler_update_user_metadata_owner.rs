use anchor_lang::prelude::*;
use solana_program::pubkey::Pubkey;

use crate::{utils::seeds::BASE_SEED_USER_METADATA, LendingError, UserMetadata};

pub fn process(ctx: Context<UpdateUserMetadataOwner>, owner: Pubkey) -> Result<()> {
    let user_metadata = &mut ctx.accounts.user_metadata.load_mut()?;

    if user_metadata.owner != Pubkey::default() {
        return err!(LendingError::UserMetadataOwnerAlreadySet);
    }

    user_metadata.owner = owner;

    Ok(())
}

#[derive(Accounts)]
#[instruction(owner: Pubkey)]
pub struct UpdateUserMetadataOwner<'info> {
    #[account(mut,
        seeds = [BASE_SEED_USER_METADATA, owner.as_ref()],
        bump = user_metadata.load()?.bump as u8,
    )]
    pub user_metadata: AccountLoader<'info, UserMetadata>,
}
