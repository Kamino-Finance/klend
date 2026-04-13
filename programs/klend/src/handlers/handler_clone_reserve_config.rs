use std::ops::{Deref, DerefMut};

use anchor_lang::{prelude::*, Accounts};

use crate::{
    lending_market::lending_operations, utils::accounts::default_array, LendingError,
    LendingMarket, Reserve, ReserveConfigCustomizations,
};

pub fn process<'info>(
    ctx: Context<'_, '_, '_, 'info, CloneReserveConfig<'info>>,
    customizations: ReserveConfigCustomizationArgs,
) -> Result<()> {
    let source_reserve = ctx.accounts.source_reserve.load()?;
    let mut target_reserve = ctx.accounts.target_reserve.load_mut()?;

    lending_operations::clone_reserve_config(
        source_reserve.deref(),
        target_reserve.deref_mut(),
        customizations.try_into()?,
    )?;

    Ok(())
}

#[derive(Accounts)]
pub struct CloneReserveConfig<'info> {
    #[account(constraint = lending_operations::utils::is_allowed_signer_to_clone_reserve_config(
        signer.key(),
        target_lending_market.load()?.deref(),
        target_reserve.load()?.deref(),
    ) @ LendingError::InvalidSigner)]
    signer: Signer<'info>,




   
    #[account(
        address = target_reserve.load()?.lending_market,
        constraint = !target_lending_market.load()?.is_immutable() @ LendingError::OperationNotPermittedMarketImmutable
    )]
    target_lending_market: AccountLoader<'info, LendingMarket>,


    #[account(
        constraint = !source_reserve.load()?.is_usage_blocked() @ LendingError::CloneSourceReserveDisabled,
        constraint = !source_reserve.load()?.config.is_emergency_mode() @ LendingError::ReserveEmergencyMode,
    )]
    source_reserve: AccountLoader<'info, Reserve>,


    #[account(mut,
        constraint = target_reserve.load()?.is_predeposit(target_lending_market.load()?.min_initial_deposit_amount) @ LendingError::CloneTargetReserveAlreadyInUse,
        constraint = target_reserve.load()?.liquidity.mint_pubkey == source_reserve.load()?.liquidity.mint_pubkey @ LendingError::ClonedReserveLiquidityMintMismatch,
        constraint = !target_reserve.load()?.config.is_emergency_mode() @ LendingError::ReserveEmergencyMode,
    )]
    target_reserve: AccountLoader<'info, Reserve>,
}


#[derive(AnchorDeserialize, AnchorSerialize, Clone, Debug, Default)]
pub struct ReserveConfigCustomizationArgs {

    pub override_fixed_rate_bps: u8,



    pub fixed_borrow_rate_bps: u32,


    pub override_debt_term_seconds: u8,



    pub debt_term_seconds: u64,






    pub clear_elevation_groups: u8,
}

impl TryFrom<ReserveConfigCustomizationArgs> for ReserveConfigCustomizations {
    type Error = Error;

    fn try_from(args: ReserveConfigCustomizationArgs) -> Result<Self> {
       
        fn gated<T: Default + PartialEq>(gate: u8, value: T) -> Result<Option<T>> {
            Ok(if gate == false as u8 {
                if value != T::default() {
                    msg!("Overridden value must be zeroed when not overriding");
                    return err!(LendingError::InvalidConfig);
                }
                None
            } else {
                Some(value)
            })
        }

       
        let ReserveConfigCustomizationArgs {
            override_fixed_rate_bps,
            fixed_borrow_rate_bps,
            override_debt_term_seconds,
            debt_term_seconds,
            clear_elevation_groups,
        } = args;
        Ok(Self {
            overridden_fixed_rate_bps: gated(override_fixed_rate_bps, fixed_borrow_rate_bps)?,
            overridden_debt_term_seconds: gated(override_debt_term_seconds, debt_term_seconds)?,
            overridden_elevation_groups: gated(clear_elevation_groups, default_array())?,
        })
    }
}
