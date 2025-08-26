use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};
use anchor_spl::{
    token::Token,
    token_interface::{self, Mint, TokenAccount, TokenInterface},
};

use crate::{
    check_refresh_ixs, gen_signer_seeds,
    handler_refresh_obligation_farms_for_reserve::*,
    lending_market::{lending_checks, lending_operations},
    refresh_farms,
    state::{nested_accounts::*, obligation::Obligation, LendingMarket, Reserve},
    utils::{seeds, token_transfer},
    DepositLiquidityResult, LendingAction, MaxReservesAsCollateralCheck, ReserveFarmKind,
};

pub fn process_v1(
    ctx: Context<DepositReserveLiquidityAndObligationCollateral>,
    liquidity_amount: u64,
) -> Result<()> {
    check_refresh_ixs!(
        ctx.accounts,
        ctx.accounts.reserve,
        ReserveFarmKind::Collateral
    );
    process_impl(
        ctx.accounts,
        liquidity_amount,
        MaxReservesAsCollateralCheck::Perform,
    )
}

pub fn process_v2(
    ctx: Context<DepositReserveLiquidityAndObligationCollateralV2>,
    liquidity_amount: u64,
) -> Result<()> {
    process_impl(
        &ctx.accounts.deposit_accounts,
        liquidity_amount,
        MaxReservesAsCollateralCheck::Perform,
    )?;

    refresh_farms!(
        ctx.accounts.deposit_accounts,
        [(
            ctx.accounts.deposit_accounts.reserve,
            ctx.accounts.farms_accounts,
            Collateral,
        )],
    );

    Ok(())
}

pub(super) fn process_impl(
    accounts: &DepositReserveLiquidityAndObligationCollateral,
    liquidity_amount: u64,
    max_reserves_as_collateral_check: MaxReservesAsCollateralCheck,
) -> Result<()> {
    msg!(
        "DepositReserveLiquidityAndObligationCollateral Reserve {} amount {}",
        accounts.reserve.key(),
        liquidity_amount
    );

    lending_checks::deposit_reserve_liquidity_and_obligation_collateral_checks(
        &DepositReserveLiquidityAndObligationCollateralAccounts {
            user_source_liquidity: accounts.user_source_liquidity.clone(),
            reserve: accounts.reserve.clone(),
            reserve_liquidity_mint: accounts.reserve_liquidity_mint.clone(),
        },
    )?;

    let reserve = &mut accounts.reserve.load_mut()?;
    let obligation = &mut accounts.obligation.load_mut()?;

    let lending_market = &accounts.lending_market.load()?;
    let lending_market_key = accounts.lending_market.key();
    let clock = Clock::get()?;

    let authority_signer_seeds =
        gen_signer_seeds!(lending_market_key, lending_market.bump_seed as u8);

    let initial_reserve_token_balance =
        token_interface::accessor::amount(&accounts.reserve_liquidity_supply.to_account_info())?;
    let initial_reserve_available_liquidity = reserve.liquidity.available_amount;
    let DepositLiquidityResult {
        liquidity_amount,
        collateral_amount,
    } = lending_operations::deposit_reserve_liquidity(reserve, &clock, liquidity_amount)?;

    lending_operations::refresh_reserve(reserve, &clock, None, lending_market.referral_fee_bps)?;

    lending_operations::deposit_obligation_collateral(
        lending_market,
        reserve,
        obligation,
        clock.slot,
        collateral_amount,
        accounts.reserve.key(),
        max_reserves_as_collateral_check,
    )?;

    msg!(
        "pnl: Deposit reserve liquidity {} and obligation collateral {}",
        liquidity_amount,
        collateral_amount
    );

    token_transfer::deposit_reserve_liquidity_and_obligation_collateral_transfer(
        accounts.user_source_liquidity.to_account_info(),
        accounts.reserve_liquidity_supply.to_account_info(),
        accounts.owner.to_account_info(),
        accounts.reserve_liquidity_mint.to_account_info(),
        accounts.liquidity_token_program.to_account_info(),
        accounts.reserve_collateral_mint.to_account_info(),
        accounts
            .reserve_destination_deposit_collateral
            .to_account_info(),
        accounts.collateral_token_program.to_account_info(),
        accounts.lending_market_authority.clone(),
        authority_signer_seeds,
        liquidity_amount,
        accounts.reserve_liquidity_mint.decimals,
        collateral_amount,
    )?;

    lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
        token_interface::accessor::amount(&accounts.reserve_liquidity_supply.to_account_info())
            .unwrap(),
        reserve.liquidity.available_amount,
        initial_reserve_token_balance,
        initial_reserve_available_liquidity,
        LendingAction::Additive(liquidity_amount),
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct DepositReserveLiquidityAndObligationCollateral<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,

    #[account(mut,
        has_one = lending_market,
        has_one = owner
    )]
    pub obligation: AccountLoader<'info, Obligation>,

    pub lending_market: AccountLoader<'info, LendingMarket>,
    /// CHECK: Verified through create_program_address function
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(mut, has_one = lending_market)]
    pub reserve: AccountLoader<'info, Reserve>,

    #[account(
        address = reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = liquidity_token_program,
    )]
    pub reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut,
        address = reserve.load()?.liquidity.supply_vault,
    )]
    pub reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut, address = reserve.load()?.collateral.mint_pubkey)]
    pub reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut, address = reserve.load()?.collateral.supply_vault)]
    pub reserve_destination_deposit_collateral: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        token::mint = reserve.load()?.liquidity.mint_pubkey,
        token::authority = owner,
    )]
    pub user_source_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,

    pub placeholder_user_destination_collateral: Option<AccountInfo<'info>>,

    pub collateral_token_program: Program<'info, Token>,
    pub liquidity_token_program: Interface<'info, TokenInterface>,

    /// CHECK: Sysvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct DepositReserveLiquidityAndObligationCollateralV2<'info> {
    pub deposit_accounts: DepositReserveLiquidityAndObligationCollateral<'info>,
    pub farms_accounts: OptionalObligationFarmsAccounts<'info>,
    pub farms_program: Program<'info, farms::program::Farms>,
}
