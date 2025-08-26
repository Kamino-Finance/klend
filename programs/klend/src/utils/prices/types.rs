use anchor_lang::prelude::*;

use super::utils::ten_pow;
use crate::utils::{Fraction, U256};







#[derive(Clone, Copy, Debug, AnchorDeserialize, AnchorSerialize, Default)]
pub(crate) struct Price<T>
where
    T: Into<U256>,
{

    pub value: T,

   
    pub exp: u32,
}

impl<T> Price<T>
where
    T: Into<U256> + Copy + TryFrom<U256>,
{





    pub fn to_adjusted_exp(self, target_exp: u32) -> Option<Price<T>> {
        if target_exp == self.exp {
            return Some(self);
        }
        let value: U256 = self.value.into();
        let exp = self.exp;

        let value_256 = if exp > target_exp {
           
            let diff = exp - target_exp;
            let factor = ten_pow(diff).into();
            value.checked_div(factor)
        } else {
           
            let diff = target_exp - exp;
            let factor = ten_pow(diff).into();
            value.checked_mul(factor)
        };

        value_256.and_then(|value| {
            Some(Price {
                value: T::try_from(value).ok()?,
                exp: target_exp,
            })
        })
    }





    pub fn reduce_exp_lossy(self, target_exp: u32) -> Option<Price<T>> {
        if self.exp <= target_exp {
            return Some(self);
        }
        self.to_adjusted_exp(target_exp)
    }
}

impl<T> Price<T>
where
    T: Into<U256> + Copy,
{
    pub fn size_up<U>(self) -> Price<U>
    where
        U: From<T> + Into<U256>,
    {
        let Price { value, exp } = self;
        Price {
            value: U::from(value),
            exp,
        }
    }
}

pub(super) struct TimestampedPrice {
    pub price_load: Box<dyn FnOnce() -> Result<Fraction>>,
    pub timestamp: u64,
}

pub(super) struct TimestampedPriceWithTwap {
    pub price: TimestampedPrice,
    pub twap: Option<TimestampedPrice>,
}


