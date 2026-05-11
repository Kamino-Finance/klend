use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};

use crate::{
    lending_market::{lending_checks, lending_operations},
    state::Obligation,
};



pub fn process(ctx: Context<InitiateObligationOwnershipTransfer>, new_owner: Pubkey) -> Result<()> {
    let obligation = &mut ctx.accounts.obligation.load_mut()?;
    let clock = &Clock::get()?;

    obligation.check_ownership_transfer_not_in_progress()?;

   
    lending_operations::clear_expired_borrow_order_on_initiating_obligation_ownership_transfer(
        obligation, clock,
    )?;

    lending_checks::obligation_ownership_transfer_precondition_checks(
        &ctx.accounts.instruction_sysvar_account,
        obligation,
    )?;

    obligation.initiate_ownership_transfer(new_owner)?;

    msg!(
        "Initiated ownership transfer for obligation: {} to new owner: {}",
        ctx.accounts.obligation.key(),
        new_owner
    );

    Ok(())
}

#[derive(Accounts)]
pub struct InitiateObligationOwnershipTransfer<'info> {

    pub owner: Signer<'info>,

    #[account(mut, has_one = owner)]
    pub obligation: AccountLoader<'info, Obligation>,

    /// CHECK: Syvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
