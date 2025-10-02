use std::fmt::Debug;

use anchor_lang::{prelude::*, Accounts};

use crate::{
    fraction::FractionExtra,
    lending_market::config_items::{self, renderings, validations},
    state::{lending_market::ElevationGroup, LendingMarket, UpdateLendingMarketMode},
    utils::{
        Fraction, ELEVATION_GROUP_NONE, FULL_BPS, MAX_NUM_ELEVATION_GROUPS,
        MIN_INITIAL_DEPOSIT_AMOUNT,
    },
    LendingError, VALUE_BYTE_MAX_ARRAY_LEN_MARKET_UPDATE,
};

pub fn process(
    ctx: Context<UpdateLendingMarket>,
    mode: u64,
    value: [u8; VALUE_BYTE_MAX_ARRAY_LEN_MARKET_UPDATE],
) -> Result<()> {
    let mode = UpdateLendingMarketMode::try_from(mode)
        .map_err(|_| ProgramError::InvalidInstructionData)?;

    let market = &mut ctx.accounts.lending_market.load_mut()?;

    msg!(
        "Updating lending market {:?} with mode {:?} and value {:?}",
        ctx.accounts.lending_market.key(),
        mode,
        &value[0..32]
    );

    require!(
        !market.is_immutable(),
        LendingError::OperationNotPermittedMarketImmutable
    );

    match mode {
        UpdateLendingMarketMode::UpdateOwner => {
            config_items::for_named_field!(&mut market.lending_market_owner_cached).set(&value)?;
        }
        UpdateLendingMarketMode::UpdateEmergencyMode => {
            config_items::for_named_field!(&mut market.emergency_mode)
                .validating(validations::check_bool)
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateLiquidationCloseFactor => {
            config_items::for_named_field!(&mut market.liquidation_max_debt_close_factor_pct)
                .validating(validations::check_in_range(5..=100))
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateLiquidationMaxValue => {
            config_items::for_named_field!(&mut market.max_liquidatable_debt_market_value_at_once)
                .validating(validations::check_not_zero)
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateGlobalAllowedBorrow => {
            config_items::for_named_field!(&mut market.global_allowed_borrow_value).set(&value)?;
        }
        UpdateLendingMarketMode::DeprecatedUpdateGlobalUnhealthyBorrow => {
            panic!("Deprecated field")
        }
        UpdateLendingMarketMode::UpdateMinFullLiquidationThreshold => {
            config_items::for_named_field!(&mut market.min_full_liquidation_value_threshold)
                .validating(validations::check_not_zero)
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateRiskCouncil => {
            config_items::for_named_field!(&mut market.risk_council).set(&value)?;
        }
        UpdateLendingMarketMode::UpdateInsolvencyRiskLtv => {
            config_items::for_named_field!(&mut market.insolvency_risk_unhealthy_ltv_pct)
                .validating(validations::check_in_range(5..=100))
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateElevationGroup => {
            config_items::for_object(market)
                .using_setter_and_getter(
                    |market, group| market.set_elevation_group(group),
                    |market, group| market.get_elevation_group(group.id),
                )
                .named("elevation_group")
                .validating(validate_new_elevation_group)
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateReferralFeeBps => {
            if market.referral_fee_bps != 0 {
                msg!("WARNING: Referral fee bps already set, unrefreshed obligations referral fees could be lost!");
            }
            config_items::for_named_field!(&mut market.referral_fee_bps)
                .validating(validations::check_valid_bps)
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdatePriceRefreshTriggerToMaxAgePct => {
            config_items::for_named_field!(&mut market.price_refresh_trigger_to_max_age_pct)
                .validating(validations::check_valid_pct)
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateAutodeleverageEnabled => {
            config_items::for_named_field!(&mut market.autodeleverage_enabled)
                .validating(validations::check_bool)
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateBorrowingDisabled => {
            config_items::for_named_field!(&mut market.borrow_disabled)
                .validating(validations::check_bool)
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateMinNetValueObligationPostAction => {
            config_items::for_named_field!(&mut market.min_net_value_in_obligation_sf)
                .rendering(renderings::as_fraction)
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateMinValueLtvSkipPriorityLiqCheck => {
            config_items::for_named_field!(&mut market.min_value_skip_liquidation_ltv_checks)
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateMinValueBfSkipPriorityLiqCheck => {
            config_items::for_named_field!(&mut market.min_value_skip_liquidation_bf_checks)
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdatePaddingFields => {
            msg!("Prv reserved0 Value is {:?}", market.reserved0);
            msg!("Prv reserved1 Value is {:?}", market.reserved1);
            market.reserved0 = [0; 8];
            market.reserved1 = [0; 8];
            msg!("New reserved0 Value is {:?}", market.reserved0);
            msg!("New reserved1 Value is {:?}", market.reserved1);
        }
        UpdateLendingMarketMode::DeprecatedUpdateMultiplierPoints => {
            panic!("Deprecated field")
        }
        UpdateLendingMarketMode::UpdateName => {
            config_items::for_named_field!(&mut market.name)
                .rendering(renderings::as_utf8_null_padded_string)
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateIndividualAutodeleverageMarginCallPeriodSecs => {
            config_items::for_named_field!(
                &mut market.individual_autodeleverage_margin_call_period_secs
            )
            .validating(validations::check_not_zero)
            .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateInitialDepositAmount => {
            config_items::for_named_field!(&mut market.min_initial_deposit_amount)
                .validating(validations::check_gte(MIN_INITIAL_DEPOSIT_AMOUNT))
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateObligationOrderExecutionEnabled => {
            config_items::for_named_field!(&mut market.obligation_order_execution_enabled)
                .validating(validations::check_bool)
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateImmutableFlag => {
            config_items::for_named_field!(&mut market.immutable)
                .validating(validations::check_bool)
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateObligationOrderCreationEnabled => {
            config_items::for_named_field!(&mut market.obligation_order_creation_enabled)
                .validating(validations::check_bool)
                .set(&value)?;
        }
        UpdateLendingMarketMode::UpdateProposerAuthority => {
            config_items::for_named_field!(&mut market.proposer_authority).set(&value)?;
        }
    }

    Ok(())
}

fn validate_new_elevation_group(elevation_group: &ElevationGroup) -> Result<()> {
   
    if elevation_group.id > MAX_NUM_ELEVATION_GROUPS {
        return err!(LendingError::InvalidElevationGroupConfig);
    }

    if elevation_group.id != ELEVATION_GROUP_NONE && elevation_group.liquidation_threshold_pct == 0
    {
        return err!(LendingError::InvalidElevationGroupConfig);
    }

   
    if elevation_group.liquidation_threshold_pct >= 100
        || elevation_group.ltv_pct >= 100
        || elevation_group.ltv_pct > elevation_group.liquidation_threshold_pct
        || elevation_group.max_liquidation_bonus_bps > FULL_BPS
    {
        return err!(LendingError::InvalidElevationGroupConfig);
    }

    if elevation_group.id != ELEVATION_GROUP_NONE
        && (elevation_group.debt_reserve == Pubkey::default()
            || elevation_group.max_reserves_as_collateral == 0)
    {
        return err!(LendingError::InvalidElevationGroupConfig);
    }

   
   
    if Fraction::from_percent(elevation_group.liquidation_threshold_pct)
        + Fraction::from_percent(elevation_group.liquidation_threshold_pct)
            * Fraction::from_bps(elevation_group.max_liquidation_bonus_bps)
        > Fraction::ONE
    {
        msg!("Max liquidation bonus * liquidation threshold is greater than 100%, invalid");
        return err!(LendingError::InvalidElevationGroupConfig);
    }

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateLendingMarket<'info> {
    lending_market_owner: Signer<'info>,

    #[account(mut, has_one = lending_market_owner)]
    pub lending_market: AccountLoader<'info, LendingMarket>,
}
