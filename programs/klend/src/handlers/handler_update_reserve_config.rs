use std::ops::Deref;

use anchor_lang::{prelude::*, Accounts};

use crate::{
    lending_market::{lending_operations, utils::is_update_reserve_config_mode_global_admin_only},
    state::{LendingMarket, Reserve, UpdateConfigMode},
    utils::seeds,
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

    let reserve_usage_was_blocked = reserve.is_usage_blocked();

    msg!(
        "Updating reserve {:?} {} config with mode {:?}",
        ctx.accounts.reserve.key(),
        name,
        mode,
    );

    require!(
        !market.is_immutable()
            || (is_update_reserve_config_mode_global_admin_only(mode)
                && ctx.accounts.signer.key() == ctx.accounts.global_config.load()?.global_admin),
        LendingError::OperationNotPermittedMarketImmutable
    );

    let clock = Clock::get()?;
    lending_operations::refresh_reserve(reserve, &clock, None, market.referral_fee_bps)?;

    lending_operations::update_reserve_config(reserve, mode, value)?;

    if skip_config_integrity_validation {
        require!(
            !reserve.is_used(market.min_initial_deposit_amount) && reserve.is_usage_blocked(),
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

   
    if reserve_usage_was_blocked && !reserve.is_usage_blocked() {
        require_keys_eq!(
            market.lending_market_owner,
            ctx.accounts.signer.key(),
            LendingError::InvalidSigner
        );
        msg!("Reserve usage is now allowed");
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
    #[account(constraint = lending_operations::utils::is_allowed_signer_to_update_reserve_config(
        signer.key(),
        mode,
        lending_market.load()?.deref(),
        reserve.load()?.deref(),
        global_config.load()?.global_admin,
    ) @ LendingError::InvalidSigner)]
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
