use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};
use anchor_spl::{token::Token, token_interface::TokenAccount};

use crate::{
    check_refresh_ixs, gen_signer_seeds,
    handler_refresh_obligation_farms_for_reserve::*,
    lending_market::{lending_checks, lending_operations},
    refresh_farms,
    state::{obligation::Obligation, LendingMarket, Reserve, WithdrawObligationCollateralAccounts},
    utils::{close_account_loader, seeds, token_transfer},
    LtvMaxWithdrawalCheck, ReserveFarmKind,
};

pub fn process_v1(
    ctx: Context<WithdrawObligationCollateral>,
    collateral_amount: u64,
) -> Result<()> {
    check_refresh_ixs!(
        ctx.accounts,
        ctx.accounts.withdraw_reserve,
        ReserveFarmKind::Collateral
    );
    process_impl(ctx.accounts, collateral_amount)
}

pub fn process_v2(
    ctx: Context<WithdrawObligationCollateralV2>,
    collateral_amount: u64,
) -> Result<()> {
    process_impl(&ctx.accounts.withdraw_accounts, collateral_amount)?;
    refresh_farms!(
        ctx.accounts.withdraw_accounts,
        [(
            ctx.accounts.withdraw_accounts.withdraw_reserve,
            ctx.accounts.farms_accounts,
            Collateral,
        )],
    );
    Ok(())
}

fn process_impl(accounts: &WithdrawObligationCollateral, collateral_amount: u64) -> Result<()> {
    let close_obligation = {
        lending_checks::withdraw_obligation_collateral_checks(
            &WithdrawObligationCollateralAccounts {
                lending_market: accounts.lending_market.clone(),
                lending_market_authority: accounts.lending_market_authority.clone(),
                withdraw_reserve: accounts.withdraw_reserve.clone(),
                obligation: accounts.obligation.clone(),
                reserve_source_collateral: accounts.reserve_source_collateral.clone(),
                user_destination_collateral: accounts.user_destination_collateral.clone(),
                obligation_owner: accounts.owner.clone(),
                token_program: accounts.token_program.clone(),
            },
        )?;
        let clock = &Clock::get()?;

        let withdraw_reserve = &mut accounts.withdraw_reserve.load_mut()?;
        let obligation = &mut accounts.obligation.load_mut()?;
        let lending_market = &mut accounts.lending_market.load()?;
        let lending_market_key = accounts.lending_market.key();

        let withdraw_amount = lending_operations::withdraw_obligation_collateral(
            lending_market,
            withdraw_reserve,
            obligation,
            collateral_amount,
            clock.slot,
            accounts.withdraw_reserve.key(),
            LtvMaxWithdrawalCheck::MaxLtv,
        )?;

        let authority_signer_seeds =
            gen_signer_seeds!(lending_market_key, lending_market.bump_seed as u8);

        token_transfer::withdraw_obligation_collateral_transfer(
            accounts.token_program.to_account_info(),
            accounts.user_destination_collateral.to_account_info(),
            accounts.reserve_source_collateral.to_account_info(),
            accounts.lending_market_authority.clone(),
            authority_signer_seeds,
            withdraw_amount,
        )?;

        msg!("pnl: Withdraw obligation collateral {}", withdraw_amount);

        obligation.active_deposits_empty() && obligation.active_borrows_empty()
    };

    close_account_loader(close_obligation, &accounts.owner, &accounts.obligation)?;

    Ok(())
}

#[derive(Accounts)]
pub struct WithdrawObligationCollateral<'info> {
   
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut,
        has_one = lending_market,
        has_one = owner
    )]
    pub obligation: AccountLoader<'info, Obligation>,

    pub lending_market: AccountLoader<'info, LendingMarket>,
    /// CHECK: Verified through create_program_address
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
        token::mint = withdraw_reserve.load()?.collateral.mint_pubkey,
        token::authority = owner
    )]
    pub user_destination_collateral: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,

    /// CHECK: Syvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct WithdrawObligationCollateralV2<'info> {
    pub withdraw_accounts: WithdrawObligationCollateral<'info>,
    pub farms_accounts: OptionalObligationFarmsAccounts<'info>,
    pub farms_program: Program<'info, farms::program::Farms>,
}
