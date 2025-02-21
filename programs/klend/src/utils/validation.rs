use anchor_lang::prelude::{require, Result};

use crate::LendingError;

pub fn validate_numerical_bool(value: u8) -> Result<()> {
    let num_matches_boolean_values = matches!(value, 0 | 1);
    require!(num_matches_boolean_values, LendingError::InvalidFlag);
    Ok(())
}

pub fn zip_and_validate_same_length<L, R>(
    lefts: impl IntoIterator<Item = L>,
    rights: impl IntoIterator<Item = R>,
) -> impl Iterator<Item = LengthCheckedResult<L, R>> {
    LengthCheckingZipIterator {
        lefts: lefts.into_iter(),
        rights: rights.into_iter(),
        errored: false,
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct LengthMismatchError;

pub type LengthCheckedResult<L, R> = core::result::Result<(L, R), LengthMismatchError>;

struct LengthCheckingZipIterator<L, R> {
    lefts: L,
    rights: R,
    errored: bool,
}

impl<L: Iterator, R: Iterator> Iterator for LengthCheckingZipIterator<L, R> {
    type Item = LengthCheckedResult<L::Item, R::Item>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.errored {
            return None;
        }
        match (self.lefts.next(), self.rights.next()) {
            (None, None) => None,
            (Some(left), Some(right)) => Some(Ok((left, right))),
            _different => {
                self.errored = true;
                Some(Err(LengthMismatchError))
            }
        }
    }
}
