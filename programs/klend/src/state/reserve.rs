use std::{
    cmp::{max, min},
    ops::{Add, Div, Mul},
};

use anchor_lang::{
    account, err,
    prelude::{msg, Pubkey, *},
    solana_program::clock::Slot,
    Result,
};
use borsh::{BorshDeserialize, BorshSerialize};
use derivative::Derivative;
use num_enum::{IntoPrimitive, TryFromPrimitive};
#[cfg(feature = "serde")]
use serde;

#[cfg(feature = "serde")]
use super::serde_bool_u8;
use super::{DepositLiquidityResult, LastUpdate, TokenInfo};
use crate::{
    fraction::FractionExtra,
    utils::{
        accounts::default_array, borrow_rate_curve::BorrowRateCurve, ten_pow, BigFraction,
        Fraction, INITIAL_COLLATERAL_RATE, PROGRAM_VERSION, RESERVE_CONFIG_SIZE, RESERVE_SIZE,
        SLOTS_PER_YEAR, U256,
    },
    BorrowSize, CalculateBorrowResult, CalculateRepayResult, LendingError, LendingResult,
    ReferrerTokenState,
};

#[derive(Default, Debug, PartialEq, Eq)]
#[zero_copy]
#[repr(C)]
pub struct BigFractionBytes {
    pub value: [u64; 4],
    pub padding: [u64; 2],
}

impl From<BigFraction> for BigFractionBytes {
    fn from(value: BigFraction) -> BigFractionBytes {
        BigFractionBytes {
            value: value.to_bits(),
            padding: [0; 2],
        }
    }
}

impl From<BigFractionBytes> for BigFraction {
    fn from(value: BigFractionBytes) -> BigFraction {
        BigFraction::from_bits(value.value)
    }
}

static_assertions::const_assert_eq!(RESERVE_SIZE, std::mem::size_of::<Reserve>());
static_assertions::const_assert_eq!(0, std::mem::size_of::<Reserve>() % 8);
#[derive(PartialEq, Derivative)]
#[derivative(Debug)]
#[account(zero_copy)]
#[repr(C)]
pub struct Reserve {

    pub version: u64,


    pub last_update: LastUpdate,


    pub lending_market: Pubkey,

    pub farm_collateral: Pubkey,
    pub farm_debt: Pubkey,


    pub liquidity: ReserveLiquidity,

    #[derivative(Debug = "ignore")]
    pub reserve_liquidity_padding: [u64; 150],


    pub collateral: ReserveCollateral,

    #[derivative(Debug = "ignore")]
    pub reserve_collateral_padding: [u64; 150],


    pub config: ReserveConfig,

    #[derivative(Debug = "ignore")]
    pub config_padding: [u64; 116],

    pub borrowed_amount_outside_elevation_group: u64,



    pub borrowed_amounts_against_this_reserve_in_elevation_groups: [u64; 32],

    #[derivative(Debug = "ignore")]
    pub padding: [u64; 207],
}

impl Default for Reserve {
    fn default() -> Self {
        Self {
            version: 0,
            last_update: LastUpdate::default(),
            lending_market: Pubkey::default(),
            liquidity: ReserveLiquidity::default(),
            collateral: ReserveCollateral::default(),
            config: ReserveConfig::default(),
            farm_collateral: Pubkey::default(),
            farm_debt: Pubkey::default(),
            reserve_liquidity_padding: default_array(),
            reserve_collateral_padding: default_array(),
            config_padding: default_array(),
            borrowed_amount_outside_elevation_group: 0,
            borrowed_amounts_against_this_reserve_in_elevation_groups: [0; 32],
            padding: default_array(),
        }
    }
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
    strum::EnumIter,
)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[repr(u8)]
pub enum ReserveFarmKind {
    Collateral = 0,
    Debt = 1,
}

impl Reserve {

    pub fn init(&mut self, params: InitReserveParams) {
        *self = Self::default();
        self.version = PROGRAM_VERSION as u64;
        self.last_update = LastUpdate::new(params.current_slot);
        self.lending_market = params.lending_market;
        self.liquidity = *params.liquidity;
        self.collateral = *params.collateral;
        self.config = *params.config;
    }

   


    pub fn current_borrow_rate(&self) -> Result<Fraction> {
        let utilization_rate = self.liquidity.utilization_rate();

        self.config
            .borrow_rate_curve
            .get_borrow_rate(utilization_rate)
    }


    pub fn borrow_factor_f(&self, is_in_elevation_group: bool) -> Fraction {
        if is_in_elevation_group {
           
            Fraction::ONE
        } else {
           
            self.config.get_borrow_factor()
        }
    }

    pub fn get_farm(&self, mode: ReserveFarmKind) -> Pubkey {
        match mode {
            ReserveFarmKind::Collateral => self.farm_collateral,
            ReserveFarmKind::Debt => self.farm_debt,
        }
    }

    pub fn token_symbol(&self) -> &str {
        self.config.token_info.symbol()
    }

    pub fn add_farm(&mut self, farm_state: &Pubkey, mode: ReserveFarmKind) {
        match mode {
            ReserveFarmKind::Collateral => self.farm_collateral = *farm_state,
            ReserveFarmKind::Debt => self.farm_debt = *farm_state,
        }
    }

    pub fn compute_depositable_amount_and_minted_collateral(
        &self,
        liquidity_amount: u64,
    ) -> Result<DepositLiquidityResult> {
       
        let collateral_amount = self
            .collateral_exchange_rate()
            .liquidity_to_collateral(liquidity_amount);

       
        let liquidity_amount_to_deposit = self
            .collateral_exchange_rate()
            .collateral_to_liquidity_ceil(collateral_amount);

        require_gte!(
            liquidity_amount,
            liquidity_amount_to_deposit,
            LendingError::MathOverflow
        );

        Ok(DepositLiquidityResult {
            liquidity_amount: liquidity_amount_to_deposit,
            collateral_amount,
        })
    }


