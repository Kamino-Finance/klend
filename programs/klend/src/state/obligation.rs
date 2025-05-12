use std::{
    cmp::Ordering,
    fmt::{self, Display, Formatter},
    ops::RangeInclusive,
};

use anchor_lang::{account, err, prelude::*, solana_program::clock::Slot, Result};
use borsh::{BorshDeserialize, BorshSerialize};
use derivative::Derivative;

use super::{LastUpdate, LtvMaxWithdrawalCheck};
use crate::{
    order_operations::{ConditionType, OpportunityType},
    utils::{
        BigFraction, Fraction, FractionExtra, IterExt, ELEVATION_GROUP_NONE, OBLIGATION_SIZE, U256,
    },
    xmsg, AssetTier, BigFractionBytes, LendingError,
};

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

    pub deposits_asset_tiers: [u8; 8],
    pub borrows_asset_tiers: [u8; 5],

    pub elevation_group: u8,

    pub num_of_obsolete_deposit_reserves: u8,

    pub has_debt: u8,

    pub referrer: Pubkey,

    pub borrowing_disabled: u8,

    pub autodeleverage_target_ltv_pct: u8,

    pub lowest_reserve_deposit_max_ltv_pct: u8,

    pub num_of_obsolete_borrow_reserves: u8,

    #[derivative(Debug = "ignore")]
    pub reserved: [u8; 4],

    pub highest_borrow_factor_pct: u64,

    pub autodeleverage_margin_call_started_timestamp: u64,

    pub orders: [ObligationOrder; 2],

    #[derivative(Debug = "ignore")]
    pub padding_3: [u64; 93],
}

