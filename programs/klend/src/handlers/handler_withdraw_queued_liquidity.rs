use anchor_lang::{prelude::*, Accounts};
use anchor_spl::{
    associated_token::AssociatedToken,
    token::Token,
    token_interface::{self, Mint, TokenAccount, TokenInterface},
};
use solana_program::{
    system_program,
    sysvar::{instructions::Instructions as SysInstructions, SysvarId},
};

use crate::{
    gen_signer_seeds,
    lending_market::{lending_checks, lending_operations},
    state::{LendingMarket, Reserve},
    utils::{
        accounts::{create_ata, has_ata_address},
        constraints, seeds, token_transfer,
    },
    withdraw_ticket::WithdrawTicket,
    LendingAction, LendingError, TicketedWithdrawResult,
};

pub fn process(ctx: Context<WithdrawQueuedLiquidity>) -> Result<bool> {
    lending_checks::withdraw_queued_liquidity_checks(ctx.accounts)?;

    let lending_market = &ctx.accounts.lending_market.load()?;
    let reserve = &mut ctx.accounts.reserve.load_mut()?;

    require!(
        lending_market.is_withdraw_ticket_redemption_enabled(),
        LendingError::WithdrawTicketRedemptionDisabled,
    );

    let withdraw_ticket = ctx.accounts.withdraw_ticket.load()?;
    let destination_ta_validity = DestinationTokenAccountValidity::resolve(
        &ctx.accounts.user_destination_liquidity,
        &withdraw_ticket.owner,
        &ctx.accounts.reserve_liquidity_mint.to_account_info(),
        &reserve.liquidity.token_program,
    );
    drop(withdraw_ticket);

    let require_closing_ticket = match destination_ta_validity {
        DestinationTokenAccountValidity::AtaToBeCreated => {
            msg!("User's destination liquidity ATA does not exist; creating it");
           
           
            create_ata(
                ctx.accounts.user_destination_liquidity.to_account_info(),
                ctx.accounts.withdraw_ticket_owner.to_account_info(),
                ctx.accounts.reserve_liquidity_mint.to_account_info(),
                ctx.accounts.liquidity_token_program.to_account_info(),
                ctx.accounts.associated_token_program.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.payer.to_account_info(),
                &[],
            )?;
           
           
           
            token_transfer::destination_ata_rent_refund_transfer(
                ctx.accounts.withdraw_ticket.to_account_info(),
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.user_destination_liquidity.data_len(),
            )?;
            true
        }
        DestinationTokenAccountValidity::Invalid => {
            msg!("User's destination liquidity account became incompatible; marking ticket as invalid and skipping over it");
            let mut withdraw_ticket = ctx.accounts.withdraw_ticket.load_mut()?;
            withdraw_ticket.invalid = 1;
            reserve
                .withdraw_queue
                .dequeue(withdraw_ticket.queued_collateral_amount, true);
            return Ok(false);
        }
        DestinationTokenAccountValidity::Valid => {
            false
        }
    };

    let initial_reserve_token_balance = ctx.accounts.reserve_liquidity_supply.amount;
    let initial_reserve_available_liquidity = reserve.total_available_liquidity_amount();
    let initial_owner_queued_collateral_vault_balance =
        ctx.accounts.owner_queued_collateral_vault.amount;
    let initial_queued_collateral_amount = reserve.withdraw_queue.queued_collateral_amount;

    let clock = Clock::get()?;

   
   
    lending_operations::refresh_reserve(reserve, &clock, None, lending_market.referral_fee_bps)?;

    let mut withdraw_ticket = ctx.accounts.withdraw_ticket.load_mut()?;
    let TicketedWithdrawResult {
        liquidity_amount_to_withdraw,
        collateral_amount_to_burn,
    } = lending_operations::withdraw_queued_liquidity(
        lending_market,
        reserve,
        &mut withdraw_ticket,
        &clock,
    )?;

    msg!(
        "pnl: withdrawing queued liquidity {} and burning collateral {}",
        liquidity_amount_to_withdraw,
        collateral_amount_to_burn
    );

    token_transfer::withdraw_and_redeem_reserve_collateral_transfer(
        ctx.accounts.collateral_token_program.to_account_info(),
        ctx.accounts.liquidity_token_program.to_account_info(),
        ctx.accounts.reserve_liquidity_mint.to_account_info(),
        ctx.accounts.reserve_collateral_mint.to_account_info(),
        ctx.accounts.owner_queued_collateral_vault.to_account_info(),
        ctx.accounts.reserve_liquidity_supply.to_account_info(),
        ctx.accounts.user_destination_liquidity.clone(),
        ctx.accounts.lending_market_authority.clone(),
        gen_signer_seeds!(
            ctx.accounts.lending_market.key(),
            lending_market.bump_seed as u8
        ),
        collateral_amount_to_burn,
        liquidity_amount_to_withdraw,
        ctx.accounts.reserve_liquidity_mint.decimals,
    )?;

    lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
        token_interface::accessor::amount(
            &ctx.accounts.reserve_liquidity_supply.to_account_info(),
        )?,
        reserve.total_available_liquidity_amount(),
        initial_reserve_token_balance,
        initial_reserve_available_liquidity,
        LendingAction::Subtractive(liquidity_amount_to_withdraw),
    )?;
    lending_checks::post_transfer_owner_queued_collateral_vault_balance_checks(
        token_interface::accessor::amount(
            &ctx.accounts.owner_queued_collateral_vault.to_account_info(),
        )?,
        reserve.withdraw_queue.queued_collateral_amount,
        initial_owner_queued_collateral_vault_balance,
        initial_queued_collateral_amount,
        LendingAction::Subtractive(collateral_amount_to_burn),
    )?;

    if withdraw_ticket.queued_collateral_amount == 0 {
        msg!("Redeemed entire ticket collateral; closing the ticket account");
        drop(withdraw_ticket);
        ctx.accounts
            .withdraw_ticket
            .close(ctx.accounts.withdraw_ticket_owner.to_account_info())?;
    } else {
        msg!(
            "Ticket's remaining queued collateral: {}",
            withdraw_ticket.queued_collateral_amount
        );
        if require_closing_ticket {
            msg!("Cannot spend the ticket's prepaid rent for destination ATA without closing the ticket itself");
            return err!(LendingError::WithdrawTicketRequiresFullRedemption);
        }
    }

    Ok(true)
}

