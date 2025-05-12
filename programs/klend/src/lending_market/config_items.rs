use std::{
    fmt::{Debug, Display},
    marker::PhantomData,
};

use anchor_lang::prelude::*;
use borsh::BorshDeserialize;
use solana_program::msg;

pub struct ConfigItemUpdater<'h, H, T, S, G, V, R> {
    target: &'h mut H,
    name: String,
    setter: S,
    getter: G,
    validator: V,
    renderer: R,
    value_type_phantom: PhantomData<T>,
}

#[must_use]
pub fn for_field<T: Debug>(
    field: &mut T,
) -> ConfigItemUpdater<
    T,
    T,
    impl Setter<T, T>,
    impl Getter<T, T>,
    impl Validator<T>,
    impl Renderer<T>,
> {
    for_object(field).using_setter_and_getter(set_field_directly, get_field_directly)
}

macro_rules! for_named_field {
    ($expr:expr) => {
        $crate::lending_market::config_items::for_field($expr).named(stringify!($expr))
    };
}
pub(crate) use for_named_field;

#[must_use]
pub fn for_object<H>(target: &mut H) -> ConfigItemUpdater<H, (), (), (), (), ()> {
    ConfigItemUpdater {
        target,
        name: "<unnamed>".to_string(),
        getter: (),
        setter: (),
        validator: (),
        renderer: (),
        value_type_phantom: PhantomData,
    }
}

impl<'h, H, T, S: Setter<H, T>, G: Getter<H, T>, V: Validator<T>, R: Renderer<T>>
    ConfigItemUpdater<'h, H, T, S, G, V, R>
{
    #[must_use]
    pub fn named(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    #[must_use]
    pub fn validating<NV: Validator<T>>(self, new: NV) -> ConfigItemUpdater<'h, H, T, S, G, NV, R> {
        let Self {
            target,
            name,
            value_type_phantom,
            getter,
            setter,
            validator: _replaced,
            renderer,
        } = self;
        ConfigItemUpdater {
            target,
            name,
            value_type_phantom,
            getter,
            setter,
            validator: new,
            renderer,
        }
    }

    #[must_use]
    pub fn rendering<NR: Renderer<T>>(self, new: NR) -> ConfigItemUpdater<'h, H, T, S, G, V, NR> {
        let Self {
            target,
            name,
            value_type_phantom,
            getter,
            setter,
            validator,
            renderer: _replaced,
        } = self;
        ConfigItemUpdater {
            target,
            name,
            value_type_phantom,
            getter,
            setter,
            validator,
            renderer: new,
        }
    }

    pub fn set(self, source: &[u8]) -> Result<()>
    where
        T: BorshDeserialize,
    {
        let new_value = T::deserialize(&mut &source[..])?;
        let ConfigItemUpdater {
            target,
            name,
            getter,
            setter,
            validator,
            renderer,
            ..
        } = self;
        validator(&new_value)?;
        let prv = getter(target, &new_value);
        msg!("Prv value: {} = {}", name, RenderedOption(&renderer, prv));
        msg!("New value: {} = {}", name, Rendered(&renderer, &new_value));
        setter(target, new_value)?;
        Ok(())
    }
}

impl<'h, H, S: Setter<H, u8>, G: Getter<H, u8>, V: Validator<u8>, R: Renderer<u8>>
    ConfigItemUpdater<'h, H, u8, S, G, V, R>
{
    #[must_use]
    pub fn representing_u8_enum<E: TryFrom<u8> + Debug>(
        self,
    ) -> ConfigItemUpdater<'h, H, u8, S, G, impl Validator<u8>, impl Renderer<u8>> {
        self.validating(validations::check_valid_u8_enum::<E>)
            .rendering(renderings::as_u8_enum::<E>)
    }
}

impl<'h, H> ConfigItemUpdater<'h, H, (), (), (), (), ()> {
    #[must_use]
    pub fn using_setter_and_getter<T: Debug, S: Setter<H, T>, G: Getter<H, T>>(
        self,
        setter: S,
        getter: G,
    ) -> ConfigItemUpdater<'h, H, T, S, G, impl Validator<T>, impl Renderer<T>> {
        ConfigItemUpdater {
            target: self.target,
            name: self.name,
            getter,
            setter,
            validator: accept_anything,
            renderer: write_debug,
            value_type_phantom: PhantomData,
        }
    }
}

pub mod validations {
    use std::{any::type_name, ops::RangeInclusive};

    use super::*;
    use crate::LendingError;

    pub fn check_bool<T: Into<u128> + Clone>(value: &T) -> Result<()> {
        let value = value.clone().into();
        if value > 1 {
            msg!("A boolean flag must be 0 or 1, got {:?}", value);
            return err!(LendingError::InvalidFlag);
        }
        Ok(())
    }

    pub fn check_not_zero<T: Into<u128> + Clone>(value: &T) -> Result<()> {
        if value.clone().into() == 0 {
            msg!("Value cannot be 0");
            return err!(LendingError::InvalidConfig);
        }
        Ok(())
    }

