use anchor_lang::{prelude::*, Accounts};

use crate::{
    utils::{
        seeds::{BASE_SEED_REFERRER_STATE, BASE_SEED_SHORT_URL},
        REFERRER_STATE_SIZE, SHORT_URL_SIZE,
    },
    LendingError, ReferrerState, ShortUrl,
};

pub fn process(ctx: Context<InitReferrerStateAndShortUrl>, short_url: String) -> Result<()> {
    require!(
        short_url
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || char == '_' || char == '-'),
        LendingError::ShortUrlNotAsciiAlphanumeric
    );

    ctx.accounts.referrer_short_url.referrer = *ctx.accounts.referrer.key;
    ctx.accounts.referrer_short_url.short_url = short_url;

    let referrer_state = &mut ctx.accounts.referrer_state.load_init()?;

    referrer_state.short_url = ctx.accounts.referrer_short_url.key();

    Ok(())
}

#[derive(Accounts)]
#[instruction(short_url: String)]
pub struct InitReferrerStateAndShortUrl<'info> {
    #[account(mut)]
    pub referrer: Signer<'info>,

    #[account(
        init,
        seeds = [BASE_SEED_REFERRER_STATE, referrer.key.as_ref()],
        bump,
        payer = referrer,
        space = REFERRER_STATE_SIZE + 8
    )]
    pub referrer_state: AccountLoader<'info, ReferrerState>,

    #[account(
        init,
        seeds = [BASE_SEED_SHORT_URL, short_url.as_bytes()],
        bump,
        payer = referrer,
        space = SHORT_URL_SIZE + 8
    )]
    pub referrer_short_url: Account<'info, ShortUrl>,

    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}