    pub fn deposit_liquidity(
        &mut self,
        liquidity_amount: u64,
        collateral_amount: u64,
    ) -> Result<()> {
        self.liquidity.deposit(liquidity_amount)?;
        self.collateral.mint(collateral_amount)?;

        Ok(())
    }


    pub fn redeem_collateral(&mut self, collateral_amount: u64) -> Result<u64> {
        let collateral_exchange_rate = self.collateral_exchange_rate();

        let liquidity_amount = collateral_exchange_rate.collateral_to_liquidity(collateral_amount);

        self.collateral.burn(collateral_amount)?;
        self.liquidity.withdraw(liquidity_amount)?;

        Ok(liquidity_amount)
    }


    pub fn collateral_exchange_rate(&self) -> CollateralExchangeRate {
        let total_liquidity = self.liquidity.total_supply();
        self.collateral.exchange_rate(total_liquidity)
    }


    pub fn accrue_interest(&mut self, current_slot: Slot, referral_fee_bps: u16) -> Result<()> {
        let slots_elapsed = self.last_update.slots_elapsed(current_slot)?;
        if slots_elapsed > 0 {
            let current_borrow_rate = self.current_borrow_rate()?;
            let protocol_take_rate = Fraction::from_percent(self.config.protocol_take_rate_pct);
            let referral_rate = Fraction::from_bps(referral_fee_bps);
            let host_fixed_interest_rate =
                Fraction::from_bps(self.config.host_fixed_interest_rate_bps);

            self.liquidity.compound_interest(
                current_borrow_rate,
                host_fixed_interest_rate,
                slots_elapsed,
                protocol_take_rate,
                referral_rate,
            )?;
        }

        Ok(())
    }


    pub fn update_deposit_limit_crossed_timestamp(&mut self, timestamp: u64) {
        if self.deposit_limit_crossed() {
            if self.liquidity.deposit_limit_crossed_timestamp == 0 {
                self.liquidity.deposit_limit_crossed_timestamp = timestamp;
            }
           
        } else {
            self.liquidity.deposit_limit_crossed_timestamp = 0;
        }
    }


    pub fn update_borrow_limit_crossed_timestamp(&mut self, timestamp: u64) {
        if self.borrow_limit_crossed() {
            if self.liquidity.borrow_limit_crossed_timestamp == 0 {
                self.liquidity.borrow_limit_crossed_timestamp = timestamp;
            }
           
        } else {
            self.liquidity.borrow_limit_crossed_timestamp = 0;
        }
    }


