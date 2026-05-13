use anchor_lang::{prelude::*, Accounts};
use anchor_spl::{
    token::Token,
    token_interface::{self, Mint, TokenAccount},
};
use solana_program::program_option::COption;

use crate::{
    gen_signer_seeds,
    lending_market::{lending_checks, lending_operations},
    state::{LendingMarket, Reserve},
    utils::{seeds, token_transfer},
    withdraw_ticket::WithdrawTicket,
    LendingError,
};










pub fn process(
    ctx: Context<CancelWithdrawTicket>,
    _ticket_sequence_number: u64,    
    collateral_amount_to_cancel: u64,
) -> Result<()> {
    lending_checks::cancel_withdraw_ticket_checks(ctx.accounts)?;

    let lending_market = &ctx.accounts.lending_market.load()?;
    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let withdraw_ticket = &mut ctx.accounts.withdraw_ticket.load_mut()?;
    let clock = &Clock::get()?;

    require!(
        lending_market.is_withdraw_ticket_cancellation_enabled(),
        LendingError::WithdrawTicketCancellationDisabled,
    );

   
   
    lending_operations::refresh_reserve(
        reserve,
        clock,
        None,
        lending_market.referral_fee_bps,
        lending_market.reserve_rewards_max_apr_bps,
    )?;

   
    let initial_owner_queued_collateral_vault_balance =
        ctx.accounts.owner_queued_collateral_vault.amount;
    let initial_user_destination_collateral_balance =
        ctx.accounts.user_destination_collateral.amount;
    let initial_queued_collateral_amount = reserve.withdraw_queue.queued_collateral_amount;

   
    let amount_to_cancel = lending_operations::cancel_withdraw_ticket(
        lending_market,
        reserve,
        withdraw_ticket,
        collateral_amount_to_cancel,
    )?;

    msg!(
        "pnl: Cancelling withdraw ticket and returning {} ctokens to owner, {} ctokens left in the ticket",
        amount_to_cancel, withdraw_ticket.queued_collateral_amount
    );

   
    token_transfer::recover_withdraw_queue_collateral_transfer(
        ctx.accounts.collateral_token_program.to_account_info(),
        ctx.accounts.reserve_collateral_mint.to_account_info(),
        ctx.accounts.owner_queued_collateral_vault.to_account_info(),
        ctx.accounts.user_destination_collateral.to_account_info(),
        ctx.accounts.lending_market_authority.clone(),
        gen_signer_seeds!(
            ctx.accounts.lending_market.key(),
            lending_market.bump_seed as u8
        ),
        amount_to_cancel,
        ctx.accounts.reserve_collateral_mint.decimals,
    )?;

   
    lending_checks::post_cancel_withdraw_ticket_balance_checks(
        token_interface::accessor::amount(
            &ctx.accounts.owner_queued_collateral_vault.to_account_info(),
        )?,
        token_interface::accessor::amount(
            &ctx.accounts.user_destination_collateral.to_account_info(),
        )?,
        reserve.withdraw_queue.queued_collateral_amount,
        initial_owner_queued_collateral_vault_balance,
        initial_user_destination_collateral_balance,
        initial_queued_collateral_amount,
        amount_to_cancel,
    )?;

    Ok(())
}

#[derive(Accounts)]
#[instruction(ticket_sequence_number: u64, collateral_amount_to_cancel: u64)]
pub struct CancelWithdrawTicket<'info> {

    pub owner: Signer<'info>,


    pub lending_market: AccountLoader<'info, LendingMarket>,



    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,


    #[account(mut,
        has_one = lending_market
    )]
    pub reserve: AccountLoader<'info, Reserve>,


    #[account(
        address = reserve.load()?.collateral.mint_pubkey,
        mint::token_program = collateral_token_program,
    )]
    pub reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,



    #[account(mut,
        seeds = [seeds::OWNER_QUEUED_COLLATERAL_VAULT, reserve.key().as_ref(), owner.key().as_ref()],
        bump,
        token::mint = reserve_collateral_mint,
        token::authority = lending_market_authority,
    )]
    pub owner_queued_collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,


    #[account(mut,
        token::mint = reserve_collateral_mint,
        token::authority = owner,
        constraint = user_destination_collateral.delegate == COption::None @ LendingError::InvalidTokenAccountState,
    )]
    pub user_destination_collateral: Box<InterfaceAccount<'info, TokenAccount>>,


    pub collateral_token_program: Program<'info, Token>,


    #[account(mut,
        seeds = [seeds::WITHDRAW_TICKET, reserve.key().as_ref(), &ticket_sequence_number.to_le_bytes()],
        bump,
        has_one = reserve,
        constraint = withdraw_ticket.load()?.owner == owner.key() @ LendingError::InvalidSigner,
        constraint = withdraw_ticket.load()?.is_valid() @ LendingError::WithdrawTicketInvalid,
        constraint = !withdraw_ticket.load()?.is_fully_cancelled() @ LendingError::WithdrawTicketFullyCancelled,
    )]
    pub withdraw_ticket: AccountLoader<'info, WithdrawTicket>,
}
