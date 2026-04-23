use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};

use crate::{
    lending_market::{lending_checks, lending_operations},
    state::Obligation,
    LendingError,
};




pub fn process(ctx: Context<AcceptObligationOwnership>) -> Result<()> {
    let obligation = &mut ctx.accounts.obligation.load_mut()?;
    let clock = &Clock::get()?;

    lending_operations::clear_expired_borrow_order_on_initiating_obligation_ownership_transfer(
        obligation, clock,
    )?;
    lending_checks::obligation_ownership_transfer_precondition_checks(
        &ctx.accounts.instruction_sysvar_account,
        obligation,
    )?;
    obligation.check_ownership_transfer_approved()?;

    obligation.accept_ownership()?;

    msg!(
        "Completed ownership transfer for obligation {} to new owner {}",
        ctx.accounts.obligation.key(),
        ctx.accounts.pending_owner.key()
    );

    Ok(())
}

#[derive(Accounts)]
pub struct AcceptObligationOwnership<'info> {

    pub pending_owner: Signer<'info>,

    #[account(
        mut,
        has_one = pending_owner @ LendingError::ObligationInvalidPendingOwner
    )]
    pub obligation: AccountLoader<'info, Obligation>,

    /// CHECK: Syvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
