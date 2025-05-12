use std::fmt::Debug;

use anchor_lang::{prelude::*, Accounts};

use crate::{
    state::{GlobalConfig, UpdateGlobalConfigMode},
    utils::seeds,
};

pub fn process(
    ctx: Context<UpdateGlobalConfig>,
    mode: UpdateGlobalConfigMode,
    value: &[u8],
) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config.load_mut()?;

    msg!(
        "Updating global config with mode {:?} and value {:?}",
        mode,
        &value
    );

    global_config.update_value(mode, value)?;

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateGlobalConfig<'info> {
    global_admin: Signer<'info>,

    #[account(
        mut,
        seeds = [seeds::GLOBAL_CONFIG_STATE],
        bump,
        has_one = global_admin)]
    pub global_config: AccountLoader<'info, GlobalConfig>,
}
