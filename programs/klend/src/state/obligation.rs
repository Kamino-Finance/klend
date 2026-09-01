use std::{
    cmp::Ordering,
    fmt::{self, Display, Formatter},
    iter,
    ops::RangeInclusive,
};

use anchor_lang::{account, err, prelude::*, Result};
use borsh::{BorshDeserialize, BorshSerialize};
use derivative::Derivative;
use num_enum::{IntoPrimitive, TryFromPrimitive};
#[cfg(feature = "serde")]
use strum::EnumIter;
use strum::EnumString;

use crate::{
    obligation_order_operations::{
        ConditionType, OpportunityType, OrderCondition, OrderOpportunity, OrderSize,
    },
    state::{LastUpdate, LtvMaxWithdrawalCheck, Reserve},
    utils::{
        accounts::default_array, BigFraction, Fraction, FractionExtra, ELEVATION_GROUP_NONE,
        OBLIGATION_SIZE, SECONDS_PER_DAY, U256,
    },
    xmsg, BigFractionBytes, LendingError, ReserveConfig,
};


#[derive(Clone, Copy, Debug, PartialEq, Eq, IntoPrimitive, TryFromPrimitive)]
#[repr(u8)]
pub enum OwnershipTransferState {

    None = 0,

    Initiated = 1,

    Approved = 2,
}



pub const MAX_TAIL_BORROW_ORDERS: usize = 2;


pub const MAX_BORROW_ORDERS: usize = MAX_TAIL_BORROW_ORDERS + 1;

static_assertions::const_assert_eq!(OBLIGATION_SIZE, std::mem::size_of::<Obligation>());
static_assertions::const_assert_eq!(0, std::mem::size_of::<Obligation>() % 8);

#[derive(PartialEq, Derivative)]
#[derivative(Debug)]
#[account(zero_copy)]
#[repr(C)]
pub struct Obligation {

    pub tag: u64,

    pub last_update: LastUpdate,

    pub lending_market: Pubkey,

    pub owner: Pubkey,

    pub deposits: [ObligationCollateral; 8],

    pub lowest_reserve_deposit_liquidation_ltv: u64,

    pub deposited_value_sf: u128,


    pub borrows: [ObligationLiquidity; 5],

    pub borrow_factor_adjusted_debt_value_sf: u128,

    pub borrowed_assets_market_value_sf: u128,

    pub allowed_borrow_value_sf: u128,

    pub unhealthy_borrow_value_sf: u128,


    pub padding_deprecated_asset_tiers: [u8; 13],


    pub elevation_group: u8,


    pub num_of_obsolete_deposit_reserves: u8,


    pub has_debt: u8,


    pub referrer: Pubkey,


    pub borrowing_disabled: u8,



    pub autodeleverage_target_ltv_pct: u8,


    pub lowest_reserve_deposit_max_ltv_pct: u8,


    pub num_of_obsolete_borrow_reserves: u8,


    pub ownership_transfer_state: u8,

    #[derivative(Debug = "ignore")]
    pub reserved: [u8; 3],

    pub highest_borrow_factor_pct: u64,



    pub autodeleverage_margin_call_started_timestamp: u64,



    pub obligation_orders: [ObligationOrder; 2],












    pub head_borrow_order: BorrowOrder,



    pub pending_owner: Pubkey,


    pub tail_borrow_orders: [BorrowOrder; MAX_TAIL_BORROW_ORDERS],

    #[derivative(Debug = "ignore")]
    pub padding_3: [u64; 29],
}

impl Default for Obligation {
    fn default() -> Self {
        Self {
            tag: 0,
            last_update: LastUpdate::default(),
            lending_market: Pubkey::default(),
            owner: Pubkey::default(),
            deposits: default_array(),
            borrows: default_array(),
            deposited_value_sf: 0,
            borrowed_assets_market_value_sf: 0,
            allowed_borrow_value_sf: 0,
            unhealthy_borrow_value_sf: 0,
            padding_deprecated_asset_tiers: default_array(),
            lowest_reserve_deposit_liquidation_ltv: 0,
            borrow_factor_adjusted_debt_value_sf: 0,
            elevation_group: ELEVATION_GROUP_NONE,
            num_of_obsolete_deposit_reserves: 0,
            num_of_obsolete_borrow_reserves: 0,
            has_debt: 0,
            borrowing_disabled: 0,
            highest_borrow_factor_pct: 0,
            lowest_reserve_deposit_max_ltv_pct: 0,
            reserved: default_array(),
            padding_3: default_array(),
            referrer: Pubkey::default(),
            autodeleverage_target_ltv_pct: 0,
            autodeleverage_margin_call_started_timestamp: 0,
            obligation_orders: default_array(),
            head_borrow_order: Default::default(),
            pending_owner: Pubkey::default(),
            tail_borrow_orders: default_array(),
            ownership_transfer_state: OwnershipTransferState::None.into(),
        }
    }
}

impl Display for Obligation {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Obligation summary, collateral value ${}, liquidity risk adjusted value ${}, liquidity risk unadjusted value ${} ltv {}%",
            Fraction::from_bits(self.deposited_value_sf).to_display(),
            Fraction::from_bits(self.borrow_factor_adjusted_debt_value_sf).to_display(),
            Fraction::from_bits(self.borrowed_assets_market_value_sf).to_display(),
            if self.deposited_value_sf > 0 {self.loan_to_value().to_percent().unwrap_or(u16::MAX)} else { 0 },
        )?;

        for collateral in self.active_deposits() {
            write!(
                f,
                "\n  Collateral reserve: {}, value: ${}, lamports: {}",
                collateral.deposit_reserve,
                Fraction::from_bits(collateral.market_value_sf).to_display(),
                collateral.deposited_amount,
            )?;
        }

        for liquidity in self.active_borrows() {
            write!(
                f,
                "\n  Borrowed reserve  : {}, value: ${}, lamports: {}",
                liquidity.borrow_reserve,
                liquidity.market_value().to_display(),
                liquidity
                    .borrowed_amount()
                    .checked_to_num()
                    .unwrap_or(u128::MAX),
            )?;
        }

        Ok(())
    }
}


#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub enum WithdrawResult {

    Full,

    Partial,
}

impl Obligation {
    pub const LEN: usize = 1784;


