use anchor_lang::prelude::{require, Result};

use crate::LendingError;

pub fn validate_numerical_bool(value: u8) -> Result<()> {
    let num_matches_boolean_values = matches!(value, 0 | 1);
    require!(num_matches_boolean_values, LendingError::InvalidFlag);
    Ok(())
}
