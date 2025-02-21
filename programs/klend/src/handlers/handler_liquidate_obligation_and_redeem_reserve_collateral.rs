use anchor_lang::{
    prelude::*,
    solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId},
    Accounts,
};
use anchor_spl::{
    token,
    token::Token,
    token_interface,
    token_interface::{Mint, TokenAccount, TokenInterface},
};

use crate::{
    check_refresh_ixs, gen_signer_seeds,
    handler_refresh_obligation_farms_for_reserve::*,
    lending_market::{lending_checks, lending_operations},
    refresh_farms,
    state::{obligation::Obligation, LendingMarket, RedeemReserveCollateralAccounts, Reserve},
    utils::{seeds, token_transfer, FatAccountLoader},
    xmsg, LendingAction, LiquidateAndRedeemResult, ReserveFarmKind,
};

pub fn process_v1(
    ctx: Context<LiquidateObligationAndRedeemReserveCollateral>,
    liquidity_amount: u64,
    min_acceptable_received_liquidity_amount: u64,
    max_allowed_ltv_override_percent: u64,
) -> Result<()> {
    check_refresh_ixs!(
        ctx.accounts,
        ctx.accounts.withdraw_reserve,
        ctx.accounts.repay_reserve,
        ReserveFarmKind::Collateral,
        ReserveFarmKind::Debt
    );
    process_impl(
        ctx.accounts,
        ctx.remaining_accounts,
        liquidity_amount,
        min_acceptable_received_liquidity_amount,
        max_allowed_ltv_override_percent,
    )
}

pub fn process_v2(
    ctx: Context<LiquidateObligationAndRedeemReserveCollateralV2>,
    liquidity_amount: u64,
    min_acceptable_received_liquidity_amount: u64,
    max_allowed_ltv_override_percent: u64,
) -> Result<()> {
    process_impl(
        &ctx.accounts.liquidation_accounts,
        ctx.remaining_accounts,
        liquidity_amount,
        min_acceptable_received_liquidity_amount,
        max_allowed_ltv_override_percent,
    )?;
    refresh_farms!(
        ctx.accounts.liquidation_accounts,
        [
            (
                ctx.accounts.liquidation_accounts.withdraw_reserve,
                ctx.accounts.collateral_farms_accounts,
                Collateral,
            ),
            (
                ctx.accounts.liquidation_accounts.repay_reserve,
                ctx.accounts.debt_farms_accounts,
                Debt,
            )
        ],
    );
    Ok(())
}

