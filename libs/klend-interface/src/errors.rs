//! Klend program error codes.
//!
//! Anchor custom errors start at offset 6000. Each variant's error code is `6000 + variant index`.
//! Use [`LendingError::from_error_code`] to convert an on-chain error code back to a variant,
//! or `TryFrom<u32>` for the same purpose.
//!
//! # Example
//!
//! ```rust
//! use klend_interface::LendingError;
//!
//! // Decode an error code from a failed transaction
//! let error = LendingError::from_error_code(6008);
//! assert_eq!(error, Some(LendingError::InsufficientLiquidity));
//! assert_eq!(error.unwrap().error_code(), 6008);
//! assert_eq!(
//!     error.unwrap().to_string(),
//!     "Insufficient liquidity available"
//! );
//! ```

/// Anchor error-code base for custom program errors.
const ANCHOR_ERROR_BASE: u32 = 6000;

macro_rules! define_lending_errors {
    (
        $(
            $(#[doc = $doc:expr])*
            $variant:ident = $offset:literal => $msg:expr
        ),*
        $(,)?
    ) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        #[repr(u32)]
        pub enum LendingError {
            $(
                $(#[doc = $doc])*
                $variant = ANCHOR_ERROR_BASE + $offset,
            )*
        }

        impl LendingError {
            /// Returns the Anchor error code for this variant (`6000 + offset`).
            pub const fn error_code(self) -> u32 {
                self as u32
            }

            /// Returns the human-readable error message.
            pub const fn message(self) -> &'static str {
                match self {
                    $(Self::$variant => $msg,)*
                }
            }

            /// Converts an Anchor error code to a `LendingError`, if it matches a known variant.
            pub const fn from_error_code(code: u32) -> Option<Self> {
                match code {
                    $(x if x == ANCHOR_ERROR_BASE + $offset => Some(Self::$variant),)*
                    _ => None,
                }
            }
        }

        impl core::fmt::Display for LendingError {
            fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, "{}", self.message())
            }
        }

        impl TryFrom<u32> for LendingError {
            type Error = u32;

            /// Converts an error code to a `LendingError`.
            /// Returns `Err(code)` if the code doesn't match any known variant.
            fn try_from(code: u32) -> Result<Self, Self::Error> {
                Self::from_error_code(code).ok_or(code)
            }
        }
    };
}

