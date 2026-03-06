use anchor_lang::{
    prelude::*,
    solana_program::{instruction::Instruction, program},
};

use crate::{
    handlers::WithdrawQueuedLiquidity,
    utils::{seeds, UPDATE_KLEND_QUEUE_ACCOUNTING_DISCRIMINATOR},
    withdraw_ticket::WithdrawTicketProgressEvent,
    LendingError, TicketedWithdrawResult,
};



pub fn cpi_update_klend_queue_accounting(
    ctx: &Context<WithdrawQueuedLiquidity>,
    withdraw_ticket_sequence_number: u64,
    ticketed_withdraw_result: TicketedWithdrawResult,
) -> Result<()> {
   
    let (kvault_program, vault) = match (
        &ctx.accounts.progress_callback_program,
        &ctx.accounts.progress_callback_custom_account_0,
    ) {
        (Some(progress_callback_program), Some(vault)) => (progress_callback_program, vault),
        _ => return err!(LendingError::WithdrawTicketProgressCallbackAccountsMissing),
    };
    let TicketedWithdrawResult {
        collateral_amount_to_burn,
        liquidity_amount_to_withdraw,
    } = ticketed_withdraw_result;
    let reserve_address = ctx.accounts.reserve.key();

   
    let accounts = vec![
        AccountMeta::new_readonly(ctx.accounts.withdraw_ticket.key(), true),
        AccountMeta::new_readonly(reserve_address, false),
        AccountMeta::new_readonly(ctx.accounts.lending_market.key(), false),
        AccountMeta::new_readonly(ctx.accounts.owner_queued_collateral_vault.key(), false),
        AccountMeta::new_readonly(ctx.accounts.user_destination_liquidity.key(), false),
        AccountMeta::new(vault.key(), false),
    ];

   
    let account_infos = [
        kvault_program.clone(),
        ctx.accounts.withdraw_ticket.to_account_info(),
        ctx.accounts.reserve.to_account_info(),
        ctx.accounts.lending_market.to_account_info(),
        ctx.accounts.owner_queued_collateral_vault.to_account_info(),
        ctx.accounts.user_destination_liquidity.to_account_info(),
        vault.clone(),
    ];

   
    let withdraw_ticket_seeds = &[
        seeds::WITHDRAW_TICKET,
        reserve_address.as_ref(),
        &withdraw_ticket_sequence_number.to_le_bytes(),
        &[ctx.bumps.withdraw_ticket],
    ];

   
    let data_items = [
        UPDATE_KLEND_QUEUE_ACCOUNTING_DISCRIMINATOR.as_slice(),
        &[WithdrawTicketProgressEvent::QueuedLiquidityWithdrawn.into()],
        &collateral_amount_to_burn.to_le_bytes(),
        &liquidity_amount_to_withdraw.to_le_bytes(),
    ];

   
    let instruction = Instruction {
        program_id: kvault_program.key(),
        accounts,
        data: data_items.concat(),
    };
    program::invoke_signed(&instruction, &account_infos, &[withdraw_ticket_seeds])?;

    Ok(())
}
