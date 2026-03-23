use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface};

use crate::{
    fraction::Fraction,
    gen_signer_seeds,
    handlers::handler_refresh_obligation_farms_for_reserve::*,
    lending_market::{lending_checks, lending_operations},
    refresh_farms,
    state::{obligation::Obligation, LendingMarket, Reserve},
    utils::{seeds, token_transfer},
    FixedTermRolloverResult, LendingError, ReserveFarmKind,
};

pub fn process<'info>(
    ctx: Context<'_, '_, '_, 'info, RolloverFixedTermBorrow<'info>>,
) -> Result<()> {
    let needs_farms_refresh = process_impl(&ctx.accounts.rollover_accounts)?;
    if needs_farms_refresh {
        refresh_farms!(
            ctx.accounts.rollover_accounts,
            [
                (
                    ctx.accounts.rollover_accounts.source_borrow_reserve,
                    ctx.accounts.source_farms_accounts,
                    Debt,
                ),
                (
                    ctx.accounts.rollover_accounts.target_borrow_reserve,
                    ctx.accounts.target_farms_accounts,
                    Debt,
                )
            ],
        );
    }
    Ok(())
}

fn process_impl(accounts: &RolloverAccounts) -> Result<bool> {
    lending_checks::rollover_fixed_term_borrow_checks(accounts)?;

   
    let market_address = accounts.lending_market.key();
    let market = &accounts.lending_market.load()?;
    let obligation = &mut accounts.obligation.load_mut()?;
    let source_borrow_reserve_address = accounts.source_borrow_reserve.key();
    let target_borrow_reserve_address = accounts.target_borrow_reserve.key();
    let source_borrow_reserve = &mut accounts.source_borrow_reserve.load_mut()?;
    let clock = &Clock::get()?;

   
    let source_reserve_before = capture_reserve_accounting_and_balance(
        source_borrow_reserve,
        &accounts.source_borrow_reserve_liquidity,
    )?;
    let obligation_before = capture_obligation_borrows_accounting(
        obligation,
        source_borrow_reserve_address,
        target_borrow_reserve_address,
    )?;

    if source_borrow_reserve_address == target_borrow_reserve_address {
       
        lending_operations::rollover_borrow_into_same_reserve(
            market,
            source_borrow_reserve_address,
            source_borrow_reserve,
            obligation,
            clock,
        )?;

       

       
        let source_reserve_after = capture_reserve_accounting_and_balance(
            source_borrow_reserve,
            &accounts.source_borrow_reserve_liquidity,
        )?;
        let obligation_after = capture_obligation_borrows_accounting(
            obligation,
            source_borrow_reserve_address,
            target_borrow_reserve_address,
        )?;

       
        lending_checks::rollover_fixed_term_borrow_into_same_reserve_post_checks(
            source_reserve_before,
            obligation_before,
            source_reserve_after,
            obligation_after,
        )?;

        return Ok(false);
    }

   
    let target_borrow_reserve = &mut accounts.target_borrow_reserve.load_mut()?;

   
    let target_reserve_before = capture_reserve_accounting_and_balance(
        target_borrow_reserve,
        &accounts.target_borrow_reserve_liquidity,
    )?;

   
    let rollover_result = lending_operations::rollover_borrow_into_different_reserve(
        market,
        source_borrow_reserve_address,
        source_borrow_reserve,
        target_borrow_reserve_address,
        target_borrow_reserve,
        obligation,
        clock,
    )?;

   
    let FixedTermRolloverResult {
        repaid_amount: _,  
        borrowed_amount: _,
        tokens_to_transfer_over,
    } = &rollover_result;

   
    let authority_signer_seeds = gen_signer_seeds!(market_address.as_ref(), market.bump_seed as u8);
    token_transfer::borrow_obligation_liquidity_transfer(
        accounts.token_program.to_account_info(),
        accounts.liquidity_mint.to_account_info(),
        accounts.target_borrow_reserve_liquidity.to_account_info(),
        accounts.source_borrow_reserve_liquidity.to_account_info(),
        accounts.lending_market_authority.to_account_info(),
        authority_signer_seeds,
        *tokens_to_transfer_over,
        accounts.liquidity_mint.decimals,
    )?;

   
    let source_reserve_after = capture_reserve_accounting_and_balance(
        source_borrow_reserve,
        &accounts.source_borrow_reserve_liquidity,
    )?;
    let target_reserve_after = capture_reserve_accounting_and_balance(
        target_borrow_reserve,
        &accounts.target_borrow_reserve_liquidity,
    )?;
    let obligation_after = capture_obligation_borrows_accounting(
        obligation,
        source_borrow_reserve_address,
        target_borrow_reserve_address,
    )?;

   
    lending_checks::rollover_fixed_term_borrow_into_different_reserve_post_checks(
        lending_checks::RolloverAccountingAndBalances {
            source_reserve: source_reserve_before,
            target_reserve: target_reserve_before,
            obligation: obligation_before,
        },
        lending_checks::RolloverAccountingAndBalances {
            source_reserve: source_reserve_after,
            target_reserve: target_reserve_after,
            obligation: obligation_after,
        },
        rollover_result,
    )?;

    Ok(true)
}