define_lending_errors! {
    InvalidMarketAuthority = 0 => "Market authority is invalid",
    InvalidMarketOwner = 1 => "Market owner is invalid",
    InvalidAccountOwner = 2 => "Input account owner is not the program address",
    InvalidAmount = 3 => "Input amount is invalid",
    InvalidConfig = 4 => "Input config value is invalid",
    InvalidSigner = 5 => "Signer is not allowed to perform this action",
    InvalidAccountInput = 6 => "Invalid account input",
    MathOverflow = 7 => "Math operation overflow",
    InsufficientLiquidity = 8 => "Insufficient liquidity available",
    ReserveStale = 9 => "Reserve state needs to be refreshed",
    WithdrawTooSmall = 10 => "Withdraw amount too small",
    WithdrawTooLarge = 11 => "Withdraw amount too large",
    BorrowTooSmall = 12 => "Borrow amount too small to receive liquidity after fees",
    BorrowTooLarge = 13 => "Borrow amount too large for deposited collateral",
    RepayTooSmall = 14 => "Repay amount too small to transfer liquidity",
    LiquidationTooSmall = 15 => "Liquidation amount too small to receive collateral",
    ObligationHealthy = 16 => "Cannot liquidate healthy obligations",
    ObligationStale = 17 => "Obligation state needs to be refreshed",
    ObligationReserveLimit = 18 => "Obligation reserve limit exceeded",
    InvalidObligationOwner = 19 => "Obligation owner is invalid",
    ObligationDepositsEmpty = 20 => "Obligation deposits are empty",
    ObligationBorrowsEmpty = 21 => "Obligation borrows are empty",
    ObligationDepositsZero = 22 => "Obligation deposits have zero value",
    ObligationBorrowsZero = 23 => "Obligation borrows have zero value",
    InvalidObligationCollateral = 24 => "Invalid obligation collateral",
    InvalidObligationLiquidity = 25 => "Invalid obligation liquidity",
    ObligationCollateralEmpty = 26 => "Obligation collateral is empty",
    ObligationLiquidityEmpty = 27 => "Obligation liquidity is empty",
    NegativeInterestRate = 28 => "Interest rate is negative",
    InvalidOracleConfig = 29 => "Input oracle config is invalid",
    InsufficientProtocolFeesToRedeem = 30 => "Insufficient protocol fees to claim or no liquidity available",
    FlashBorrowCpi = 31 => "No cpi flash borrows allowed",
    NoFlashRepayFound = 32 => "No corresponding repay found for flash borrow",
    InvalidFlashRepay = 33 => "Invalid repay found",
    FlashRepayCpi = 34 => "No cpi flash repays allowed",
    MultipleFlashBorrows = 35 => "Multiple flash borrows not allowed in the same transaction",
    FlashLoansDisabled = 36 => "Flash loans are disabled for this reserve",
    SwitchboardV2Error = 37 => "Switchboard error",
    CouldNotDeserializeScope = 38 => "Cannot deserialize the scope price account",
    PriceTooOld = 39 => "Price too old",
    PriceTooDivergentFromTwap = 40 => "Price too divergent from twap",
    InvalidTwapPrice = 41 => "Invalid twap price",
    GlobalEmergencyMode = 42 => "Emergency mode is enabled",
    InvalidFlag = 43 => "Invalid lending market config",
    PriceNotValid = 44 => "Price is not valid",
    PriceIsBiggerThanHeuristic = 45 => "Price is bigger than allowed by heuristic",
    PriceIsLowerThanHeuristic = 46 => "Price lower than allowed by heuristic",
    PriceIsZero = 47 => "Price is zero",
    PriceConfidenceTooWide = 48 => "Price confidence too wide",
    IntegerOverflow = 49 => "Conversion between integers failed",
    NoFarmForReserve = 50 => "This reserve does not have a farm",
    IncorrectInstructionInPosition = 51 => "Wrong instruction at expected position",
    NoPriceFound = 52 => "No price found",
    InvalidTwapConfig = 53 => "Invalid Twap configuration: Twap is enabled but one of the enabled price doesn't have a twap",
    InvalidPythPriceAccount = 54 => "Pyth price account does not match configuration",
    InvalidSwitchboardAccount = 55 => "Switchboard account(s) do not match configuration",
    InvalidScopePriceAccount = 56 => "Scope price account does not match configuration",
    ObligationCollateralLtvZero = 57 => "The obligation has one collateral with an LTV set to 0. Withdraw it before withdrawing other collaterals",
    InvalidObligationSeedsValue = 58 => "Seeds must be default pubkeys for tag 0, and mint addresses for tag 1 or 2",
    DeprecatedInvalidObligationId = 59 => "[DEPRECATED] Obligation id must be 0",
    InvalidBorrowRateCurvePoint = 60 => "Invalid borrow rate curve point",
    InvalidUtilizationRate = 61 => "Invalid utilization rate",
    CannotSocializeObligationWithCollateral = 62 => "Obligation hasn't been fully liquidated and debt cannot be socialized.",
    ObligationEmpty = 63 => "Obligation has no borrows or deposits.",
    WithdrawalCapReached = 64 => "Withdrawal cap is reached",
    LastTimestampGreaterThanCurrent = 65 => "The last interval start timestamp is greater than the current timestamp",
    LiquidationRewardTooSmall = 66 => "The reward amount is less than the minimum acceptable received liquidity",
    IsolatedAssetTierViolation = 67 => "Isolated Asset Tier Violation",
    InconsistentElevationGroup = 68 => "The obligation's elevation group and the reserve's are not the same",
    InvalidElevationGroup = 69 => "The elevation group chosen for the reserve does not exist in the lending market",
    InvalidElevationGroupConfig = 70 => "The elevation group updated has wrong parameters set",
    UnhealthyElevationGroupLtv = 71 => "The current obligation must have most or all its debt repaid before changing the elevation group",
    ElevationGroupNewLoansDisabled = 72 => "Elevation group does not accept any new loans or any new borrows/withdrawals",
    ReserveDeprecated = 73 => "Reserve was deprecated, no longer usable",
    ReferrerAccountNotInitialized = 74 => "Referrer account not initialized",
    ReferrerAccountMintMissmatch = 75 => "Referrer account mint does not match the operation reserve mint",
    ReferrerAccountWrongAddress = 76 => "Referrer account address is not a valid program address",
    ReferrerAccountReferrerMissmatch = 77 => "Referrer account referrer does not match the owner referrer",
    ReferrerAccountMissing = 78 => "Referrer account missing for obligation with referrer",
    InsufficientReferralFeesToRedeem = 79 => "Insufficient referral fees to claim or no liquidity available",
    CpiDisabled = 80 => "CPI disabled for this instruction",
    ShortUrlNotAsciiAlphanumeric = 81 => "Referrer short_url is not ascii alphanumeric",
    ReserveObsolete = 82 => "Reserve is marked as obsolete",
    ElevationGroupAlreadyActivated = 83 => "Obligation already part of the same elevation group",
    ObligationInObsoleteReserve = 84 => "Obligation has a deposit or borrow in an obsolete reserve",
    ReferrerStateOwnerMismatch = 85 => "Referrer state owner does not match the given signer",
    UserMetadataOwnerAlreadySet = 86 => "User metadata owner is already set",
    CollateralNonLiquidatable = 87 => "This collateral cannot be liquidated (LTV set to 0)",
    BorrowingDisabled = 88 => "Borrowing is disabled",
    BorrowLimitExceeded = 89 => "Cannot borrow above borrow limit",
    DepositLimitExceeded = 90 => "Cannot deposit above deposit limit",
    BorrowingDisabledOutsideElevationGroup = 91 => "Reserve does not accept any new borrows outside elevation group",
    NetValueRemainingTooSmall = 92 => "Net value remaining too small",
    WorseLtvBlocked = 93 => "Cannot get the obligation in a worse position",
    LiabilitiesBiggerThanAssets = 94 => "Cannot have more liabilities than assets in a position",
    ReserveTokenBalanceMismatch = 95 => "Reserve state and token account cannot drift",
    ReserveVaultBalanceMismatch = 96 => "Reserve token account has been unexpectedly modified",
    ReserveAccountingMismatch = 97 => "Reserve internal state accounting has been unexpectedly modified",
    BorrowingAboveUtilizationRateDisabled = 98 => "Borrowing above set utilization rate is disabled",
    LiquidationBorrowFactorPriority = 99 => "Liquidation must prioritize the debt with the highest borrow factor",
    LiquidationLowestLiquidationLtvPriority = 100 => "Liquidation must prioritize the collateral with the lowest liquidation LTV",
    ElevationGroupBorrowLimitExceeded = 101 => "Elevation group borrow limit exceeded",
    ElevationGroupWithoutDebtReserve = 102 => "The elevation group does not have a debt reserve defined",
    ElevationGroupMaxCollateralReserveZero = 103 => "The elevation group does not allow any collateral reserves",
    ElevationGroupHasAnotherDebtReserve = 104 => "In elevation group attempt to borrow from a reserve that is not the debt reserve",
    ElevationGroupDebtReserveAsCollateral = 105 => "The elevation group's debt reserve cannot be used as a collateral reserve",
    ObligationCollateralExceedsElevationGroupLimit = 106 => "Obligation have more collateral than the maximum allowed by the elevation group",
    ObligationElevationGroupMultipleDebtReserve = 107 => "Obligation is an elevation group but have more than one debt reserve",
    UnsupportedTokenExtension = 108 => "Mint has a token (2022) extension that is not supported",
    InvalidTokenAccount = 109 => "Can't have an spl token mint with a t22 account",
    DepositDisabledOutsideElevationGroup = 110 => "Can't deposit into this reserve outside elevation group",
    CannotCalculateReferralAmountDueToSlotsMismatch = 111 => "Cannot calculate referral amount due to slots mismatch",
    ObligationOwnersMustMatch = 112 => "Obligation owners must match",
    ObligationsMustMatch = 113 => "Obligations must match",
    LendingMarketsMustMatch = 114 => "Lending markets must match",
    ObligationCurrentlyMarkedForDeleveraging = 115 => "Obligation is already marked for deleveraging",
    MaximumWithdrawValueZero = 116 => "Maximum withdrawable value of this collateral is zero, LTV needs improved",
    ZeroMaxLtvAssetsInDeposits = 117 => "No max LTV 0 assets allowed in deposits for repay and withdraw",
    LowestLtvAssetsPriority = 118 => "Withdrawing must prioritize the collateral with the lowest reserve max-LTV",
    WorseLtvThanUnhealthyLtv = 119 => "Cannot get the obligation liquidatable",
    FarmAccountsMissing = 120 => "Farm accounts to refresh are missing",
    RepayTooSmallForFullLiquidation = 121 => "Repay amount is too small to satisfy the mandatory full liquidation",
    InsufficientRepayAmount = 122 => "Liquidator provided repay amount lower than required by liquidation rules",
    OrderIndexOutOfBounds = 123 => "Obligation order of the given index cannot exist",
    InvalidOrderConfiguration = 124 => "Given order configuration has wrong parameters",
    OrderConfigurationNotSupportedByObligation = 125 => "Given order configuration cannot be used with the current state of the obligation",
    OperationNotPermittedWithCurrentObligationOrders = 126 => "Single debt, single collateral obligation orders have to be cancelled before changing the deposit/borrow count",
    OperationNotPermittedMarketImmutable = 127 => "Cannot update lending market because it is set as immutable",
    OrderCreationDisabled = 128 => "Creation of new orders is disabled",
    NoUpgradeAuthority = 129 => "Cannot initialize global config because there is no upgrade authority to the program",
    InitialAdminDepositExecuted = 130 => "Initial admin deposit in reserve already executed",
    ReserveHasNotReceivedInitialDeposit = 131 => "Reserve has not received the initial deposit, cannot update config",
    CTokenUsageBlocked = 132 => "CToken minting/redeeming is blocked for this reserve",
    CannotUseSameReserve = 133 => "Cannot call ix with same reserve",
    TransactionIncludesRestrictedPrograms = 134 => "Transaction includes restricted programs",
    BorrowOrderDebtLiquidityMintMismatch = 135 => "There is no borrow order requesting debt in the given asset",
    BorrowOrderMaxBorrowRateExceeded = 136 => "Reserve used for fill exceeds the maximum borrow rate specified by the order",
    BorrowOrderMinDebtTermInsufficient = 137 => "Reserve used for fill defines a debt term shorter than specified by the order",
    BorrowOrderFillTimeLimitExceeded = 138 => "Borrow order can no longer be filled",
    ReserveDebtMaturityReached = 139 => "Cannot borrow from a reserve that reached its debt maturity timestamp",
    NonUpdatableOrderConfiguration = 140 => "Some piece of the order's configuration cannot be updated (the order should be cancelled and placed again)",
    BorrowOrderExecutionDisabled = 141 => "Execution of borrow orders is disabled",
    DebtReachedReserveDebtTerm = 142 => "Cannot increase the debt that has reached its end of term configured by the reserve",
    ExpectationNotMet = 143 => "The on-chain state does not meet expectation specified by the caller, so the operation must be aborted (to avoid race conditions)",
    BorrowOrderFillValueTooSmall = 144 => "Available liquidity could not satisfy the minimum required borrow order fill value",
    WithdrawTicketIssuanceDisabled = 145 => "Issuing new withdraw tickets is disabled by the market",
    WithdrawTicketRedemptionDisabled = 146 => "Redeeming withdraw tickets is disabled by the market",
    WithdrawTicketStillValid = 147 => "Recovering collateral is only available after the withdraw ticket has been marked invalid",
    WithdrawTicketRequiresFullRedemption = 148 => "The withdraw ticket's current state requires that it is fully redeemed (e.g. due to owner ATA creation), but there is not enough liquidity",
    UserTokenBalanceMismatch = 149 => "The user's token account has changed its balance in an unexpected way",
    WithdrawQueuedLiquidityValueTooSmall = 150 => "Available liquidity could not satisfy the minimum required ticketed withdrawal value",
    InvalidTokenAccountState = 151 => "Token account is in a state preventing the handler's operation (e.g. frozen or delegate)",
    WithdrawTicketInvalid = 152 => "Cannot use ticket that was already marked invalid",
    BorrowOrderValueTooSmall = 153 => "Borrow order's value would be below the market-configured minimum",
    WithdrawTicketValueTooSmall = 154 => "Withdraw ticket's value would be below the market-configured minimum",
    InvalidWithdrawTicketProgressCallbackConfig = 155 => "Invalid configuration or required custom accounts for the requested withdraw ticket callback type",
    WithdrawTicketProgressCallbackAccountsMissing = 156 => "One or more accounts required by the ticket's configured progress callback are missing",
    BorrowRolloverConfigurationDisabled = 157 => "Configuring auto-rollover on loans is disabled by market owner",
    InvalidObligationConfigUpdateSubject = 158 => "Invalid specification of the Obligation's part to be configured",
    BorrowRolloverLiquidityMintMismatch = 159 => "Auto-rollover must use a target reserve of the same token",
    ObligationBorrowRolloverNotApplicable = 160 => "The given borrow is not fixed-term and does not require rolling over",
    ObligationBorrowOutsideRolloverWindow = 161 => "The given borrow is outside the corresponding market-configured rollover window",
    ObligationBorrowRolloverNotEnabledByOwner = 162 => "Obligation's owner did not opt-in for auto-rollover of the given borrow",
    ObligationBorrowRolloverTargetReserveMismatch = 163 => "Obligation's owner did not allow to roll over into terms offered by the given reserve",
    BorrowRolloverExecutionDisabled = 164 => "Executing auto-rollover is disabled by market owner",
    ObligationAccountingMismatch = 165 => "Obligation internal state accounting has been unexpectedly modified",
    PartialRolloverValueTooSmall = 166 => "Partial rollover amount is below the market-configured minimum value",
    ObligationBorrowRolloverConfigMismatch = 167 => "Pre-existing rollover configuration of the loan cannot be overwritten by the operation",
    ObligationBorrowRolloverMustProlongDebtTerm = 168 => "Rollover into existing borrow must prolong the remaining debt term",
    RolloverNotSupportedInElevationGroup = 169 => "Rollover is not supported for obligations in an elevation group",
    WithdrawTicketCancellationDisabled = 170 => "Cancelling withdraw tickets is disabled by the market",
    WithdrawTicketFullyCancelled = 171 => "Cannot use ticket that was already fully-cancelled",
    CloneSourceReserveDisabled = 172 => "Cannot clone config from a reserve that is disabled",
    CloneTargetReserveAlreadyInUse = 173 => "Cannot clone config into a reserve that has been in use",
    ClonedReserveLiquidityMintMismatch = 174 => "Cannot clone config between reserves of different mints",
    ReserveEmergencyMode = 175 => "Reserve emergency mode is enabled",
    ObligationOwnershipTransferInProgress = 176 => "Obligation ownership transfer is in progress",
    ObligationOwnershipTransferNotInInitiatedState = 177 => "Obligation ownership transfer is not in initiated state",
    ObligationPendingOwnerNotSet = 178 => "Obligation pending owner not set",
    ObligationInvalidPendingOwner = 179 => "Invalid pending owner address",
    ObligationOwnershipTransferNotApproved = 180 => "Obligation ownership transfer not approved by admin",
    ObligationHasActiveBorrowOrders = 181 => "Obligation has active borrow orders",
    OnlyComputeBudgetCompanionIxsAllowed = 182 => "Only ComputeBudget instructions may accompany this instruction",
    MissingPermissioner = 183 => "Required permissioning account is missing",
    ReserveRewardsDisabled = 184 => "Reserve rewards are disabled on this market (reserve_rewards_max_apr_bps is 0)",
}

