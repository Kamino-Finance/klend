use anchor_lang::{prelude::*, Accounts};

use crate::{
    borsh::BorshDeserialize,
    fraction::FractionExtra,
    state::{lending_market::ElevationGroup, LendingMarket, UpdateLendingMarketMode},
    utils::{Fraction, ELEVATION_GROUP_NONE, FULL_BPS, MAX_NUM_ELEVATION_GROUPS},
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
            market.lending_market_owner_cached = value;
            msg!("Value is {:?}", value);
        }
        UpdateLendingMarketMode::UpdateEmergencyMode => {
            let emergency_mode = value[0];
            msg!("Value is {:?}", emergency_mode);
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
            msg!("Value is {:?}", liquidation_close_factor);
            if !(5..=100).contains(&liquidation_close_factor) {
                return err!(LendingError::InvalidFlag);
            }
            market.liquidation_max_debt_close_factor_pct = liquidation_close_factor;
        }
        UpdateLendingMarketMode::UpdateLiquidationMaxValue => {
            let value = u64::from_le_bytes(value[..8].try_into().unwrap());
            msg!("Value is {:?}", value);
            market.max_liquidatable_debt_market_value_at_once = value;
        }
        UpdateLendingMarketMode::UpdateGlobalAllowedBorrow => {
            let value = u64::from_le_bytes(value[..8].try_into().unwrap());
            msg!("Value is {:?}", value);
            market.global_allowed_borrow_value = value;
        }
        UpdateLendingMarketMode::UpdateGlobalUnhealthyBorrow => {
            let value = u64::from_le_bytes(value[..8].try_into().unwrap());
            msg!("Value is {:?}", value);
            market.global_unhealthy_borrow_value = value;
        }
        UpdateLendingMarketMode::UpdateMinFullLiquidationThreshold => {
            let value = u64::from_le_bytes(value[..8].try_into().unwrap());
            msg!("Value is {:?}", value);
            market.min_full_liquidation_value_threshold = value;
        }
        UpdateLendingMarketMode::UpdateRiskCouncil => {
            let value: [u8; 32] = value[0..32].try_into().unwrap();
            let value = Pubkey::from(value);
            market.risk_council = value;
            msg!("Value is {:?}", value);
        }
        UpdateLendingMarketMode::UpdateInsolvencyRiskLtv => {
            let insolvency_risk_ltv = value[0];
            msg!("Value is {:?}", insolvency_risk_ltv);

            if !(5..=100).contains(&insolvency_risk_ltv) {
                return err!(LendingError::InvalidFlag);
            }
            market.insolvency_risk_unhealthy_ltv_pct = insolvency_risk_ltv;
        }
        UpdateLendingMarketMode::UpdateElevationGroup => {
            let elevation_group: ElevationGroup =
                BorshDeserialize::deserialize(&mut &value[..]).unwrap();
            msg!("Value is {:?}", elevation_group);

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

            if Fraction::from_percent(elevation_group.liquidation_threshold_pct)
                + Fraction::from_percent(elevation_group.liquidation_threshold_pct)
                    * Fraction::from_bps(elevation_group.max_liquidation_bonus_bps)
                > Fraction::ONE
            {
                msg!("Max liquidation bonus * liquidation threshold is greater than 100%, invalid");
                return err!(LendingError::InvalidElevationGroupConfig);
            }

            market.set_elevation_group(elevation_group)?;
        }
        UpdateLendingMarketMode::UpdateReferralFeeBps => {
            let value = u16::from_le_bytes(value[..2].try_into().unwrap());
            msg!("Value is {:?}", value);
            if value > FULL_BPS {
                msg!("Referral fee bps must be in range [0, 10000]");
                return err!(LendingError::InvalidConfig);
            }
            market.referral_fee_bps = value;
        }
        UpdateLendingMarketMode::UpdateMultiplierPoints => {
            msg!("Value is {:?}", value);
            let value: [u8; 8] = value[..8].try_into().unwrap();
            market.multiplier_points_tag_boost = value;
            msg!("Setting multiplier tag to {value:?}",);
        }
        UpdateLendingMarketMode::UpdatePriceRefreshTriggerToMaxAgePct => {
            let value = value[0];
            msg!("Value is {:?}", value);
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
    }

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateLendingMarket<'info> {
    lending_market_owner: Signer<'info>,

    #[account(mut, has_one = lending_market_owner)]
    pub lending_market: AccountLoader<'info, LendingMarket>,
}
