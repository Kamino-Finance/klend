use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};

use crate::{lending_market::ix_utils, state::GlobalConfig, utils::seeds};

pub fn process(ctx: Context<UpdateGlobalConfigAdmin>) -> Result<()> {
    ix_utils::check_no_advance_nonce_ix_within_tx(&ctx.accounts.instruction_sysvar_account)?;

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

    /// CHECK: Sysvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
