use std::ops::Deref;

use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use super::handler_refresh_obligation_farms_for_reserve::*;
use crate::{
    borrow_order_operations,
    handlers::{borrow_obligation_liquidity_process_impl, BorrowObligationLiquidity},
    refresh_farms,
    state::{obligation::Obligation, LendingMarket, Reserve},
    utils::{ctx_event_emitter, seeds},
    BorrowSize, LendingError, ReferrerTokenState, ReserveFarmKind,
};

pub fn process<'info>(ctx: Context<'_, '_, '_, 'info, FillBorrowOrder<'info>>) -> Result<()> {
    process_impl(&ctx)?;
    refresh_farms!(
        ctx.accounts.borrow_accounts,
        [(
            ctx.accounts.borrow_accounts.borrow_reserve,
            ctx.accounts.farms_accounts,
            Debt,
        )],
    );
    Ok(())
}

fn process_impl<'info>(ctx: &Context<'_, '_, '_, 'info, FillBorrowOrder<'info>>) -> Result<()> {
   
   

    let accounts = &ctx.accounts.borrow_accounts;
    let remaining_accounts = ctx.remaining_accounts;

   
    let order_remaining_amount = accounts
        .obligation
        .load()?
        .borrow_order
        .remaining_debt_amount;

   
    let fill_amount = borrow_obligation_liquidity_process_impl(
        &BorrowObligationLiquidity::from(accounts.clone()),
        remaining_accounts,
        BorrowSize::AtMost(order_remaining_amount),
    )?;

   
    let borrow_reserve = accounts.borrow_reserve.load()?;
    let lending_market = accounts.lending_market.load()?;
    let mut obligation = accounts.obligation.load_mut()?;
    let clock = &Clock::get()?;

   
    borrow_order_operations::fill_borrow_order(
        lending_market.deref(),
        &borrow_reserve,
        &mut obligation.borrow_order,
        clock,
        fill_amount,
        ctx_event_emitter!(ctx),
    )
}

impl<'info> From<FillBorrowOrderAccounts<'info>> for BorrowObligationLiquidity<'info> {
    fn from(accounts: FillBorrowOrderAccounts<'info>) -> Self {
        let FillBorrowOrderAccounts {
            payer,
            obligation,
            lending_market,
            lending_market_authority,
            borrow_reserve,
            borrow_reserve_liquidity_mint,
            reserve_source_liquidity,
            borrow_reserve_liquidity_fee_receiver,
            user_destination_liquidity,
            referrer_token_state,
            token_program,
        } = accounts;

       
       
       
       
       
       
       
        let owner = payer.clone();

       
       
       
        let instruction_sysvar_account = payer.to_account_info();

       
       
       
       
       
       
        Self {
            owner,
            obligation,
            lending_market,
            lending_market_authority,
            borrow_reserve,
            borrow_reserve_liquidity_mint,
            reserve_source_liquidity,
            borrow_reserve_liquidity_fee_receiver,
            user_destination_liquidity,
            referrer_token_state,
            token_program,
            instruction_sysvar_account,
        }
    }
}

#[derive(Accounts, Clone)]
pub struct FillBorrowOrderAccounts<'info> {
    pub payer: Signer<'info>,


    #[account(mut,
        has_one = lending_market,
    )]
    pub obligation: AccountLoader<'info, Obligation>,


    pub lending_market: AccountLoader<'info, LendingMarket>,



    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,




    #[account(mut,
        has_one = lending_market,
        constraint = borrow_reserve.load()?.liquidity.mint_pubkey == obligation.load()?.borrow_order.debt_liquidity_mint @ LendingError::BorrowOrderDebtLiquidityMintMismatch,
    )]
    pub borrow_reserve: AccountLoader<'info, Reserve>,


    #[account(
        address = borrow_reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = token_program,
    )]
    pub borrow_reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,


    #[account(mut,
        address = borrow_reserve.load()?.liquidity.supply_vault,
    )]
    pub reserve_source_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,


    #[account(mut,
        address = borrow_reserve.load()?.liquidity.fee_vault,
    )]
    pub borrow_reserve_liquidity_fee_receiver: Box<InterfaceAccount<'info, TokenAccount>>,






    #[account(mut,
        address = obligation.load()?.borrow_order.filled_debt_destination,
        token::mint = reserve_source_liquidity.mint,
        token::authority = obligation.load()?.owner,
    )]
    pub user_destination_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,


    #[account(mut)]
    pub referrer_token_state: Option<AccountLoader<'info, ReferrerTokenState>>,


    pub token_program: Interface<'info, TokenInterface>,
}

#[event_cpi]
#[derive(Accounts)]
pub struct FillBorrowOrder<'info> {
    pub borrow_accounts: FillBorrowOrderAccounts<'info>,
    pub farms_accounts: OptionalObligationFarmsAccounts<'info>,
    pub farms_program: Program<'info, farms::program::Farms>,
}
