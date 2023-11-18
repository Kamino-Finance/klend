use anchor_lang::{prelude::*, Accounts};

use crate::{
    lending_market::lending_operations,
    state::{LendingMarket, Reserve, UpdateConfigMode},
    VALUE_BYTE_ARRAY_LEN_RESERVE,
};

pub fn process(
    ctx: Context<UpdateReserveConfig>,
    mode: u64,
    value: [u8; VALUE_BYTE_ARRAY_LEN_RESERVE],
) -> Result<()> {
    let mode =
        UpdateConfigMode::try_from(mode).map_err(|_| ProgramError::InvalidInstructionData)?;

    let reserve = &mut ctx.accounts.reserve.load_mut()?;
    let market = ctx.accounts.lending_market.load()?;

    msg!(
        "Updating reserve {:?} config with mode {:?} and value {:?}",
        ctx.accounts.reserve.key(),
        mode,
        &value[0..32]
    );

    let clock = Clock::get()?;
    lending_operations::refresh_reserve_interest(reserve, clock.slot, market.referral_fee_bps)?;

    lending_operations::update_reserve_config(reserve, mode, value);

    lending_operations::utils::validate_reserve_config(&reserve.config, &market)?;

    Ok(())
}

#[derive(Accounts)]
pub struct UpdateReserveConfig<'info> {
    lending_market_owner: Signer<'info>,

    #[account(has_one = lending_market_owner)]
    lending_market: AccountLoader<'info, LendingMarket>,

    #[account(mut,
        has_one = lending_market
    )]
    reserve: AccountLoader<'info, Reserve>,
}
