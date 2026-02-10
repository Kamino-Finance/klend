use anchor_lang::{prelude::*, Accounts};
use anchor_spl::{
    token::Token,
    token_interface::{self, Mint, TokenAccount},
};
use solana_program::{
    program_option::COption,
    sysvar::{instructions::Instructions as SysInstructions, SysvarId},
};

use crate::{
    gen_signer_seeds,
    lending_market::{lending_checks, lending_operations},
    state::{LendingMarket, Reserve},
    utils::{seeds, token_transfer},
    withdraw_ticket::WithdrawTicket,
    LendingError,
};

pub fn process(ctx: Context<RecoverInvalidTicketCollateral>) -> Result<()> {
    lending_checks::recover_invalid_ticket_collateral_checks(ctx.accounts)?;

    let lending_market = &ctx.accounts.lending_market.load()?;
    let withdraw_ticket = &ctx.accounts.withdraw_ticket.load()?;

    let initial_owner_queued_collateral_vault_balance =
        ctx.accounts.owner_queued_collateral_vault.amount;
    let initial_user_source_collateral_balance = ctx.accounts.user_source_collateral.amount;

    token_transfer::recover_withdraw_queue_collateral_transfer(
        ctx.accounts.collateral_token_program.to_account_info(),
        ctx.accounts.reserve_collateral_mint.to_account_info(),
        ctx.accounts.owner_queued_collateral_vault.to_account_info(),
        ctx.accounts.user_source_collateral.to_account_info(),
        ctx.accounts.lending_market_authority.clone(),
        gen_signer_seeds!(
            ctx.accounts.lending_market.key(),
            lending_market.bump_seed as u8
        ),
        withdraw_ticket.queued_collateral_amount,
        ctx.accounts.reserve_collateral_mint.decimals,
    )?;

    lending_checks::post_ticket_collateral_recovery_owner_queued_collateral_vault_balance_checks(
        token_interface::accessor::amount(
            &ctx.accounts.owner_queued_collateral_vault.to_account_info(),
        )?,
        token_interface::accessor::amount(&ctx.accounts.user_source_collateral.to_account_info())?,
        initial_owner_queued_collateral_vault_balance,
        initial_user_source_collateral_balance,
        withdraw_ticket.queued_collateral_amount,
    )?;

    Ok(())
}

#[derive(Accounts)]
#[instruction(ticket_sequence_number: u64)]
pub struct RecoverInvalidTicketCollateral<'info> {





    pub payer: Signer<'info>,


    pub lending_market: AccountLoader<'info, LendingMarket>,



    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,


    #[account(
        has_one = lending_market
    )]
    pub reserve: AccountLoader<'info, Reserve>,


    #[account(
        address = reserve.load()?.collateral.mint_pubkey,
        mint::token_program = collateral_token_program,
    )]
    pub reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,



    #[account(mut,
        seeds = [seeds::OWNER_QUEUED_COLLATERAL_VAULT, reserve.key().as_ref(), withdraw_ticket.load()?.owner.as_ref()],
        bump,
        token::mint = reserve_collateral_mint,
        token::authority = lending_market_authority,
    )]
    pub owner_queued_collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,





   
    #[account(mut,
        token::mint = reserve_collateral_mint,
        token::authority = withdraw_ticket.load()?.owner,
        constraint = user_source_collateral.delegate == COption::None @ LendingError::InvalidTokenAccountState,
        constraint = lending_operations::utils::is_allowed_signer_to_use_destination_token_account(
            payer.key(),
            user_source_collateral.key(),
            withdraw_ticket.load()?.owner,
            reserve_collateral_mint.key(),
            collateral_token_program.key()
        ) @ LendingError::InvalidSigner,
    )]
    pub user_source_collateral: Box<InterfaceAccount<'info, TokenAccount>>,


    pub collateral_token_program: Program<'info, Token>,



    #[account(mut,
        close = withdraw_ticket_owner,
        seeds = [seeds::WITHDRAW_TICKET, reserve.key().as_ref(), &ticket_sequence_number.to_le_bytes()],
        constraint = !withdraw_ticket.load()?.is_valid() @ LendingError::WithdrawTicketStillValid,
        bump,
    )]
    pub withdraw_ticket: AccountLoader<'info, WithdrawTicket>,



    #[account(mut,
        address = withdraw_ticket.load()?.owner,
    )]
    pub withdraw_ticket_owner: AccountInfo<'info>,

    /// CHECK: Sysvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
