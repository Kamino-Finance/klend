use std::ops::Deref;

use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::{
    gen_signer_seeds,
    handler_refresh_obligation_farms_for_reserve::*,
    lending_market::{lending_checks, lending_operations},
    obligation_order_operations::OpportunityType,
    refresh_farms,
    state::{obligation::Obligation, LendingMarket, Reserve},
    utils::{accounts::ObligationRemainingAccounts, seeds, token_transfer},
    ExecuteDeleverageOrderResult, ExecuteLeverUpOrderResult, ExecuteObligationOrderResult,
    LendingAction, LendingError, ReferrerTokenState, ReserveFarmKind,
};

pub fn process<'info>(
    ctx: Context<'_, '_, '_, 'info, ExecuteObligationOrder<'info>>,
    order_index: u8,
    expected_opportunity_type: u8,
    max_given_liquidity_amount: u64,
    min_received_liquidity_amount: u64,
) -> Result<()> {
    process_impl(
        &ctx.accounts.order_execution_accounts,
        ctx.remaining_accounts,
        order_index,
        expected_opportunity_type,
        max_given_liquidity_amount,
        min_received_liquidity_amount,
    )?;
    refresh_farms!(
        ctx.accounts.order_execution_accounts,
        [
            (
                ctx.accounts.order_execution_accounts.collateral_reserve,
                ctx.accounts.collateral_farms_accounts,
                Collateral,
            ),
            (
                ctx.accounts.order_execution_accounts.debt_reserve,
                ctx.accounts.debt_farms_accounts,
                Debt,
            )
        ],
    );
    Ok(())
}