    pub fn init(&mut self, params: InitObligationParams) {
        *self = Self::default();
        self.tag = params.tag;
        self.last_update = LastUpdate::new(&params.clock);
        self.lending_market = params.lending_market;
        self.owner = params.owner;
        self.deposits = params.deposits;
        self.borrows = params.borrows;
        self.referrer = params.referrer;
    }


    pub fn loan_to_value(&self) -> Fraction {
        Fraction::from_bits(self.borrow_factor_adjusted_debt_value_sf)
            / Fraction::from_bits(self.deposited_value_sf)
    }

    pub fn deposited_value(&self) -> Fraction {
        Fraction::from_bits(self.deposited_value_sf)
    }

    pub fn no_bf_loan_to_value(&self) -> Fraction {
        Fraction::from_bits(self.borrowed_assets_market_value_sf)
            / Fraction::from_bits(self.deposited_value_sf)
    }


    pub fn unhealthy_loan_to_value(&self) -> Fraction {
        Fraction::from_bits(self.unhealthy_borrow_value_sf)
            / Fraction::from_bits(self.deposited_value_sf)
    }


    pub fn repay(&mut self, settle_amount: Fraction, liquidity_index: usize) {
        let liquidity = &mut self.borrows[liquidity_index];
        if settle_amount == liquidity.borrowed_amount() {
            self.borrows[liquidity_index] = ObligationLiquidity::default();
        } else {
            liquidity.repay(settle_amount);
        }
    }




    pub fn withdraw(
        &mut self,
        withdraw_amount: u64,
        collateral_index: usize,
    ) -> Result<WithdrawResult> {
        let collateral = &mut self.deposits[collateral_index];
        if withdraw_amount == collateral.deposited_amount {
            self.deposits[collateral_index] = ObligationCollateral::default();
            Ok(WithdrawResult::Full)
        } else {
            collateral.withdraw(withdraw_amount)?;
            Ok(WithdrawResult::Partial)
        }
    }


    pub fn max_withdraw_value(
        &self,
        obligation_collateral: &ObligationCollateral,
        reserve_max_ltv_pct: u8,
        reserve_liq_threshold_pct: u8,
        ltv_max_withdrawal_check: LtvMaxWithdrawalCheck,
    ) -> Fraction {
        let (highest_allowed_borrow_value, withdraw_collateral_ltv_pct) =
            if ltv_max_withdrawal_check == LtvMaxWithdrawalCheck::LiquidationThreshold {
                (
                    Fraction::from_bits(self.unhealthy_borrow_value_sf.saturating_sub(1)),
                    reserve_liq_threshold_pct,
                )
            } else {
                (
                    Fraction::from_bits(self.allowed_borrow_value_sf),
                    reserve_max_ltv_pct,
                )
            };

        let borrow_factor_adjusted_debt_value =
            Fraction::from_bits(self.borrow_factor_adjusted_debt_value_sf);

        if highest_allowed_borrow_value <= borrow_factor_adjusted_debt_value {
            return Fraction::ZERO;
        }

       
        if withdraw_collateral_ltv_pct == 0 {
            return Fraction::from_bits(obligation_collateral.market_value_sf);
        }

        highest_allowed_borrow_value.saturating_sub(borrow_factor_adjusted_debt_value) * 100_u128
            / u128::from(withdraw_collateral_ltv_pct)
    }


    pub fn remaining_borrow_value(&self) -> Fraction {
       
       
        Fraction::from_bits(
            self.allowed_borrow_value_sf
                .saturating_sub(self.borrow_factor_adjusted_debt_value_sf),
        )
    }


    pub fn find_collateral_in_deposits(
        &self,
        deposit_reserve: Pubkey,
    ) -> Result<&ObligationCollateral> {
        let collateral_index = self.position_of_collateral_in_deposits(deposit_reserve)?;
        Ok(&self.deposits[collateral_index])
    }



    pub fn find_or_add_collateral_to_deposits(
        &mut self,
        deposit_reserve: Pubkey,
    ) -> Result<(&mut ObligationCollateral, SlotAssignment)> {
        let (index, is_new) = if let Some(collateral_index) =
            self.find_collateral_index_in_deposits(deposit_reserve)
        {
            (collateral_index, false)
        } else if let Some(collateral_index) = self.deposits.iter().position(|c| !c.is_active()) {
            let collateral = &mut self.deposits[collateral_index];
            *collateral = ObligationCollateral::new(deposit_reserve);
            (collateral_index, true)
        } else {
            xmsg!("Obligation has no empty deposits");
            return err!(LendingError::ObligationReserveLimit);
        };
        Ok((&mut self.deposits[index], SlotAssignment { index, is_new }))
    }

    pub fn position_of_collateral_in_deposits(&self, deposit_reserve: Pubkey) -> Result<usize> {
        if self.is_active_deposits_empty() {
            xmsg!("Obligation has no deposits");
            return err!(LendingError::ObligationDepositsEmpty);
        }
        self.find_collateral_index_in_deposits(deposit_reserve)
            .ok_or(error!(LendingError::InvalidObligationCollateral))
    }

    pub fn find_collateral_index_in_deposits(&self, deposit_reserve: Pubkey) -> Option<usize> {
        self.deposits
            .iter()
            .position(|collateral| collateral.deposit_reserve == deposit_reserve)
    }


    pub fn find_liquidity_in_borrows(
        &self,
        borrow_reserve: Pubkey,
    ) -> Result<(&ObligationLiquidity, usize)> {
        if self.is_active_borrows_empty() {
            xmsg!("Obligation has no borrows");
            return err!(LendingError::ObligationBorrowsEmpty);
        }
        let liquidity_index = self
            .find_liquidity_index_in_borrows(borrow_reserve)
            .ok_or_else(|| error!(LendingError::InvalidObligationLiquidity))?;
        Ok((&self.borrows[liquidity_index], liquidity_index))
    }


    pub fn find_liquidity_in_borrows_mut(
        &mut self,
        borrow_reserve: Pubkey,
    ) -> Result<(&mut ObligationLiquidity, usize)> {
        if self.is_active_borrows_empty() {
            xmsg!("Obligation has no borrows");
            return err!(LendingError::ObligationBorrowsEmpty);
        }
        let liquidity_index = self
            .find_liquidity_index_in_borrows(borrow_reserve)
            .ok_or_else(|| error!(LendingError::InvalidObligationLiquidity))?;
        Ok((&mut self.borrows[liquidity_index], liquidity_index))
    }