    pub fn calculate_borrow(
        &self,
        borrow_size: BorrowSize,
        max_borrow_factor_adjusted_debt_value: Fraction,
        remaining_reserve_borrow: Fraction,
        referral_fee_bps: u16,
        is_in_elevation_group: bool,
        has_referrer: bool,
    ) -> Result<CalculateBorrowResult> {
        let decimals = self.liquidity.mint_factor();
        let market_price_f = self.liquidity.get_market_price();
        let borrow_factor_f = self.borrow_factor_f(is_in_elevation_group);

        match borrow_size {
            BorrowSize::AllAvailable => self.calculate_borrow_all_available(
                max_borrow_factor_adjusted_debt_value,
                remaining_reserve_borrow,
                referral_fee_bps,
                has_referrer,
                decimals,
                market_price_f,
                borrow_factor_f,
            ),
            BorrowSize::Exact(receive_amount) => self.calculate_borrow_exact(
                receive_amount,
                max_borrow_factor_adjusted_debt_value,
                remaining_reserve_borrow,
                referral_fee_bps,
                has_referrer,
                decimals,
                market_price_f,
                borrow_factor_f,
            ),
            BorrowSize::AtMost(requested_receive_amount) => {
                let borrow_exact_result = self.calculate_borrow_exact(
                    requested_receive_amount,
                    max_borrow_factor_adjusted_debt_value,
                    remaining_reserve_borrow,
                    referral_fee_bps,
                    has_referrer,
                    decimals,
                    market_price_f,
                    borrow_factor_f,
                );
               
               
               
               
               
                if borrow_exact_result == err!(LendingError::BorrowTooLarge)
                    || borrow_exact_result == err!(LendingError::BorrowLimitExceeded)
                {
                    self.calculate_borrow_all_available(
                        max_borrow_factor_adjusted_debt_value,
                        remaining_reserve_borrow,
                        referral_fee_bps,
                        has_referrer,
                        decimals,
                        market_price_f,
                        borrow_factor_f,
                    )
                } else {
                    borrow_exact_result
                }
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn calculate_borrow_all_available(
        &self,
        max_borrow_factor_adjusted_debt_value: Fraction,
        remaining_reserve_borrow: Fraction,
        referral_fee_bps: u16,
        has_referrer: bool,
        decimals: u64,
        market_price_f: Fraction,
        borrow_factor_f: Fraction,
    ) -> Result<CalculateBorrowResult> {
        let borrow_amount_f = (max_borrow_factor_adjusted_debt_value * u128::from(decimals)
            / market_price_f
            / borrow_factor_f)
            .min(remaining_reserve_borrow)
            .min(self.liquidity.available_amount.into());
        let (origination_fee, referrer_fee) = self.config.fees.calculate_borrow_fees(
            borrow_amount_f,
            FeeCalculation::Inclusive,
            referral_fee_bps,
            has_referrer,
        )?;
        let borrow_amount: u64 = borrow_amount_f.to_floor();
        let receive_amount = borrow_amount - origination_fee - referrer_fee;

        Ok(CalculateBorrowResult {
            borrow_amount_f,
            receive_amount,
            origination_fee,
            referrer_fee,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn calculate_borrow_exact(
        &self,
        receive_amount: u64,
        max_borrow_factor_adjusted_debt_value: Fraction,
        remaining_reserve_borrow: Fraction,
        referral_fee_bps: u16,
        has_referrer: bool,
        decimals: u64,
        market_price_f: Fraction,
        borrow_factor_f: Fraction,
    ) -> Result<CalculateBorrowResult> {
        let receive_amount_f = Fraction::from(receive_amount);
        let (origination_fee, referrer_fee) = self.config.fees.calculate_borrow_fees(
            receive_amount_f,
            FeeCalculation::Exclusive,
            referral_fee_bps,
            has_referrer,
        )?;
        let borrow_amount_f = receive_amount_f + Fraction::from_num(origination_fee + referrer_fee);
        let borrow_factor_adjusted_debt_value = borrow_amount_f
            .mul(market_price_f)
            .div(u128::from(decimals))
            .mul(borrow_factor_f);

        if borrow_factor_adjusted_debt_value > max_borrow_factor_adjusted_debt_value {
            msg!(
                "Borrow value {} cannot exceed maximum borrow value {}",
                borrow_factor_adjusted_debt_value.to_display(),
                max_borrow_factor_adjusted_debt_value.to_display(),
            );
            return err!(LendingError::BorrowTooLarge);
        }
        if borrow_amount_f > remaining_reserve_borrow {
            msg!(
                "Borrowing {} (after fees: {}) would exceed the reserve's remaining limit {}",
                receive_amount,
                borrow_amount_f.to_display(),
                remaining_reserve_borrow.to_display(),
            );
            return err!(LendingError::BorrowLimitExceeded);
        }
        Ok(CalculateBorrowResult {
            borrow_amount_f,
            receive_amount,
            origination_fee,
            referrer_fee,
        })
    }


    pub fn calculate_repay(
        &self,
        amount_to_repay: u64,
        borrowed_amount: Fraction,
    ) -> CalculateRepayResult {
        let settle_amount = if amount_to_repay == u64::MAX {
            borrowed_amount
        } else {
            min(Fraction::from(amount_to_repay), borrowed_amount)
        };
        let repay_amount = settle_amount.to_ceil();

        CalculateRepayResult {
            settle_amount,
            repay_amount,
        }
    }


    pub fn calculate_redeem_fees(&self) -> u64 {
        min(
            self.liquidity.available_amount,
            Fraction::from_bits(self.liquidity.accumulated_protocol_fees_sf).to_floor(),
        )
    }


    pub fn deposit_limit_crossed(&self) -> bool {
        self.liquidity.total_supply() > Fraction::from(self.config.deposit_limit)
    }


    pub fn borrow_limit_crossed(&self) -> bool {
        self.liquidity.total_borrow() > Fraction::from(self.config.borrow_limit)
    }


    pub fn get_withdraw_referrer_fees(&self, referrer_token_state: &ReferrerTokenState) -> u64 {
        let available_unclaimed_sf = min(
            referrer_token_state.amount_unclaimed_sf,
            self.liquidity.accumulated_referrer_fees_sf,
        );
        let available_unclaimed: u64 = Fraction::from_bits(available_unclaimed_sf).to_floor();
        min(available_unclaimed, self.liquidity.available_amount)
    }





    pub fn is_used(&self, min_initial_deposit_amount: u64) -> bool {
        self.liquidity.available_amount > min_initial_deposit_amount
            || self.liquidity.total_borrow() > Fraction::ZERO
            || self.collateral.mint_total_supply > min_initial_deposit_amount
    }


    pub fn is_usage_blocked(&self) -> bool {
        self.config.deposit_limit == 0 && self.config.borrow_limit == 0
    }
}


pub struct InitReserveParams {

    pub current_slot: Slot,

    pub lending_market: Pubkey,

    pub liquidity: Box<ReserveLiquidity>,

    pub collateral: Box<ReserveCollateral>,

    pub config: Box<ReserveConfig>,
}


#[derive(Debug, PartialEq, Eq)]
#[zero_copy]
#[repr(C)]
pub struct ReserveLiquidity {

    pub mint_pubkey: Pubkey,

    pub supply_vault: Pubkey,

    pub fee_vault: Pubkey,

    pub available_amount: u64,

    pub borrowed_amount_sf: u128,

    pub market_price_sf: u128,

    pub market_price_last_updated_ts: u64,

    pub mint_decimals: u64,



    pub deposit_limit_crossed_timestamp: u64,


    pub borrow_limit_crossed_timestamp: u64,


    pub cumulative_borrow_rate_bsf: BigFractionBytes,

    pub accumulated_protocol_fees_sf: u128,

    pub accumulated_referrer_fees_sf: u128,

    pub pending_referrer_fees_sf: u128,

    pub absolute_referral_rate_sf: u128,

    pub token_program: Pubkey,

    pub padding2: [u64; 51],
    pub padding3: [u128; 32],
}

impl Default for ReserveLiquidity {
    fn default() -> Self {
        Self {
            mint_pubkey: Pubkey::default(),
            supply_vault: Pubkey::default(),
            fee_vault: Pubkey::default(),
            available_amount: 0,
            borrowed_amount_sf: 0,
            cumulative_borrow_rate_bsf: BigFractionBytes::from(BigFraction::from(Fraction::ONE)),
            accumulated_protocol_fees_sf: 0,
            market_price_sf: 0,
            mint_decimals: 0,
            deposit_limit_crossed_timestamp: 0,
            borrow_limit_crossed_timestamp: 0,
            accumulated_referrer_fees_sf: 0,
            pending_referrer_fees_sf: 0,
            absolute_referral_rate_sf: 0,
            market_price_last_updated_ts: 0,
            token_program: Pubkey::default(),
            padding2: [0; 51],
            padding3: [0; 32],
        }
    }
}

impl ReserveLiquidity {

    pub fn new(params: NewReserveLiquidityParams) -> Self {
        let NewReserveLiquidityParams {
            mint_pubkey,
            mint_decimals,
            mint_token_program,
            supply_vault,
            fee_vault,
            market_price_sf,
            initial_amount_deposited_in_reserve,
        } = params;

        Self {
            mint_pubkey,
            mint_decimals: mint_decimals.into(),
            supply_vault,
            fee_vault,
            available_amount: initial_amount_deposited_in_reserve,
            borrowed_amount_sf: 0,
            cumulative_borrow_rate_bsf: BigFractionBytes::from(BigFraction::from(Fraction::ONE)),
            accumulated_protocol_fees_sf: 0,
            market_price_sf,
            deposit_limit_crossed_timestamp: 0,
            borrow_limit_crossed_timestamp: 0,
            accumulated_referrer_fees_sf: 0,
            pending_referrer_fees_sf: 0,
            absolute_referral_rate_sf: 0,
            market_price_last_updated_ts: 0,
            token_program: mint_token_program,
            padding2: [0; 51],
            padding3: [0; 32],
        }
    }


    pub fn total_supply(&self) -> Fraction {
        Fraction::from(self.available_amount) + Fraction::from_bits(self.borrowed_amount_sf)
            - Fraction::from_bits(self.accumulated_protocol_fees_sf)
            - Fraction::from_bits(self.accumulated_referrer_fees_sf)
            - Fraction::from_bits(self.pending_referrer_fees_sf)
    }


    pub fn total_borrow(&self) -> Fraction {
        Fraction::from_bits(self.borrowed_amount_sf)
    }


    pub fn deposit(&mut self, liquidity_amount: u64) -> Result<()> {
        self.available_amount = self
            .available_amount
            .checked_add(liquidity_amount)
            .ok_or(LendingError::MathOverflow)?;
        Ok(())
    }


    pub fn withdraw(&mut self, liquidity_amount: u64) -> Result<()> {
        if liquidity_amount > self.available_amount {
            msg!("Withdraw amount cannot exceed available amount");
            return err!(LendingError::InsufficientLiquidity);
        }
        self.available_amount = self
            .available_amount
            .checked_sub(liquidity_amount)
            .ok_or(LendingError::MathOverflow)?;
        Ok(())
    }


    pub fn borrow(&mut self, borrow_f: Fraction) -> Result<()> {
        let borrow_amount: u64 = borrow_f.to_floor();

        if borrow_amount > self.available_amount {
            msg!("Borrow amount cannot exceed available amount borrow_amount={}, available_amount={}", borrow_amount, self.available_amount);
            return err!(LendingError::InsufficientLiquidity);
        }

        let borrowed_amount_f = Fraction::from_bits(self.borrowed_amount_sf);

        self.available_amount -= borrow_amount;
        self.borrowed_amount_sf = (borrowed_amount_f + borrow_f).to_bits();

        Ok(())
    }


    pub fn repay(&mut self, repay_amount: u64, settle_amount: Fraction) -> LendingResult<()> {
        self.available_amount = self
            .available_amount
            .checked_add(repay_amount)
            .ok_or(LendingError::MathOverflow)?;
        let borrowed_amount_f = Fraction::from_bits(self.borrowed_amount_sf);
        let safe_settle_amount = min(settle_amount, borrowed_amount_f);
        self.borrowed_amount_sf = borrowed_amount_f
            .checked_sub(safe_settle_amount)
            .ok_or_else(|| {
                msg!(
                    "Borrowed amount {} cannot be less than settle amount {}",
                    borrowed_amount_f.to_display(),
                    safe_settle_amount.to_display()
                );
                LendingError::MathOverflow
            })?
            .to_bits();

        Ok(())
    }


    pub fn redeem_fees(&mut self, withdraw_amount: u64) -> Result<()> {
        self.available_amount = self
            .available_amount
            .checked_sub(withdraw_amount)
            .ok_or_else(|| {
                msg!(
                    "Available amount {} cannot be less than withdraw amount {withdraw_amount}",
                    self.available_amount
                );
                LendingError::MathOverflow
            })?;
        let accumulated_protocol_fees_f = Fraction::from_bits(self.accumulated_protocol_fees_sf);
        let withdraw_amount_f = Fraction::from_num(withdraw_amount);
        self.accumulated_protocol_fees_sf = accumulated_protocol_fees_f
            .checked_sub(withdraw_amount_f)
            .ok_or_else(|| {
                msg!(
                    "Accumulated protocol fees {} cannot be less than withdraw amount {}",
                    accumulated_protocol_fees_f.to_display(),
                    withdraw_amount_f.to_display()
                );
                error!(LendingError::MathOverflow)
            })?
            .to_bits();

        Ok(())
    }


    pub fn utilization_rate(&self) -> Fraction {
        let total_supply = self.total_supply();
        if total_supply == Fraction::ZERO {
            return Fraction::ZERO;
        }
        Fraction::from_bits(self.borrowed_amount_sf) / total_supply
    }


    pub fn mint_factor(&self) -> u64 {
        ten_pow(usize::try_from(self.mint_decimals).expect("mint decimals is expected to be <20"))
    }








    fn compound_interest(
        &mut self,
        current_borrow_rate: Fraction,
        host_fixed_interest_rate: Fraction,
        slots_elapsed: u64,
        protocol_take_rate: Fraction,
        referral_rate: Fraction,
    ) -> LendingResult<()> {
       
        let previous_cumulative_borrow_rate = BigFraction::from(self.cumulative_borrow_rate_bsf);
        let previous_debt_f = Fraction::from_bits(self.borrowed_amount_sf);
        let acc_protocol_fees_f = Fraction::from_bits(self.accumulated_protocol_fees_sf);

       
        let compounded_interest_rate = approximate_compounded_interest(
            current_borrow_rate + host_fixed_interest_rate,
            slots_elapsed,
        );
       
        let compounded_fixed_rate =
            approximate_compounded_interest(host_fixed_interest_rate, slots_elapsed);

        let new_cumulative_borrow_rate: BigFraction =
            previous_cumulative_borrow_rate * BigFraction::from(compounded_interest_rate);

        let new_debt_f = previous_debt_f * compounded_interest_rate;

       
       

       
       

       
       

       
       

        let fixed_host_fee = (previous_debt_f * compounded_fixed_rate) - previous_debt_f;
        let net_new_variable_debt_f = new_debt_f - previous_debt_f - fixed_host_fee;

        let variable_protocol_fee_f = net_new_variable_debt_f * protocol_take_rate;
        let absolute_referral_rate = protocol_take_rate * referral_rate;
        let max_referrers_fees_f = net_new_variable_debt_f * absolute_referral_rate;

        let new_acc_protocol_fees_f =
            acc_protocol_fees_f + fixed_host_fee + variable_protocol_fee_f - max_referrers_fees_f;

       
        self.cumulative_borrow_rate_bsf = new_cumulative_borrow_rate.into();
        self.pending_referrer_fees_sf += max_referrers_fees_f.to_bits();
        self.accumulated_protocol_fees_sf = new_acc_protocol_fees_f.to_bits();
        self.borrowed_amount_sf = new_debt_f.to_bits();
        self.absolute_referral_rate_sf = absolute_referral_rate.to_bits();

        Ok(())
    }



    pub fn forgive_debt(&mut self, liquidity_amount: Fraction) {
        let amt = Fraction::from_bits(self.borrowed_amount_sf);
        let new_amt = amt - liquidity_amount;
        self.borrowed_amount_sf = new_amt.to_bits();
    }


    pub fn withdraw_referrer_fees(
        &mut self,
        withdraw_amount: u64,
        referrer_token_state: &mut ReferrerTokenState,
    ) -> Result<()> {
        self.available_amount = self
            .available_amount
            .checked_sub(withdraw_amount)
            .ok_or_else(|| {
                msg!("Available amount {} cannot be less than withdraw amount on referrer fees {withdraw_amount}", self.available_amount);
                LendingError::MathOverflow
            })?;

        let accumulated_referrer_fees_f = Fraction::from_bits(self.accumulated_referrer_fees_sf);

        let withdraw_amount_f = Fraction::from_num(withdraw_amount);

        let new_accumulated_referrer_fees_f = accumulated_referrer_fees_f
            .checked_sub(withdraw_amount_f)
            .ok_or_else(|| {
                msg!(
                    "Accumulated referrer fees {} cannot be less than withdraw amount {}",
                    accumulated_referrer_fees_f.to_display(),
                    withdraw_amount_f.to_display()
                );
                error!(LendingError::MathOverflow)
            })?;

        self.accumulated_referrer_fees_sf = new_accumulated_referrer_fees_f.to_bits();

        let referrer_amount_unclaimed_f =
            Fraction::from_bits(referrer_token_state.amount_unclaimed_sf);

        let new_referrer_amount_unclaimed_f = referrer_amount_unclaimed_f
            .checked_sub(withdraw_amount_f)
            .ok_or_else(|| {
                msg!(
                    "Unclaimed referrer fees {} cannot be less than withdraw amount {}",
                    referrer_amount_unclaimed_f.to_display(),
                    withdraw_amount_f.to_display()
                );
                error!(LendingError::MathOverflow)
            })?;

        referrer_token_state.amount_unclaimed_sf = new_referrer_amount_unclaimed_f.to_bits();

        Ok(())
    }

    pub fn get_market_price(&self) -> Fraction {
        Fraction::from_bits(self.market_price_sf)
    }
}


pub struct NewReserveLiquidityParams {

    pub mint_pubkey: Pubkey,

    pub mint_decimals: u8,

    pub mint_token_program: Pubkey,

    pub supply_vault: Pubkey,

    pub fee_vault: Pubkey,

    pub market_price_sf: u128,

    pub initial_amount_deposited_in_reserve: u64,
}


#[derive(Debug, Default, PartialEq, Eq)]
#[zero_copy]
#[repr(C)]
pub struct ReserveCollateral {

    pub mint_pubkey: Pubkey,

    pub mint_total_supply: u64,

    pub supply_vault: Pubkey,
    pub padding1: [u128; 32],
    pub padding2: [u128; 32],
}

impl ReserveCollateral {

    pub fn new(params: NewReserveCollateralParams) -> Self {
        let NewReserveCollateralParams {
            mint_pubkey,
            supply_vault,
            initial_collateral_supply,
        } = params;
        Self {
            mint_pubkey,
            mint_total_supply: initial_collateral_supply,
            supply_vault,
            padding1: [0; 32],
            padding2: [0; 32],
        }
    }


    pub fn mint(&mut self, collateral_amount: u64) -> Result<()> {
        self.mint_total_supply = self
            .mint_total_supply
            .checked_add(collateral_amount)
            .ok_or(LendingError::MathOverflow)?;
        Ok(())
    }


    pub fn burn(&mut self, collateral_amount: u64) -> Result<()> {
        self.mint_total_supply = self
            .mint_total_supply
            .checked_sub(collateral_amount)
            .ok_or_else(|| {
                msg!(
                    "Mint total supply {} cannot be less than collateral amount {}",
                    self.mint_total_supply,
                    collateral_amount
                );
                LendingError::MathOverflow
            })?;
        Ok(())
    }


    fn exchange_rate(&self, total_liquidity: Fraction) -> CollateralExchangeRate {
        if self.mint_total_supply == 0 || total_liquidity == Fraction::ZERO {
            INITIAL_COLLATERAL_RATE
        } else {
            CollateralExchangeRate::from_supply_and_liquidity(
                self.mint_total_supply,
                total_liquidity,
            )
        }
    }
}


#[derive(Clone, Copy, Debug)]
pub struct CollateralExchangeRate {
    collateral_supply: u128,
    liquidity: Fraction,
}

impl Default for CollateralExchangeRate {
    fn default() -> Self {
        Self::ONE
    }
}

impl CollateralExchangeRate {
    pub const ONE: Self = Self {
        collateral_supply: 1,
        liquidity: Fraction::ONE,
    };

    pub fn from_supply_and_liquidity(collateral_supply: u64, liquidity: Fraction) -> Self {
        Self {
            collateral_supply: collateral_supply.into(),
            liquidity,
        }
    }


    pub fn collateral_to_liquidity(&self, collateral_amount: u64) -> u64 {
        self.fraction_collateral_to_liquidity(collateral_amount.into())
            .to_floor()
    }



    pub fn collateral_to_liquidity_ceil(&self, collateral_amount: u64) -> u64 {
        let collateral_amount_u256 = U256::from(collateral_amount);
        let liquidity_sbf = BigFraction::from(self.liquidity).0;
        let collateral_supply_u256 = U256::from(self.collateral_supply);

        let liquidity_ceil_sbf = collateral_amount_u256
            .checked_mul(liquidity_sbf)
            .and_then(|res| res.checked_add(collateral_supply_u256 - U256::one()))
            .and_then(|res| res.checked_div(collateral_supply_u256))
            .expect("collateral_to_liquidity_ceil: liquidity_amount overflow on calculation");

        let liquidity_ceil_bf = BigFraction(liquidity_ceil_sbf);

        let liquidity_ceil_f = Fraction::try_from(liquidity_ceil_bf).expect(
            "collateral_to_liquidity_ceil: liquidity_amount overflow on fraction conversion",
        );

        liquidity_ceil_f.to_ceil()
    }


    pub fn fraction_collateral_to_liquidity(&self, collateral_amount: Fraction) -> Fraction {
        (BigFraction::from(collateral_amount) * BigFraction::from(self.liquidity)
            / self.collateral_supply)
            .try_into()
           
           
            .expect("fraction_collateral_to_liquidity: liquidity_amount overflow")
    }


    pub fn fraction_liquidity_to_collateral(&self, liquidity_amount: Fraction) -> Fraction {
        (BigFraction::from(liquidity_amount) * self.collateral_supply / self.liquidity)
            .try_into()
           
           
            .expect("fraction_liquidity_to_collateral: collateral_amount overflow")
    }


    pub fn fraction_liquidity_to_collateral_ceil(&self, liquidity_amount: Fraction) -> Fraction {
        (((BigFraction::from(liquidity_amount) * self.collateral_supply)
            + BigFraction::from(self.liquidity - Fraction::DELTA))
            / self.liquidity)
            .try_into()
           
           
            .expect("fraction_liquidity_to_collateral_ceil: collateral_amount overflow")
    }


    pub fn liquidity_to_collateral_fraction(&self, liquidity_amount: u64) -> Fraction {
        (BigFraction::from_num(self.collateral_supply * u128::from(liquidity_amount))
            / self.liquidity)
            .try_into()
           
           
            .expect("liquidity_to_collateral_fraction: collateral_amount overflow")
    }


    pub fn liquidity_to_collateral(&self, liquidity_amount: u64) -> u64 {
        let collateral_f = self.liquidity_to_collateral_fraction(liquidity_amount);
        collateral_f.try_to_floor().unwrap_or_else(|| {
           
           
            #[cfg(target_os = "solana")]
            panic!(
                "liquidity_to_collateral: collateral_amount overflow, collateral_f_scaled: {}",
                collateral_f.to_bits()
            );
            #[cfg(not(target_os = "solana"))]
            panic!(
                "liquidity_to_collateral: collateral_amount overflow, collateral_f: {}",
                collateral_f
            );
        })
    }


    pub fn liquidity_to_collateral_ceil(&self, liquidity_amount: u64) -> u64 {
        self.liquidity_to_collateral_fraction(liquidity_amount)
            .to_ceil()
    }
}


pub struct NewReserveCollateralParams {

    pub mint_pubkey: Pubkey,

    pub supply_vault: Pubkey,

    pub initial_collateral_supply: u64,
}

static_assertions::const_assert_eq!(RESERVE_CONFIG_SIZE, std::mem::size_of::<ReserveConfig>());
static_assertions::const_assert_eq!(0, std::mem::size_of::<ReserveConfig>() % 8);

#[derive(BorshDeserialize, BorshSerialize, PartialEq, Eq, Derivative, Default)]
#[derivative(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[zero_copy]
#[repr(C)]
pub struct ReserveConfig {

    pub status: u8,

    pub asset_tier: u8,

    pub host_fixed_interest_rate_bps: u16,

    pub min_deleveraging_bonus_bps: u16,

    #[cfg_attr(feature = "serde", serde(skip_serializing, default))]
    #[derivative(Debug = "ignore")]
    pub reserved_1: [u8; 7],

    pub protocol_order_execution_fee_pct: u8,

    pub protocol_take_rate_pct: u8,

    pub protocol_liquidation_fee_pct: u8,


    pub loan_to_value_pct: u8,

    pub liquidation_threshold_pct: u8,

    pub min_liquidation_bonus_bps: u16,

    pub max_liquidation_bonus_bps: u16,

    pub bad_debt_liquidation_bonus_bps: u16,



    pub deleveraging_margin_call_period_secs: u64,


    pub deleveraging_threshold_decrease_bps_per_day: u64,

    pub fees: ReserveFees,

    pub borrow_rate_curve: BorrowRateCurve,

    pub borrow_factor_pct: u64,


    pub deposit_limit: u64,

    pub borrow_limit: u64,

    pub token_info: TokenInfo,


    pub deposit_withdrawal_cap: WithdrawalCaps,

    pub debt_withdrawal_cap: WithdrawalCaps,

    pub elevation_groups: [u8; 20],
    pub disable_usage_as_coll_outside_emode: u8,


    pub utilization_limit_block_borrowing_above_pct: u8,






    #[cfg_attr(feature = "serde", serde(with = "serde_bool_u8"))]
    pub autodeleverage_enabled: u8,






    #[cfg_attr(feature = "serde", serde(with = "serde_bool_u8"))]
    pub proposer_authority_locked: u8,




    pub borrow_limit_outside_elevation_group: u64,





    pub borrow_limit_against_this_collateral_in_elevation_group: [u64; 32],



    pub deleveraging_bonus_increase_bps_per_day: u64,
}

impl ReserveConfig {

    pub fn get_asset_tier(&self) -> AssetTier {
        AssetTier::try_from(self.asset_tier).unwrap()
    }


    pub fn get_borrow_factor(&self) -> Fraction {
        max(
            Fraction::ONE,
            Fraction::from_percent(self.borrow_factor_pct),
        )
    }


    pub fn status(&self) -> ReserveStatus {
        ReserveStatus::try_from(self.status).unwrap()
    }

    pub fn is_autodeleverage_enabled(&self) -> bool {
        self.autodeleverage_enabled != false as u8
    }
}

#[repr(u8)]
#[derive(
    AnchorSerialize,
    AnchorDeserialize,
    TryFromPrimitive,
    IntoPrimitive,
    PartialEq,
    Eq,
    Debug,
    Clone,
    Copy,
)]
pub enum ReserveStatus {
    Active = 0,
    Obsolete = 1,
    Hidden = 2,
}


#[derive(BorshDeserialize, BorshSerialize, PartialEq, Eq, Default, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[zero_copy]
#[repr(C)]
pub struct WithdrawalCaps {
    pub config_capacity: i64,
    #[cfg_attr(
        all(feature = "serde", not(feature = "serialize_caps_interval_values")),
        serde(skip)
    )]
    pub current_total: i64,
    #[cfg_attr(
        all(feature = "serde", not(feature = "serialize_caps_interval_values")),
        serde(skip)
    )]
    pub last_interval_start_timestamp: u64,
    pub config_interval_length_seconds: u64,
}






