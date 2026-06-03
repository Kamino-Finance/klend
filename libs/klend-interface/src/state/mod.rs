mod common;
mod global_config;
mod lending_market;
mod obligation;
mod permissioning;
mod pod;
mod referral;
mod reserve;
mod withdraw_ticket;

pub use common::*;
pub use global_config::*;
pub use lending_market::*;
pub use obligation::*;
pub use permissioning::*;
pub use pod::PodU128;
pub use referral::*;
pub use reserve::*;
pub use spl_discriminator::{ArrayDiscriminator, SplDiscriminate};
pub use withdraw_ticket::*;

/// Size of the Anchor account discriminator (8 bytes).
pub const DISCRIMINATOR_SIZE: usize = ArrayDiscriminator::LENGTH;

/// Errors returned by [`from_account_data`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountDataError {
    /// Account data is shorter than discriminator + struct size.
    DataTooShort { expected: usize, actual: usize },
    /// The 8-byte discriminator does not match the expected value.
    InvalidDiscriminator { expected: [u8; 8], actual: [u8; 8] },
    /// The data slice is not properly aligned for the target type.
    AlignmentError,
}

impl core::fmt::Display for AccountDataError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::DataTooShort { expected, actual } => {
                write!(
                    f,
                    "account data too short: expected {expected}, got {actual}"
                )
            }
            Self::InvalidDiscriminator { expected, actual } => {
                write!(
                    f,
                    "invalid discriminator: expected {expected:?}, got {actual:?}"
                )
            }
            Self::AlignmentError => {
                write!(
                    f,
                    "account data is not properly aligned for the target type"
                )
            }
        }
    }
}

impl std::error::Error for AccountDataError {}

/// Cast raw account data (including the 8-byte Anchor discriminator) to `&T`.
///
/// Verifies the discriminator matches `T::SPL_DISCRIMINATOR` before casting.
pub fn from_account_data<T: bytemuck::Pod + SplDiscriminate>(
    data: &[u8],
) -> Result<&T, AccountDataError> {
    let expected_len = DISCRIMINATOR_SIZE + core::mem::size_of::<T>();
    if data.len() < expected_len {
        return Err(AccountDataError::DataTooShort {
            expected: expected_len,
            actual: data.len(),
        });
    }
    let disc = &data[..DISCRIMINATOR_SIZE];
    if disc != T::SPL_DISCRIMINATOR_SLICE {
        let mut actual = [0u8; 8];
        actual.copy_from_slice(disc);
        let mut expected = [0u8; 8];
        expected.copy_from_slice(T::SPL_DISCRIMINATOR_SLICE);
        return Err(AccountDataError::InvalidDiscriminator { expected, actual });
    }
    bytemuck::try_from_bytes(&data[DISCRIMINATOR_SIZE..expected_len])
        .map_err(|_| AccountDataError::AlignmentError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verify_account_sizes() {
        assert_eq!(core::mem::size_of::<Obligation>(), 3336);
        assert_eq!(core::mem::size_of::<Reserve>(), 8616);
        assert_eq!(core::mem::size_of::<ReserveConfig>(), 952);
        assert_eq!(core::mem::size_of::<TokenInfo>(), 384);
        assert_eq!(core::mem::size_of::<LendingMarket>(), 4656);
        assert_eq!(core::mem::size_of::<GlobalConfig>(), 1024);
        assert_eq!(core::mem::size_of::<ReferrerTokenState>(), 352);
        assert_eq!(core::mem::size_of::<UserMetadata>(), 1024);
        assert_eq!(core::mem::size_of::<ReferrerState>(), 64);
        assert_eq!(core::mem::size_of::<WithdrawTicket>(), 512);
    }

    #[test]
    fn verify_account_discriminators() {
        // Anchor account discriminators use sha256("account:<Name>")[..8]
        // but compute_discriminator uses "global:<name>". We verify against
        // sha256 directly.
        use sha2::{Digest, Sha256};

        macro_rules! check {
            ($ty:ty, $name:expr) => {{
                let mut h = Sha256::new();
                h.update(concat!("account:", $name).as_bytes());
                let hash = h.finalize();
                let mut expected = [0u8; 8];
                expected.copy_from_slice(&hash[..8]);
                assert_eq!(
                    <$ty as SplDiscriminate>::SPL_DISCRIMINATOR_SLICE,
                    &expected,
                    concat!("Discriminator mismatch for ", $name),
                );
            }};
        }

        check!(Obligation, "Obligation");
        check!(Reserve, "Reserve");
        check!(LendingMarket, "LendingMarket");
        check!(GlobalConfig, "GlobalConfig");
        check!(ReferrerTokenState, "ReferrerTokenState");
        check!(UserMetadata, "UserMetadata");
        check!(ReferrerState, "ReferrerState");
        check!(WithdrawTicket, "WithdrawTicket");
    }
}
