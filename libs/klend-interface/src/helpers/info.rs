use solana_pubkey::Pubkey;

use crate::state::AccountDataError;

/// On-chain reserve data that cannot be derived from PDAs.
///
/// The caller reads these fields from the deserialized `Reserve` account.
/// Reserve PDAs (supply vault, fee vault, collateral mint/supply) are derived
/// automatically by the helpers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReserveInfo {
    /// Reserve account address.
    pub address: Pubkey,
    /// The lending market this reserve belongs to.
    pub lending_market: Pubkey,
    /// SPL token mint for the reserve's liquidity (e.g. USDC mint).
    pub liquidity_mint: Pubkey,
    /// Token program for the liquidity mint (`TOKEN_PROGRAM_ID` or Token-2022).
    pub liquidity_token_program: Pubkey,
    /// Pyth oracle, if configured for this reserve.
    pub pyth_oracle: Option<Pubkey>,
    /// Switchboard price oracle, if configured.
    pub switchboard_price_oracle: Option<Pubkey>,
    /// Switchboard TWAP oracle, if configured.
    pub switchboard_twap_oracle: Option<Pubkey>,
    /// Scope prices account, if configured.
    pub scope_prices: Option<Pubkey>,
}

/// Obligation metadata needed for building refresh and main instructions.
///
/// The caller reads these fields from the deserialized `Obligation` account.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObligationInfo {
    /// Obligation account address.
    pub address: Pubkey,
    /// Reserve pubkeys for each **active** deposit position, in order.
    pub deposit_reserves: Vec<Pubkey>,
    /// Reserve pubkeys for each **active** borrow position, in order.
    pub borrow_reserves: Vec<Pubkey>,
    /// Referrer wallet if the obligation has one (`obligation.referrer`).
    pub referrer: Option<Pubkey>,
}

/// Helper: returns `Some(key)` if it is not `Pubkey::default()`.
pub(super) fn non_default(key: Pubkey) -> Option<Pubkey> {
    if key == Pubkey::default() {
        None
    } else {
        Some(key)
    }
}

impl ReserveInfo {
    /// Build a `ReserveInfo` directly from raw on-chain account data bytes.
    ///
    /// This deserializes the `Reserve` account and extracts the fields needed
    /// by the helpers. Useful when working with raw RPC responses.
    pub fn from_account_data(
        address: Pubkey,
        data: &[u8],
    ) -> Result<Self, crate::state::AccountDataError> {
        let reserve = crate::state::from_account_data::<crate::state::Reserve>(data)?;
        Ok(Self::from_reserve(address, reserve))
    }

    /// Build a `ReserveInfo` from a deserialized [`crate::state::Reserve`] account.
    pub fn from_reserve(address: Pubkey, reserve: &crate::state::Reserve) -> Self {
        Self {
            address,
            lending_market: reserve.lending_market,
            liquidity_mint: reserve.liquidity.mint_pubkey,
            liquidity_token_program: reserve.liquidity.token_program,
            pyth_oracle: non_default(reserve.config.token_info.pyth_configuration.price),
            switchboard_price_oracle: non_default(
                reserve
                    .config
                    .token_info
                    .switchboard_configuration
                    .price_aggregator,
            ),
            switchboard_twap_oracle: non_default(
                reserve
                    .config
                    .token_info
                    .switchboard_configuration
                    .twap_aggregator,
            ),
            scope_prices: non_default(reserve.config.token_info.scope_configuration.price_feed),
        }
    }
}

impl ObligationInfo {
    /// Build an `ObligationInfo` directly from raw on-chain account data bytes.
    pub fn from_account_data(
        address: Pubkey,
        data: &[u8],
    ) -> Result<Self, crate::state::AccountDataError> {
        let obligation = crate::state::from_account_data::<crate::state::Obligation>(data)?;
        Ok(Self::from_obligation(address, obligation))
    }

    /// Build an `ObligationInfo` from a deserialized [`crate::state::Obligation`] account.
    pub fn from_obligation(address: Pubkey, obligation: &crate::state::Obligation) -> Self {
        let deposit_reserves = obligation
            .deposits
            .iter()
            .filter(|d| d.deposit_reserve != Pubkey::default())
            .map(|d| d.deposit_reserve)
            .collect();
        let borrow_reserves = obligation
            .borrows
            .iter()
            .filter(|b| b.borrow_reserve != Pubkey::default())
            .map(|b| b.borrow_reserve)
            .collect();
        Self {
            address,
            deposit_reserves,
            borrow_reserves,
            referrer: non_default(obligation.referrer),
        }
    }
}

impl From<(Pubkey, &crate::state::Reserve)> for ReserveInfo {
    fn from((address, reserve): (Pubkey, &crate::state::Reserve)) -> Self {
        Self::from_reserve(address, reserve)
    }
}

impl TryFrom<(Pubkey, &[u8])> for ReserveInfo {
    type Error = AccountDataError;
    fn try_from((address, data): (Pubkey, &[u8])) -> Result<Self, Self::Error> {
        Self::from_account_data(address, data)
    }
}