fn process_impl(
    accounts: &LiquidateObligationAndRedeemReserveCollateral,
    remaining_accounts: &[AccountInfo],
    liquidity_amount: u64,
    min_acceptable_received_liquidity_amount: u64,
    max_allowed_ltv_override_percent: u64,
) -> Result<()> {
    xmsg!(
        "LiquidateObligationAndRedeemReserveCollateral amount {} max_allowed_ltv_override_percent {}",
        liquidity_amount,
        max_allowed_ltv_override_percent
    );

    lending_checks::liquidate_obligation_checks(accounts)?;
    lending_checks::redeem_reserve_collateral_checks(&RedeemReserveCollateralAccounts {
        user_source_collateral: accounts.user_destination_collateral.clone(),
        user_destination_liquidity: accounts.user_destination_liquidity.clone(),
        reserve: accounts.withdraw_reserve.clone(),
        reserve_liquidity_mint: accounts.withdraw_reserve_liquidity_mint.clone(),
        reserve_collateral_mint: accounts.withdraw_reserve_collateral_mint.clone(),
        reserve_liquidity_supply: accounts.withdraw_reserve_liquidity_supply.clone(),
        lending_market: accounts.lending_market.clone(),
        lending_market_authority: accounts.lending_market_authority.clone(),
        owner: accounts.liquidator.clone(),
        collateral_token_program: accounts.collateral_token_program.clone(),
        liquidity_token_program: accounts.withdraw_liquidity_token_program.clone(),
    })?;

    let lending_market = &accounts.lending_market.load()?;
    let obligation = &mut accounts.obligation.load_mut()?;
    let lending_market_key = accounts.lending_market.key();
    let clock = &Clock::get()?;

    let max_allowed_ltv_override_pct_opt =
        if accounts.liquidator.key() == obligation.owner && max_allowed_ltv_override_percent > 0 {
            if cfg!(feature = "staging") {
                Some(max_allowed_ltv_override_percent)
            } else {
                msg!("Warning! Attempting to set an ltv override outside the staging program");
                None
            }
        } else {
            None
        };

    let initial_withdraw_reserve_token_balance =
        token::accessor::amount(&accounts.withdraw_reserve_liquidity_supply.to_account_info())?;

    let initial_repay_reserve_token_balance =
        token::accessor::amount(&accounts.repay_reserve_liquidity_supply.to_account_info())?;

    let (initial_repay_reserve_available_amount, initial_withdraw_reserve_available_amount) =
        lending_checks::initial_liquidation_reserve_liquidity_available_amount(
            &accounts.repay_reserve,
            &accounts.withdraw_reserve,
        );

    let authority_signer_seeds =
        gen_signer_seeds!(lending_market_key, lending_market.bump_seed as u8);

    let LiquidateAndRedeemResult {
        repay_amount,
        withdraw_collateral_amount,
        withdraw_amount,
        total_withdraw_liquidity_amount,
        ..
    } = lending_operations::liquidate_and_redeem(
        lending_market,
        &accounts.repay_reserve,
        &accounts.withdraw_reserve,
        obligation,
        clock,
        liquidity_amount,
        min_acceptable_received_liquidity_amount,
        max_allowed_ltv_override_pct_opt,
        remaining_accounts.iter().map(|a| {
            FatAccountLoader::try_from(a).expect("Remaining account is not a valid deposit reserve")
        }),
    )?;

    token_transfer::repay_obligation_liquidity_transfer(
        accounts.repay_liquidity_token_program.to_account_info(),
        accounts.repay_reserve_liquidity_mint.to_account_info(),
        accounts.user_source_liquidity.to_account_info(),
        accounts.repay_reserve_liquidity_supply.to_account_info(),
        accounts.liquidator.to_account_info(),
        repay_amount,
        accounts.repay_reserve_liquidity_mint.decimals,
    )?;

    token_transfer::withdraw_obligation_collateral_transfer(
        accounts.collateral_token_program.to_account_info(),
        accounts.user_destination_collateral.to_account_info(),
        accounts
            .withdraw_reserve_collateral_supply
            .to_account_info(),
        accounts.lending_market_authority.to_account_info(),
        authority_signer_seeds,
        withdraw_amount,
    )?;

    if let Some((withdraw_liquidity_amount, protocol_fee)) = total_withdraw_liquidity_amount {
        token_transfer::redeem_reserve_collateral_transfer(
            accounts.collateral_token_program.to_account_info(),
            accounts.withdraw_liquidity_token_program.to_account_info(),
            accounts.withdraw_reserve_liquidity_mint.to_account_info(),
            accounts.withdraw_reserve_collateral_mint.to_account_info(),
            accounts.user_destination_collateral.to_account_info(),
            accounts.liquidator.to_account_info(),
            accounts.withdraw_reserve_liquidity_supply.to_account_info(),
            accounts.user_destination_liquidity.to_account_info(),
            accounts.lending_market_authority.to_account_info(),
            authority_signer_seeds,
            withdraw_collateral_amount,
            withdraw_liquidity_amount,
            accounts.withdraw_reserve_liquidity_mint.decimals,
        )?;

        token_interface::transfer_checked(
            CpiContext::new(
                accounts.withdraw_liquidity_token_program.to_account_info(),
                token_interface::TransferChecked {
                    from: accounts.user_destination_liquidity.to_account_info(),
                    to: accounts
                        .withdraw_reserve_liquidity_fee_receiver
                        .to_account_info(),
                    authority: accounts.liquidator.to_account_info(),
                    mint: accounts.withdraw_reserve_liquidity_mint.to_account_info(),
                },
            ),
            protocol_fee,
            accounts.withdraw_reserve_liquidity_mint.decimals,
        )?;
        let withdraw_reserve = &accounts.withdraw_reserve.load()?;

        let net_withdrawal_amount = if accounts
            .withdraw_reserve_liquidity_supply
            .to_account_info()
            .key
            == accounts
                .repay_reserve_liquidity_supply
                .to_account_info()
                .key
        {
            withdraw_liquidity_amount as i64 - repay_amount as i64
        } else {
            withdraw_liquidity_amount as i64
        };

        lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
            token::accessor::amount(&accounts.withdraw_reserve_liquidity_supply.to_account_info())
                .unwrap(),
            withdraw_reserve.liquidity.available_amount,
            initial_withdraw_reserve_token_balance,
            initial_withdraw_reserve_available_amount,
            LendingAction::SubstractiveSigned(net_withdrawal_amount),
        )?;
    }
    let repay_reserve = &accounts.repay_reserve.load()?;

    if accounts
        .withdraw_reserve_liquidity_supply
        .to_account_info()
        .key
        != accounts
            .repay_reserve_liquidity_supply
            .to_account_info()
            .key
        || total_withdraw_liquidity_amount.is_none()
    {
        lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
            token::accessor::amount(&accounts.repay_reserve_liquidity_supply.to_account_info())
                .unwrap(),
            repay_reserve.liquidity.available_amount,
            initial_repay_reserve_token_balance,
            initial_repay_reserve_available_amount,
            LendingAction::Additive(repay_amount),
        )?;
    }

    lending_checks::post_liquidate_repay_amount_check(liquidity_amount, repay_amount)?;

    Ok(())
}

