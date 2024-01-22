use anchor_lang::{prelude::*, Accounts};

use crate::{utils::seeds::BASE_SEED_REFERRER_STATE, LendingError, ReferrerState, ShortUrl};

pub fn process(_ctx: Context<DeleteReferrerStateAndShortUrl>) -> Result<()> {
    Ok(())
}

#[derive(Accounts)]
pub struct DeleteReferrerStateAndShortUrl<'info> {
    #[account(mut)]
    pub referrer: Signer<'info>,

    #[account(mut,
        seeds = [BASE_SEED_REFERRER_STATE, referrer.key.as_ref()],
        bump,
        has_one = short_url,
        constraint = referrer_state.load()?.owner == referrer.key() @ LendingError::ReferrerStateOwnerMismatch,
        close = referrer
    )]
    pub referrer_state: AccountLoader<'info, ReferrerState>,

    #[account(mut,
        has_one = referrer,
        close = referrer
    )]
    pub short_url: Account<'info, ShortUrl>,

    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}
