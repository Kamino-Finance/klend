pub mod account_loader_trait;
pub mod account_ops;
pub mod accounts;
pub mod borrow_rate_curve;
pub mod constraints;
pub mod consts;
pub mod emit_cpi_support;
pub mod fraction;
pub mod macros;
pub mod permissioning;
pub mod prices;
pub mod refresh_ix_utils;
pub mod secs;
pub mod seeds;
pub mod spltoken;
pub mod token_transfer;
pub mod validation;

use std::sync::Arc;

pub use account_loader_trait::*;
pub use account_ops::*;
use anchor_lang::prelude::Pubkey;
pub use constraints::*;
pub use consts::*;
pub use emit_cpi_support::*;
pub use fraction::*;
pub use prices::*;
pub use refresh_ix_utils::*;
pub use spltoken::*;
pub use token_transfer::*;
pub use validation::*;

pub fn maybe_null_pk(pubkey: Pubkey) -> Option<Pubkey> {
    if pubkey == Pubkey::default() || pubkey == NULL_PUBKEY {
        None
    } else {
        Some(pubkey)
    }
}










pub trait JustRef<T> {

    fn just_ref(&self) -> &T;
}

impl<T> JustRef<T> for T {
    fn just_ref(&self) -> &T {
        self
    }
}

impl<T> JustRef<T> for &T {
    fn just_ref(&self) -> &T {
        self
    }
}

impl<T> JustRef<T> for Arc<T> {
    fn just_ref(&self) -> &T {
        self
    }
}

impl<T> JustRef<T> for &Arc<T> {
    fn just_ref(&self) -> &T {
        self
    }
}

pub fn borsh_deserialize<T: borsh::BorshDeserialize>(mut data: &[u8]) -> T {
    T::deserialize(&mut data).expect("Borsh deserialization failed")
}
