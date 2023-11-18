use anchor_lang::{
    err,
    prelude::{AccountLoader, Context},
    Result,
};

use crate::{state::LendingMarket, LendingError};

pub fn emergency_mode_disabled(lending_market: &AccountLoader<LendingMarket>) -> Result<()> {
    if lending_market.load()?.emergency_mode > 0 {
        return err!(LendingError::GlobalEmergencyMode);
    }
    Ok(())
}

pub fn check_remaining_accounts<T>(ctx: &Context<T>) -> Result<()> {
    if !ctx.remaining_accounts.is_empty() {
        return err!(LendingError::InvalidAccountInput);
    }

    Ok(())
}
