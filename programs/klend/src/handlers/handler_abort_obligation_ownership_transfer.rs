use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};

use crate::{lending_market::lending_checks, state::Obligation};




pub fn process(ctx: Context<AbortObligationOwnershipTransfer>) -> Result<()> {
    let obligation = &mut ctx.accounts.obligation.load_mut()?;

    lending_checks::obligation_ownership_transfer_execution_context_checks(
        &ctx.accounts.instruction_sysvar_account,
    )?;
    obligation.check_ownership_transfer_in_initiated_state()?;

    obligation.abort_ownership_transfer()?;

    msg!(
        "Aborted ownership transfer for obligation {}",
        ctx.accounts.obligation.key()
    );

    Ok(())
}

#[derive(Accounts)]
pub struct AbortObligationOwnershipTransfer<'info> {

    pub owner: Signer<'info>,

    #[account(mut, has_one = owner)]
    pub obligation: AccountLoader<'info, Obligation>,

    /// CHECK: Syvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