fn capture_reserve_accounting_and_balance(
    reserve: &Reserve,
    reserve_liquidity_vault: &InterfaceAccount<TokenAccount>,
) -> Result<lending_checks::ReserveAccountingAndBalance> {
    Ok(lending_checks::ReserveAccountingAndBalance {
        total_available_liquidity_amount: reserve.total_available_liquidity_amount(),
        borrowed_amount: reserve.liquidity.total_borrow(),
        vault_balance: token_interface::accessor::amount(
            &reserve_liquidity_vault.to_account_info(),
        )?,
    })
}

fn capture_obligation_borrows_accounting(
    obligation: &Obligation,
    source_reserve_address: Pubkey,
    target_reserve_address: Pubkey,
) -> Result<lending_checks::ObligationRolloverAccounting> {
    Ok(lending_checks::ObligationRolloverAccounting {
        source_reserve_borrowed_amount: capture_borrowed_amount(obligation, source_reserve_address),
        target_reserve_borrowed_amount: capture_borrowed_amount(obligation, target_reserve_address),
    })
}

fn capture_borrowed_amount(obligation: &Obligation, borrow_reserve_address: Pubkey) -> Fraction {
    obligation
        .find_liquidity_in_borrows(borrow_reserve_address)
        .ok()
        .map(|(borrow, _index)| borrow.borrowed_amount())
        .unwrap_or_default()
}

#[derive(Accounts, Clone)]
pub struct RolloverAccounts<'info> {
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
    )]
    pub source_borrow_reserve: AccountLoader<'info, Reserve>,



    #[account(mut,
        has_one = lending_market,
        constraint = target_borrow_reserve.load()?.liquidity.mint_pubkey == source_borrow_reserve.load()?.liquidity.mint_pubkey @ LendingError::BorrowRolloverLiquidityMintMismatch,
    )]
    pub target_borrow_reserve: AccountLoader<'info, Reserve>,



    #[account(
        address = source_borrow_reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = token_program,
    )]
    pub liquidity_mint: Box<InterfaceAccount<'info, Mint>>,


    #[account(mut,
        address = source_borrow_reserve.load()?.liquidity.supply_vault,
    )]
    pub source_borrow_reserve_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,


    #[account(mut,
        address = target_borrow_reserve.load()?.liquidity.supply_vault,
    )]
    pub target_borrow_reserve_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,


    pub token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct RolloverFixedTermBorrow<'info> {
    pub rollover_accounts: RolloverAccounts<'info>,
    pub source_farms_accounts: OptionalObligationFarmsAccounts<'info>,
    pub target_farms_accounts: OptionalObligationFarmsAccounts<'info>,
    pub farms_program: Program<'info, farms::program::Farms>,
}