#[derive(BorshDeserialize, BorshSerialize, Default, PartialEq, Eq, Derivative)]
#[derivative(Debug)]
#[zero_copy]
#[repr(C)]
pub struct ReserveFees {






    pub origination_fee_sf: u64,


    pub flash_loan_fee_sf: u64,

    #[derivative(Debug = "ignore")]
    pub padding: [u8; 8],
}

#[cfg(feature = "serde")]
mod serde_reserve_fees {
    use std::{fmt, result::Result};

    use serde::{
        de::{self, Deserialize, Deserializer, MapAccess, SeqAccess, Visitor},
        ser::Serialize,
    };

    use super::*;

    impl<'de> Deserialize<'de> for ReserveFees {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: Deserializer<'de>,
        {
            #[derive(serde::Deserialize)]
            #[serde(field_identifier, rename_all = "snake_case")]
            enum Field {
                OriginationFee,
                FlashLoanFee,
            }

            struct ReserveFeesVisitor;
            impl<'de> Visitor<'de> for ReserveFeesVisitor {
                type Value = ReserveFees;

                fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                    formatter.write_str("struct ReserveFees")
                }

                fn visit_seq<V>(self, mut seq: V) -> Result<ReserveFees, V::Error>
                where
                    V: SeqAccess<'de>,
                {
                    let origination_fee_sf = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                    let flash_loan_fee_sf = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                    Ok(ReserveFees {
                        origination_fee_sf,
                        flash_loan_fee_sf,
                        padding: [0; 8],
                    })
                }