enum DestinationTokenAccountValidity {
    Valid,
    Invalid,
    AtaToBeCreated,
}

impl DestinationTokenAccountValidity {
    pub fn resolve(
        account: &AccountInfo,
        owner: &Pubkey,
        mint: &AccountInfo,
        token_program: &Pubkey,
    ) -> Self {
        let is_system_owned = account.owner == &system_program::ID;
        let is_token_program_owned = account.owner == token_program;

        if !is_system_owned && !is_token_program_owned {
            return Self::Invalid;
        }

       
        if constraints::token_2022::check_only_supported_extensions_on_liquidity_mint(mint).is_err()
        {
            return Self::Invalid;
        }

        if account.data_is_empty() {
           
            return if has_ata_address(account, owner, mint.key, token_program) {
               

                if constraints::token_2022::check_default_account_state_initialized(mint).is_err() {
                    return Self::Invalid;
                }

                Self::AtaToBeCreated
            } else {
                Self::Invalid
            };
        }

        if is_system_owned {
            return Self::Invalid;
        }

        let Ok(token_account) = TokenAccount::try_deserialize(&mut account.data.borrow().as_ref())
        else {
            return Self::Invalid;
        };

        if &token_account.owner != owner || &token_account.mint != mint.key {
            return Self::Invalid;
        }

       
       
        if token_account.delegate.is_some() || token_account.is_frozen() {
            return Self::Invalid;
        }

       
        if constraints::token_2022::check_only_supported_extensions_on_liquidity_ta(account)
            .is_err()
        {
            return Self::Invalid;
        }

        Self::Valid
    }
}

#[derive(Accounts)]
pub struct WithdrawQueuedLiquidity<'info> {

   
    #[account(mut)]
    pub payer: Signer<'info>,


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
        address = reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = liquidity_token_program,
    )]
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,


    #[account(mut,
        address = reserve.load()?.collateral.mint_pubkey,
        mint::token_program = collateral_token_program,
    )]
    pub reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,


    #[account(mut,
        address = reserve.load()?.liquidity.supply_vault,
    )]
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,



    #[account(mut,
        seeds = [seeds::OWNER_QUEUED_COLLATERAL_VAULT, reserve.key().as_ref(), withdraw_ticket.load()?.owner.as_ref()],
        bump,
        token::mint = reserve_collateral_mint,
        token::authority = lending_market_authority,
    )]
    pub owner_queued_collateral_vault: Box<InterfaceAccount<'info, TokenAccount>>,



   
   
   
   
   
   
   
   
   
    #[account(mut,
        address = withdraw_ticket.load()?.user_destination_liquidity_ta,
    )]
    pub user_destination_liquidity: AccountInfo<'info>,


    pub collateral_token_program: Program<'info, Token>,


    pub liquidity_token_program: Interface<'info, TokenInterface>,






    #[account(mut,
        seeds = [seeds::WITHDRAW_TICKET, reserve.key().as_ref(), &reserve.load()?.withdraw_queue.next_withdrawable_ticket_sequence_number.to_le_bytes()],
        bump,
        constraint = withdraw_ticket.load()?.is_valid() @ LendingError::WithdrawTicketInvalid,
    )]
    pub withdraw_ticket: AccountLoader<'info, WithdrawTicket>,



    #[account(mut,
        address = withdraw_ticket.load()?.owner,
    )]
    pub withdraw_ticket_owner: AccountInfo<'info>,


    pub associated_token_program: Program<'info, AssociatedToken>,


    pub system_program: Program<'info, System>,

    /// CHECK: Sysvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
