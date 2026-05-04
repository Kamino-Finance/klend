use anchor_lang::{prelude::*, Accounts};

use crate::{
    lending_market::{lending_checks, lending_operations},
    state::{GlobalConfig, Obligation},
    utils::seeds,
    LendingError,
};





pub fn process(ctx: Context<ApproveObligationOwnershipTransfer>) -> Result<()> {
    let obligation = &mut ctx.accounts.obligation.load_mut()?;
    let clock = &Clock::get()?;

    lending_operations::clear_expired_borrow_order_on_initiating_obligation_ownership_transfer(
        obligation, clock,
    )?;
    obligation.check_ownership_transfer_in_initiated_state()?;
    lending_checks::obligation_has_no_active_borrow_orders_check(obligation)?;

    obligation.approve_ownership_transfer()?;

    msg!(
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
}
