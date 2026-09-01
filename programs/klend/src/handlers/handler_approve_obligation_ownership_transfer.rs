use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};

use crate::{
    lending_market::{ix_utils, lending_checks, lending_operations},
    state::{GlobalConfig, Obligation},
    utils::seeds,
    xmsg, LendingError,
};





pub fn process(ctx: Context<ApproveObligationOwnershipTransfer>) -> Result<()> {
    ix_utils::check_no_advance_nonce_ix_within_tx(&ctx.accounts.instruction_sysvar_account)?;

    let obligation = &mut ctx.accounts.obligation.load_mut()?;
    let clock = &Clock::get()?;

    lending_operations::clear_expired_borrow_orders_for_ownership_transfer(obligation, clock)?;
    obligation.check_ownership_transfer_in_initiated_state()?;
    lending_checks::obligation_has_no_active_orders_check(obligation)?;

    obligation.approve_ownership_transfer()?;

    xmsg!(
        "Approved ownership transfer for obligation {} to pending owner {}",
        ctx.accounts.obligation.key(),
        ctx.accounts.pending_owner.key()
    );

    Ok(())
}

#[derive(Accounts)]
pub struct ApproveObligationOwnershipTransfer<'info> {

    pub global_admin: Signer<'info>,

    #[account(
        seeds = [seeds::GLOBAL_CONFIG_STATE],
        bump,
        has_one = global_admin
    )]
    pub global_config: AccountLoader<'info, GlobalConfig>,

    #[account(
        mut,
        has_one = pending_owner @ LendingError::ObligationInvalidPendingOwner
    )]
    pub obligation: AccountLoader<'info, Obligation>,



    /// CHECK: Verified via constraint that matches obligation.pending_owner
    pub pending_owner: AccountInfo<'info>,

    /// CHECK: Sysvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
