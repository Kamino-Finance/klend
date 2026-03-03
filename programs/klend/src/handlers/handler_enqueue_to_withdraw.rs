use anchor_lang::{prelude::*, solana_program::program_option::COption, Accounts};
use anchor_spl::{
    token::Token,
    token_interface::{self, Mint, TokenAccount},
};
use solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId};

use crate::{
    lending_market::{lending_checks, lending_operations},
    utils::{
        accounts::{default_array, has_ata_address},
        seeds, token_transfer, WITHDRAW_TICKET_SIZE,
    },
    withdraw_ticket::WithdrawTicket,
    LendingAction, LendingError, LendingMarket, Reserve,
};

pub fn process(ctx: Context<EnqueueToWithdraw>, collateral_amount: u64) -> Result<()> {
    lending_checks::enqueue_to_withdraw_checks(ctx.accounts)?;

    let lending_market = &ctx.accounts.lending_market.load()?;
    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let mut withdraw_ticket = ctx.accounts.withdraw_ticket.load_init()?;
    let clock = &Clock::get()?;

    require!(
        lending_market.is_withdraw_ticket_issuance_enabled(),
        LendingError::WithdrawTicketIssuanceDisabled,
    );

   
   
    lending_operations::refresh_reserve(reserve, clock, None, lending_market.referral_fee_bps)?;

    let initial_owner_queued_collateral_vault_balance =
        ctx.accounts.owner_queued_collateral_vault.amount;
    let initial_queued_collateral_amount = reserve.withdraw_queue.queued_collateral_amount;

    let sequence_number =
        lending_operations::enqueue_to_withdraw(lending_market, reserve, collateral_amount)?;

    *withdraw_ticket = WithdrawTicket {
        sequence_number,
        owner: ctx.accounts.owner.key(),
        reserve: ctx.accounts.reserve.key(),
        user_destination_liquidity_ta: ctx.accounts.user_destination_liquidity_ta.key(),
        queued_collateral_amount: collateral_amount,
        created_at_timestamp: clock.unix_timestamp.try_into().expect("negative timestamp"),
        invalid: 0,
        alignment_padding: default_array(),
        end_padding: default_array(),
    };

    token_transfer::enqueue_collateral_transfer(
        ctx.accounts.user_source_collateral_ta.to_account_info(),
        ctx.accounts.owner_queued_collateral_vault.to_account_info(),
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.reserve_collateral_mint.to_account_info(),
        ctx.accounts.collateral_token_program.to_account_info(),
        collateral_amount,
        ctx.accounts.reserve_collateral_mint.decimals,
    )?;

   
   
   
   
   
    if has_ata_address(
        ctx.accounts.user_destination_liquidity_ta.as_ref(),
        ctx.accounts.owner.key,
        &reserve.liquidity.mint_pubkey,
        &reserve.liquidity.token_program,
    ) {
        drop(withdraw_ticket);
        token_transfer::destination_ata_rent_prepayment_transfer(
            ctx.accounts.owner.to_account_info(),
            ctx.accounts.withdraw_ticket.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
        )?;
    }

    lending_checks::post_transfer_owner_queued_collateral_vault_balance_checks(
        token_interface::accessor::amount(
            &ctx.accounts.owner_queued_collateral_vault.to_account_info(),
        )?,
        reserve.withdraw_queue.queued_collateral_amount,
        initial_owner_queued_collateral_vault_balance,
        initial_queued_collateral_amount,
        LendingAction::Additive(collateral_amount),
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct EnqueueToWithdraw<'info> {

   
    #[account(mut)]
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


    #[account(mut,
        token::mint = reserve_collateral_mint,
        token::authority = owner,
        token::token_program = collateral_token_program,
    )]
    pub user_source_collateral_ta: Box<InterfaceAccount<'info, TokenAccount>>,



   
   
   
    #[account(
        token::mint = reserve_liquidity_mint,
        token::authority = owner,
        token::token_program = reserve.load()?.liquidity.token_program,
        constraint = user_destination_liquidity_ta.delegate == COption::None @ LendingError::InvalidTokenAccountState,
        constraint = !user_destination_liquidity_ta.is_frozen() @ LendingError::InvalidTokenAccountState,
    )]
    pub user_destination_liquidity_ta: Box<InterfaceAccount<'info, TokenAccount>>,

   
    #[account(
        address = reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = reserve.load()?.liquidity.token_program,
    )]
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        address = reserve.load()?.collateral.mint_pubkey,
    )]
    pub reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,



    pub collateral_token_program: Program<'info, Token>,


    #[account(init,
        seeds = [seeds::WITHDRAW_TICKET, reserve.key().as_ref(), &reserve.load()?.withdraw_queue.next_issued_ticket_sequence_number.to_le_bytes()],
        bump,
        payer = owner,
        space = WITHDRAW_TICKET_SIZE + 8,
    )]
    pub withdraw_ticket: AccountLoader<'info, WithdrawTicket>,



    #[account(init_if_needed,
        seeds = [seeds::OWNER_QUEUED_COLLATERAL_VAULT, reserve.key().as_ref(), owner.key().as_ref()],
        bump,
        payer = owner,
        token::mint = reserve_collateral_mint,
        token::authority = lending_market_authority,
        token::token_program = collateral_token_program,
    )]
    pub owner_queued_collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,


    pub system_program: Program<'info, System>,

    /// CHECK: Sysvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
