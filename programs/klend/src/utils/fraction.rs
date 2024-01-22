use fixed::traits::{FromFixed, ToFixed};
pub use fixed::types::U68F60 as Fraction;
pub use fixed_macro::types::U68F60 as fraction;

use crate::LendingError;

#[allow(clippy::assign_op_pattern)]
#[allow(clippy::reversed_empty_ranges)]
mod uint_types {
    use uint::construct_uint;
    construct_uint! {
               pub struct U256(4);
    }
    construct_uint! {
               pub struct U128(2);
    }
}

pub use uint_types::{U128, U256};

pub const FRACTION_ONE_SCALED: u128 = Fraction::ONE.to_bits();

pub fn pow_fraction(fraction: Fraction, power: u32) -> Option<Fraction> {
    if power == 0 {
        return Some(Fraction::ONE);
    }

    let mut x = fraction;
    let mut y = Fraction::ONE;
    let mut n = power;

    while n > 1 {
        if n % 2 == 1 {
            y = x.checked_mul(y)?;
        }
        x = x.checked_mul(x)?;
        n /= 2;
    }

    x.checked_mul(y)
}

#[inline]
pub const fn bps_u128_to_fraction(bps: u128) -> Fraction {
    if bps == 10_000 {
        return Fraction::ONE;
    }
    Fraction::const_from_int(bps).unwrapped_div_int(10_000)
}

#[inline]
pub const fn pct_u128_to_fraction(percent: u128) -> Fraction {
    if percent == 100 {
        return Fraction::ONE;
    }
    Fraction::const_from_int(percent).unwrapped_div_int(100)
}

pub trait FractionExtra {
    fn to_percent<Dst: FromFixed>(&self) -> Option<Dst>;
    fn to_bps<Dst: FromFixed>(&self) -> Option<Dst>;
    fn from_percent<Src: ToFixed>(percent: Src) -> Self;
    fn from_bps<Src: ToFixed>(bps: Src) -> Self;
    fn checked_pow(&self, power: u32) -> Option<Self>
    where
        Self: std::marker::Sized;

    fn to_floor<Dst: FromFixed>(&self) -> Dst;
    fn to_ceil<Dst: FromFixed>(&self) -> Dst;
    fn to_round<Dst: FromFixed>(&self) -> Dst;

    fn to_sf(&self) -> u128;
    fn from_sf(sf: u128) -> Self;

    fn to_display(&self) -> FractionDisplay;
}

impl FractionExtra for Fraction {
    #[inline]
    fn to_percent<Dst: FromFixed>(&self) -> Option<Dst> {
        (self * 100).round().checked_to_num()
    }

    #[inline]
    fn to_bps<Dst: FromFixed>(&self) -> Option<Dst> {
        (self * 10_000).round().checked_to_num()
    }

    #[inline]
    fn from_percent<Src: ToFixed>(percent: Src) -> Self {
        let percent = Fraction::from_num(percent);
        percent / 100
    }

    #[inline]
    fn from_bps<Src: ToFixed>(bps: Src) -> Self {
        let bps = Fraction::from_num(bps);
        bps / 10_000
    }

    #[inline]
    fn checked_pow(&self, power: u32) -> Option<Self>
    where
        Self: std::marker::Sized,
    {
        pow_fraction(*self, power)
    }

    #[inline]
    fn to_floor<Dst: FromFixed>(&self) -> Dst {
        self.floor().to_num()
    }

    #[inline]
    fn to_ceil<Dst: FromFixed>(&self) -> Dst {
        self.ceil().to_num()
    }

    #[inline]
    fn to_round<Dst: FromFixed>(&self) -> Dst {
        self.round().to_num()
    }

    #[inline]
    fn to_sf(&self) -> u128 {
        self.to_bits()
    }

    #[inline]
    fn from_sf(sf: u128) -> Self {
        Fraction::from_bits(sf)
    }

    #[inline]
    fn to_display(&self) -> FractionDisplay {
        FractionDisplay(self)
    }
}

pub fn to_sf<Src: ToFixed>(src: Src) -> u128 {
    Fraction::from_num(src).to_bits()
}

pub const fn to_sf_const(src: u128) -> u128 {
    Fraction::const_from_int(src).to_bits()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Default, PartialOrd, Ord)]
pub struct BigFraction(pub U256);

