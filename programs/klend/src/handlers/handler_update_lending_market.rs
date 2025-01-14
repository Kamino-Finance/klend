use anchor_lang::{prelude::*, Accounts};

use crate::{
    borsh::BorshDeserialize,
    fraction::FractionExtra,
    state::{lending_market::ElevationGroup, LendingMarket, UpdateLendingMarketMode},
    utils::{
        validate_numerical_bool, Fraction, ELEVATION_GROUP_NONE, FULL_BPS, MAX_NUM_ELEVATION_GROUPS,
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
        "Updating lending market with mode {:?} and value {:?}",
        mode,
        &value[0..32]
    );

    match mode {
        UpdateLendingMarketMode::UpdateOwner => {
            let value: [u8; 32] = value[0..32].try_into().unwrap();
            let value = Pubkey::from(value);
            msg!("Prv value is {:?}", market.lending_market_owner_cached);
            msg!("New value is {:?}", value);
            market.lending_market_owner_cached = value;
        }
        UpdateLendingMarketMode::UpdateEmergencyMode => {
            let emergency_mode = value[0];
            msg!("Prv value is {:?}", market.emergency_mode);
            msg!("New value is {:?}", emergency_mode);
            if emergency_mode == 0 {
                market.emergency_mode = 0
            } else if emergency_mode == 1 {
                market.emergency_mode = 1;
            } else {
                return err!(LendingError::InvalidFlag);
            }
        }
        UpdateLendingMarketMode::UpdateLiquidationCloseFactor => {
            let liquidation_close_factor = value[0];
            msg!(
                "Prv value is {:?}",
                market.liquidation_max_debt_close_factor_pct
            );
            msg!("New value is {:?}", liquidation_close_factor);
            if !(5..=100).contains(&liquidation_close_factor) {
                return err!(LendingError::InvalidFlag);
            }
            market.liquidation_max_debt_close_factor_pct = liquidation_close_factor;
        }
        UpdateLendingMarketMode::UpdateLiquidationMaxValue => {
            let value = u64::from_le_bytes(value[..8].try_into().unwrap());
            msg!(
                "Prv value is {:?}",
                market.max_liquidatable_debt_market_value_at_once
            );
            msg!("New value is {:?}", value);
            if value == 0 {
                return err!(LendingError::InvalidFlag);
            }
            market.max_liquidatable_debt_market_value_at_once = value;
        }
        UpdateLendingMarketMode::UpdateGlobalAllowedBorrow => {
            let value = u64::from_le_bytes(value[..8].try_into().unwrap());
            msg!("Prv value is {:?}", market.global_allowed_borrow_value);
            msg!("New value is {:?}", value);
            market.global_allowed_borrow_value = value;
        }
        UpdateLendingMarketMode::DeprecatedUpdateGlobalUnhealthyBorrow => {
            panic!("Deprecated field")
        }
        UpdateLendingMarketMode::UpdateMinFullLiquidationThreshold => {
            let value = u64::from_le_bytes(value[..8].try_into().unwrap());
            msg!(
                "Prv value is {:?}",
                market.min_full_liquidation_value_threshold
            );
            msg!("New value is {:?}", value);
            if value == 0 {
                return err!(LendingError::InvalidFlag);
            }
            market.min_full_liquidation_value_threshold = value;
        }
        UpdateLendingMarketMode::UpdateRiskCouncil => {
            let value: [u8; 32] = value[0..32].try_into().unwrap();
            let value = Pubkey::from(value);
            msg!("Prv value is {:?}", market.risk_council);
            msg!("New value is {:?}", value);
            market.risk_council = value;
        }
        UpdateLendingMarketMode::UpdateInsolvencyRiskLtv => {
            let insolvency_risk_ltv = value[0];
            msg!(
                "Prv value is {:?}",
                market.insolvency_risk_unhealthy_ltv_pct
            );
            msg!("New value is {:?}", value);

            if !(5..=100).contains(&insolvency_risk_ltv) {
                return err!(LendingError::InvalidFlag);
            }
            market.insolvency_risk_unhealthy_ltv_pct = insolvency_risk_ltv;
        }
        UpdateLendingMarketMode::UpdateElevationGroup => {
            let elevation_group: ElevationGroup =
                BorshDeserialize::deserialize(&mut &value[..]).unwrap();

            if elevation_group.id > MAX_NUM_ELEVATION_GROUPS {
                return err!(LendingError::InvalidElevationGroupConfig);
            }

            if elevation_group.id != ELEVATION_GROUP_NONE
                && elevation_group.liquidation_threshold_pct == 0
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

            let prev_elevation_group = market.get_elevation_group(elevation_group.id);

            msg!("Prev value is {:?}", prev_elevation_group);
            msg!("New value is {:?}", elevation_group);

            market.set_elevation_group(elevation_group)?;
        }
        UpdateLendingMarketMode::UpdateReferralFeeBps => {
            let value = u16::from_le_bytes(value[..2].try_into().unwrap());
            msg!("Prev value is {:?}", market.referral_fee_bps);
            msg!("New value is {:?}", value);
            if value > FULL_BPS {
                msg!("Referral fee bps must be in range [0, 10000]");
                return err!(LendingError::InvalidConfig);
            }
            if market.referral_fee_bps != 0 {
                msg!("WARNING: Referral fee bps already set, unrefreshed obligations referral fees could be lost!");
            }
            market.referral_fee_bps = value;
        }
        UpdateLendingMarketMode::UpdatePriceRefreshTriggerToMaxAgePct => {
            let value = value[0];
            msg!(
                "Prev value is {:?}",
                market.price_refresh_trigger_to_max_age_pct
            );
            msg!("New value is {:?}", value);
            if value > 100 {
                msg!("Price refresh trigger to max age pct must be in range [0, 100]");
                return err!(LendingError::InvalidConfig);
            }
            market.price_refresh_trigger_to_max_age_pct = value;
        }
        UpdateLendingMarketMode::UpdateAutodeleverageEnabled => {
            let autodeleverage_enabled = value[0];
            msg!("Prev Value is {:?}", market.autodeleverage_enabled);
            msg!("New Value is {:?}", autodeleverage_enabled);
            if autodeleverage_enabled == 0 {
                market.autodeleverage_enabled = 0
            } else if autodeleverage_enabled == 1 {
                market.autodeleverage_enabled = 1;
            } else {
                msg!(
                    "Autodeleverage enabled flag must be 0 or 1, got {:?}",
                    autodeleverage_enabled
                );
                return err!(LendingError::InvalidFlag);
            }
        }
        UpdateLendingMarketMode::UpdateBorrowingDisabled => {
            let borrow_disabled = value[0];
            msg!("Prev Value is {:?}", market.borrow_disabled);
            msg!("New Value is {:?}", borrow_disabled);
            validate_numerical_bool(borrow_disabled)?;
            market.borrow_disabled = borrow_disabled;
        }
        UpdateLendingMarketMode::UpdateMinNetValueObligationPostAction => {
            let min_net_value_in_obligation_sf =
                u128::from_le_bytes(value[..16].try_into().unwrap());
            msg!(
                "Prev Value is {}",
                Fraction::from_bits(market.min_net_value_in_obligation_sf)
            );
            msg!(
                "New Value is {}",
                Fraction::from_bits(min_net_value_in_obligation_sf)
            );
            market.min_net_value_in_obligation_sf = min_net_value_in_obligation_sf;
        }
        UpdateLendingMarketMode::UpdateMinValueLtvSkipPriorityLiqCheck => {
            let min_value_skip_liquidation_ltv_checks =
                u64::from_le_bytes(value[..8].try_into().unwrap());
            msg!(
                "Prev Value is {}",
                market.min_value_skip_liquidation_ltv_checks
            );
            msg!("New Value is {}", min_value_skip_liquidation_ltv_checks);

            market.min_value_skip_liquidation_ltv_checks = min_value_skip_liquidation_ltv_checks;
        }
        UpdateLendingMarketMode::UpdateMinValueBfSkipPriorityLiqCheck => {
            let min_value_skip_liquidation_bf_checks =
                u64::from_le_bytes(value[..8].try_into().unwrap());
            msg!(
                "Prev Value is {}",
                market.min_value_skip_liquidation_bf_checks
            );
            msg!("New Value is {}", min_value_skip_liquidation_bf_checks);

            market.min_value_skip_liquidation_bf_checks = min_value_skip_liquidation_bf_checks;
        }
        UpdateLendingMarketMode::UpdatePaddingFields => {
            msg!("Prev Value is {:?}", market.reserved1);
            market.reserved1 = [0; 8];
            msg!("New Value is {:?}", market.reserved1);
        }
        UpdateLendingMarketMode::DeprecatedUpdateMultiplierPoints => {
            panic!("Deprecated field")
        }
        UpdateLendingMarketMode::UpdateName => {
            let name_bytes = &value[0..market.name.len()];
            let name = std::str::from_utf8(name_bytes).unwrap();
            let previous_name = std::str::from_utf8(&market.name).unwrap();
            msg!("Prev Value is {}", previous_name.trim_end_matches('\0'));
            msg!("New Value is {}", name.trim_end_matches('\0'));
            market.name.copy_from_slice(name_bytes);
        }
    }

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateLendingMarket<'info> {
    lending_market_owner: Signer<'info>,

    #[account(mut, has_one = lending_market_owner)]
    pub lending_market: AccountLoader<'info, LendingMarket>,
}
