use std::fmt::{self, Display, Formatter};

use anchor_lang::prelude::*;
use derivative::Derivative;
use solana_program::pubkey::Pubkey;

use crate::utils::{Fraction, REFERRER_STATE_SIZE, REFERRER_TOKEN_STATE_SIZE, USER_METADATA_SIZE};

static_assertions::const_assert_eq!(
    REFERRER_TOKEN_STATE_SIZE,
    std::mem::size_of::<ReferrerTokenState>()
);
static_assertions::const_assert_eq!(0, std::mem::size_of::<ReferrerTokenState>() % 8);
#[derive(PartialEq, Derivative, Default)]
#[derivative(Debug)]
#[account(zero_copy)]
#[repr(C)]
pub struct ReferrerTokenState {
    pub referrer: Pubkey,
    pub mint: Pubkey,
    pub amount_unclaimed_sf: u128,
    pub amount_cumulative_sf: u128,
    pub bump: u64,

    #[derivative(Debug = "ignore")]
    pub padding: [u64; 31],
}

impl Display for ReferrerTokenState {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let Self {
            referrer,
            mint,
            amount_unclaimed_sf,
            amount_cumulative_sf,
            bump: _,
            padding: _,
        } = self;
        let amount_unclaimed: u64 = Fraction::from_bits(*amount_unclaimed_sf).to_num();
        let amount_cumulative: u64 = Fraction::from_bits(*amount_cumulative_sf).to_num();
        write!(
            f,
            "Referrer Account: referrer: {}, mint: {}, amount_unclaimed (integer part): {}, amount_cumulative (integer part): {}",
            referrer, mint, amount_unclaimed, amount_cumulative
        )?;

        Ok(())
    }
}

static_assertions::const_assert_eq!(USER_METADATA_SIZE, std::mem::size_of::<UserMetadata>());
static_assertions::const_assert_eq!(0, std::mem::size_of::<UserMetadata>() % 8);
#[derive(PartialEq, Derivative)]
#[derivative(Debug)]
#[account(zero_copy)]
#[repr(C)]
pub struct UserMetadata {
    pub referrer: Pubkey,
    pub bump: u64,
    pub user_lookup_table: Pubkey,
    pub owner: Pubkey,

    #[derivative(Debug = "ignore")]
    pub padding_1: [u64; 51],
    #[derivative(Debug = "ignore")]
    pub padding_2: [u64; 64],
}

impl Default for UserMetadata {
    fn default() -> Self {
        Self {
            referrer: Pubkey::default(),
            bump: 0,
            user_lookup_table: Pubkey::default(),
            owner: Pubkey::default(),
            padding_1: [0; 51],
            padding_2: [0; 64],
        }
    }
}

static_assertions::const_assert_eq!(REFERRER_STATE_SIZE, std::mem::size_of::<ReferrerState>());
static_assertions::const_assert_eq!(0, std::mem::size_of::<ReferrerState>() % 8);
#[derive(PartialEq, Derivative)]
#[derivative(Debug)]
#[account(zero_copy)]
#[repr(C)]
pub struct ReferrerState {
    pub short_url: Pubkey,
    pub owner: Pubkey,
}

#[derive(PartialEq, Debug)]
#[account()]
pub struct ShortUrl {
    pub referrer: Pubkey,
    pub short_url: String,
}