    pub fn find_or_add_liquidity_to_borrows(
        &mut self,
        borrow_reserve: Pubkey,
        cumulative_borrow_rate: BigFraction,
    ) -> Result<(&mut ObligationLiquidity, usize)> {
        if let Some(liquidity_index) = self.find_liquidity_index_in_borrows(borrow_reserve) {
            Ok((&mut self.borrows[liquidity_index], liquidity_index))
        } else if let Some((index, liquidity)) = self
            .borrows
            .iter_mut()
            .enumerate()
            .find(|c| !c.1.is_active())
        {
            *liquidity = ObligationLiquidity::new(borrow_reserve, cumulative_borrow_rate);

            Ok((liquidity, index))
        } else {
            xmsg!("Obligation has no empty borrows");
            err!(LendingError::ObligationReserveLimit)
        }
    }

    pub fn find_liquidity_index_in_borrows(&self, borrow_reserve: Pubkey) -> Option<usize> {
        self.borrows
            .iter()
            .position(|liquidity| liquidity.borrow_reserve == borrow_reserve)
    }

    pub fn is_active_deposits_empty(&self) -> bool {
       
       
        self.deposits.iter().all(|deposit| !deposit.is_active())
    }

    pub fn is_active_borrows_empty(&self) -> bool {
       
       
        self.borrows.iter().all(|borrow| !borrow.is_active())
    }

    pub fn active_deposits_count(&self) -> usize {
        self.active_deposits().count()
    }

    pub fn active_borrows_count(&self) -> usize {
        self.active_borrows().count()
    }

    pub fn active_deposits(&self) -> impl Iterator<Item = &ObligationCollateral> {
        self.deposits.iter().filter(|c| c.is_active())
    }

    pub fn active_borrows(&self) -> impl Iterator<Item = &ObligationLiquidity> {
        self.borrows.iter().filter(|c| c.is_active())
    }

    pub fn active_deposits_mut(&mut self) -> impl Iterator<Item = &mut ObligationCollateral> {
        self.deposits.iter_mut().filter(|c| c.is_active())
    }

    pub fn active_borrows_mut(&mut self) -> impl Iterator<Item = &mut ObligationLiquidity> {
        self.borrows.iter_mut().filter(|c| c.is_active())
    }


    pub fn get_active_borrow_mut(&mut self, index: usize) -> Result<&mut ObligationLiquidity> {
        let Some(borrow) = self.borrows.get_mut(index) else {
            xmsg!("Invalid obligation borrow index: {}", index);
            return err!(LendingError::InvalidObligationLiquidity);
        };
        if !borrow.is_active() {
            xmsg!("Obligation borrow slot {} not active", index);
            return err!(LendingError::InvalidObligationLiquidity);
        }
        Ok(borrow)
    }


    pub fn get_borrow_order(&self, index: usize) -> Result<&BorrowOrder> {
        if index == 0 {
            return Ok(&self.head_borrow_order);
        }
        self.tail_borrow_orders
            .get(index - 1)
            .ok_or_else(|| error!(LendingError::OrderIndexOutOfBounds))
    }


    pub fn get_borrow_order_mut(&mut self, index: usize) -> Result<&mut BorrowOrder> {
        if index == 0 {
            return Ok(&mut self.head_borrow_order);
        }
        self.tail_borrow_orders
            .get_mut(index - 1)
            .ok_or_else(|| error!(LendingError::OrderIndexOutOfBounds))
    }


    pub fn borrow_orders(&self) -> impl Iterator<Item = &BorrowOrder> {
        iter::once(&self.head_borrow_order).chain(self.tail_borrow_orders.iter())
    }



    pub fn active_borrow_orders(&self) -> impl Iterator<Item = &BorrowOrder> {
        self.borrow_orders()
            .filter(|order| order.remaining_debt_amount > 0)
    }


    pub fn active_obligation_orders(&self) -> impl Iterator<Item = &ObligationOrder> {
        self.obligation_orders
            .iter()
            .filter(|order| order.is_active())
    }



    pub fn clear_expired_borrow_orders(&mut self, timestamp: u64) {
        for borrow_order in
            iter::once(&mut self.head_borrow_order).chain(self.tail_borrow_orders.iter_mut())
        {
            borrow_order.clear_if_past_fillable_timestamp(timestamp);
        }
    }








    pub fn get_borrowed_amount_if_single_token(&self) -> Option<u64> {
        if self.active_borrows_count() > 1 {
            None
        } else {
            Some(
                Fraction::from_bits(self.borrows.iter().map(|l| l.borrowed_amount_sf).sum())
                    .to_ceil::<u64>(),
            )
        }
    }

    pub fn get_bf_adjusted_debt_value(&self) -> Fraction {
        Fraction::from_bits(self.borrow_factor_adjusted_debt_value_sf)
    }

    pub fn get_allowed_borrow_value(&self) -> Fraction {
        Fraction::from_bits(self.allowed_borrow_value_sf)
    }

    pub fn get_unhealthy_borrow_value(&self) -> Fraction {
        Fraction::from_bits(self.unhealthy_borrow_value_sf)
    }


    pub fn get_borrowed_assets_market_value(&self) -> Fraction {
        Fraction::from_bits(self.borrowed_assets_market_value_sf)
    }

    pub fn has_referrer(&self) -> bool {
        self.referrer != Pubkey::default()
    }


    pub fn referrer(&self) -> Option<Pubkey> {
        Some(self.referrer).filter(|referrer| referrer != &Pubkey::default())
    }

    pub fn elevation_group(&self) -> Option<u8> {
        Some(self.elevation_group).filter(|group| *group != ELEVATION_GROUP_NONE)
    }

    pub fn update_has_debt(&mut self) {
        self.has_debt = u8::from(!self.is_active_borrows_empty());
    }





    pub fn has_debt(&self) -> bool {
        self.has_debt == true as u8
    }

    pub fn is_marked_for_deleveraging(&self) -> bool {
        self.autodeleverage_margin_call_started_timestamp != 0
    }

    pub fn mark_for_deleveraging(&mut self, current_timestamp: u64, target_ltv_pct: u8) {
        if current_timestamp == 0 {
            panic!("value reserved for non-marked state");
        }
        self.autodeleverage_margin_call_started_timestamp = current_timestamp;
        self.autodeleverage_target_ltv_pct = target_ltv_pct;
    }

