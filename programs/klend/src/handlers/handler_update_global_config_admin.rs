use anchor_lang::{prelude::*, Accounts};

use crate::{state::GlobalConfig, utils::seeds};

pub fn process(ctx: Context<UpdateGlobalConfigAdmin>) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config.load_mut()?;

    global_config.apply_pending_admin()?;
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateGlobalConfigAdmin<'info> {
    pending_admin: Signer<'info>,

    #[account(mut,
        seeds = [seeds::GLOBAL_CONFIG_STATE],
        bump,
        has_one = pending_admin)]
    pub global_config: AccountLoader<'info, GlobalConfig>,
}