fn process_impl<'info>(
    accounts: &ExecuteObligationOrderAccounts<'info>,
    remaining_accounts: &[AccountInfo<'info>],
    order_index: u8,
    expected_opportunity_type: u8,
    max_given_liquidity_amount: u64,
    min_received_liquidity_amount: u64,
) -> Result<()> {
    let expected_opportunity_type = OpportunityType::try_from(expected_opportunity_type)
        .map_err(|_| error!(LendingError::ObligationOrderOpportunityTypeMismatch))?;
    lending_checks::execute_obligation_order_checks(accounts, expected_opportunity_type)?;

   
    let clock = Clock::get()?;
    let lending_market_address = accounts.lending_market.key();
    let lending_market = accounts.lending_market.load()?;
    let mut obligation = accounts.obligation.load_mut()?;
    let other_accounts = ObligationRemainingAccounts::parse(&obligation, remaining_accounts)?;

   
    let debt_reserve_before = lending_checks::capture_reserve_accounting_and_balance(
        accounts.debt_reserve.load()?.deref(),
        &accounts.debt_reserve_liquidity_supply,
    )?;
    let collateral_reserve_before = lending_checks::capture_reserve_accounting_and_balance(
        accounts.collateral_reserve.load()?.deref(),
        &accounts.collateral_reserve_liquidity_supply,
    )?;

   
    let execution_result = lending_operations::execute_obligation_order(
        &lending_market,
        &accounts.collateral_reserve,
        &accounts.debt_reserve,
        &mut obligation,
        &clock,
        usize::from(order_index),
        expected_opportunity_type,
        max_given_liquidity_amount,
        min_received_liquidity_amount,
        other_accounts.deposit_reserves(),
        other_accounts.borrow_reserves(),
        other_accounts.referrer_token_states(),
        accounts.referrer_token_state.as_ref(),
    )?;
    drop(obligation);

   
    lending_checks::execute_obligation_order_slippage_check(
        &execution_result,
        max_given_liquidity_amount,
        min_received_liquidity_amount,
    )?;

   
    let authority_signer_seeds = gen_signer_seeds!(
        lending_market_address.as_ref(),
        lending_market.bump_seed as u8
    );
    match execution_result {
        ExecuteObligationOrderResult::Deleverage(result) => apply_deleverage_effects(
            accounts,
            result,
            authority_signer_seeds,
            debt_reserve_before,
            collateral_reserve_before,
        ),
        ExecuteObligationOrderResult::LeverUp(result) => apply_lever_up_effects(
            accounts,
            result,
            authority_signer_seeds,
            debt_reserve_before,
            collateral_reserve_before,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn apply_deleverage_effects(
    accounts: &ExecuteObligationOrderAccounts<'_>,
    result: ExecuteDeleverageOrderResult,
    authority_signer_seeds: &[&[u8]],
    debt_reserve_before: lending_checks::ReserveAccountingAndBalance,
    collateral_reserve_before: lending_checks::ReserveAccountingAndBalance,
) -> Result<()> {
    let ExecuteDeleverageOrderResult {
        repay_amount,
        early_repay_penalty,
        withdraw_collateral_amount,
        withdraw_liquidity_amount,
        protocol_fee,
        obligation_closed,
    } = result;

   
    let repay_amount_with_penalty = repay_amount + early_repay_penalty;
    token_transfer::repay_obligation_liquidity_transfer(
        accounts.debt_liquidity_token_program.to_account_info(),
        accounts.debt_reserve_liquidity_mint.to_account_info(),
        accounts.executor_debt_liquidity_ta.to_account_info(),
        accounts.debt_reserve_liquidity_supply.to_account_info(),
        accounts.executor.to_account_info(),
        repay_amount_with_penalty,
        accounts.debt_reserve_liquidity_mint.decimals,
    )?;
    token_transfer::withdraw_obligation_collateral_transfer(
        accounts.collateral_token_program.to_account_info(),
        accounts.executor_collateral_ctoken_ta.to_account_info(),
        accounts
            .collateral_reserve_collateral_supply
            .to_account_info(),
        accounts.lending_market_authority.to_account_info(),
        authority_signer_seeds,
        withdraw_collateral_amount,
    )?;
    token_transfer::redeem_reserve_collateral_transfer(
        accounts.collateral_token_program.to_account_info(),
        accounts
            .collateral_liquidity_token_program
            .to_account_info(),
        accounts.collateral_reserve_liquidity_mint.to_account_info(),
        accounts
            .collateral_reserve_collateral_mint
            .to_account_info(),
        accounts.executor_collateral_ctoken_ta.to_account_info(),
        accounts.executor.to_account_info(),
        accounts
            .collateral_reserve_liquidity_supply
            .to_account_info(),
        accounts.executor_collateral_liquidity_ta.to_account_info(),
        accounts.lending_market_authority.to_account_info(),
        authority_signer_seeds,
        withdraw_collateral_amount,
        withdraw_liquidity_amount,
        accounts.collateral_reserve_liquidity_mint.decimals,
    )?;
    token_transfer::reserve_fee_transfer(
        accounts
            .collateral_liquidity_token_program
            .to_account_info(),
        accounts.collateral_reserve_liquidity_mint.to_account_info(),
        accounts.executor_collateral_liquidity_ta.to_account_info(),
        accounts
            .collateral_reserve_liquidity_fee_receiver
            .to_account_info(),
        accounts.executor.to_account_info(),
        protocol_fee,
        accounts.collateral_reserve_liquidity_mint.decimals,
    )?;

   
    if obligation_closed {
        accounts
            .obligation
            .close(accounts.obligation_owner.to_account_info())?;
    }

   
    let collateral_reserve_after = lending_checks::capture_reserve_accounting_and_balance(
        accounts.collateral_reserve.load()?.deref(),
        &accounts.collateral_reserve_liquidity_supply,
    )?;
    let debt_reserve_after = lending_checks::capture_reserve_accounting_and_balance(
        accounts.debt_reserve.load()?.deref(),
        &accounts.debt_reserve_liquidity_supply,
    )?;

   
    lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
        debt_reserve_after.vault_balance,
        debt_reserve_after.total_available_liquidity_amount,
        debt_reserve_before.vault_balance,
        debt_reserve_before.total_available_liquidity_amount,
        LendingAction::Additive(repay_amount_with_penalty),
    )?;
    lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
        collateral_reserve_after.vault_balance,
        collateral_reserve_after.total_available_liquidity_amount,
        collateral_reserve_before.vault_balance,
        collateral_reserve_before.total_available_liquidity_amount,
        LendingAction::Subtractive(withdraw_liquidity_amount),
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn apply_lever_up_effects(
    accounts: &ExecuteObligationOrderAccounts<'_>,
    result: ExecuteLeverUpOrderResult,
    authority_signer_seeds: &[&[u8]],
    debt_reserve_before: lending_checks::ReserveAccountingAndBalance,
    collateral_reserve_before: lending_checks::ReserveAccountingAndBalance,
) -> Result<()> {
    let ExecuteLeverUpOrderResult {
        origination_fee,
        borrow_liquidity_amount,
        deposit_liquidity_amount,
        deposit_collateral_amount,
        protocol_fee,
    } = result;

   
    token_transfer::deposit_reserve_liquidity_and_obligation_collateral_transfer(
        accounts.executor_collateral_liquidity_ta.to_account_info(),
        accounts
            .collateral_reserve_liquidity_supply
            .to_account_info(),
        accounts.executor.to_account_info(),
        accounts.collateral_reserve_liquidity_mint.to_account_info(),
        accounts
            .collateral_liquidity_token_program
            .to_account_info(),
        accounts
            .collateral_reserve_collateral_mint
            .to_account_info(),
        accounts
            .collateral_reserve_collateral_supply
            .to_account_info(),
        accounts.collateral_token_program.to_account_info(),
        accounts.lending_market_authority.to_account_info(),
        authority_signer_seeds,
        deposit_liquidity_amount,
        accounts.collateral_reserve_liquidity_mint.decimals,
        deposit_collateral_amount,
    )?;
    token_transfer::borrow_obligation_liquidity_transfer(
        accounts.debt_liquidity_token_program.to_account_info(),
        accounts.debt_reserve_liquidity_mint.to_account_info(),
        accounts.debt_reserve_liquidity_supply.to_account_info(),
        accounts.executor_debt_liquidity_ta.to_account_info(),
        accounts.lending_market_authority.to_account_info(),
        authority_signer_seeds,
        borrow_liquidity_amount,
        accounts.debt_reserve_liquidity_mint.decimals,
    )?;
    if origination_fee > 0 {
        token_transfer::send_origination_fees_transfer(
            accounts.debt_liquidity_token_program.to_account_info(),
            accounts.debt_reserve_liquidity_mint.to_account_info(),
            accounts.debt_reserve_liquidity_supply.to_account_info(),
            accounts
                .debt_reserve_liquidity_fee_receiver
                .to_account_info(),
            accounts.lending_market_authority.to_account_info(),
            authority_signer_seeds,
            origination_fee,
            accounts.debt_reserve_liquidity_mint.decimals,
        )?;
    }
    token_transfer::reserve_fee_transfer(
        accounts.debt_liquidity_token_program.to_account_info(),
        accounts.debt_reserve_liquidity_mint.to_account_info(),
        accounts.executor_debt_liquidity_ta.to_account_info(),
        accounts
            .debt_reserve_liquidity_fee_receiver
            .to_account_info(),
        accounts.executor.to_account_info(),
        protocol_fee,
        accounts.debt_reserve_liquidity_mint.decimals,
    )?;

   
    let collateral_reserve_after = lending_checks::capture_reserve_accounting_and_balance(
        accounts.collateral_reserve.load()?.deref(),
        &accounts.collateral_reserve_liquidity_supply,
    )?;
    let debt_reserve_after = lending_checks::capture_reserve_accounting_and_balance(
        accounts.debt_reserve.load()?.deref(),
        &accounts.debt_reserve_liquidity_supply,
    )?;

   
    lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
        debt_reserve_after.vault_balance,
        debt_reserve_after.total_available_liquidity_amount,
        debt_reserve_before.vault_balance,
        debt_reserve_before.total_available_liquidity_amount,
        LendingAction::Subtractive(borrow_liquidity_amount + origination_fee),
    )?;
    lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
        collateral_reserve_after.vault_balance,
        collateral_reserve_after.total_available_liquidity_amount,
        collateral_reserve_before.vault_balance,
        collateral_reserve_before.total_available_liquidity_amount,
        LendingAction::Additive(deposit_liquidity_amount),
    )?;

    Ok(())
}

#[derive(Accounts, Clone)]
pub struct ExecuteObligationOrderAccounts<'info> {

    pub executor: Signer<'info>,


    #[account(mut,
        has_one = lending_market,
    )]
    pub obligation: AccountLoader<'info, Obligation>,





    /// CHECK: address-verified against [Self::obligation]'s `owner` field.
    #[account(mut,
        address = obligation.load()?.owner,
    )]
    pub obligation_owner: AccountInfo<'info>,



    pub lending_market: AccountLoader<'info, LendingMarket>,



    /// CHECK: Verified through create_program_address.
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,


    #[account(mut,
        has_one = lending_market,
    )]
    pub debt_reserve: AccountLoader<'info, Reserve>,


    #[account(
        address = debt_reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = debt_liquidity_token_program,
    )]
    pub debt_reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,


    #[account(mut,
        address = debt_reserve.load()?.liquidity.supply_vault,
    )]
    pub debt_reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,



    #[account(mut,
        address = debt_reserve.load()?.liquidity.fee_vault,
    )]
    pub debt_reserve_liquidity_fee_receiver: Box<InterfaceAccount<'info, TokenAccount>>,



    #[account(mut,
        has_one = lending_market,
    )]
    pub collateral_reserve: AccountLoader<'info, Reserve>,


    #[account(
        address = collateral_reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = collateral_liquidity_token_program,
    )]
    pub collateral_reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,


    #[account(mut,
        address = collateral_reserve.load()?.collateral.mint_pubkey,
    )]
    pub collateral_reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,



    #[account(mut,
        address = collateral_reserve.load()?.collateral.supply_vault,
    )]
    pub collateral_reserve_collateral_supply: Box<InterfaceAccount<'info, TokenAccount>>,



    #[account(mut,
        address = collateral_reserve.load()?.liquidity.supply_vault,
    )]
    pub collateral_reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,


    #[account(mut,
        address = collateral_reserve.load()?.liquidity.fee_vault,
    )]
    pub collateral_reserve_liquidity_fee_receiver: Box<InterfaceAccount<'info, TokenAccount>>,



    #[account(mut,
        token::mint = debt_reserve_liquidity_mint,
        token::authority = executor,
    )]
    pub executor_debt_liquidity_ta: Box<InterfaceAccount<'info, TokenAccount>>,


    #[account(mut,
        token::mint = collateral_reserve_collateral_mint,
        token::authority = executor,
    )]
    pub executor_collateral_ctoken_ta: Box<InterfaceAccount<'info, TokenAccount>>,



    #[account(mut,
        token::mint = collateral_reserve_liquidity_mint,
        token::authority = executor,
    )]
    pub executor_collateral_liquidity_ta: Box<InterfaceAccount<'info, TokenAccount>>,



    pub collateral_token_program: Program<'info, Token>,


    pub debt_liquidity_token_program: Interface<'info, TokenInterface>,


    pub collateral_liquidity_token_program: Interface<'info, TokenInterface>,

    /// CHECK: Sysvar Instruction allowing introspection, fixed address.
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,


    #[account(mut)]
    pub referrer_token_state: Option<AccountLoader<'info, ReferrerTokenState>>,
}

#[derive(Accounts)]
pub struct ExecuteObligationOrder<'info> {
    pub order_execution_accounts: ExecuteObligationOrderAccounts<'info>,
    pub collateral_farms_accounts: OptionalObligationFarmsAccounts<'info>,
    pub debt_farms_accounts: OptionalObligationFarmsAccounts<'info>,
    pub farms_program: Program<'info, farms::program::Farms>,
}