    pub fn check_not_negative<T: Into<i128> + Clone>(value: &T) -> Result<()> {
        let value = value.clone().into();
        if value < 0 {
            msg!("Value cannot be negative, got {:?}", value);
            return err!(LendingError::InvalidConfig);
        }
        Ok(())
    }

    pub fn check_in_range<T: Into<u128> + Clone>(
        range: RangeInclusive<u128>,
    ) -> impl Fn(&T) -> Result<()> {
        move |value| {
            let value = value.clone().into();
            if !range.contains(&value) {
                msg!("Value must be in range {:?}, got {:?}", range, value);
                return err!(LendingError::InvalidConfig);
            }
            Ok(())
        }
    }

    pub fn check_valid_pct<T: Into<u128> + Clone>(value: &T) -> Result<()> {
        check_in_range(0..=100)(value)
    }

    pub fn check_valid_bps<T: Into<u128> + Clone>(value: &T) -> Result<()> {
        check_in_range(0..=10_000)(value)
    }

    pub fn check_gte<T: PartialOrd + Display, U: Into<T> + Clone>(
        min: T,
    ) -> impl Fn(&U) -> Result<()> {
        move |value| {
            let value_t: T = value.clone().into();
            if value_t < min {
                msg!("Value cannot be lower than {}, got {}", min, value_t);
                return err!(LendingError::InvalidConfig);
            }
            Ok(())
        }
    }

    pub fn check_lte<T: PartialOrd + Display, U: Into<T> + Clone>(
        max: T,
    ) -> impl Fn(&U) -> Result<()> {
        move |value| {
            let value_t: T = value.clone().into();
            if value_t > max {
                msg!("Value cannot be greater than {}, got {}", max, value_t);
                return err!(LendingError::InvalidConfig);
            }
            Ok(())
        }
    }

    pub fn check_valid_u8_enum<E: TryFrom<u8>>(repr: &u8) -> Result<()> {
        match E::try_from(*repr) {
            Ok(_) => Ok(()),
            Err(_) => {
                msg!(
                    "Enum {} cannot be represented by u8 {}",
                    type_name::<E>(),
                    repr
                );
                err!(LendingError::InvalidConfig)
            }
        }
    }
}

pub mod renderings {
    use std::any::type_name;

    use super::*;
    use crate::fraction::Fraction;

    pub fn as_utf8_null_padded_string<const N: usize>(
        value: &[u8; N],
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        let str = std::str::from_utf8(value).unwrap().trim_end_matches('\0');
        Display::fmt(str, f)
    }

    pub fn as_fraction<T: Into<u128> + Clone>(
        value: &T,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        let fraction = Fraction::from_bits(value.clone().into());
        Display::fmt(&fraction, f)
    }

    pub fn as_u8_enum<E: TryFrom<u8> + Debug>(
        repr: &u8,
        f: &mut std::fmt::Formatter,
    ) -> std::fmt::Result {
        match E::try_from(*repr) {
            Ok(value) => value.fmt(f),
            Err(_) => write!(f, "<unknown {} = {}>", type_name::<E>(), repr),
        }
    }
}

pub trait Setter<H, T>: for<'a> Fn(&'a mut H, T) -> Result<()> {}
impl<F: for<'a> Fn(&'a mut H, T) -> Result<()>, H, T> Setter<H, T> for F {}

pub trait Getter<H, T>: for<'a> Fn(&'a H, &T) -> Result<Option<&'a T>> {}
impl<F: for<'a> Fn(&'a H, &T) -> Result<Option<&'a T>>, H, T> Getter<H, T> for F {}

pub trait Validator<T>: for<'a> Fn(&'a T) -> Result<()> {}
impl<F: for<'a> Fn(&'a T) -> Result<()>, T> Validator<T> for F {}

pub trait Renderer<T>:
    for<'a, 'b> Fn(&'a T, &mut std::fmt::Formatter<'b>) -> std::fmt::Result
{
}
impl<F: for<'a, 'b> Fn(&'a T, &mut std::fmt::Formatter<'b>) -> std::fmt::Result, T> Renderer<T>
    for F
{
}

struct Rendered<'a, R, T>(&'a R, &'a T);

impl<'a, R: Renderer<T>, T> Display for Rendered<'a, R, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let &Rendered(renderer, value) = self;
        renderer(value, f)
    }
}

struct RenderedOption<'a, R, T>(&'a R, Result<Option<&'a T>>);

impl<'a, R: Renderer<T>, T> Display for RenderedOption<'a, R, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let RenderedOption(renderer, value) = self;
        match value {
            Ok(Some(value)) => renderer(*value, f),
            Ok(None) => f.write_str("<not present>"),
            Err(error) => write!(f, "<unavailable: {:?}>", error),
        }
    }
}

fn accept_anything<T>(_value: &T) -> Result<()> {
    Ok(())
}

fn write_debug<T: Debug>(value: &T, f: &mut std::fmt::Formatter) -> std::fmt::Result {
    value.fmt(f)
}

fn set_field_directly<T>(field: &mut T, new_value: T) -> Result<()> {
    *field = new_value;
    Ok(())
}

fn get_field_directly<'t, T>(field: &'t T, _irrelevant_new_value: &T) -> Result<Option<&'t T>> {
    Ok(Some(field))
}