                fn visit_map<V>(self, mut map: V) -> Result<ReserveFees, V::Error>
                where
                    V: MapAccess<'de>,
                {
                    let mut origination_fee_f: Option<Fraction> = None;
                    let mut flash_loan_fee_f: Option<Fraction> = None;
                    while let Some(key) = map.next_key()? {
                        match key {
                            Field::OriginationFee => {
                                if origination_fee_f.is_some() {
                                    return Err(de::Error::duplicate_field("origination_fee"));
                                }
                                origination_fee_f = Some(map.next_value()?);
                            }
                            Field::FlashLoanFee => {
                                if flash_loan_fee_f.is_some() {
                                    return Err(de::Error::duplicate_field("flash_loan_fee"));
                                }

                               

                                let flash_loan_fee_str: Option<String> = map.next_value()?;
                                match flash_loan_fee_str.as_deref() {
                                    Some("disabled") => {
                                        flash_loan_fee_f = None;
                                    }
                                    Some(x) => {
                                        flash_loan_fee_f =
                                            Some(Fraction::from_str(x).map_err(|_| {
                                                de::Error::custom(
                                                    "flash_loan_fee must be a fraction",
                                                )
                                            })?);
                                    }
                                    None => {
                                        return Err(de::Error::custom(
                                            "flash_loan_fee must be a fraction or 'disabled'",
                                        ));
                                    }
                                }
                            }
                        }
                    }

                    let origination_fee_f = origination_fee_f
                        .ok_or_else(|| de::Error::missing_field("origination_fee"))?;
                    let flash_loan_fee_f =
                        flash_loan_fee_f.unwrap_or(Fraction::from_bits(u64::MAX.into()));
                    Ok(ReserveFees {
                        origination_fee_sf: u64::try_from(origination_fee_f.to_bits()).map_err(
                            |_| de::Error::custom("origination_fee does not fit in u64"),
                        )?,
                        flash_loan_fee_sf: u64::try_from(flash_loan_fee_f.to_bits())
                            .map_err(|_| de::Error::custom("flash_loan_fee does not fit in u64"))?,
                        padding: [0; 8],
                    })
                }
            }

