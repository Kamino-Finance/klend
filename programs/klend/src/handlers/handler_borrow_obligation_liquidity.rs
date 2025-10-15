use std::cell::RefMut;

use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface};
use lending_checks::validate_referrer_token_state;

use super::handler_refresh_obligation_farms_for_reserve::*;
use crate::{
    check_refresh_ixs, gen_signer_seeds,
    lending_market::{lending_checks, lending_operations},
    refresh_farms,
    state::{obligation::Obligation, CalculateBorrowResult, LendingMarket, Reserve},
    utils::{seeds, token_transfer, FatAccountLoader},
    xmsg, BorrowSize, LendingAction, LendingError, ReferrerTokenState, ReserveFarmKind,
};

pub fn process_v1<'info>(
    ctx: Context<'_, '_, '_, 'info, BorrowObligationLiquidity<'info>>,
    liquidity_amount: u64,
) -> Result<()> {
    check_refresh_ixs!(
        ctx.accounts,
        ctx.accounts.borrow_reserve,
        ReserveFarmKind::Debt
    );
    process_impl(ctx.accounts, ctx.remaining_accounts, liquidity_amount)
}

pub fn process_v2<'info>(
    ctx: Context<'_, '_, '_, 'info, BorrowObligationLiquidityV2<'info>>,
    liquidity_amount: u64,
) -> Result<()> {
    process_impl(
        &ctx.accounts.borrow_accounts,
        ctx.remaining_accounts,
        liquidity_amount,
    )?;
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

fn process_impl<'info>(
    accounts: &BorrowObligationLiquidity<'info>,
    remaining_accounts: &[AccountInfo<'info>],
    liquidity_amount: u64,
) -> Result<()> {
    msg!("liquidity_amount {}", liquidity_amount);
    lending_checks::borrow_obligation_liquidity_checks(accounts)?;

    let borrow_reserve = &mut accounts.borrow_reserve.load_mut()?;
    let lending_market = &accounts.lending_market.load()?;
    let obligation = &mut accounts.obligation.load_mut()?;
    let lending_market_key = accounts.lending_market.key();
    let clock = &Clock::get()?;

    let authority_signer_seeds =
        gen_signer_seeds!(lending_market_key.as_ref(), lending_market.bump_seed as u8);

    let deposit_reserves_iter = remaining_accounts
        .iter()
        .map(|account_info| FatAccountLoader::<Reserve>::try_from(account_info).unwrap());

    let referrer_token_state_option: Option<RefMut<ReferrerTokenState>> =
        if obligation.has_referrer() {
            match &accounts.referrer_token_state {
                Some(referrer_token_state_loader) => {
                    let referrer_token_state = referrer_token_state_loader.load_mut()?;

                    validate_referrer_token_state(
                        &crate::ID,
                        &referrer_token_state,
                        referrer_token_state_loader.key(),
                        borrow_reserve.liquidity.mint_pubkey,
                        obligation.referrer,
                        accounts.borrow_reserve.key(),
                    )?;

                    Some(referrer_token_state)
                }
                None => return err!(LendingError::ReferrerAccountMissing),
            }
        } else {
            None
        };

    let initial_reserve_token_balance =
        token_interface::accessor::amount(&accounts.reserve_source_liquidity.to_account_info())?;
    let initial_reserve_available_liquidity = borrow_reserve.liquidity.available_amount;

    let CalculateBorrowResult {
        receive_amount,
        origination_fee,
        ..
    } = lending_operations::borrow_obligation_liquidity(
        lending_market,
        borrow_reserve,
        obligation,
        BorrowSize::exact_or_all_available(liquidity_amount),
        clock,
        accounts.borrow_reserve.key(),
        referrer_token_state_option,
        deposit_reserves_iter,
    )?;

    xmsg!(
        "pnl: Borrow obligation liquidity {receive_amount} with origination_fee {origination_fee}",
    );

    if origination_fee > 0 {
        token_transfer::send_origination_fees_transfer(
            accounts.token_program.to_account_info(),
            accounts.borrow_reserve_liquidity_mint.to_account_info(),
            accounts.reserve_source_liquidity.to_account_info(),
            accounts
                .borrow_reserve_liquidity_fee_receiver
                .to_account_info(),
            accounts.lending_market_authority.to_account_info(),
            authority_signer_seeds,
            origination_fee,
            accounts.borrow_reserve_liquidity_mint.decimals,
        )?;
    }

    token_transfer::borrow_obligation_liquidity_transfer(
        accounts.token_program.to_account_info(),
        accounts.borrow_reserve_liquidity_mint.to_account_info(),
        accounts.reserve_source_liquidity.to_account_info(),
        accounts.user_destination_liquidity.to_account_info(),
        accounts.lending_market_authority.to_account_info(),
        authority_signer_seeds,
        receive_amount,
        accounts.borrow_reserve_liquidity_mint.decimals,
    )?;

    lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
        token_interface::accessor::amount(&accounts.reserve_source_liquidity.to_account_info())
            .unwrap(),
        borrow_reserve.liquidity.available_amount,
        initial_reserve_token_balance,
        initial_reserve_available_liquidity,
        LendingAction::Subtractive(origination_fee + receive_amount),
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct BorrowObligationLiquidity<'info> {
    pub owner: Signer<'info>,

    #[account(mut,
        has_one = lending_market,
        has_one = owner @ LendingError::InvalidObligationOwner
    )]
    pub obligation: AccountLoader<'info, Obligation>,

    pub lending_market: AccountLoader<'info, LendingMarket>,
    /// CHECK: Verified through create_program_address
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(mut,
        has_one = lending_market,
    )]
    pub borrow_reserve: AccountLoader<'info, Reserve>,

    #[account(
        address = borrow_reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = token_program,
    )]
    pub borrow_reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut,
        address = borrow_reserve.load()?.liquidity.supply_vault
    )]
    pub reserve_source_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        address = borrow_reserve.load()?.liquidity.fee_vault
    )]
    pub borrow_reserve_liquidity_fee_receiver: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = reserve_source_liquidity.mint,
        token::authority = owner,
    )]
    pub user_destination_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub referrer_token_state: Option<AccountLoader<'info, ReferrerTokenState>>,

    pub token_program: Interface<'info, TokenInterface>,

    /// CHECK: Syvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct BorrowObligationLiquidityV2<'info> {
    pub borrow_accounts: BorrowObligationLiquidity<'info>,
    pub farms_accounts: OptionalObligationFarmsAccounts<'info>,
    pub farms_program: Program<'info, farms::program::Farms>,
}
