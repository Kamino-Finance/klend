use std::cell::RefMut;

use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};
use anchor_spl::token::{self, Token, TokenAccount};
use lending_checks::validate_referrer_token_state;

use crate::{
    check_refresh_ixs, gen_signer_seeds,
    lending_market::{lending_checks, lending_operations},
    state::{obligation::Obligation, CalculateBorrowResult, LendingMarket, Reserve},
    utils::{seeds, token_transfer},
    xmsg, LendingAction, LendingError, ReferrerTokenState, ReserveFarmKind,
};

pub fn process<'info>(
    ctx: Context<'_, '_, '_, 'info, BorrowObligationLiquidity<'info>>,
    liquidity_amount: u64,
) -> Result<()> {
    check_refresh_ixs!(ctx, borrow_reserve, ReserveFarmKind::Debt);
    lending_checks::borrow_obligation_liquidity_checks(&ctx)?;

    let borrow_reserve = &mut ctx.accounts.borrow_reserve.load_mut()?;
    let lending_market = &ctx.accounts.lending_market.load()?;
    let obligation = &mut ctx.accounts.obligation.load_mut()?;
    let lending_market_key = ctx.accounts.lending_market.key();
    let clock = &Clock::get()?;

    let authority_signer_seeds =
        gen_signer_seeds!(lending_market_key.as_ref(), lending_market.bump_seed as u8);

    let referrer_token_state_option: Option<RefMut<ReferrerTokenState>> =
        if obligation.has_referrer() {
            match &ctx.accounts.referrer_token_state {
                Some(referrer_token_state_loader) => {
                    let referrer_token_state = referrer_token_state_loader.load_mut()?;

                    validate_referrer_token_state(
                        &referrer_token_state,
                        referrer_token_state_loader.key(),
                        borrow_reserve.liquidity.mint_pubkey,
                        obligation.referrer,
                        ctx.accounts.borrow_reserve.key(),
                    )?;

                    Some(referrer_token_state)
                }
                None => return err!(LendingError::ReferrerAccountMissing),
            }
        } else {
            None
        };

    let initial_reserve_token_balance =
        token::accessor::amount(&ctx.accounts.reserve_source_liquidity.to_account_info())?;
    let initial_reserve_available_liquidity = borrow_reserve.liquidity.available_amount;

    let CalculateBorrowResult {
        receive_amount,
        borrow_fee,
        ..
    } = lending_operations::borrow_obligation_liquidity(
        lending_market,
        borrow_reserve,
        obligation,
        liquidity_amount,
        clock,
        ctx.accounts.borrow_reserve.key(),
        referrer_token_state_option,
    )?;

    xmsg!("pnl: Borrow obligation liquidity {receive_amount} with borrow_fee {borrow_fee}",);

    if borrow_fee > 0 {
        token_transfer::send_origination_fees_transfer(
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.reserve_source_liquidity.to_account_info(),
            ctx.accounts
                .borrow_reserve_liquidity_fee_receiver
                .to_account_info(),
            ctx.accounts.lending_market_authority.to_account_info(),
            authority_signer_seeds,
            borrow_fee,
        )?;
    }

    token_transfer::borrow_obligation_liquidity_transfer(
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.reserve_source_liquidity.to_account_info(),
        ctx.accounts.user_destination_liquidity.to_account_info(),
        ctx.accounts.lending_market_authority.to_account_info(),
        authority_signer_seeds,
        receive_amount,
    )?;

    lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
        token::accessor::amount(&ctx.accounts.reserve_source_liquidity.to_account_info()).unwrap(),
        borrow_reserve.liquidity.available_amount,
        initial_reserve_token_balance,
        initial_reserve_available_liquidity,
        LendingAction::Subtractive(borrow_fee + receive_amount),
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
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(mut,
        has_one = lending_market
    )]
    pub borrow_reserve: AccountLoader<'info, Reserve>,

    #[account(mut,
        address = borrow_reserve.load()?.liquidity.supply_vault
    )]
    pub reserve_source_liquidity: Box<Account<'info, TokenAccount>>,

    #[account(mut,
        address = borrow_reserve.load()?.liquidity.fee_vault
    )]
    pub borrow_reserve_liquidity_fee_receiver: Box<Account<'info, TokenAccount>>,

    #[account(mut,
        token::mint = reserve_source_liquidity.mint,
        token::authority = owner
    )]
    pub user_destination_liquidity: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub referrer_token_state: Option<AccountLoader<'info, ReferrerTokenState>>,

    pub token_program: Program<'info, Token>,

    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