#[cfg(feature = "solana-account")]
impl TryFrom<(Pubkey, &solana_account::Account)> for ReserveInfo {
    type Error = AccountDataError;
    fn try_from(
        (address, account): (Pubkey, &solana_account::Account),
    ) -> Result<Self, Self::Error> {
        Self::from_account_data(address, &account.data)
    }
}

impl From<(Pubkey, &crate::state::Obligation)> for ObligationInfo {
    fn from((address, obligation): (Pubkey, &crate::state::Obligation)) -> Self {
        Self::from_obligation(address, obligation)
    }
}

impl TryFrom<(Pubkey, &[u8])> for ObligationInfo {
    type Error = AccountDataError;
    fn try_from((address, data): (Pubkey, &[u8])) -> Result<Self, Self::Error> {
        Self::from_account_data(address, data)
    }
}

#[cfg(feature = "solana-account")]
impl TryFrom<(Pubkey, &solana_account::Account)> for ObligationInfo {
    type Error = AccountDataError;
    fn try_from(
        (address, account): (Pubkey, &solana_account::Account),
    ) -> Result<Self, Self::Error> {
        Self::from_account_data(address, &account.data)
    }
}

/// Reserve info bundled with its farm state pubkeys (crate-internal).
#[derive(Clone, Debug)]
pub(crate) struct ReserveWithFarms {
    pub info: ReserveInfo,
    pub farm_collateral: Option<Pubkey>,
    pub farm_debt: Option<Pubkey>,
}

/// Error returned by [`ObligationContext`] methods.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObligationContextError {
    /// The specified reserve was not found among the reserves provided at construction time.
    ReserveNotFound(Pubkey),
}

impl core::fmt::Display for ObligationContextError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ReserveNotFound(pk) => write!(f, "reserve not found in context: {pk}"),
        }
    }
}

impl std::error::Error for ObligationContextError {}

/// A high-level context that bundles an obligation with all its reserves and
/// auto-resolves farm accounts, providing ergonomic one-call methods for every
/// obligation operation.
///
/// # Construction
///
/// ```no_run
/// use klend_interface::helpers::ObligationContext;
/// # use solana_pubkey::Pubkey;
/// # let obligation_address = Pubkey::default();
/// # let obligation = unsafe { std::mem::zeroed::<klend_interface::state::Obligation>() };
/// # let reserve_addr = Pubkey::default();
/// # let reserve = unsafe { std::mem::zeroed::<klend_interface::state::Reserve>() };
///
/// let ctx = ObligationContext::new(
///     obligation_address,
///     &obligation,
///     &[(reserve_addr, &reserve)],
/// );
/// ```
#[derive(Clone, Debug)]
pub struct ObligationContext {
    pub(crate) obligation: ObligationInfo,
    pub(crate) lending_market: Pubkey,
    pub(crate) reserves: Vec<ReserveWithFarms>,
}

impl ObligationContext {
    /// Build a context from deserialized on-chain accounts.
    pub fn new(
        obligation_address: Pubkey,
        obligation: &crate::state::Obligation,
        reserves: &[(Pubkey, &crate::state::Reserve)],
    ) -> Self {
        let obligation_info = ObligationInfo::from_obligation(obligation_address, obligation);
        let lending_market = obligation.lending_market;
        let reserves = reserves
            .iter()
            .map(|(addr, reserve)| ReserveWithFarms {
                info: ReserveInfo::from_reserve(*addr, reserve),
                farm_collateral: non_default(reserve.farm_collateral),
                farm_debt: non_default(reserve.farm_debt),
            })
            .collect();

        Self {
            obligation: obligation_info,
            lending_market,
            reserves,
        }
    }

    /// Parse an obligation and return the unique reserve addresses that must be
    /// fetched to build a complete context.
    ///
    /// Typical RPC flow:
    /// 1. Fetch the obligation account
    /// 2. Call `reserve_addresses_for_obligation` to discover which reserves are needed
    /// 3. Fetch those reserve accounts (e.g. `getMultipleAccounts`)
    /// 4. Call [`ObligationContext::from_account_data`] with both
    pub fn reserve_addresses_for_obligation(
        obligation_data: &[u8],
    ) -> Result<Vec<Pubkey>, crate::state::AccountDataError> {
        let obligation =
            crate::state::from_account_data::<crate::state::Obligation>(obligation_data)?;
        let mut addrs: Vec<Pubkey> = obligation
            .deposits
            .iter()
            .filter(|d| d.deposit_reserve != Pubkey::default())
            .map(|d| d.deposit_reserve)
            .chain(
                obligation
                    .borrows
                    .iter()
                    .filter(|b| b.borrow_reserve != Pubkey::default())
                    .map(|b| b.borrow_reserve),
            )
            .collect();
        addrs.sort_unstable();
        addrs.dedup();
        Ok(addrs)
    }

