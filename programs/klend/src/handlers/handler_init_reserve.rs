use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token::Token;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::utils::constraints;
use crate::{
    state::{
        reserve::{
            InitReserveParams, NewReserveCollateralParams, NewReserveLiquidityParams,
            ReserveCollateral, ReserveLiquidity,
        },
        LendingMarket, Reserve, ReserveConfig,
    },
    utils::seeds,
    LendingError, ReserveStatus,
};

pub fn process<'info>(ctx: Context<'_, '_, '_, 'info, InitReserve<'info>>) -> Result<()> {
    let clock = &Clock::get()?;
    let reserve = &mut ctx.accounts.reserve.load_init()?;
    constraints::token_2022::validate_liquidity_token_extensions(
        &ctx.accounts.reserve_liquidity_mint.to_account_info(),
        &ctx.accounts.reserve_liquidity_supply.to_account_info(),
    )?;

    reserve.init(InitReserveParams {
        current_slot: clock.slot,
        lending_market: ctx.accounts.lending_market.key(),
        liquidity: Box::new(ReserveLiquidity::new(NewReserveLiquidityParams {
            mint_pubkey: ctx.accounts.reserve_liquidity_mint.key(),
            mint_decimals: ctx.accounts.reserve_liquidity_mint.decimals,
            mint_token_program: ctx.accounts.liquidity_token_program.key(),
            supply_vault: ctx.accounts.reserve_liquidity_supply.key(),
            fee_vault: ctx.accounts.fee_receiver.key(),
            market_price_sf: 0,
        })),
        collateral: Box::new(ReserveCollateral::new(NewReserveCollateralParams {
            mint_pubkey: ctx.accounts.reserve_collateral_mint.key(),
            supply_vault: ctx.accounts.reserve_collateral_supply.key(),
        })),
        config: Box::new(ReserveConfig {
            status: ReserveStatus::Hidden.into(),
            ..Default::default()
        }),
    });

    Ok(())
}

#[derive(Accounts)]
pub struct InitReserve<'info> {
    #[account(mut)]
    pub lending_market_owner: Signer<'info>,
    #[account(
        has_one = lending_market_owner @ LendingError::InvalidMarketOwner,
    )]
    pub lending_market: AccountLoader<'info, LendingMarket>,
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(zero)]
    pub reserve: AccountLoader<'info, Reserve>,

    #[account(
        mint::token_program = liquidity_token_program,
    )]
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(init,
        seeds = [seeds::RESERVE_LIQ_SUPPLY, lending_market.key().as_ref(), reserve_liquidity_mint.key().as_ref()],
        bump,
        payer = lending_market_owner,
        token::mint = reserve_liquidity_mint,
        token::authority = lending_market_authority,
        token::token_program = liquidity_token_program,
    )]
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(init,
        seeds = [seeds::FEE_RECEIVER, lending_market.key().as_ref(), reserve_liquidity_mint.key().as_ref()],
        bump,
        payer = lending_market_owner,
        token::mint = reserve_liquidity_mint,
        token::authority = lending_market_authority,
        token::token_program = liquidity_token_program,
    )]
    pub fee_receiver: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(init,
        seeds = [seeds::RESERVE_COLL_MINT, lending_market.key().as_ref(), reserve_liquidity_mint.key().as_ref()],
        bump,
        payer = lending_market_owner,
        mint::decimals = 6,
        mint::authority = lending_market_authority,
        mint::token_program = collateral_token_program,
    )]
    pub reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(init,
        seeds = [seeds::RESERVE_COLL_SUPPLY, lending_market.key().as_ref(), reserve_liquidity_mint.key().as_ref()],
        bump,
        payer = lending_market_owner,
        token::mint = reserve_collateral_mint,
        token::authority = lending_market_authority,
        token::token_program = collateral_token_program,
    )]
    pub reserve_collateral_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    pub rent: Sysvar<'info, Rent>,
    pub liquidity_token_program: Interface<'info, TokenInterface>,
    pub collateral_token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