            const FIELDS: &[&str] = &["origination_fee", "flash_loan_fee"];
            deserializer.deserialize_struct("ReserveFees", FIELDS, ReserveFeesVisitor)
        }
    }

    impl Serialize for ReserveFees {
        fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
        where
            S: serde::Serializer,
        {
            #[derive(serde::Serialize)]
            struct ReserveFeesSerde {
                origination_fee: Fraction,
                flash_loan_fee: String,
            }

            let origination_fee_f = Fraction::from_bits(self.origination_fee_sf.into());

            let flash_loan_fee = if self.flash_loan_fee_sf == u64::MAX {
                "disabled".to_string()
            } else {
                Fraction::from_bits(self.flash_loan_fee_sf.into()).to_string()
            };

            let fees = ReserveFeesSerde {
                origination_fee: origination_fee_f,
                flash_loan_fee,
            };
            fees.serialize(serializer)
        }
    }
}

impl ReserveFees {

    pub fn calculate_borrow_fees(
        &self,
        borrow_amount: Fraction,
        fee_calculation: FeeCalculation,
        referral_fee_bps: u16,
        has_referrer: bool,
    ) -> Result<(u64, u64)> {
        self.calculate_fees(
            borrow_amount,
            self.origination_fee_sf,
            fee_calculation,
            referral_fee_bps,
            has_referrer,
        )
    }


