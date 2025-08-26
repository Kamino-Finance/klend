#[allow(clippy::derivable_impls)]
impl Default for crate::accounts::OptionalObligationFarmsAccounts {
    fn default() -> Self {
        Self {
            obligation_farm_user_state: None,
            reserve_farm_state: None,
        }
    }
}

impl Clone for crate::accounts::OptionalObligationFarmsAccounts {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for crate::accounts::OptionalObligationFarmsAccounts {}

pub fn default_array<T: Default + Copy, const N: usize>() -> [T; N] {
    [T::default(); N]
}






pub fn is_default_array<T: Default + PartialEq>(array: &[T]) -> bool {
    let default_value = T::default();
    array.iter().all(|element| *element == default_value)
}