impl std::error::Error for LendingError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_codes() {
        assert_eq!(LendingError::InvalidMarketAuthority.error_code(), 6000);
        assert_eq!(LendingError::InsufficientLiquidity.error_code(), 6008);
        assert_eq!(
            LendingError::ClonedReserveLiquidityMintMismatch.error_code(),
            6174
        );
        assert_eq!(LendingError::MissingPermissioner.error_code(), 6183);
        assert_eq!(
            LendingError::OnlyComputeBudgetCompanionIxsAllowed.error_code(),
            6182
        );
    }

    #[test]
    fn test_from_error_code() {
        assert_eq!(
            LendingError::from_error_code(6000),
            Some(LendingError::InvalidMarketAuthority)
        );
        assert_eq!(
            LendingError::from_error_code(6008),
            Some(LendingError::InsufficientLiquidity)
        );
        assert_eq!(LendingError::from_error_code(5999), None);
        assert_eq!(
            LendingError::from_error_code(6175),
            Some(LendingError::ReserveEmergencyMode)
        );
        assert_eq!(
            LendingError::from_error_code(6182),
            Some(LendingError::OnlyComputeBudgetCompanionIxsAllowed)
        );
        assert_eq!(
            LendingError::from_error_code(6183),
            Some(LendingError::MissingPermissioner)
        );
        assert_eq!(
            LendingError::from_error_code(6184),
            Some(LendingError::ReserveRewardsDisabled)
        );
        assert_eq!(LendingError::from_error_code(6185), None);
    }

    #[test]
    fn test_try_from() {
        assert_eq!(
            LendingError::try_from(6042),
            Ok(LendingError::GlobalEmergencyMode)
        );
        assert_eq!(LendingError::try_from(9999), Err(9999));
    }

    #[test]
    fn test_display() {
        assert_eq!(
            LendingError::MathOverflow.to_string(),
            "Math operation overflow"
        );
    }

    /// Verify variant count matches the on-chain program (183 variants, codes 6000..=6182).
    #[test]
    fn test_all_codes_roundtrip() {
        let mut count = 0u32;
        for code in 6000..=6183 {
            let err = LendingError::from_error_code(code)
                .unwrap_or_else(|| panic!("Missing variant for code {code}"));
            assert_eq!(err.error_code(), code);
            count += 1;
        }
        assert_eq!(count, 184);
    }
}