    pub fn calculate_flash_loan_fees(
        &self,
        flash_loan_amount_f: Fraction,
        referral_fee_bps: u16,
        has_referrer: bool,
    ) -> Result<(u64, u64)> {
        let (protocol_fee, referral_fee) = self.calculate_fees(
            flash_loan_amount_f,
            self.flash_loan_fee_sf,
            FeeCalculation::Exclusive,
            referral_fee_bps,
            has_referrer,
        )?;

        Ok((protocol_fee, referral_fee))
    }



    fn calculate_fees(
        &self,
        amount: Fraction,
        fee_sf: u64,
        fee_calculation: FeeCalculation,
        referral_fee_bps: u16,
        has_referrer: bool,
    ) -> Result<(u64, u64)> {
        let origination_fee_rate = Fraction::from_bits(fee_sf.into());
        let referral_fee_rate = Fraction::from_bps(referral_fee_bps);
        if origination_fee_rate > Fraction::ZERO && amount > Fraction::ZERO {
            let need_to_assess_referral_fee = referral_fee_rate > Fraction::ZERO && has_referrer;
            let minimum_fee = 1u64;

            let origination_fee_amount = match fee_calculation {
               
                FeeCalculation::Exclusive => amount.mul(origination_fee_rate),
               
                FeeCalculation::Inclusive => {
                    let origination_fee_rate =
                        origination_fee_rate.div(origination_fee_rate.add(Fraction::ONE));
                    amount.mul(origination_fee_rate)
                }
            };

            let origination_fee_f = origination_fee_amount.max(minimum_fee.into());
            if origination_fee_f >= amount {
                msg!("Borrow amount is too small to receive liquidity after fees");
                return err!(LendingError::BorrowTooSmall);
            }

            let origination_fee: u64 = origination_fee_f.to_round();
            let referral_fee = if need_to_assess_referral_fee {
               
                if referral_fee_bps == 10_000 {
                    origination_fee
                } else {
                    let referral_fee_f = origination_fee_f * referral_fee_rate;
                    referral_fee_f.to_floor::<u64>()
                }
            } else {
                0
            };

            let protocol_fee = origination_fee - referral_fee;

            Ok((protocol_fee, referral_fee))
        } else {
            Ok((0, 0))
        }
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Debug, PartialEq, Eq)]

