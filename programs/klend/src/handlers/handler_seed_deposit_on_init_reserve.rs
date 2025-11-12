use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::{
    state::{LendingMarket, Reserve},
    utils::{constraints, token_transfer},
    LendingError,
};

pub fn process(ctx: Context<SeedDepositOnInitReserve>) -> Result<()> {
    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let market = &ctx.accounts.lending_market.load()?;

   

    require!(
        !reserve.has_initial_deposit(),
        LendingError::InitialAdminDepositExecuted
    );

    constraints::token_2022::validate_liquidity_token_extensions(
        &ctx.accounts.reserve_liquidity_mint.to_account_info(),
        &ctx.accounts.reserve_liquidity_supply.to_account_info(),
    )?;

    reserve.liquidity.available_amount = market.min_initial_deposit_amount;
    reserve.collateral.mint_total_supply = market.min_initial_deposit_amount;

   
    token_transfer::deposit_initial_reserve_liquidity_transfer(
        ctx.accounts.initial_liquidity_source.to_account_info(),
        ctx.accounts.reserve_liquidity_supply.to_account_info(),
        ctx.accounts.signer.to_account_info(),
        ctx.accounts.reserve_liquidity_mint.to_account_info(),
        ctx.accounts.liquidity_token_program.to_account_info(),
        market.min_initial_deposit_amount,
        ctx.accounts.reserve_liquidity_mint.decimals,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct SeedDepositOnInitReserve<'info> {
    pub signer: Signer<'info>,

    pub lending_market: AccountLoader<'info, LendingMarket>,
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
        address = reserve.load()?.liquidity.supply_vault,
    )]
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = reserve_liquidity_mint,
        token::authority = signer,
        token::token_program = liquidity_token_program,
    )]
    pub initial_liquidity_source: Box<InterfaceAccount<'info, TokenAccount>>,
    pub liquidity_token_program: Interface<'info, TokenInterface>,
}
