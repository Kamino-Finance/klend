use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};
use anchor_spl::{token::Token, token_interface::TokenAccount};

use super::OptionalObligationFarmsAccounts;
use crate::{
    check_refresh_ixs,
    handler_refresh_obligation_farms_for_reserve::*,
    lending_market::{lending_checks, lending_operations},
    refresh_farms,
    state::{obligation::Obligation, DepositObligationCollateralAccounts, LendingMarket, Reserve},
    utils::{seeds, token_transfer},
    MaxReservesAsCollateralCheck, ReserveFarmKind,
};

pub fn process_v1(ctx: Context<DepositObligationCollateral>, collateral_amount: u64) -> Result<()> {
    check_refresh_ixs!(
        ctx.accounts,
        ctx.accounts.deposit_reserve,
        ReserveFarmKind::Collateral
    );
    process_impl(ctx.accounts, collateral_amount)
}

pub fn process_v2(
    ctx: Context<DepositObligationCollateralV2>,
    collateral_amount: u64,
) -> Result<()> {
    process_impl(&ctx.accounts.deposit_accounts, collateral_amount)?;
    refresh_farms!(
        ctx.accounts.deposit_accounts,
        ctx.accounts.lending_market_authority,
        [(
            ctx.accounts.deposit_accounts.deposit_reserve,
            ctx.accounts.farms_accounts,
            Collateral,
        )],
    );
    Ok(())
}

fn process_impl(accounts: &DepositObligationCollateral, collateral_amount: u64) -> Result<()> {
    lending_checks::deposit_obligation_collateral_checks(&DepositObligationCollateralAccounts {
        obligation: accounts.obligation.clone(),
        deposit_reserve: accounts.deposit_reserve.clone(),
        reserve_destination_collateral: accounts.reserve_destination_collateral.clone(),
        user_source_collateral: accounts.user_source_collateral.clone(),
        obligation_owner: accounts.owner.clone(),
        token_program: accounts.token_program.clone(),
    })?;

    let clock = Clock::get()?;

    let lending_market = &accounts.lending_market.load()?;
    let deposit_reserve = &mut accounts.deposit_reserve.load_mut()?;
    let obligation = &mut accounts.obligation.load_mut()?;

    lending_operations::refresh_reserve(
        deposit_reserve,
        &clock,
        None,
        lending_market.referral_fee_bps,
    )?;

    lending_operations::deposit_obligation_collateral(
        lending_market,
        deposit_reserve,
        obligation,
        clock.slot,
        collateral_amount,
        accounts.deposit_reserve.key(),
        MaxReservesAsCollateralCheck::Perform,
    )?;

    msg!(
        "pnl: Depositing obligation collateral {}",
        collateral_amount
    );

    token_transfer::deposit_obligation_collateral_transfer(
        accounts.user_source_collateral.to_account_info(),
        accounts.reserve_destination_collateral.to_account_info(),
        accounts.owner.to_account_info(),
        accounts.token_program.to_account_info(),
        collateral_amount,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct DepositObligationCollateral<'info> {
    pub owner: Signer<'info>,

    #[account(mut,
        has_one = owner,
        has_one = lending_market,
    )]
    pub obligation: AccountLoader<'info, Obligation>,

    pub lending_market: AccountLoader<'info, LendingMarket>,

    #[account(mut,
        has_one = lending_market
    )]
    pub deposit_reserve: AccountLoader<'info, Reserve>,

    #[account(mut,
        address = deposit_reserve.load()?.collateral.supply_vault
    )]
    pub reserve_destination_collateral: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = deposit_reserve.load()?.collateral.mint_pubkey
    )]
    pub user_source_collateral: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,

    /// CHECK: Syvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct DepositObligationCollateralV2<'info> {
    pub deposit_accounts: DepositObligationCollateral<'info>,
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, deposit_accounts.lending_market.key().as_ref()],
        bump = deposit_accounts.lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,
    pub farms_accounts: OptionalObligationFarmsAccounts<'info>,
    pub farms_program: Program<'info, farms::program::Farms>,
}