pub enum FeeCalculation {

    Exclusive,

    Inclusive,
}

#[derive(
    AnchorSerialize,
    AnchorDeserialize,
    Debug,
    PartialEq,
    Eq,
    num_enum::IntoPrimitive,
    num_enum::TryFromPrimitive,
)]
#[repr(u8)]
pub enum AssetTier {
    Regular = 0,
    IsolatedCollateral = 1,
    IsolatedDebt = 2,
}


















pub fn approximate_compounded_interest(rate: Fraction, elapsed_slots: u64) -> Fraction {
    let base = rate / u128::from(SLOTS_PER_YEAR);

    match elapsed_slots {
        0 => return Fraction::ONE,
        1 => return Fraction::ONE + base,
        2 => return (Fraction::ONE + base) * (Fraction::ONE + base),
        3 => return (Fraction::ONE + base) * (Fraction::ONE + base) * (Fraction::ONE + base),
        4 => {
            let pow_two = (Fraction::ONE + base) * (Fraction::ONE + base);
            return pow_two * pow_two;
        }
        _ => (),
    }

    let exp: u128 = elapsed_slots.into();
   
    let exp_minus_one = exp.wrapping_sub(1);
    let exp_minus_two = exp.wrapping_sub(2);

    let base_power_two = base * base;
    let base_power_three = base_power_two * base;

    let first_term = base * exp;

    let second_term = (base_power_two * exp * exp_minus_one) / 2;

    let third_term = (base_power_three * exp * exp_minus_one * exp_minus_two) / 6;

    Fraction::ONE + first_term + second_term + third_term
}
















