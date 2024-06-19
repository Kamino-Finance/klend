use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};
use anchor_spl::token::Token;
use anchor_spl::token_interface::TokenAccount;

use crate::{
    check_refresh_ixs, gen_signer_seeds,
    lending_market::{lending_checks, lending_operations},
    state::{obligation::Obligation, LendingMarket, Reserve, WithdrawObligationCollateralAccounts},
    utils::{close_account_loader, seeds, token_transfer},
    ReserveFarmKind,
};

pub fn process(ctx: Context<WithdrawObligationCollateral>, collateral_amount: u64) -> Result<()> {
    let close_obligation = {
        check_refresh_ixs!(ctx, withdraw_reserve, ReserveFarmKind::Collateral);

        lending_checks::withdraw_obligation_collateral_checks(
            &WithdrawObligationCollateralAccounts {
                lending_market: ctx.accounts.lending_market.clone(),
                lending_market_authority: ctx.accounts.lending_market_authority.clone(),
                withdraw_reserve: ctx.accounts.withdraw_reserve.clone(),
                obligation: ctx.accounts.obligation.clone(),
                reserve_source_collateral: ctx.accounts.reserve_source_collateral.clone(),
                user_destination_collateral: ctx.accounts.user_destination_collateral.clone(),
                obligation_owner: ctx.accounts.owner.clone(),
                token_program: ctx.accounts.token_program.clone(),
            },
        )?;
        let clock = &Clock::get()?;

        let withdraw_reserve = &mut ctx.accounts.withdraw_reserve.load_mut()?;
        let obligation = &mut ctx.accounts.obligation.load_mut()?;
        let lending_market = &mut ctx.accounts.lending_market.load()?;
        let lending_market_key = ctx.accounts.lending_market.key();

        let withdraw_amount = lending_operations::withdraw_obligation_collateral(
            lending_market,
            withdraw_reserve,
            obligation,
            collateral_amount,
            clock.slot,
            ctx.accounts.withdraw_reserve.key(),
        )?;

        let authority_signer_seeds =
            gen_signer_seeds!(lending_market_key, lending_market.bump_seed as u8);

        token_transfer::withdraw_obligation_collateral_transfer(
            ctx.accounts.token_program.to_account_info(),
            ctx.accounts.user_destination_collateral.to_account_info(),
            ctx.accounts.reserve_source_collateral.to_account_info(),
            ctx.accounts.lending_market_authority.clone(),
            authority_signer_seeds,
            withdraw_amount,
        )?;

        msg!("pnl: Withdraw obligation collateral {}", withdraw_amount);

        obligation.deposits_empty() && obligation.borrows_empty()
    };

    close_account_loader(
        close_obligation,
        &ctx.accounts.owner,
        &ctx.accounts.obligation,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawObligationCollateral<'info> {
    pub owner: Signer<'info>,
    #[account(mut,
        has_one = lending_market,
        has_one = owner
    )]
    pub obligation: AccountLoader<'info, Obligation>,

    pub lending_market: AccountLoader<'info, LendingMarket>,
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(
        mut,
        has_one = lending_market
    )]
    pub withdraw_reserve: AccountLoader<'info, Reserve>,

    #[account(mut,
        address = withdraw_reserve.load()?.collateral.supply_vault,
    )]
    pub reserve_source_collateral: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = withdraw_reserve.load()?.collateral.mint_pubkey
    )]
    pub user_destination_collateral: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,

    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
