use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};
use anchor_spl::token_interface::{self, Mint, TokenAccount, TokenInterface};

use crate::{
    check_refresh_ixs,
    handler_refresh_obligation_farms_for_reserve::*,
    lending_market::{lending_checks, lending_operations},
    refresh_farms,
    state::{obligation::Obligation, LendingMarket, Reserve},
    utils::{seeds, token_transfer, FatAccountLoader},
    xmsg, LendingAction, ReserveFarmKind,
};

pub fn process_v1(ctx: Context<RepayObligationLiquidity>, liquidity_amount: u64) -> Result<()> {
    check_refresh_ixs!(
        ctx.accounts,
        ctx.accounts.repay_reserve,
        ReserveFarmKind::Debt
    );

    process_impl(
        ctx.accounts,
        ctx.remaining_accounts.iter(),
        liquidity_amount,
    )
}

pub fn process_v2(ctx: Context<RepayObligationLiquidityV2>, liquidity_amount: u64) -> Result<()> {
    process_impl(
        &ctx.accounts.repay_accounts,
        ctx.remaining_accounts.iter(),
        liquidity_amount,
    )?;
    refresh_farms!(
        ctx.accounts.repay_accounts,
        ctx.accounts.lending_market_authority,
        [(
            ctx.accounts.repay_accounts.repay_reserve,
            ctx.accounts.farms_accounts,
            Debt,
        ),],
    );
    Ok(())
}

pub(super) fn process_impl<'a, 'info>(
    accounts: &RepayObligationLiquidity,
    remaining_accounts: impl Iterator<Item = &'a AccountInfo<'info>>,
    liquidity_amount: u64,
) -> Result<()>
where
    'info: 'a,
{
    lending_checks::repay_obligation_liquidity_checks(accounts)?;

    let clock = Clock::get()?;

    let repay_reserve = &mut accounts.repay_reserve.load_mut()?;
    let obligation = &mut accounts.obligation.load_mut()?;
    let lending_market = &accounts.lending_market.load()?;

    let initial_reserve_token_balance = token_interface::accessor::amount(
        &accounts.reserve_destination_liquidity.to_account_info(),
    )?;
    let initial_reserve_available_liquidity = repay_reserve.liquidity.available_amount;

    let repay_amount = lending_operations::repay_obligation_liquidity(
        repay_reserve,
        obligation,
        &clock,
        liquidity_amount,
        accounts.repay_reserve.key(),
        lending_market,
        remaining_accounts.map(|a| {
            FatAccountLoader::try_from(a).expect("Remaining account is not a valid deposit reserve")
        }),
    )?;

    xmsg!(
        "pnl: Repaying obligation liquidity {} liquidity_amount {}",
        repay_amount,
        liquidity_amount
    );

    token_transfer::repay_obligation_liquidity_transfer(
        accounts.token_program.to_account_info(),
        accounts.reserve_liquidity_mint.to_account_info(),
        accounts.user_source_liquidity.to_account_info(),
        accounts.reserve_destination_liquidity.to_account_info(),
        accounts.owner.to_account_info(),
        repay_amount,
        accounts.reserve_liquidity_mint.decimals,
    )?;

    lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
        token_interface::accessor::amount(
            &accounts.reserve_destination_liquidity.to_account_info(),
        )
        .unwrap(),
        repay_reserve.liquidity.available_amount,
        initial_reserve_token_balance,
        initial_reserve_available_liquidity,
        LendingAction::Additive(repay_amount),
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct RepayObligationLiquidity<'info> {
    pub owner: Signer<'info>,

    #[account(mut,
        has_one = lending_market,
        constraint = obligation.load()?.lending_market == repay_reserve.load()?.lending_market
    )]
    pub obligation: AccountLoader<'info, Obligation>,

    pub lending_market: AccountLoader<'info, LendingMarket>,

    #[account(mut,
        has_one = lending_market
    )]
    pub repay_reserve: AccountLoader<'info, Reserve>,

    #[account(
        address = repay_reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = token_program,
    )]
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut,
        address = repay_reserve.load()?.liquidity.supply_vault,
    )]
    pub reserve_destination_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = repay_reserve.load()?.liquidity.mint_pubkey
    )]
    pub user_source_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,

    pub token_program: Interface<'info, TokenInterface>,

    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct RepayObligationLiquidityV2<'info> {
    pub repay_accounts: RepayObligationLiquidity<'info>,
    pub farms_accounts: OptionalObligationFarmsAccounts<'info>,
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, repay_accounts.lending_market.key().as_ref()],
        bump = repay_accounts.lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,
    pub farms_program: Program<'info, farms::program::Farms>,
}
