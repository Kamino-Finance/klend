pub mod account_loader_trait;
pub mod account_ops;
pub mod borrow_rate_curve;
pub mod constraints;
pub mod consts;
pub mod fraction;
pub mod macros;
pub mod prices;
pub mod refresh_ix_utils;
pub mod seeds;
pub mod slots;
pub mod spltoken;
pub mod token_transfer;

pub use account_loader_trait::*;
pub use account_ops::*;
use anchor_lang::prelude::Pubkey;
pub use constraints::*;
pub use consts::*;
pub use fraction::*;
pub use prices::*;
pub use refresh_ix_utils::*;
pub use spltoken::*;
pub use token_transfer::*;

pub fn maybe_null_pk(pubkey: Pubkey) -> Option<Pubkey> {
    if pubkey == Pubkey::default() || pubkey == NULL_PUBKEY {
        None
    } else {
        Some(pubkey)
    }
}
