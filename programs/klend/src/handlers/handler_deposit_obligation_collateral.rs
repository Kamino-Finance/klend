use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};
use anchor_spl::token::{Token, TokenAccount};

use crate::{
    check_refresh_ixs,
    lending_market::{lending_checks, lending_operations},
    state::{obligation::Obligation, DepositObligationCollateralAccounts, LendingMarket, Reserve},
    utils::token_transfer,
    ReserveFarmKind,
};

pub fn process(ctx: Context<DepositObligationCollateral>, collateral_amount: u64) -> Result<()> {
    check_refresh_ixs!(ctx, deposit_reserve, ReserveFarmKind::Collateral);
    lending_checks::deposit_obligation_collateral_checks(&DepositObligationCollateralAccounts {
        obligation: ctx.accounts.obligation.clone(),
        deposit_reserve: ctx.accounts.deposit_reserve.clone(),
        reserve_destination_collateral: ctx.accounts.reserve_destination_collateral.clone(),
        user_source_collateral: ctx.accounts.user_source_collateral.clone(),
        obligation_owner: ctx.accounts.owner.clone(),
        token_program: ctx.accounts.token_program.clone(),
    })?;

    let clock = Clock::get()?;

    let lending_market = &ctx.accounts.lending_market.load()?;
    let deposit_reserve = &mut ctx.accounts.deposit_reserve.load_mut()?;
    let obligation = &mut ctx.accounts.obligation.load_mut()?;

    lending_operations::refresh_reserve(
        deposit_reserve,
        &clock,
        None,
        lending_market.referral_fee_bps,
    )?;

    lending_operations::deposit_obligation_collateral(
        deposit_reserve,
        obligation,
        clock.slot,
        collateral_amount,
        ctx.accounts.deposit_reserve.key(),
        lending_market,
    )?;

    msg!(
        "pnl: Depositing obligation collateral {}",
        collateral_amount
    );

    token_transfer::deposit_obligation_collateral_transfer(
        ctx.accounts.user_source_collateral.to_account_info(),
        ctx.accounts
            .reserve_destination_collateral
            .to_account_info(),
        ctx.accounts.owner.to_account_info(),
        ctx.accounts.token_program.to_account_info(),
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
    pub reserve_destination_collateral: Box<Account<'info, TokenAccount>>,

    #[account(mut,
        token::mint = deposit_reserve.load()?.collateral.mint_pubkey
    )]
    pub user_source_collateral: Box<Account<'info, TokenAccount>>,

    pub token_program: Program<'info, Token>,

    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}
