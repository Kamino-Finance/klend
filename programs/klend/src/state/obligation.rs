use std::{
    cmp::Ordering,
    fmt::{self, Display, Formatter},
};

use anchor_lang::{account, err, prelude::*, solana_program::clock::Slot, Result};
use derivative::Derivative;

use super::LastUpdate;
use crate::{
    utils::{BigFraction, Fraction, FractionExtra, ELEVATION_GROUP_NONE, OBLIGATION_SIZE, U256},
    xmsg, AssetTier, BigFractionBytes, LendingError, LendingResult,
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
    pub lowest_reserve_deposit_ltv: u64,
    pub deposited_value_sf: u128,

    pub borrows: [ObligationLiquidity; 5],
    pub borrow_factor_adjusted_debt_value_sf: u128,
    pub borrowed_assets_market_value_sf: u128,
    pub allowed_borrow_value_sf: u128,
    pub unhealthy_borrow_value_sf: u128,

    pub deposits_asset_tiers: [u8; 8],
    pub borrows_asset_tiers: [u8; 5],

    pub elevation_group: u8,

    pub num_of_obsolete_reserves: u8,

    pub has_debt: u8,

    pub referrer: Pubkey,

    #[derivative(Debug = "ignore")]
    pub padding_3: [u64; 128],
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
            lowest_reserve_deposit_ltv: 0,
            borrow_factor_adjusted_debt_value_sf: 0,
            deposits_asset_tiers: [u8::MAX; 8],
            borrows_asset_tiers: [u8::MAX; 5],
            elevation_group: ELEVATION_GROUP_NONE,
            num_of_obsolete_reserves: 0,
            padding_3: [0; 128],
            has_debt: 0,
            referrer: Pubkey::default(),
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

        for collateral in self
            .deposits
            .iter()
            .filter(|c| c.deposit_reserve != Pubkey::default())
        {
            write!(
                f,
                "\n  Collateral reserve: {}, value: ${}, lamports: {}",
                collateral.deposit_reserve,
                Fraction::from_bits(collateral.market_value_sf).to_display(),
                collateral.deposited_amount,
            )?;
        }

        for liquidity in self
            .borrows
            .iter()
            .filter(|l| l.borrow_reserve != Pubkey::default())
        {
            write!(
                f,
                "\n  Borrowed reserve  : {}, value: ${}, lamports: {}",
                liquidity.borrow_reserve,
                Fraction::from_bits(liquidity.market_value_sf).to_display(),
                Fraction::from_bits(liquidity.borrowed_amount_sf).to_num::<u128>(),
            )?;
        }

        Ok(())
    }
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

    pub fn unhealthy_loan_to_value(&self) -> Fraction {
        Fraction::from_bits(self.unhealthy_borrow_value_sf)
            / Fraction::from_bits(self.deposited_value_sf)
    }

    pub fn repay(&mut self, settle_amount: Fraction, liquidity_index: usize) -> Result<()> {
        let liquidity = &mut self.borrows[liquidity_index];
        if settle_amount == Fraction::from_bits(liquidity.borrowed_amount_sf) {
            self.borrows[liquidity_index] = ObligationLiquidity::default();
            self.borrows_asset_tiers[liquidity_index] = u8::MAX;
        } else {
            liquidity.repay(settle_amount);
        }
        Ok(())
    }

    pub fn withdraw(&mut self, withdraw_amount: u64, collateral_index: usize) -> Result<()> {
        let collateral = &mut self.deposits[collateral_index];
        if withdraw_amount == collateral.deposited_amount {
            self.deposits[collateral_index] = ObligationCollateral::default();
            self.deposits_asset_tiers[collateral_index] = u8::MAX;
        } else {
            collateral.withdraw(withdraw_amount)?;
        }
        Ok(())
    }

    pub fn max_withdraw_value(&self, withdraw_collateral_ltv_pct: u8) -> LendingResult<Fraction> {
        let allowed_borrow_value = Fraction::from_bits(self.allowed_borrow_value_sf);
        let borrow_factor_adjusted_debt_value =
            Fraction::from_bits(self.borrow_factor_adjusted_debt_value_sf);

        if allowed_borrow_value <= borrow_factor_adjusted_debt_value {
            return Ok(Fraction::ZERO);
        }

        if withdraw_collateral_ltv_pct == 0 {
            return Ok(Fraction::from_bits(self.deposited_value_sf));
        }

        Ok(
            allowed_borrow_value.saturating_sub(borrow_factor_adjusted_debt_value) * 100_u128
                / u128::from(withdraw_collateral_ltv_pct),
        )
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
    ) -> Result<(&ObligationCollateral, usize)> {
        if self.deposits_empty() {
            xmsg!("Obligation has no deposits");
            return err!(LendingError::ObligationDepositsEmpty);
        }
        let collateral_index = self
            .find_collateral_index_in_deposits(deposit_reserve)
            .ok_or(LendingError::InvalidObligationCollateral)?;
        Ok((&self.deposits[collateral_index], collateral_index))
    }

    pub fn find_or_add_collateral_to_deposits(
        &mut self,
        deposit_reserve: Pubkey,
        deposit_reserve_asset_tier: AssetTier,
    ) -> Result<&mut ObligationCollateral> {
        if let Some(collateral_index) = self.find_collateral_index_in_deposits(deposit_reserve) {
            return Ok(&mut self.deposits[collateral_index]);
        }

        if let Some((index, collateral)) = self
            .deposits
            .iter_mut()
            .enumerate()
            .find(|c| c.1.deposit_reserve == Pubkey::default())
        {
            *collateral = ObligationCollateral::new(deposit_reserve);
            self.deposits_asset_tiers[index] = deposit_reserve_asset_tier.into();

            Ok(collateral)
        } else {
            xmsg!("Obligation has no empty deposits");
            err!(LendingError::ObligationReserveLimit)
        }
    }

    fn find_collateral_index_in_deposits(&self, deposit_reserve: Pubkey) -> Option<usize> {
        self.deposits
            .iter()
            .position(|collateral| collateral.deposit_reserve == deposit_reserve)
    }

    pub fn find_liquidity_in_borrows(
        &self,
        borrow_reserve: Pubkey,
    ) -> Result<(&ObligationLiquidity, usize)> {
        if self.borrows_empty() {
            xmsg!("Obligation has no borrows");
            return err!(LendingError::ObligationBorrowsEmpty);
        }
        let liquidity_index = self
            .find_liquidity_index_in_borrows(borrow_reserve)
            .ok_or(LendingError::InvalidObligationLiquidity)?;
        Ok((&self.borrows[liquidity_index], liquidity_index))
    }

    pub fn find_liquidity_in_borrows_mut(
        &mut self,
        borrow_reserve: Pubkey,
    ) -> Result<(&mut ObligationLiquidity, usize)> {
        if self.borrows_empty() {
            xmsg!("Obligation has no borrows");
            return err!(LendingError::ObligationBorrowsEmpty);
        }
        let liquidity_index = self
            .find_liquidity_index_in_borrows(borrow_reserve)
            .ok_or(LendingError::InvalidObligationLiquidity)?;
        Ok((&mut self.borrows[liquidity_index], liquidity_index))
    }

    pub fn find_or_add_liquidity_to_borrows(
        &mut self,
        borrow_reserve: Pubkey,
        cumulative_borrow_rate: BigFraction,
        borrow_reserve_asset_tier: AssetTier,
    ) -> Result<&mut ObligationLiquidity> {
        if let Some(liquidity_index) = self.find_liquidity_index_in_borrows(borrow_reserve) {
            return Ok(&mut self.borrows[liquidity_index]);
        }

        if let Some((index, liquidity)) = self
            .borrows
            .iter_mut()
            .enumerate()
            .find(|c| c.1.borrow_reserve == Pubkey::default())
        {
            *liquidity = ObligationLiquidity::new(borrow_reserve, cumulative_borrow_rate);
            self.borrows_asset_tiers[index] = borrow_reserve_asset_tier.into();

            Ok(liquidity)
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

    pub fn deposits_empty(&self) -> bool {
        self.deposits
            .iter()
            .all(|c| c.deposit_reserve == Pubkey::default())
    }

    pub fn borrows_empty(&self) -> bool {
        self.borrows
            .iter()
            .all(|l| l.borrow_reserve == Pubkey::default())
    }

    pub fn deposits_count(&self) -> usize {
        self.deposits
            .iter()
            .filter(|c| c.deposit_reserve != Pubkey::default())
            .count()
    }

    pub fn borrows_count(&self) -> usize {
        self.borrows
            .iter()
            .filter(|l| l.borrow_reserve != Pubkey::default())
            .count()
    }

    pub fn get_deposit_asset_tiers(&self) -> Vec<AssetTier> {
        self.deposits
            .iter()
            .enumerate()
            .filter_map(|(index, deposit)| {
                if deposit.deposit_reserve != Pubkey::default() && deposit.deposited_amount > 0 {
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
                if borrow.borrow_reserve != Pubkey::default() && borrow.borrowed_amount_sf > 0 {
                    Some(AssetTier::try_from(self.borrows_asset_tiers[index]).unwrap())
                } else {
                    None
                }
            })
            .collect::<Vec<AssetTier>>()
    }

    pub fn has_referrer(&self) -> bool {
        self.referrer != Pubkey::default()
    }

    pub fn update_has_debt(&mut self) {
        if self.borrows_empty() {
            self.has_debt = 0;
        } else {
            self.has_debt = 1;
        }
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
    pub padding: [u64; 10],
}

impl ObligationCollateral {
    pub fn new(deposit_reserve: Pubkey) -> Self {
        Self {
            deposit_reserve,
            deposited_amount: 0,
            market_value_sf: 0,
            padding: [0; 10],
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

    pub padding2: [u64; 8],
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
            padding2: [0; 8],
        }
    }

    pub fn repay(&mut self, settle_amount: Fraction) {
        self.borrowed_amount_sf =
            (Fraction::from_bits(self.borrowed_amount_sf) - settle_amount).to_bits();
    }

    pub fn borrow(&mut self, borrow_amount: Fraction) {
        self.borrowed_amount_sf =
            (Fraction::from_bits(self.borrowed_amount_sf) + borrow_amount).to_bits();
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
}