    /// Build a context from raw on-chain account data bytes.
    ///
    /// `reserves` must include an entry for every reserve referenced by the
    /// obligation (see [`ObligationContext::reserve_addresses_for_obligation`]).
    /// Extra reserves are allowed and will be available for lookups.
    pub fn from_account_data(
        obligation_address: Pubkey,
        obligation_data: &[u8],
        reserves: &[(Pubkey, &[u8])],
    ) -> Result<Self, crate::state::AccountDataError> {
        let obligation =
            crate::state::from_account_data::<crate::state::Obligation>(obligation_data)?;
        let parsed_reserves: Result<Vec<_>, _> = reserves
            .iter()
            .map(|(addr, data)| {
                let r = crate::state::from_account_data::<crate::state::Reserve>(data)?;
                Ok((*addr, r))
            })
            .collect();
        let parsed_reserves = parsed_reserves?;
        let reserve_pairs: Vec<(Pubkey, &crate::state::Reserve)> =
            parsed_reserves.iter().map(|(a, r)| (*a, *r)).collect();
        Ok(Self::new(obligation_address, obligation, &reserve_pairs))
    }

    /// Build a context from pre-built [`ReserveInfo`] values (no farm auto-resolution).
    ///
    /// Use this when you already have `ReserveInfo` and `ObligationInfo` constructed
    /// from another source, or when farm accounts are not needed.
    pub fn from_infos(
        lending_market: Pubkey,
        obligation: ObligationInfo,
        reserves: &[ReserveInfo],
    ) -> Self {
        Self {
            obligation,
            lending_market,
            reserves: reserves
                .iter()
                .map(|info| ReserveWithFarms {
                    info: info.clone(),
                    farm_collateral: None,
                    farm_debt: None,
                })
                .collect(),
        }
    }

    /// Get the obligation info.
    pub fn obligation(&self) -> &ObligationInfo {
        &self.obligation
    }

    /// Look up a reserve by address.
    pub fn reserve_info(&self, address: &Pubkey) -> Option<&ReserveInfo> {
        self.reserves
            .iter()
            .find(|r| r.info.address == *address)
            .map(|r| &r.info)
    }

    pub(crate) fn find_reserve(&self, address: &Pubkey) -> Option<&ReserveWithFarms> {
        self.reserves.iter().find(|r| r.info.address == *address)
    }

    pub(crate) fn all_reserve_infos(&self) -> Vec<ReserveInfo> {
        self.reserves.iter().map(|r| r.info.clone()).collect()
    }

    pub(crate) fn collateral_farms(&self, reserve_address: &Pubkey) -> Option<FarmsAccounts> {
        let r = self.find_reserve(reserve_address)?;
        let farm_state = r.farm_collateral?;
        Some(FarmsAccounts {
            reserve_farm_state: farm_state,
            obligation_farm_user_state: crate::pda::farms_user_state(
                &farm_state,
                &self.obligation.address,
            )
            .0,
        })
    }

    pub(crate) fn debt_farms(&self, reserve_address: &Pubkey) -> Option<FarmsAccounts> {
        let r = self.find_reserve(reserve_address)?;
        let farm_state = r.farm_debt?;
        Some(FarmsAccounts {
            reserve_farm_state: farm_state,
            obligation_farm_user_state: crate::pda::farms_user_state(
                &farm_state,
                &self.obligation.address,
            )
            .0,
        })
    }
}

/// Farm state accounts for a single reserve+obligation pair.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FarmsAccounts {
    pub obligation_farm_user_state: Pubkey,
    pub reserve_farm_state: Pubkey,
}

/// Optional progress-callback accounts for `enqueue_to_withdraw` and
/// `withdraw_queued_liquidity`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallbackAccounts {
    pub progress_callback_type: crate::types::ProgressCallbackType,
    pub custom_account_0: Option<Pubkey>,
    pub custom_account_1: Option<Pubkey>,
}

impl CallbackAccounts {
    /// Extract callback configuration from a deserialized [`crate::state::WithdrawTicket`].
    ///
    /// Returns `None` if the ticket has no callback (`ProgressCallbackType::None`).
    pub fn from_withdraw_ticket(ticket: &crate::state::WithdrawTicket) -> Option<Self> {
        let cb_type = match ticket.progress_callback_type {
            0 => return None,
            1 => crate::types::ProgressCallbackType::KlendQueueAccountingHandlerOnKvault,
            _ => return None,
        };
        Some(Self {
            progress_callback_type: cb_type,
            custom_account_0: non_default(ticket.progress_callback_custom_accounts[0]),
            custom_account_1: non_default(ticket.progress_callback_custom_accounts[1]),
        })
    }

    /// Extract callback configuration from raw withdraw ticket account data bytes.
    ///
    /// Returns `Ok(None)` if the ticket has no callback.
    pub fn from_withdraw_ticket_data(
        data: &[u8],
    ) -> Result<Option<Self>, crate::state::AccountDataError> {
        let ticket = crate::state::from_account_data::<crate::state::WithdrawTicket>(data)?;
        Ok(Self::from_withdraw_ticket(ticket))
    }

    /// Extract callback configuration from a [`solana_account::Account`].
    ///
    /// Returns `Ok(None)` if the ticket has no callback.
    #[cfg(feature = "solana-account")]
    pub fn from_withdraw_ticket_account(
        account: &solana_account::Account,
    ) -> Result<Option<Self>, crate::state::AccountDataError> {
        Self::from_withdraw_ticket_data(&account.data)
    }
}
