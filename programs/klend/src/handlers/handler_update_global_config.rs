use std::fmt::Debug;

use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};

use crate::{
    lending_market::ix_utils,
    state::{GlobalConfig, UpdateGlobalConfigMode},
    utils::seeds,
    xmsg,
};

pub fn process(
    ctx: Context<UpdateGlobalConfig>,
    mode: UpdateGlobalConfigMode,
    value: &[u8],
) -> Result<()> {
    ix_utils::check_no_advance_nonce_ix_within_tx(&ctx.accounts.instruction_sysvar_account)?;

    let global_config = &mut ctx.accounts.global_config.load_mut()?;

    xmsg!(
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

    /// CHECK: Sysvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
