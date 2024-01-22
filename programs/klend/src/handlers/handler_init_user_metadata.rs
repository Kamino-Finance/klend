use anchor_lang::{prelude::*, Accounts};

use crate::{
    utils::{seeds::BASE_SEED_USER_METADATA, USER_METADATA_SIZE},
    UserMetadata,
};

pub fn process(ctx: Context<InitUserMetadata>, user_lookup_table: Pubkey) -> Result<()> {
    let referrer = match &ctx.accounts.referrer_user_metadata {
        Some(referrer_user_metadata) => {
            let referrer_user_metadata = referrer_user_metadata.load()?;
            referrer_user_metadata.owner
        }
        None => Pubkey::default(),
    };

    let mut user_metadata = ctx.accounts.user_metadata.load_init()?;
    let bump = *ctx.bumps.get("user_metadata").unwrap();

    *user_metadata = UserMetadata {
        referrer,
        bump: bump.into(),
        user_lookup_table,
        owner: ctx.accounts.owner.key(),
        padding_1: [0; 51],
        padding_2: [0; 64],
    };

    Ok(())
}

#[derive(Accounts)]
pub struct InitUserMetadata<'info> {
    pub owner: Signer<'info>,

    #[account(mut)]
    pub fee_payer: Signer<'info>,

    #[account(init,
        seeds = [BASE_SEED_USER_METADATA, owner.key().as_ref()],
        bump,
        payer = fee_payer,
        space = USER_METADATA_SIZE + 8,
    )]
    pub user_metadata: AccountLoader<'info, UserMetadata>,

    pub referrer_user_metadata: Option<AccountLoader<'info, UserMetadata>>,

    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}
