use anchor_lang::{prelude::*, solana_program::sysvar, Accounts};
use anchor_spl::token::{Token, TokenAccount};
use lending_checks::validate_referrer_token_state;
use lending_operations::add_referrer_fee;

use crate::{
    lending_market::{flash_ixs, lending_checks, lending_operations},
    state::{LendingMarket, Reserve},
    utils::{seeds, token_transfer, Fraction},
    LendingError, ReferrerTokenState,
};

pub fn process(
    ctx: Context<FlashRepayReserveLiquidity>,
    liquidity_amount: u64,
    borrow_instruction_index: u8,
) -> Result<()> {
    lending_checks::flash_repay_reserve_liquidity_checks(&ctx)?;
    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let lending_market = &ctx.accounts.lending_market.load()?;

    flash_ixs::flash_repay_checks(&ctx, borrow_instruction_index, liquidity_amount)?;

    let (flash_loan_amount, mut reserve_origination_fee, referrer_fee) =
        lending_operations::flash_repay_reserve_liquidity(
            lending_market,
            reserve,
            liquidity_amount,
            Clock::get()?.slot,
        )?;

    token_transfer::repay_obligation_liquidity_transfer(
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.user_source_liquidity.to_account_info(),
        ctx.accounts.reserve_destination_liquidity.to_account_info(),
        ctx.accounts.user_transfer_authority.to_account_info(),
        flash_loan_amount,
    )?;

    let referrer_account = &ctx.accounts.referrer_account;

    if referrer_account.is_some() {
        match &ctx.accounts.referrer_token_state {
            Some(referrer_token_state_loader) => {
                let referrer_token_state = &mut referrer_token_state_loader.load_mut()?;

                validate_referrer_token_state(
                    referrer_token_state,
                    referrer_token_state_loader.key(),
                    reserve.liquidity.mint_pubkey,
                    referrer_account.as_ref().unwrap().key(),
                    ctx.accounts.reserve.key(),
                )?;

                add_referrer_fee(
                    reserve,
                    referrer_token_state,
                    Fraction::from_num(referrer_fee),
                )?;

                reserve_origination_fee = reserve_origination_fee
                    .checked_sub(referrer_fee)
                    .ok_or(LendingError::MathOverflow)?;
            }
            None => msg!("No referrer account provided"),
        }
    }

    if reserve_origination_fee > 0 {
        token_transfer::pay_borrowing_fees_transfer(
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.user_source_liquidity.to_account_info(),
            ctx.accounts
                .reserve_liquidity_fee_receiver
                .to_account_info(),
            ctx.accounts.user_transfer_authority.to_account_info(),
            reserve_origination_fee,
        )?;
    }

    Ok(())
}

#[derive(Accounts)]
pub struct FlashRepayReserveLiquidity<'info> {
    pub user_transfer_authority: Signer<'info>,

    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    pub lending_market: AccountLoader<'info, LendingMarket>,

    #[account(mut,
        has_one = lending_market
    )]
    pub reserve: AccountLoader<'info, Reserve>,

    #[account(mut,
        address = reserve.load()?.liquidity.supply_vault
    )]
    pub reserve_destination_liquidity: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub user_source_liquidity: Box<Account<'info, TokenAccount>>,

    #[account(mut,
        address = reserve.load()?.liquidity.fee_vault
    )]
    pub reserve_liquidity_fee_receiver: Box<Account<'info, TokenAccount>>,

    #[account(mut)]
    pub referrer_token_state: Option<AccountLoader<'info, ReferrerTokenState>>,

    #[account(mut)]
    pub referrer_account: Option<AccountInfo<'info>>,

    #[account(address = sysvar::instructions::ID)]
    pub sysvar_info: AccountInfo<'info>,
    pub token_program: Program<'info, Token>,
}
