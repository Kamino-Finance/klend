use std::ops::Deref;

use anchor_lang::{prelude::*, Accounts};
use anchor_spl::{
    token::Token,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::{
    gen_reserve_token_account_signer_seeds,
    lending_market::lending_operations,
    state::{
        reserve::{
            InitReserveParams, NewReserveCollateralParams, NewReserveLiquidityParams,
            ReserveCollateral, ReserveLiquidity,
        },
        LendingMarket, Reserve, ReserveConfig,
    },
    utils::{account_ops, constraints, seeds, spltoken, token_transfer},
    LendingError, ReserveStatus,
};

pub fn process<'info>(ctx: Context<'_, '_, '_, 'info, InitReserve<'info>>) -> Result<()> {
    let clock = &Clock::get()?;
    let reserve = &mut ctx.accounts.reserve.load_init()?;
    let market = &ctx.accounts.lending_market.load()?;
    let reserve_key = ctx.accounts.reserve.key();
    let reserve_liquidity_supply_signer_seeds = gen_reserve_token_account_signer_seeds!(
        seeds::RESERVE_LIQ_SUPPLY,
        reserve_key,
        ctx.bumps.reserve_liquidity_supply
    );
    let fee_receiver_signer_seeds = gen_reserve_token_account_signer_seeds!(
        seeds::FEE_RECEIVER,
        reserve_key,
        ctx.bumps.fee_receiver
    );

    account_ops::initialize_pda_token_account(
        &ctx.accounts.signer.to_account_info(),
        &ctx.accounts.reserve_liquidity_supply,
        &ctx.accounts.reserve_liquidity_mint,
        &ctx.accounts.lending_market_authority.to_account_info(),
        &ctx.accounts.liquidity_token_program,
        &ctx.accounts.system_program.to_account_info(),
        &[reserve_liquidity_supply_signer_seeds],
    )?;

    account_ops::initialize_pda_token_account(
        &ctx.accounts.signer.to_account_info(),
        &ctx.accounts.fee_receiver,
        &ctx.accounts.reserve_liquidity_mint,
        &ctx.accounts.lending_market_authority.to_account_info(),
        &ctx.accounts.liquidity_token_program,
        &ctx.accounts.system_program.to_account_info(),
        &[fee_receiver_signer_seeds],
    )?;

    constraints::token_2022::validate_liquidity_token_extensions(
        &ctx.accounts.reserve_liquidity_mint.to_account_info(),
        &ctx.accounts.reserve_liquidity_supply.to_account_info(),
    )?;

    let is_frozen_default_account_state_extension =
        spltoken::is_frozen_default_account_state_extension(
            &ctx.accounts.reserve_liquidity_mint.to_account_info(),
        )?;

   
   
   
    let min_initial_deposit_amount = if is_frozen_default_account_state_extension {
        0
    } else {
        market.min_initial_deposit_amount
    };

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
            initial_amount_deposited_in_reserve: min_initial_deposit_amount,
        })),
        collateral: Box::new(ReserveCollateral::new(NewReserveCollateralParams {
            mint_pubkey: ctx.accounts.reserve_collateral_mint.key(),
            supply_vault: ctx.accounts.reserve_collateral_supply.key(),
            initial_collateral_supply: min_initial_deposit_amount,
        })),
        config: Box::new(ReserveConfig {
            status: ReserveStatus::Hidden.into(),
            ..Default::default()
        }),
    });

   
    token_transfer::deposit_initial_reserve_liquidity_transfer(
        ctx.accounts.initial_liquidity_source.to_account_info(),
        ctx.accounts.reserve_liquidity_supply.to_account_info(),
        ctx.accounts.signer.to_account_info(),
        ctx.accounts.reserve_liquidity_mint.to_account_info(),
        ctx.accounts.liquidity_token_program.to_account_info(),
        min_initial_deposit_amount,
        ctx.accounts.reserve_liquidity_mint.decimals,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct InitReserve<'info> {
    #[account(mut,
        constraint = lending_operations::utils::is_allowed_signer_to_init_reserve(
            signer.key(),
            lending_market.load()?.deref()
        ) @ LendingError::InvalidSigner,
    )]
    pub signer: Signer<'info>,

    pub lending_market: AccountLoader<'info, LendingMarket>,
    /// CHECK: Checked through create_program_address
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

    #[account(mut,
        seeds = [seeds::RESERVE_LIQ_SUPPLY, reserve.key().as_ref()],
        bump
    )]
    pub reserve_liquidity_supply: AccountInfo<'info>,

    #[account(mut,
        seeds = [seeds::FEE_RECEIVER, reserve.key().as_ref()],
        bump
    )]
    pub fee_receiver: AccountInfo<'info>,

    #[account(init,
        seeds = [seeds::RESERVE_COLL_MINT, reserve.key().as_ref()],
        bump,
        payer = signer,
        mint::decimals = 6,
        mint::authority = lending_market_authority,
        mint::token_program = collateral_token_program,
    )]
    pub reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(init,
        seeds = [seeds::RESERVE_COLL_SUPPLY, reserve.key().as_ref()],
        bump,
        payer = signer,
        token::mint = reserve_collateral_mint,
        token::authority = lending_market_authority,
        token::token_program = collateral_token_program,
    )]
    pub reserve_collateral_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = reserve_liquidity_mint,
        token::authority = signer,
        token::token_program = liquidity_token_program,
    )]
    pub initial_liquidity_source: Box<InterfaceAccount<'info, TokenAccount>>,

    pub rent: Sysvar<'info, Rent>,
    pub liquidity_token_program: Interface<'info, TokenInterface>,
    pub collateral_token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}