    pub fn unmark_for_deleveraging(&mut self) {
        self.autodeleverage_margin_call_started_timestamp = 0;
        self.autodeleverage_target_ltv_pct = 0;
    }

    pub fn check_not_marked_for_deleveraging(&self) -> Result<()> {
        if self.is_marked_for_deleveraging() {
            xmsg!(
                "Obligation marked for deleveraging since {}",
                self.autodeleverage_margin_call_started_timestamp
            );
            return err!(LendingError::ObligationCurrentlyMarkedForDeleveraging);
        }
        Ok(())
    }

    pub fn has_obsolete_reserves(&self) -> bool {
        self.num_of_obsolete_borrow_reserves > 0 || self.num_of_obsolete_deposit_reserves > 0
    }






    pub fn is_single_debt_single_coll(&self) -> bool {
        self.active_deposits_count() == 1 && self.active_borrows_count() == 1
    }


    pub fn ownership_transfer_state(&self) -> OwnershipTransferState {
        OwnershipTransferState::try_from(self.ownership_transfer_state)
            .expect("Invalid serialized ownership transfer state")
    }


    pub fn is_ownership_transfer_in_progress(&self) -> bool {
        self.ownership_transfer_state() != OwnershipTransferState::None
    }

    pub fn check_ownership_transfer_not_in_progress(&self) -> Result<()> {
        if self.is_ownership_transfer_in_progress() {
            xmsg!("Obligation ownership transfer in progress");
            return err!(LendingError::ObligationOwnershipTransferInProgress);
        }
        Ok(())
    }


    pub fn is_ownership_transfer_in_initiated_state(&self) -> bool {
        self.ownership_transfer_state() == OwnershipTransferState::Initiated
    }


    pub fn is_ownership_transfer_approved(&self) -> bool {
        self.ownership_transfer_state() == OwnershipTransferState::Approved
    }

    pub fn check_ownership_transfer_in_initiated_state(&self) -> Result<()> {
        if !self.is_ownership_transfer_in_initiated_state() {
            xmsg!("Obligation ownership transfer not in initiated state");
            return err!(LendingError::ObligationOwnershipTransferNotInInitiatedState);
        }
        Ok(())
    }

    pub fn check_ownership_transfer_approved(&self) -> Result<()> {
        if !self.is_ownership_transfer_approved() {
            xmsg!("Obligation ownership transfer not approved");
            return err!(LendingError::ObligationOwnershipTransferNotApproved);
        }
        Ok(())
    }




    pub fn initiate_ownership_transfer(&mut self, pending_owner: Pubkey) -> Result<()> {
       
        if pending_owner == Pubkey::default() {
            xmsg!("Pending owner cannot be the default pubkey");
            return err!(LendingError::ObligationInvalidPendingOwner);
        }

       
        if pending_owner == self.owner {
            xmsg!("Pending owner cannot be the current owner");
            return err!(LendingError::ObligationInvalidPendingOwner);
        }

        self.ownership_transfer_state = OwnershipTransferState::Initiated.into();
        self.pending_owner = pending_owner;
        Ok(())
    }





    pub fn approve_ownership_transfer(&mut self) -> Result<()> {
        self.ownership_transfer_state = OwnershipTransferState::Approved.into();
        Ok(())
    }




    pub fn accept_ownership(&mut self) -> Result<()> {
        self.owner = self.pending_owner;
        self.pending_owner = Pubkey::default();
        self.ownership_transfer_state = OwnershipTransferState::None.into();
        Ok(())
    }




    pub fn abort_ownership_transfer(&mut self) -> Result<()> {
        self.pending_owner = Pubkey::default();
        self.ownership_transfer_state = OwnershipTransferState::None.into();
        Ok(())
    }
}


pub struct InitObligationParams {

    pub clock: Clock,

    pub lending_market: Pubkey,

    pub owner: Pubkey,

    pub deposits: [ObligationCollateral; 8],

    pub borrows: [ObligationLiquidity; 5],

    pub tag: u64,

    pub referrer: Pubkey,
}

#[derive(AnchorDeserialize, AnchorSerialize)]
pub struct InitObligationArgs {
    pub tag: u8,
    pub id: u8,
}


#[derive(Debug, Default, PartialEq, Eq)]
#[zero_copy]
#[repr(C)]
pub struct ObligationCollateral {

    pub deposit_reserve: Pubkey,

    pub deposited_amount: u64,

    pub market_value_sf: u128,




    pub borrowed_amount_against_this_collateral_in_elevation_group: u64,
    pub padding: [u64; 9],
}

impl ObligationCollateral {

    pub fn new(deposit_reserve: Pubkey) -> Self {
        Self {
            deposit_reserve,
            deposited_amount: 0,
            market_value_sf: 0,
            borrowed_amount_against_this_collateral_in_elevation_group: 0,
            padding: [0; 9],
        }
    }


    pub fn deposit(&mut self, collateral_amount: u64) -> Result<()> {
        self.deposited_amount = self
            .deposited_amount
            .checked_add(collateral_amount)
            .ok_or(LendingError::MathOverflow)?;
        Ok(())
    }


    pub fn withdraw(&mut self, collateral_amount: u64) -> Result<()> {
        self.deposited_amount = self
            .deposited_amount
            .checked_sub(collateral_amount)
            .ok_or(LendingError::MathOverflow)?;
        Ok(())
    }



    pub fn is_active(&self) -> bool {
        self.deposit_reserve != Pubkey::default()
    }


    pub fn market_value(&self) -> Fraction {
        Fraction::from_bits(self.market_value_sf)
    }
}


#[derive(Debug, Default, PartialEq, Eq)]
#[zero_copy]
#[repr(C)]
pub struct ObligationLiquidity {

    pub borrow_reserve: Pubkey,

    pub cumulative_borrow_rate_bsf: BigFractionBytes,










    pub last_borrowed_at_timestamp: u64,


    pub borrowed_amount_sf: u128,

    pub market_value_sf: u128,

    pub borrow_factor_adjusted_market_value_sf: u128,


    pub borrowed_amount_outside_elevation_groups: u64,



    pub fixed_term_borrow_rollover_config: FixedTermBorrowRolloverConfig,











    pub borrowed_amount_at_expiration: u64,

    pub padding2: [u64; 4],
}

impl ObligationLiquidity {

