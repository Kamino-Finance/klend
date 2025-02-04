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
