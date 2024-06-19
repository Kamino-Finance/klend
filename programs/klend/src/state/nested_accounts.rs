use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token::Token;
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use super::{obligation::Obligation, LendingMarket, Reserve};

#[derive(Accounts)]
pub struct DepositReserveLiquidityAccounts<'info> {
    pub user_source_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,
    pub user_destination_collateral: Box<InterfaceAccount<'info, TokenAccount>>,
    pub reserve: AccountLoader<'info, Reserve>,
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,
    pub reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,
    pub lending_market: AccountLoader<'info, LendingMarket>,
    pub lending_market_authority: AccountInfo<'info>,
    pub owner: Signer<'info>,
    pub liquidity_token_program: Interface<'info, TokenInterface>,
}

#[derive(Accounts)]
pub struct DepositObligationCollateralAccounts<'info> {
    pub user_source_collateral: Box<InterfaceAccount<'info, TokenAccount>>,
    pub reserve_destination_collateral: Box<InterfaceAccount<'info, TokenAccount>>,
    pub deposit_reserve: AccountLoader<'info, Reserve>,
    pub obligation: AccountLoader<'info, Obligation>,
    pub obligation_owner: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct DepositReserveLiquidityAndObligationCollateralAccounts<'info> {
    pub user_source_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,
    pub reserve: AccountLoader<'info, Reserve>,
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,
}

#[derive(Accounts)]
pub struct WithdrawObligationCollateralAccounts<'info> {
    pub reserve_source_collateral: Box<InterfaceAccount<'info, TokenAccount>>,
    pub user_destination_collateral: Box<InterfaceAccount<'info, TokenAccount>>,
    pub withdraw_reserve: AccountLoader<'info, Reserve>,
    pub obligation: AccountLoader<'info, Obligation>,
    pub lending_market: AccountLoader<'info, LendingMarket>,
    pub lending_market_authority: AccountInfo<'info>,
    pub obligation_owner: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct WithdrawObligationCollateralAndRedeemReserveCollateralAccounts<'info> {
    pub withdraw_reserve: AccountLoader<'info, Reserve>,
    pub user_destination_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,
}

#[derive(Accounts)]
pub struct RedeemReserveCollateralAccounts<'info> {
    #[account(mut)]
    pub user_source_collateral: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub user_destination_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub reserve: AccountLoader<'info, Reserve>,
    #[account(mut)]
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut)]
    pub reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut)]
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,
    pub lending_market: AccountLoader<'info, LendingMarket>,
    pub lending_market_authority: AccountInfo<'info>,
    pub owner: Signer<'info>,
    pub collateral_token_program: Program<'info, Token>,
    pub liquidity_token_program: Interface<'info, TokenInterface>,
}
