use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::{Mint, TokenAccount, TokenInterface};

use crate::utils::constraints;
use crate::{
    gen_signer_seeds,
    state::{LendingMarket, Reserve},
    utils::{seeds, token_transfer},
};

pub fn process(ctx: Context<WithdrawProtocolFees>, amount: u64) -> Result<()> {
    constraints::token_2022::validate_liquidity_token_extensions(
        &ctx.accounts.reserve_liquidity_mint.to_account_info(),
        &ctx.accounts.fee_vault.to_account_info(),
    )?;

    let market = ctx.accounts.lending_market.load()?;
    let lending_market_key = ctx.accounts.lending_market.key();

    let amount = amount.min(ctx.accounts.fee_vault.amount);

    let authority_signer_seeds = gen_signer_seeds!(lending_market_key, market.bump_seed as u8);

    msg!("Withdrawing fees: {}", amount);

    token_transfer::withdraw_fees_from_reserve(
        ctx.accounts.token_program.to_account_info(),
        ctx.accounts.reserve_liquidity_mint.to_account_info(),
        ctx.accounts.fee_vault.to_account_info(),
        ctx.accounts.lending_market_owner_ata.to_account_info(),
        ctx.accounts.lending_market_authority.to_account_info(),
        authority_signer_seeds,
        amount,
        ctx.accounts.reserve_liquidity_mint.decimals,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawProtocolFees<'info> {
    pub lending_market_owner: Signer<'info>,

    #[account(has_one = lending_market_owner)]
    pub lending_market: AccountLoader<'info, LendingMarket>,

    #[account(
        has_one = lending_market
    )]
    pub reserve: AccountLoader<'info, Reserve>,

    #[account(mut,
        address = reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = token_program,
    )]
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(mut,
        address = reserve.load()?.liquidity.fee_vault,
        token::authority = lending_market_authority,
    )]
    pub fee_vault: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = reserve.load()?.liquidity.mint_pubkey,
    )]
    pub lending_market_owner_ata: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,
}

impl Clone for crate::accounts::WithdrawProtocolFees {
    fn clone(&self) -> Self {
        Self {
            lending_market_owner: self.lending_market_owner,
            lending_market: self.lending_market,
            reserve: self.reserve,
            reserve_liquidity_mint: self.reserve_liquidity_mint,
            lending_market_authority: self.lending_market_authority,
            fee_vault: self.fee_vault,
            lending_market_owner_ata: self.lending_market_owner_ata,
            token_program: self.token_program,
        }
    }
}
