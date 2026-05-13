use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface};

use crate::{
    lending_market::{lending_checks, lending_operations},
    state::{LendingMarket, Reserve},
    utils::token_transfer,
    LendingError,
};

pub fn process(ctx: Context<TopupReserveRewards>, amount: u64) -> Result<()> {
    require!(amount > 0, LendingError::InvalidAmount);

    lending_checks::topup_reserve_rewards_checks(ctx.accounts)?;

    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let market = ctx.accounts.lending_market.load()?;

    require!(
        market.is_reserve_rewards_enabled(),
        LendingError::ReserveRewardsDisabled
    );

   
   
   
    let clock = Clock::get()?;
    lending_operations::refresh_reserve(
        reserve,
        &clock,
        None,
        market.referral_fee_bps,
        market.reserve_rewards_max_apr_bps,
    )?;

    let initial_vault_balance = token_interface::accessor::amount(
        &ctx.accounts.reserve_liquidity_supply.to_account_info(),
    )?;
    let initial_rewards_amount_available = reserve.liquidity.rewards_amount_available;
    let initial_total_available_amount = reserve.liquidity.total_available_amount;

    token_transfer::deposit_initial_reserve_liquidity_transfer(
        ctx.accounts.source_liquidity.to_account_info(),
        ctx.accounts.reserve_liquidity_supply.to_account_info(),
        ctx.accounts.signer.to_account_info(),
        ctx.accounts.reserve_liquidity_mint.to_account_info(),
        ctx.accounts.liquidity_token_program.to_account_info(),
        amount,
        ctx.accounts.reserve_liquidity_mint.decimals,
    )?;

    reserve.liquidity.rewards_amount_available += amount;

    let final_vault_balance = token_interface::accessor::amount(
        &ctx.accounts.reserve_liquidity_supply.to_account_info(),
    )?;
    lending_checks::post_transfer_vault_balance_rewards_deposit_checks(
        initial_vault_balance,
        final_vault_balance,
        initial_rewards_amount_available,
        reserve.liquidity.rewards_amount_available,
        initial_total_available_amount,
        reserve.liquidity.total_available_amount,
        amount,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct TopupReserveRewards<'info> {
    pub signer: Signer<'info>,

    pub lending_market: AccountLoader<'info, LendingMarket>,

    #[account(mut, has_one = lending_market)]
    pub reserve: AccountLoader<'info, Reserve>,

    #[account(
        address = reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = liquidity_token_program,
    )]
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, address = reserve.load()?.liquidity.supply_vault)]
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = reserve_liquidity_mint,
        token::authority = signer,
        token::token_program = liquidity_token_program,
    )]
    pub source_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,

    pub liquidity_token_program: Interface<'info, TokenInterface>,
}