    pub fn new(borrow_reserve: Pubkey, cumulative_borrow_rate_bf: BigFraction) -> Self {
        Self {
            borrow_reserve,
            cumulative_borrow_rate_bsf: cumulative_borrow_rate_bf.into(),
            last_borrowed_at_timestamp: 0,
            borrowed_amount_sf: 0,
            market_value_sf: 0,
            borrow_factor_adjusted_market_value_sf: 0,
            borrowed_amount_outside_elevation_groups: 0,
            fixed_term_borrow_rollover_config: FixedTermBorrowRolloverConfig::default(),
            borrowed_amount_at_expiration: 0,
            padding2: default_array(),
        }
    }


    pub fn repay(&mut self, settle_amount: Fraction) {
        self.borrowed_amount_sf = (self.borrowed_amount() - settle_amount).to_bits();
    }


    pub fn borrow(&mut self, borrow_amount: Fraction, current_timestamp: u64) {
        self.borrowed_amount_sf = (self.borrowed_amount() + borrow_amount).to_bits();
        self.last_borrowed_at_timestamp = current_timestamp;
    }


    pub fn accrue_interest(&mut self, new_cumulative_borrow_rate: BigFraction) -> Result<()> {
        let former_cumulative_borrow_rate_bsf: U256 = U256(self.cumulative_borrow_rate_bsf.value);
        let new_cumulative_borrow_rate_bsf: U256 = new_cumulative_borrow_rate.0;

        match new_cumulative_borrow_rate_bsf.cmp(&former_cumulative_borrow_rate_bsf) {
            Ordering::Less => {
                xmsg!("Interest rate cannot be negative");
                return err!(LendingError::NegativeInterestRate);
            }
            Ordering::Equal => {}
            Ordering::Greater => {
                self.borrowed_amount_sf = Self::calculate_amount_with_accrued_interest(
                    self.borrowed_amount_sf,
                    former_cumulative_borrow_rate_bsf,
                    new_cumulative_borrow_rate_bsf,
                )?;
                self.cumulative_borrow_rate_bsf.value = new_cumulative_borrow_rate_bsf.0;
            }
        }

        Ok(())
    }











    pub fn capture_borrowed_amount_at_expiration(
        &mut self,
        reserve_config: &ReserveConfig,
        timestamp: u64,
    ) {
        if self.borrowed_amount_at_expiration != 0 {
            return;
        }
        if self
            .get_secs_since_reserve_debt_term_end(reserve_config, timestamp)
            .is_none()
        {
            return;
        }
        self.borrowed_amount_at_expiration = self.borrowed_amount().to_ceil();
    }



    fn calculate_amount_with_accrued_interest(
        amount_sf: u128,
        former_cumulative_borrow_rate_bsf: U256,
        new_cumulative_borrow_rate_bsf: U256,
    ) -> Result<u128> {
       
       
       

        let amount_sf_u256 = U256::from(amount_sf) * new_cumulative_borrow_rate_bsf
            / former_cumulative_borrow_rate_bsf;
        amount_sf_u256
            .try_into()
            .map_err(|_| error!(LendingError::MathOverflow))
    }








    fn calculate_interest_for_period(
        &self,
        amount: Fraction,
        time_period: Fraction,
        reserve: &Reserve,
    ) -> Result<Fraction> {
        let projected_duration = reserve.projected_accrual_duration(time_period)?;
        let future_cumulative_borrow_rate_bsf =
            reserve.calculate_future_cumulative_borrow_rate(projected_duration)?;

        let amount_with_interest =
            Fraction::from_bits(Self::calculate_amount_with_accrued_interest(
                amount.to_bits(),
                U256(self.cumulative_borrow_rate_bsf.value),
                future_cumulative_borrow_rate_bsf.0,
            )?);

        Ok(amount_with_interest - amount)
    }



    pub fn is_active(&self) -> bool {
        self.borrow_reserve != Pubkey::default()
    }


    pub fn market_value(&self) -> Fraction {
        Fraction::from_bits(self.market_value_sf)
    }


    pub fn borrowed_amount(&self) -> Fraction {
        Fraction::from_bits(self.borrowed_amount_sf)
    }


    pub fn borrowed_amount_at_expiration(&self) -> Fraction {
        Fraction::from_num(self.borrowed_amount_at_expiration)
    }



    pub fn get_secs_since_reserve_debt_term_end(
        &self,
        reserve_config: &ReserveConfig,
        timestamp: u64,
    ) -> Option<u64> {
        timestamp.checked_sub(self.get_debt_term_end_timestamp(reserve_config)?)
    }


    pub fn get_debt_term_end_timestamp(&self, reserve_config: &ReserveConfig) -> Option<u64> {
        let Some(debt_term_seconds) = reserve_config.get_debt_term_seconds() else {
            return None;
        };
        if self.last_borrowed_at_timestamp == 0 {
            xmsg!(
                "Debt reserve has a debt term of {} seconds, but an Obligation did not track its borrow timestamp; ignoring it",
                debt_term_seconds,
            );
            return None;
        }
        Some(self.last_borrowed_at_timestamp + debt_term_seconds)
    }












    pub fn calculate_early_repay_penalty(
        &self,
        reserve: &Reserve,
        repay_amount: Fraction,
        current_timestamp: u64,
    ) -> Result<u64> {
        if repay_amount > self.borrowed_amount() {
            panic!("caller must cap repay_amount to borrowed_amount");
        }
        let penalty =
            repay_amount * self.calculate_early_repay_penalty_rate(reserve, current_timestamp)?;
        Ok(penalty.to_ceil())
    }



    pub fn calculate_early_repay_penalty_rate(
        &self,
        reserve: &Reserve,
        current_timestamp: u64,
    ) -> Result<Fraction> {
        let Some(debt_term_seconds) = reserve.config.get_debt_term_seconds() else {
            return Ok(Fraction::ZERO);
        };

        if self.last_borrowed_at_timestamp == 0 {
            xmsg!(
                "Debt reserve has a debt term of {} seconds, but an Obligation did not track its last borrow timestamp; ignoring it",
                debt_term_seconds,
            );
            return Ok(Fraction::ZERO);
        }

        if self.borrowed_amount_sf == 0 {
            return Ok(Fraction::ZERO);
        }

        let seconds_since_last_borrowed =
            current_timestamp.saturating_sub(self.last_borrowed_at_timestamp);
        if seconds_since_last_borrowed >= debt_term_seconds {
            return Ok(Fraction::ZERO);
        }

        let remaining_secs = debt_term_seconds - seconds_since_last_borrowed;
        let remaining_interest_rate = self.calculate_interest_for_period(
            Fraction::ONE,
            Fraction::from_num(remaining_secs),
            reserve,
        )?;

        let penalty_pct = reserve
            .config
            .get_early_repay_penalty_remaining_interest_pct();

        Ok(remaining_interest_rate * penalty_pct)
    }
}









