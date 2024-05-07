use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token::{Mint, Token, TokenAccount};

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

    reserve.init(InitReserveParams {
        current_slot: clock.slot,
        lending_market: ctx.accounts.lending_market.key(),
        liquidity: Box::new(ReserveLiquidity::new(NewReserveLiquidityParams {
            mint_pubkey: ctx.accounts.reserve_liquidity_mint.key(),
            mint_decimals: ctx.accounts.reserve_liquidity_mint.decimals,
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

    pub reserve_liquidity_mint: Box<Account<'info, Mint>>,

    #[account(init,
        seeds = [seeds::RESERVE_LIQ_SUPPLY, lending_market.key().as_ref(), reserve_liquidity_mint.key().as_ref()],
        bump,
        payer = lending_market_owner,
        token::mint = reserve_liquidity_mint,
        token::authority = lending_market_authority
    )]
    pub reserve_liquidity_supply: Box<Account<'info, TokenAccount>>,

    #[account(init,
        seeds = [seeds::FEE_RECEIVER, lending_market.key().as_ref(), reserve_liquidity_mint.key().as_ref()],
        bump,
        payer = lending_market_owner,
        token::mint = reserve_liquidity_mint,
        token::authority = lending_market_authority
    )]
    pub fee_receiver: Box<Account<'info, TokenAccount>>,

    #[account(init,
        seeds = [seeds::RESERVE_COLL_MINT, lending_market.key().as_ref(), reserve_liquidity_mint.key().as_ref()],
        bump,
        payer = lending_market_owner,
        mint::decimals = 6,
        mint::authority = lending_market_authority
    )]
    pub reserve_collateral_mint: Box<Account<'info, Mint>>,

    #[account(init,
        seeds = [seeds::RESERVE_COLL_SUPPLY, lending_market.key().as_ref(), reserve_liquidity_mint.key().as_ref()],
        bump,
        payer = lending_market_owner,
        token::mint = reserve_collateral_mint,
        token::authority = lending_market_authority
    )]
    pub reserve_collateral_supply: Box<Account<'info, TokenAccount>>,

    pub rent: Sysvar<'info, Rent>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
