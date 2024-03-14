use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token::{Mint, Token, TokenAccount};

use super::{obligation::Obligation, LendingMarket, Reserve};

#[derive(Accounts)]
pub struct DepositReserveLiquidityAccounts<'info> {
    pub user_source_liquidity: Box<Account<'info, TokenAccount>>,
    pub user_destination_collateral: Box<Account<'info, TokenAccount>>,
    pub reserve: AccountLoader<'info, Reserve>,
    pub reserve_liquidity_supply: Box<Account<'info, TokenAccount>>,
    pub reserve_collateral_mint: Box<Account<'info, Mint>>,
    pub lending_market: AccountLoader<'info, LendingMarket>,
    pub lending_market_authority: AccountInfo<'info>,
    pub owner: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct DepositObligationCollateralAccounts<'info> {
    pub user_source_collateral: Box<Account<'info, TokenAccount>>,
    pub reserve_destination_collateral: Box<Account<'info, TokenAccount>>,
    pub deposit_reserve: AccountLoader<'info, Reserve>,
    pub obligation: AccountLoader<'info, Obligation>,
    pub obligation_owner: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct WithdrawObligationCollateralAccounts<'info> {
    pub reserve_source_collateral: Box<Account<'info, TokenAccount>>,
    pub user_destination_collateral: Box<Account<'info, TokenAccount>>,
    pub withdraw_reserve: AccountLoader<'info, Reserve>,
    pub obligation: AccountLoader<'info, Obligation>,
    pub lending_market: AccountLoader<'info, LendingMarket>,
    pub lending_market_authority: AccountInfo<'info>,
    pub obligation_owner: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[derive(Accounts)]
pub struct RedeemReserveCollateralAccounts<'info> {
    #[account(mut)]
    pub user_source_collateral: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub user_destination_liquidity: Box<Account<'info, TokenAccount>>,
    #[account(mut)]
    pub reserve: AccountLoader<'info, Reserve>,
    #[account(mut)]
    pub reserve_collateral_mint: Box<Account<'info, Mint>>,
    #[account(mut)]
    pub reserve_liquidity_supply: Box<Account<'info, TokenAccount>>,
    pub lending_market: AccountLoader<'info, LendingMarket>,
    pub lending_market_authority: AccountInfo<'info>,
    pub owner: Signer<'info>,
    pub token_program: Program<'info, Token>,
}