#[derive(Debug, Default, PartialEq, Eq)]
#[zero_copy]
#[repr(C)]
pub struct FixedTermBorrowRolloverConfig {












    pub auto_rollover_enabled: u8,








    pub open_term_allowed: u8,










    pub migration_to_fixed_enabled: u8,








   
   
    pub fixed_term_rollover_window_duration_days: u8,





    pub max_borrow_rate_bps: u32,










    pub min_debt_term_seconds: u64,
}

impl FixedTermBorrowRolloverConfig {
    pub fn is_auto_rollover_enabled(&self) -> bool {
        self.auto_rollover_enabled != false as u8
    }

    pub fn is_migration_to_fixed_enabled(&self) -> bool {
        self.migration_to_fixed_enabled != false as u8
    }



    pub fn is_compatible_with(&self, other: &Self) -> bool {
       
        let Self {
            auto_rollover_enabled,
            open_term_allowed,
            migration_to_fixed_enabled,
            fixed_term_rollover_window_duration_days: _,
            max_borrow_rate_bps,
            min_debt_term_seconds,
        } = self;
        auto_rollover_enabled == &other.auto_rollover_enabled
            && open_term_allowed == &other.open_term_allowed
            && migration_to_fixed_enabled == &other.migration_to_fixed_enabled
            && max_borrow_rate_bps == &other.max_borrow_rate_bps
            && min_debt_term_seconds == &other.min_debt_term_seconds
    }



    pub fn get_fixed_term_rollover_window_duration_seconds(&self) -> Option<u64> {
        if self.fixed_term_rollover_window_duration_days == 0 {
            return None;
        }
        Some(u64::from(self.fixed_term_rollover_window_duration_days) * SECONDS_PER_DAY)
    }


    pub fn reserve_constraint(&self) -> DebtReserveConstraint {
        DebtReserveConstraint {
            max_borrow_rate_bps: self.max_borrow_rate_bps,
            min_debt_term_seconds: self.min_debt_term_seconds,
        }
    }



    pub fn resolve_rollover_mode(
        &self,
        source_reserve_config: &ReserveConfig,
        target_reserve_config: &ReserveConfig,
        timestamp: u64,
    ) -> Result<RolloverMode> {
        if source_reserve_config.get_debt_term_seconds().is_some() {
           
            self.resolve_rollover_from_fixed_term_mode(target_reserve_config, timestamp)
        } else {
           
            self.check_migration_to_fixed_term_possible(target_reserve_config, timestamp)?;
            Ok(RolloverMode::FromOpenToFixedTerm)
        }
    }


    fn resolve_rollover_from_fixed_term_mode(
        &self,
        target_reserve_config: &ReserveConfig,
        timestamp: u64,
    ) -> Result<RolloverMode> {
       
        if !self.is_auto_rollover_enabled() {
            return err!(LendingError::ObligationBorrowRolloverNotEnabledByOwner);
        }

       
        if target_reserve_config.get_debt_term_seconds().is_none() {
            return if self.open_term_allowed == false as u8 {
               
                xmsg!("Owner did not allow rollover into open-term reserve");
                err!(LendingError::ObligationBorrowRolloverTargetReserveMismatch)
            } else {
               
                self.reserve_constraint()
                    .check_maturity_satisfying(target_reserve_config, timestamp)?;
                Ok(RolloverMode::FromFixedToOpenTerm)
               
               
               
               
               
               
            };
        }

       
        self.reserve_constraint()
            .check_satisfying(target_reserve_config, timestamp)?;

        Ok(RolloverMode::FromFixedToFixedTerm)
    }


    fn check_migration_to_fixed_term_possible(
        &self,
        target_reserve_config: &ReserveConfig,
        now: u64,
    ) -> Result<()> {
       
        if !self.is_migration_to_fixed_enabled() {
            xmsg!("Migration to fixed-term is not enabled by owner");
            return err!(LendingError::ObligationBorrowRolloverNotEnabledByOwner);
        }

       
        if target_reserve_config.get_debt_term_seconds().is_none() {
            xmsg!("Migration target must be a fixed-term reserve");
            return err!(LendingError::ObligationBorrowRolloverTargetReserveMismatch);
        }

       
        self.reserve_constraint()
            .check_satisfying(target_reserve_config, now)?;

        Ok(())
    }
}


#[derive(PartialEq, Eq, Clone, Copy, Debug, EnumString)]
pub enum RolloverMode {
    FromFixedToFixedTerm,
    FromFixedToOpenTerm,
    FromOpenToFixedTerm,
}









#[derive(
    AnchorSerialize,
    AnchorDeserialize,
    TryFromPrimitive,
    PartialEq,
    Eq,
    Clone,
    Copy,
    Debug,
    EnumString,
)]
#[cfg_attr(feature = "serde", derive(EnumIter))]
#[repr(u8)]
pub enum UpdateObligationConfigMode {


    FixedTermRolloverEnabled = 0,



    FixedTermRolloverMaxBorrowRateBps = 1,



    FixedTermRolloverMinDebtTermSeconds = 2,



    FixedTermRolloverOpenTermAllowed = 3,



    MigrationToFixedEnabled = 4,



    FixedTermRolloverWindowDurationDays = 5,
}


#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum ObligationConfigUpdateSubject {

    Obligation,


    Deposit(Pubkey),


    Borrow(Pubkey),
}

impl ObligationConfigUpdateSubject {

    pub fn resolve(
        specific_deposit_reserve_address: Option<Pubkey>,
        specific_borrow_reserve_address: Option<Pubkey>,
    ) -> Result<Self> {
        Ok(
            match (
                specific_deposit_reserve_address,
                specific_borrow_reserve_address,
            ) {
                (Some(_), Some(_)) => {
                    xmsg!("Cannot choose both a deposit and a borrow");
                    return err!(LendingError::InvalidObligationConfigUpdateSubject);
                }
                (None, None) => Self::Obligation,
                (Some(deposit), None) => Self::Deposit(deposit),
                (None, Some(borrow)) => Self::Borrow(borrow),
            },
        )
    }