impl Default for Obligation {
    fn default() -> Self {
        Self {
            tag: 0,
            last_update: LastUpdate::default(),
            lending_market: Pubkey::default(),
            owner: Pubkey::default(),
            deposits: [ObligationCollateral::default(); 8],
            borrows: [ObligationLiquidity::default(); 5],
            deposited_value_sf: 0,
            borrowed_assets_market_value_sf: 0,
            allowed_borrow_value_sf: 0,
            unhealthy_borrow_value_sf: 0,
            lowest_reserve_deposit_liquidation_ltv: 0,
            borrow_factor_adjusted_debt_value_sf: 0,
            deposits_asset_tiers: [u8::MAX; 8],
            borrows_asset_tiers: [u8::MAX; 5],
            elevation_group: ELEVATION_GROUP_NONE,
            num_of_obsolete_deposit_reserves: 0,
            num_of_obsolete_borrow_reserves: 0,
            has_debt: 0,
            borrowing_disabled: 0,
            highest_borrow_factor_pct: 0,
            lowest_reserve_deposit_max_ltv_pct: 0,
            reserved: [0; 4],
            padding_3: [0; 93],
            referrer: Pubkey::default(),
            autodeleverage_target_ltv_pct: 0,
            autodeleverage_margin_call_started_timestamp: 0,
            orders: [ObligationOrder::default(); 2],
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
            if self.deposited_value_sf > 0 {self.loan_to_value().to_percent::<u16>().unwrap()} else { 0 },
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
                liquidity.borrowed_amount().to_num::<u128>(),
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
        self.last_update = LastUpdate::new(params.current_slot);
        self.lending_market = params.lending_market;
        self.owner = params.owner;
        self.deposits = params.deposits;
        self.borrows = params.borrows;
        self.referrer = params.referrer;
        self.deposits_asset_tiers = [u8::MAX; 8];
        self.borrows_asset_tiers = [u8::MAX; 5];
    }

    pub fn loan_to_value(&self) -> Fraction {
        Fraction::from_bits(self.borrow_factor_adjusted_debt_value_sf)
            / Fraction::from_bits(self.deposited_value_sf)
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
            self.borrows_asset_tiers[liquidity_index] = u8::MAX;
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
            self.deposits_asset_tiers[collateral_index] = u8::MAX;
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
        if self.active_deposits_empty() {
            xmsg!("Obligation has no deposits");
            return err!(LendingError::ObligationDepositsEmpty);
        }
        let collateral = self
            .deposits
            .iter()
            .find(|collateral| collateral.deposit_reserve == deposit_reserve)
            .ok_or(LendingError::InvalidObligationCollateral)?;
        Ok(collateral)
    }

    pub fn find_or_add_collateral_to_deposits(
        &mut self,
        deposit_reserve: Pubkey,
        deposit_reserve_asset_tier: AssetTier,
    ) -> Result<(&mut ObligationCollateral, bool)> {
        if let Some(collateral_index) = self
            .deposits
            .iter_mut()
            .position(|collateral| collateral.deposit_reserve == deposit_reserve)
        {
            Ok((&mut self.deposits[collateral_index], false))
        } else if let Some(collateral_index) = self.deposits.iter().position(|c| !c.is_active()) {
            let collateral = &mut self.deposits[collateral_index];
            *collateral = ObligationCollateral::new(deposit_reserve);
            self.deposits_asset_tiers[collateral_index] = deposit_reserve_asset_tier.into();
            Ok((collateral, true))
        } else {
            xmsg!("Obligation has no empty deposits");
            err!(LendingError::ObligationReserveLimit)
        }
    }

    pub fn position_of_collateral_in_deposits(&self, deposit_reserve: Pubkey) -> Result<usize> {
        if self.active_deposits_empty() {
            xmsg!("Obligation has no deposits");
            return err!(LendingError::ObligationDepositsEmpty);
        }
        self.deposits
            .iter()
            .position(|collateral| collateral.deposit_reserve == deposit_reserve)
            .ok_or(error!(LendingError::InvalidObligationCollateral))
    }

    pub fn find_liquidity_in_borrows(
        &self,
        borrow_reserve: Pubkey,
    ) -> Result<(&ObligationLiquidity, usize)> {
        if self.active_borrows_empty() {
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
        if self.active_borrows_empty() {
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
        borrow_reserve_asset_tier: AssetTier,
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
            self.borrows_asset_tiers[index] = borrow_reserve_asset_tier.into();

            Ok((liquidity, index))
        } else {
            xmsg!("Obligation has no empty borrows");
            err!(LendingError::ObligationReserveLimit)
        }
    }

    fn find_liquidity_index_in_borrows(&self, borrow_reserve: Pubkey) -> Option<usize> {
        self.borrows
            .iter()
            .position(|liquidity| liquidity.borrow_reserve == borrow_reserve)
    }

    pub fn active_deposits_empty(&self) -> bool {
        self.deposits.iter().all(|deposit| !deposit.is_active())
    }

    pub fn active_borrows_empty(&self) -> bool {
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

    pub fn get_deposit_asset_tiers(&self) -> Vec<AssetTier> {
        self.deposits
            .iter()
            .enumerate()
            .filter_map(|(index, deposit)| {
                if deposit.is_active() && deposit.deposited_amount > 0 {
                    Some(AssetTier::try_from(self.deposits_asset_tiers[index]).unwrap())
                } else {
                    None
                }
            })
            .collect::<Vec<AssetTier>>()
    }

    pub fn get_borrows_asset_tiers(&self) -> Vec<AssetTier> {
        self.borrows
            .iter()
            .enumerate()
            .filter_map(|(index, borrow)| {
                if borrow.is_active() && borrow.borrowed_amount_sf > 0 {
                    Some(AssetTier::try_from(self.borrows_asset_tiers[index]).unwrap())
                } else {
                    None
                }
            })
            .collect::<Vec<AssetTier>>()
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

    pub fn update_has_debt(&mut self) {
        if self.active_borrows_empty() {
            self.has_debt = 0;
        } else {
            self.has_debt = 1;
        }
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

    pub fn single_debt(&self) -> Option<&ObligationLiquidity> {
        self.active_borrows().only_element()
    }

    pub fn single_collateral(&self) -> Option<&ObligationCollateral> {
        self.active_deposits().only_element()
    }

    pub fn is_single_debt_single_coll(&self) -> bool {
        self.active_deposits_count() == 1 && self.active_borrows_count() == 1
    }
}

pub struct InitObligationParams {
    pub current_slot: Slot,
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
}

#[derive(Debug, Default, PartialEq, Eq)]
#[zero_copy]
#[repr(C)]
pub struct ObligationLiquidity {
    pub borrow_reserve: Pubkey,
    pub cumulative_borrow_rate_bsf: BigFractionBytes,
    pub padding: u64,
    pub borrowed_amount_sf: u128,
    pub market_value_sf: u128,
    pub borrow_factor_adjusted_market_value_sf: u128,

    pub borrowed_amount_outside_elevation_groups: u64,

    pub padding2: [u64; 7],
}

impl ObligationLiquidity {
    pub fn new(borrow_reserve: Pubkey, cumulative_borrow_rate_bf: BigFraction) -> Self {
        Self {
            borrow_reserve,
            cumulative_borrow_rate_bsf: cumulative_borrow_rate_bf.into(),
            padding: 0,
            borrowed_amount_sf: 0,
            market_value_sf: 0,
            borrow_factor_adjusted_market_value_sf: 0,
            borrowed_amount_outside_elevation_groups: 0,
            padding2: [0; 7],
        }
    }

    pub fn repay(&mut self, settle_amount: Fraction) {
        self.borrowed_amount_sf = (self.borrowed_amount() - settle_amount).to_bits();
    }

    pub fn borrow(&mut self, borrow_amount: Fraction) {
        self.borrowed_amount_sf = (self.borrowed_amount() + borrow_amount).to_bits();
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
                let borrowed_amount_sf_u256 = U256::from(self.borrowed_amount_sf)
                    * new_cumulative_borrow_rate_bsf
                    / former_cumulative_borrow_rate_bsf;
                self.borrowed_amount_sf = borrowed_amount_sf_u256
                    .try_into()
                    .map_err(|_| error!(LendingError::MathOverflow))?;
                self.cumulative_borrow_rate_bsf.value = new_cumulative_borrow_rate_bsf.0;
            }
        }

        Ok(())
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

    pub padding1: [u8; 10],

    pub padding2: [u128; 5],
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

    pub fn consume(&mut self, debt_repay_amount: u64) {
        match self.opportunity_type() {
            OpportunityType::DeleverageSingleDebtAmount => {
                self.use_deleverage_single_debt_amount_opportunity(debt_repay_amount);
            }
            OpportunityType::DeleverageAllDebt => {
                xmsg!("An opportunity to liquidate all debt was used by liquidator repaying amount {} (order unaffected)", debt_repay_amount);
            }
        }
    }

    pub fn condition_to_display(&self) -> impl Display {
        match self.condition_type() {
            ConditionType::Never => "<inactive>".to_string(),
            ConditionType::UserLtvAbove => format!("LTV > {}", self.condition_threshold()),
            ConditionType::UserLtvBelow => format!("LTV < {}", self.condition_threshold()),
            ConditionType::DebtCollPriceRatioAbove => format!(
                "ratio of (debt token price / collateral token price) > {}",
                self.condition_threshold()
            ),
            ConditionType::DebtCollPriceRatioBelow => format!(
                "ratio of (debt token price / collateral token price) < {}",
                self.condition_threshold()
            ),
        }
    }

    pub fn opportunity_to_display(&self) -> impl Display {
        match self.opportunity_type() {
            OpportunityType::DeleverageSingleDebtAmount => format!(
                "repay amount {} of single debt",
                self.opportunity_parameter()
            ),
            OpportunityType::DeleverageAllDebt => "repay all debt".to_string(),
        }
    }

    fn use_deleverage_single_debt_amount_opportunity(&mut self, debt_repay_amount: u64) {
        let liquidatable_debt_amount = self.opportunity_parameter();
        let updated_liquidatable_debt_amount =
            liquidatable_debt_amount.saturating_sub(Fraction::from_num(debt_repay_amount));

        if updated_liquidatable_debt_amount.is_zero() {
            xmsg!("An opportunity to liquidate {} of single debt was fully used by liquidator repaying amount {} (order cleared)", liquidatable_debt_amount, debt_repay_amount);
            *self = ObligationOrder::default();
            return;
        }

        xmsg!("An opportunity to liquidate {} of single debt was partially used by liquidator repaying amount {} ({} left on the order)", liquidatable_debt_amount, debt_repay_amount, updated_liquidatable_debt_amount);
        self.opportunity_parameter_sf = updated_liquidatable_debt_amount.to_bits();
    }

    pub fn is_active(&self) -> bool {
        self.condition_type != 0
    }
}