impl<T> From<T> for BigFraction
where
    T: Into<Fraction>,
{
    fn from(fraction: T) -> Self {
        let fraction: Fraction = fraction.into();
        let repr_fraction = fraction.to_bits();
        Self(U256::from(repr_fraction))
    }
}

impl TryFrom<BigFraction> for Fraction {
    type Error = LendingError;

    fn try_from(value: BigFraction) -> Result<Self, Self::Error> {
        let repr_faction: u128 = value
            .0
            .try_into()
            .map_err(|_| LendingError::IntegerOverflow)?;
        Ok(Fraction::from_bits(repr_faction))
    }
}

impl BigFraction {
    pub fn to_bits(&self) -> [u64; 4] {
        self.0 .0
    }

    pub fn from_bits(bits: [u64; 4]) -> Self {
        Self(U256(bits))
    }

    pub fn to_u128_sf(&self) -> u128 {
        let v = self.0 .0;
        let low = v[0] as u128;
        let high = v[1] as u128;
        (high << 64) | low
    }

    pub fn from_num<T>(num: T) -> Self
    where
        T: Into<U256>,
    {
        let value: U256 = num.into();
        let sf = value << Fraction::FRAC_NBITS;
        Self(sf)
    }
}

use std::{
    fmt::Display,
    ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign},
};

impl Add for BigFraction {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign for BigFraction {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }
}

impl Sub for BigFraction {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

impl SubAssign for BigFraction {
    fn sub_assign(&mut self, rhs: Self) {
        self.0 -= rhs.0;
    }
}

impl Mul for BigFraction {
    type Output = Self;

    fn mul(self, rhs: Self) -> Self::Output {
        let extra_scaled = self.0 * rhs.0;
        let res = extra_scaled >> Fraction::FRAC_NBITS;
        Self(res)
    }
}

impl MulAssign for BigFraction {
    fn mul_assign(&mut self, rhs: Self) {
        *self = *self * rhs;
    }
}

impl Div for BigFraction {
    type Output = Self;

    fn div(self, rhs: Self) -> Self::Output {
        let extra_scaled = self.0 << Fraction::FRAC_NBITS;
        let res = extra_scaled / rhs.0;
        Self(res)
    }
}

impl DivAssign for BigFraction {
    fn div_assign(&mut self, rhs: Self) {
        *self = *self / rhs;
    }
}

impl<T> Mul<T> for BigFraction
where
    T: Into<U256>,
{
    type Output = Self;

    fn mul(self, rhs: T) -> Self::Output {
        let rhs: U256 = rhs.into();
        Self(self.0 * rhs)
    }
}

impl<T> MulAssign<T> for BigFraction
where
    T: Into<U256>,
{
    fn mul_assign(&mut self, rhs: T) {
        let rhs: U256 = rhs.into();
        self.0 *= rhs;
    }
}

impl<T> Div<T> for BigFraction
where
    T: Into<U256>,
{
    type Output = Self;

    fn div(self, rhs: T) -> Self::Output {
        let rhs: U256 = rhs.into();
        Self(self.0 / rhs)
    }
}

impl<T> DivAssign<T> for BigFraction
where
    T: Into<U256>,
{
    fn div_assign(&mut self, rhs: T) {
        let rhs: U256 = rhs.into();
        self.0 /= rhs;
    }
}

impl From<U128> for U256 {
    fn from(value: U128) -> Self {
        Self([value.0[0], value.0[1], 0, 0])
    }
}

impl TryFrom<U256> for U128 {
    type Error = LendingError;

    fn try_from(value: U256) -> Result<Self, Self::Error> {
        if value.0[2] != 0 || value.0[3] != 0 {
            return Err(LendingError::IntegerOverflow);
        }
        Ok(Self([value.0[0], value.0[1]]))
    }
}

pub struct FractionDisplay<'a>(&'a Fraction);

impl Display for FractionDisplay<'_> {
    fn fmt(&self, formater: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let sf = self.0.to_bits();

        const ROUND_COMP: u128 = (1 << Fraction::FRAC_NBITS) / (10_000 * 2);
        let sf = sf + ROUND_COMP;

        let i = sf >> Fraction::FRAC_NBITS;

        const FRAC_MASK: u128 = (1 << Fraction::FRAC_NBITS) - 1;
        let f_p = (sf & FRAC_MASK) as u64;
        let f_p = ((f_p >> 30) * 10_000) >> 30;
        write!(formater, "{i}.{f_p:0>4}")
    }
}