    pub fn borrow_reserve_address(&self) -> Result<Pubkey> {
        let Self::Borrow(reserve_address) = self else {
            return err!(LendingError::InvalidObligationConfigUpdateSubject);
        };
        Ok(*reserve_address)
    }
}



#[derive(BorshSerialize, BorshDeserialize, Debug, Default, PartialEq, Eq)]
#[zero_copy]
#[repr(C)]
pub struct ObligationOrder {











    pub condition_threshold_sf: u128,













    pub opportunity_parameter_sf: u128,









    pub min_execution_bonus_bps: u16,














    pub max_execution_bonus_bps: u16,







    pub condition_type: u8,






    pub opportunity_type: u8,


    pub padding1: [u8; 2],





    pub max_borrow_rate_bps: u32,













    pub min_debt_term_seconds: u32,






    pub debt_mint_address: Pubkey,






    pub collateral_mint_address: Pubkey,



    pub padding2: [u128; 1],
}

impl ObligationOrder {

    pub fn condition_type(&self) -> ConditionType {
        ConditionType::try_from(self.condition_type).expect("Invalid serialized condition")
    }


    pub fn condition_threshold(&self) -> Fraction {
        Fraction::from_bits(self.condition_threshold_sf)
    }


    pub fn opportunity_type(&self) -> OpportunityType {
        OpportunityType::try_from(self.opportunity_type).expect("Invalid serialized opportunity")
    }


    pub fn opportunity_parameter(&self) -> Fraction {
        Fraction::from_bits(self.opportunity_parameter_sf)
    }



    pub fn condition(&self) -> OrderCondition {
        let threshold = self.condition_threshold();
        match self.condition_type() {
            ConditionType::Never => OrderCondition::Never,
            ConditionType::UserLtvAbove => OrderCondition::UserLtvAbove(threshold),
            ConditionType::UserLtvBelow => OrderCondition::UserLtvBelow(threshold),
            ConditionType::DebtCollPriceRatioAbove => {
                OrderCondition::DebtCollPriceRatioAbove(threshold)
            }
            ConditionType::DebtCollPriceRatioBelow => {
                OrderCondition::DebtCollPriceRatioBelow(threshold)
            }
            ConditionType::Always => OrderCondition::Always,
            ConditionType::LiquidationLtvCloserThan => {
                OrderCondition::LiquidationLtvCloserThan(threshold)
            }
        }
    }



    pub fn opportunity(&self) -> OrderOpportunity {
        let parameter = self.opportunity_parameter();
        match self.opportunity_type() {
            OpportunityType::DeleverageDebtAmount => {
                OrderOpportunity::Deleverage(OrderSize::DebtAmount(parameter))
            }
            OpportunityType::LeverUpDebtAmount => {
                OrderOpportunity::LeverUp(OrderSize::DebtAmount(parameter))
            }
            OpportunityType::DeleverageToTargetLtv => {
                OrderOpportunity::Deleverage(OrderSize::ToTargetLtv(parameter))
            }
            OpportunityType::LeverUpToTargetLtv => {
                OrderOpportunity::LeverUp(OrderSize::ToTargetLtv(parameter))
            }
        }
    }







    pub fn check_expected_opportunity_type(
        &self,
        expected_opportunity_type: OpportunityType,
    ) -> Result<()> {
        if self.opportunity_type() != expected_opportunity_type {
            xmsg!(
                "Order's opportunity type {:?} does not match the expected {:?}",
                self.opportunity_type(),
                expected_opportunity_type
            );
            return err!(LendingError::ObligationOrderOpportunityTypeMismatch);
        }
        Ok(())
    }


    pub fn check_reserve_requirements(
        &self,
        debt_reserve: &Reserve,
        collateral_reserve: &Reserve,
        timestamp: u64,
    ) -> Result<()> {
       
        if !Self::reserve_mint_accepted(self.collateral_mint_address, collateral_reserve) {
            return err!(LendingError::ObligationOrderCollateralMintMismatch);
        }
       
        if !Self::reserve_mint_accepted(self.debt_mint_address, debt_reserve) {
            return err!(LendingError::ObligationOrderDebtMintMismatch);
        }
       
        if let Some(reserve_constraint) = self.reserve_constraint() {
            reserve_constraint.check_satisfying(&debt_reserve.config, timestamp)?;
        }
        Ok(())
    }






    pub fn execution_bonus_rate_range(&self) -> RangeInclusive<Fraction> {
        Fraction::from_bps(self.min_execution_bonus_bps)
            ..=Fraction::from_bps(self.max_execution_bonus_bps)
    }





    pub fn is_supported_by(&self, obligation: &Obligation) -> bool {
        if self == &ObligationOrder::default() {
            return true;
        }
        self.condition_type().is_supported_by(obligation)
            && self.opportunity_type().is_supported_by(obligation)
    }






    pub fn consume(&mut self, debt_amount: u64) -> Option<Fraction> {
        self.consume_fraction(Fraction::from_num(debt_amount))
    }





    pub fn consume_fully(&mut self) -> Option<Fraction> {
        self.consume_fraction(Fraction::MAX)
    }

    fn consume_fraction(&mut self, debt_amount: Fraction) -> Option<Fraction> {
        match self.opportunity_type() {
            OpportunityType::DeleverageDebtAmount => {
                Some(self.use_debt_amount_opportunity("repay", debt_amount))
            }
            OpportunityType::LeverUpDebtAmount => {
                Some(self.use_debt_amount_opportunity("borrow", debt_amount))
            }
            OpportunityType::DeleverageToTargetLtv | OpportunityType::LeverUpToTargetLtv => {
                xmsg!("A target-LTV opportunity was used by executor with debt amount {} (order unaffected)", debt_amount);
                None
            }
        }
    }



    fn reserve_constraint(&self) -> Option<DebtReserveConstraint> {
        if self.max_borrow_rate_bps == 0 {
            return None;
        }
        Some(DebtReserveConstraint {
            max_borrow_rate_bps: self.max_borrow_rate_bps,
            min_debt_term_seconds: u64::from(self.min_debt_term_seconds),
        })
    }











