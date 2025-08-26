use anchor_lang::{prelude::*, Accounts};

use crate::{
    lending_market::{lending_operations, utils::is_update_reserve_config_mode_global_admin_only},
    state::{LendingMarket, Reserve, UpdateConfigMode},
    utils::{seeds, Fraction},
    GlobalConfig, LendingError,
};

pub fn process(
    ctx: Context<UpdateReserveConfig>,
    mode: UpdateConfigMode,
    value: &[u8],
    skip_config_integrity_validation: bool,
) -> Result<()> {
    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let market = ctx.accounts.lending_market.load()?;
    let name = reserve.config.token_info.symbol();

    msg!(
        "Updating reserve {:?} {} config with mode {:?}",
        ctx.accounts.reserve.key(),
        name,
        mode,
    );

   
    require!(
        !market.is_immutable() || is_update_reserve_config_mode_global_admin_only(mode),
        LendingError::OperationNotPermittedMarketImmutable
    );

    let clock = Clock::get()?;
    lending_operations::refresh_reserve(reserve, &clock, None, market.referral_fee_bps)?;

    lending_operations::update_reserve_config(reserve, mode, value)?;

    if skip_config_integrity_validation {
        let reserve_is_used = reserve.liquidity.available_amount
            > market.min_initial_deposit_amount
            || reserve.liquidity.total_borrow() > Fraction::ZERO
            || reserve.collateral.mint_total_supply > market.min_initial_deposit_amount;

        let reserve_blocks_deposits = reserve.config.deposit_limit == 0;
        let reserve_blocks_borrows = reserve.config.borrow_limit == 0;

        require!(
            !reserve_is_used && reserve_blocks_deposits && reserve_blocks_borrows,
            LendingError::InvalidConfig
        );
        msg!("WARNING! Skipping validation of the config");
    } else {
       
        lending_operations::utils::validate_reserve_config_integrity(
            &reserve.config,
            &market,
            ctx.accounts.reserve.key(),
        )?;
    }

    Ok(())
}

#[derive(Accounts)]
#[instruction(
    mode: UpdateConfigMode,
    value: Vec<u8>,
    skip_config_integrity_validation: bool,
)]
pub struct UpdateReserveConfig<'info> {
    #[account(address = lending_operations::utils::allowed_signer_update_reserve_config(
        mode,
        lending_market.load()?.lending_market_owner,
        global_config.load()?.global_admin
    ))]
    signer: Signer<'info>,

    #[account(
        seeds = [seeds::GLOBAL_CONFIG_STATE],
        bump,
    )]
    global_config: AccountLoader<'info, GlobalConfig>,

    lending_market: AccountLoader<'info, LendingMarket>,

    #[account(mut,
        has_one = lending_market
    )]
    reserve: AccountLoader<'info, Reserve>,
}