#[derive(Accounts)]
pub struct LiquidateObligationAndRedeemReserveCollateral<'info> {
    pub liquidator: Signer<'info>,

    #[account(mut,
        has_one = lending_market
    )]
    pub obligation: AccountLoader<'info, Obligation>,

    pub lending_market: AccountLoader<'info, LendingMarket>,
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(mut,
        has_one = lending_market
    )]
    pub repay_reserve: AccountLoader<'info, Reserve>,
    #[account(
        address = repay_reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = repay_liquidity_token_program,
    )]
    pub repay_reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut,
        address = repay_reserve.load()?.liquidity.supply_vault,
    )]
    pub repay_reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut,
        has_one = lending_market
    )]
    pub withdraw_reserve: AccountLoader<'info, Reserve>,
    #[account(
        address = withdraw_reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = withdraw_liquidity_token_program,
    )]
    pub withdraw_reserve_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut,
        address = withdraw_reserve.load()?.collateral.mint_pubkey,
    )]
    pub withdraw_reserve_collateral_mint: Box<InterfaceAccount<'info, Mint>>,
    #[account(mut,
        address = withdraw_reserve.load()?.collateral.supply_vault
    )]
    pub withdraw_reserve_collateral_supply: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut,
        address = withdraw_reserve.load()?.liquidity.supply_vault
    )]
    pub withdraw_reserve_liquidity_supply: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut,
        address = withdraw_reserve.load()?.liquidity.fee_vault,
    )]
    pub withdraw_reserve_liquidity_fee_receiver: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(mut)]
    pub user_source_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub user_destination_collateral: Box<InterfaceAccount<'info, TokenAccount>>,
    #[account(mut)]
    pub user_destination_liquidity: Box<InterfaceAccount<'info, TokenAccount>>,

    pub collateral_token_program: Program<'info, Token>,
    pub repay_liquidity_token_program: Interface<'info, TokenInterface>,
    pub withdraw_liquidity_token_program: Interface<'info, TokenInterface>,

    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct LiquidateObligationAndRedeemReserveCollateralV2<'info> {
    pub liquidation_accounts: LiquidateObligationAndRedeemReserveCollateral<'info>,
    pub collateral_farms_accounts: OptionalObligationFarmsAccounts<'info>,
    pub debt_farms_accounts: OptionalObligationFarmsAccounts<'info>,
    pub farms_program: Program<'info, farms::program::Farms>,
}