    fn use_debt_amount_opportunity(
        &mut self,
        description: &str,
        debt_amount: Fraction,
    ) -> Fraction {
        let size = self.opportunity_parameter();
        let updated_size = size.saturating_sub(debt_amount);

        if updated_size.is_zero() {
            xmsg!(
                "An opportunity to {} {} of debt token {} was fully used by executor - order cleared",
                description,
                size.to_display(),
                self.debt_mint_address
            );
            *self = ObligationOrder::default();
        } else {
            xmsg!(
                "An opportunity to {} {} of debt token {} was partially used by executor (amount {}) - {} left on the order",
                description,
                size.to_display(),
                self.debt_mint_address,
                debt_amount.to_display(),
                updated_size
            );
            self.opportunity_parameter_sf = updated_size.to_bits();
        }

        updated_size
    }



    pub fn is_active(&self) -> bool {
        self.condition_type != 0
    }


    fn reserve_mint_accepted(chosen_mint: Pubkey, reserve: &Reserve) -> bool {
        chosen_mint == Pubkey::default() || chosen_mint == reserve.liquidity.mint_pubkey
    }
}












#[derive(BorshSerialize, BorshDeserialize, Debug, Default, PartialEq, Eq)]
#[zero_copy]
#[repr(C)]
pub struct BorrowOrder {


    pub debt_liquidity_mint: Pubkey,


    pub remaining_debt_amount: u64,



    pub filled_debt_destination: Pubkey,





    pub min_debt_term_seconds: u64,


    pub fillable_until_timestamp: u64,



    pub placed_at_timestamp: u64,



    pub last_updated_at_timestamp: u64,





    pub requested_debt_amount: u64,




    pub max_borrow_rate_bps: u32,







    pub active: u8,


















    pub enable_auto_rollover_on_filled_borrows: u8,


    pub padding1: [u8; 2],


    pub end_padding: [u64; 5],
}

impl BorrowOrder {

    pub fn reserve_constraint(&self) -> DebtReserveConstraint {
        DebtReserveConstraint {
            max_borrow_rate_bps: self.max_borrow_rate_bps,
            min_debt_term_seconds: self.min_debt_term_seconds,
        }
    }


    pub fn clear_if_past_fillable_timestamp(&mut self, timestamp: u64) {
        if self != &Default::default() && self.fillable_until_timestamp < timestamp {
            xmsg!(
                "Clearing expired borrow order - fillable until {}",
                self.fillable_until_timestamp
            );
            *self = Default::default();
        }
    }








    pub fn get_rollover_config_for_filled_borrow(&self) -> Option<FixedTermBorrowRolloverConfig> {
        if self.enable_auto_rollover_on_filled_borrows == false as u8 {
            return None;
        }
        Some(FixedTermBorrowRolloverConfig {
            auto_rollover_enabled: true as u8,
            open_term_allowed: true as u8,
            migration_to_fixed_enabled: (self.min_debt_term_seconds != 0) as u8,
            max_borrow_rate_bps: self.max_borrow_rate_bps,
            min_debt_term_seconds: self.min_debt_term_seconds,
            fixed_term_rollover_window_duration_days: 0,
        })
    }
}



#[derive(Clone, Debug)]
pub struct BorrowOrderConfig {
    pub debt_liquidity_mint: Pubkey,
    pub remaining_debt_amount: u64,
    pub filled_debt_destination: Pubkey,
    pub max_borrow_rate_bps: u32,
    pub min_debt_term_seconds: u64,
    pub fillable_until_timestamp: u64,
    pub enable_auto_rollover_on_filled_borrows: u8,
}

pub struct SlotAssignment {
    pub index: usize,
    pub is_new: bool,
}


#[derive(Clone, Copy, Debug)]
pub struct DebtReserveConstraint {
    pub max_borrow_rate_bps: u32,
    pub min_debt_term_seconds: u64,
}

impl DebtReserveConstraint {

    pub fn check_satisfying(&self, reserve_config: &ReserveConfig, timestamp: u64) -> Result<()> {
       
        let reserve_max_rate = reserve_config.max_borrow_rate_bps();
        if reserve_max_rate > self.max_borrow_rate_bps {
            xmsg!(
                "Reserve max borrow rate {} bps exceeds the owner's accepted maximum {} bps",
                reserve_max_rate,
                self.max_borrow_rate_bps,
            );
            return err!(LendingError::DebtReserveMaxBorrowRateExceeded);
        }

       
        let min_debt_term_seconds = self.min_debt_term_seconds();
        let reserve_debt_term_seconds = reserve_config.get_debt_term_seconds();
        if !is_term_satisfied(min_debt_term_seconds, reserve_debt_term_seconds) {
            xmsg!(
                "Reserve debt term of {:?} seconds does not satisfy the owner's minimum {:?}",
                reserve_debt_term_seconds,
                min_debt_term_seconds,
            );
            return err!(LendingError::DebtReserveMinDebtTermInsufficient);
        }

       
        self.check_maturity_satisfying(reserve_config, timestamp)?;

        Ok(())
    }



    pub fn check_maturity_satisfying(
        &self,
        reserve_config: &ReserveConfig,
        timestamp: u64,
    ) -> Result<()> {
        let min_debt_term_seconds = self.min_debt_term_seconds();
        let seconds_until_maturity = reserve_config
            .get_debt_maturity_timestamp()
            .map(|maturity_timstamp| maturity_timstamp.saturating_sub(timestamp));
        if !is_term_satisfied(min_debt_term_seconds, seconds_until_maturity) {
            xmsg!(
                "Reserve debt maturity timestamp leaves only {:?} seconds, which does not satisfy the owner's minimum {:?}",
                seconds_until_maturity,
                min_debt_term_seconds,
            );
            return err!(LendingError::DebtReserveMinDebtTermInsufficient);
        }

        Ok(())
    }

    fn min_debt_term_seconds(&self) -> Option<u64> {
        if self.min_debt_term_seconds == 0 {
            return None;
        }
        Some(self.min_debt_term_seconds)
    }
}



fn is_term_satisfied(min_requested_seconds: Option<u64>, max_offered_seconds: Option<u64>) -> bool {
    match (min_requested_seconds, max_offered_seconds) {
        (None, None) => true,
        (None, Some(_)) => false,
        (Some(_), None) => true,
        (Some(min_requested), Some(max_offered)) => min_requested <= max_offered,
    }
}

