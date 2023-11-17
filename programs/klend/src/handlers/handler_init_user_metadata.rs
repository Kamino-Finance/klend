use anchor_lang::{prelude::*, Accounts};

use crate::{
    utils::{seeds::BASE_SEED_USER_METADATA, USER_METADATA_SIZE},
    UserMetadata,
};

pub fn process(
    ctx: Context<InitUserMetadata>,
    referrer: Pubkey,
    user_lookup_table: Pubkey,
) -> Result<()> {
    let mut user_metadata = ctx.accounts.user_metadata.load_init()?;
    let bump = *ctx.bumps.get("user_metadata").unwrap();

    *user_metadata = UserMetadata {
        referrer,
        bump: bump.into(),
        user_lookup_table,
        padding_1: [0; 55],
        padding_2: [0; 64],
    };

    Ok(())
}

#[derive(Accounts)]
pub struct InitUserMetadata<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(init,
        seeds = [BASE_SEED_USER_METADATA, owner.key().as_ref()],
        bump,
        payer = owner,
        space = USER_METADATA_SIZE + 8,
    )]
    pub user_metadata: AccountLoader<'info, UserMetadata>,

    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}
