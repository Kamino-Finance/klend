use std::{
    cmp::{max, min},
    ops::{Add, Div, Mul},
};

use anchor_lang::{
    account, err,
    prelude::{msg, Pubkey, *},
    solana_program::clock::Slot,
    AnchorDeserialize, AnchorSerialize, Result,
};
use derivative::Derivative;
use num_enum::{IntoPrimitive, TryFromPrimitive};
#[cfg(feature = "serde")]
use serde;

use super::{LastUpdate, TokenInfo};
use crate::{
    fraction::FractionExtra,
    utils::{
        borrow_rate_curve::BorrowRateCurve, BigFraction, Fraction, INITIAL_COLLATERAL_RATE,
        PROGRAM_VERSION, RESERVE_CONFIG_SIZE, RESERVE_SIZE, SLOTS_PER_YEAR,
    },
    CalculateBorrowResult, CalculateRepayResult, LendingError, LendingResult, ReferrerTokenState,
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
    pub config_padding: [u64; 150],

    #[derivative(Debug = "ignore")]
    pub padding: [u64; 240],
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
            reserve_liquidity_padding: [0; 150],
            reserve_collateral_padding: [0; 150],
            config_padding: [0; 150],
            padding: [0; 240],
        }
    }
}

#[derive(TryFromPrimitive, PartialEq, Eq, Clone, Copy, Debug, strum::EnumIter)]
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

    pub fn add_farm(&mut self, farm_state: &Pubkey, mode: ReserveFarmKind) {
        match mode {
            ReserveFarmKind::Collateral => self.farm_collateral = *farm_state,
            ReserveFarmKind::Debt => self.farm_debt = *farm_state,
        }
    }

    pub fn get_farm(&self, mode: ReserveFarmKind) -> Pubkey {
        match mode {
            ReserveFarmKind::Collateral => self.farm_collateral,
            ReserveFarmKind::Debt => self.farm_debt,
        }
    }

    pub fn deposit_liquidity(&mut self, liquidity_amount: u64) -> Result<u64> {
        let collateral_amount = self
            .collateral_exchange_rate()?
            .liquidity_to_collateral(liquidity_amount);

        self.liquidity.deposit(liquidity_amount)?;
        self.collateral.mint(collateral_amount)?;

        Ok(collateral_amount)
    }

    pub fn redeem_collateral(&mut self, collateral_amount: u64) -> Result<u64> {
        let collateral_exchange_rate = self.collateral_exchange_rate()?;

        let liquidity_amount =
            collateral_exchange_rate.collateral_to_liquidity(collateral_amount)?;

        self.collateral.burn(collateral_amount)?;
        self.liquidity.withdraw(liquidity_amount)?;

        Ok(liquidity_amount)
    }

    pub fn current_borrow_rate(&self) -> Result<Fraction> {
        let utilization_rate = self.liquidity.utilization_rate()?;

        self.config
            .borrow_rate_curve
            .get_borrow_rate(utilization_rate)
    }

    pub fn borrow_factor_f(&self, elevation_group: u8) -> Fraction {
        if elevation_group == 0 {
            Fraction::from_percent(self.config.borrow_factor_pct)
        } else {
            Fraction::ONE
        }
    }

    pub fn collateral_exchange_rate(&self) -> LendingResult<CollateralExchangeRate> {
        let total_liquidity = self.liquidity.total_supply()?;
        self.collateral.exchange_rate(total_liquidity)
    }

    pub fn accrue_interest(&mut self, current_slot: Slot, referral_fee_bps: u16) -> Result<()> {
        let slots_elapsed = self.last_update.slots_elapsed(current_slot)?;
        if slots_elapsed > 0 {
            let current_borrow_rate = self.current_borrow_rate()?;
            let protocol_take_rate = Fraction::from_percent(self.config.protocol_take_rate_pct);
            let referral_rate = Fraction::from_bps(referral_fee_bps);

            self.liquidity.compound_interest(
                current_borrow_rate,
                slots_elapsed,
                protocol_take_rate,
                referral_rate,
            )?;
        }

        Ok(())
    }

    pub fn update_deposit_limit_crossed_slot(&mut self, current_slot: Slot) -> Result<()> {
        if self.deposit_limit_crossed()? {
            if self.liquidity.deposit_limit_crossed_slot == 0 {
                self.liquidity.deposit_limit_crossed_slot = current_slot;
            }
        } else {
            self.liquidity.deposit_limit_crossed_slot = 0;
        }
        Ok(())
    }

    pub fn update_borrow_limit_crossed_slot(&mut self, current_slot: Slot) -> Result<()> {
        if self.borrow_limit_crossed()? {
            if self.liquidity.borrow_limit_crossed_slot == 0 {
                self.liquidity.borrow_limit_crossed_slot = current_slot;
            }
        } else {
            self.liquidity.borrow_limit_crossed_slot = 0;
        }
        Ok(())
    }

    pub fn calculate_borrow(
        &self,
        amount_to_borrow: u64,
        max_borrow_factor_adjusted_debt_value: Fraction,
        remaining_reserve_borrow: Fraction,
        referral_fee_bps: u16,
        elevation_group: u8,
    ) -> Result<CalculateBorrowResult> {
        let decimals = 10u64
            .checked_pow(self.liquidity.mint_decimals as u32)
            .ok_or(LendingError::MathOverflow)?;
        let market_price_f = self.liquidity.get_market_price_f();

        if amount_to_borrow == u64::MAX {
            let borrow_amount_f = (max_borrow_factor_adjusted_debt_value * u128::from(decimals)
                / market_price_f
                / self.borrow_factor_f(elevation_group))
            .min(remaining_reserve_borrow)
            .min(self.liquidity.available_amount.into());
            let (borrow_fee, referrer_fee) = self.config.fees.calculate_borrow_fees(
                borrow_amount_f,
                FeeCalculation::Inclusive,
                referral_fee_bps,
            )?;
            let borrow_amount: u64 = borrow_amount_f.to_floor();
            let receive_amount = borrow_amount - borrow_fee;

            Ok(CalculateBorrowResult {
                borrow_amount_f,
                receive_amount,
                borrow_fee,
                referrer_fee,
            })
        } else {
            let receive_amount = amount_to_borrow;
            let mut borrow_amount_f = Fraction::from(receive_amount);
            let (borrow_fee, referrer_fee) = self.config.fees.calculate_borrow_fees(
                borrow_amount_f,
                FeeCalculation::Exclusive,
                referral_fee_bps,
            )?;

            borrow_amount_f += Fraction::from_num(borrow_fee);
            let borrow_factor_adjusted_debt_value = borrow_amount_f
                .mul(market_price_f)
                .div(u128::from(decimals))
                .mul(self.borrow_factor_f(elevation_group));
            if borrow_factor_adjusted_debt_value > max_borrow_factor_adjusted_debt_value {
                msg!("Borrow value cannot exceed maximum borrow value, borrow borrow_factor_adjusted_debt_value: {}, max_borrow_factor_adjusted_debt_value: {}",
                    borrow_factor_adjusted_debt_value, max_borrow_factor_adjusted_debt_value);
                return err!(LendingError::BorrowTooLarge);
            }

            Ok(CalculateBorrowResult {
                borrow_amount_f,
                receive_amount,
                borrow_fee,
                referrer_fee,
            })
        }
    }

    pub fn calculate_repay(
        &self,
        amount_to_repay: u64,
        borrowed_amount: Fraction,
    ) -> LendingResult<CalculateRepayResult> {
        let settle_amount_f = if amount_to_repay == u64::MAX {
            borrowed_amount
        } else {
            let amount_to_repay_f = Fraction::from(amount_to_repay);
            min(amount_to_repay_f, borrowed_amount)
        };
        let repay_amount = settle_amount_f.to_ceil();

        Ok(CalculateRepayResult {
            settle_amount_f,
            repay_amount,
        })
    }

    pub fn calculate_redeem_fees(&self) -> Result<u64> {
        Ok(min(
            self.liquidity.available_amount,
            Fraction::from_bits(self.liquidity.accumulated_protocol_fees_sf).to_floor(),
        ))
    }

    pub fn deposit_limit_crossed(&self) -> Result<bool> {
        let crossed = self.liquidity.total_supply()? > Fraction::from(self.config.deposit_limit);
        Ok(crossed)
    }

    pub fn borrow_limit_crossed(&self) -> Result<bool> {
        let crossed = self.liquidity.total_borrow() > Fraction::from(self.config.borrow_limit);
        Ok(crossed)
    }

    pub fn get_withdraw_referrer_fees(
        &self,
        referrer_token_state: &ReferrerTokenState,
    ) -> Result<u64> {
        let available_unclaimed_sf = min(
            referrer_token_state.amount_unclaimed_sf,
            self.liquidity.accumulated_referrer_fees_sf,
        );
        let available_unclaimed: u64 = Fraction::from_bits(available_unclaimed_sf).to_floor();
        Ok(min(available_unclaimed, self.liquidity.available_amount))
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

    pub deposit_limit_crossed_slot: u64,
    pub borrow_limit_crossed_slot: u64,

    pub cumulative_borrow_rate_bsf: BigFractionBytes,
    pub accumulated_protocol_fees_sf: u128,
    pub accumulated_referrer_fees_sf: u128,
    pub pending_referrer_fees_sf: u128,
    pub absolute_referral_rate_sf: u128,

    pub padding2: [u64; 55],
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
            deposit_limit_crossed_slot: 0,
            borrow_limit_crossed_slot: 0,
            accumulated_referrer_fees_sf: 0,
            pending_referrer_fees_sf: 0,
            absolute_referral_rate_sf: 0,
            market_price_last_updated_ts: 0,
            padding2: [0; 55],
            padding3: [0; 32],
        }
    }
}

impl ReserveLiquidity {
    pub fn new(params: NewReserveLiquidityParams) -> Self {
        Self {
            mint_pubkey: params.mint_pubkey,
            mint_decimals: params.mint_decimals as u64,
            supply_vault: params.supply_vault,
            fee_vault: params.fee_vault,
            available_amount: 0,
            borrowed_amount_sf: 0,
            cumulative_borrow_rate_bsf: BigFractionBytes::from(BigFraction::from(Fraction::ONE)),
            accumulated_protocol_fees_sf: 0,
            market_price_sf: params.market_price_sf,
            deposit_limit_crossed_slot: 0,
            borrow_limit_crossed_slot: 0,
            accumulated_referrer_fees_sf: 0,
            pending_referrer_fees_sf: 0,
            absolute_referral_rate_sf: 0,
            market_price_last_updated_ts: 0,
            padding2: [0; 55],
            padding3: [0; 32],
        }
    }

    pub fn total_supply(&self) -> LendingResult<Fraction> {
        Ok(
            Fraction::from(self.available_amount) + Fraction::from_bits(self.borrowed_amount_sf)
                - Fraction::from_bits(self.accumulated_protocol_fees_sf)
                - Fraction::from_bits(self.accumulated_referrer_fees_sf)
                - Fraction::from_bits(self.pending_referrer_fees_sf),
        )
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
                msg!("Borrowed amount cannot be less than settle amount");
                LendingError::MathOverflow
            })?
            .to_bits();

        Ok(())
    }

    pub fn redeem_fees(&mut self, withdraw_amount: u64) -> Result<()> {
        self.available_amount = self
            .available_amount
            .checked_sub(withdraw_amount)
            .ok_or(LendingError::MathOverflow)?;
        let accumulated_protocol_fees_f = Fraction::from_bits(self.accumulated_protocol_fees_sf);
        let withdraw_amount_f = Fraction::from_num(withdraw_amount);
        self.accumulated_protocol_fees_sf = accumulated_protocol_fees_f
            .checked_sub(withdraw_amount_f)
            .ok_or_else(|| {
                msg!("Accumulated protocol fees cannot be less than withdraw amount");
                error!(LendingError::MathOverflow)
            })?
            .to_bits();

        Ok(())
    }

    pub fn utilization_rate(&self) -> LendingResult<Fraction> {
        let total_supply = self.total_supply()?;
        if total_supply == Fraction::ZERO {
            return Ok(Fraction::ZERO);
        }
        Ok(Fraction::from_bits(self.borrowed_amount_sf) / total_supply)
    }

    fn compound_interest(
        &mut self,
        current_borrow_rate: Fraction,
        slots_elapsed: u64,
        protocol_take_rate: Fraction,
        referral_rate: Fraction,
    ) -> LendingResult<()> {
        let previous_cumulative_borrow_rate = BigFraction::from(self.cumulative_borrow_rate_bsf);
        let previous_debt_f = Fraction::from_bits(self.borrowed_amount_sf);
        let acc_protocol_fees_f = Fraction::from_bits(self.accumulated_protocol_fees_sf);

        let compounded_interest_rate =
            approximate_compounded_interest(current_borrow_rate, slots_elapsed);

        let new_cumulative_borrow_rate: BigFraction =
            previous_cumulative_borrow_rate * BigFraction::from(compounded_interest_rate);

        let new_debt_f = previous_debt_f * compounded_interest_rate;
        let net_new_debt_f = new_debt_f - previous_debt_f;

        let total_protocol_fee_f = net_new_debt_f * protocol_take_rate;
        let absolute_referral_rate = protocol_take_rate * referral_rate;
        let max_referrers_fees_f = net_new_debt_f * absolute_referral_rate;

        let new_acc_protocol_fees_f =
            total_protocol_fee_f - max_referrers_fees_f + acc_protocol_fees_f;

        self.cumulative_borrow_rate_bsf = new_cumulative_borrow_rate.into();
        self.pending_referrer_fees_sf += max_referrers_fees_f.to_bits();
        self.accumulated_protocol_fees_sf = new_acc_protocol_fees_f.to_bits();
        self.borrowed_amount_sf = new_debt_f.to_bits();
        self.absolute_referral_rate_sf = absolute_referral_rate.to_bits();

        Ok(())
    }

    pub fn forgive_debt(&mut self, liquidity_amount: Fraction) -> LendingResult<()> {
        let amt = Fraction::from_bits(self.borrowed_amount_sf);
        let new_amt = amt - liquidity_amount;
        self.borrowed_amount_sf = new_amt.to_bits();

        Ok(())
    }

    pub fn withdraw_referrer_fees(
        &mut self,
        withdraw_amount: u64,
        referrer_token_state: &mut ReferrerTokenState,
    ) -> Result<()> {
        self.available_amount = self
            .available_amount
            .checked_sub(withdraw_amount)
            .ok_or(LendingError::MathOverflow)?;

        let accumulated_referrer_fees_f = Fraction::from_bits(self.accumulated_referrer_fees_sf);

        let withdraw_amount_f = Fraction::from_num(withdraw_amount);

        let new_accumulated_referrer_fees_f = accumulated_referrer_fees_f
            .checked_sub(withdraw_amount_f)
            .ok_or_else(|| {
                msg!("Accumulated referrer fees cannot be less than withdraw amount");
                error!(LendingError::MathOverflow)
            })?;

        self.accumulated_referrer_fees_sf = new_accumulated_referrer_fees_f.to_bits();

        let referrer_amount_unclaimed_f =
            Fraction::from_bits(referrer_token_state.amount_unclaimed_sf);

        let new_referrer_amount_unclaimed_f = referrer_amount_unclaimed_f
            .checked_sub(withdraw_amount_f)
            .ok_or_else(|| {
                msg!("Unclaimed referrer fees cannot be less than withdraw amount");
                error!(LendingError::MathOverflow)
            })?;

        referrer_token_state.amount_unclaimed_sf = new_referrer_amount_unclaimed_f.to_bits();

        Ok(())
    }

    pub fn get_market_price_f(&self) -> Fraction {
        Fraction::from_bits(self.market_price_sf)
    }
}

pub struct NewReserveLiquidityParams {
    pub mint_pubkey: Pubkey,
    pub mint_decimals: u8,
    pub supply_vault: Pubkey,
    pub fee_vault: Pubkey,
    pub market_price_sf: u128,
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
        Self {
            mint_pubkey: params.mint_pubkey,
            mint_total_supply: 0,
            supply_vault: params.supply_vault,
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
            .ok_or(LendingError::MathOverflow)?;
        Ok(())
    }

    fn exchange_rate(&self, total_liquidity: Fraction) -> LendingResult<CollateralExchangeRate> {
        let rate = if self.mint_total_supply == 0 || total_liquidity == Fraction::ZERO {
            INITIAL_COLLATERAL_RATE
        } else {
            Fraction::from(self.mint_total_supply) / total_liquidity
        };

        Ok(CollateralExchangeRate(rate))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct CollateralExchangeRate(Fraction);

impl CollateralExchangeRate {
    pub fn collateral_to_liquidity(&self, collateral_amount: u64) -> LendingResult<u64> {
        Ok(self
            .fraction_collateral_to_liquidity(collateral_amount.into())
            .to_floor())
    }

    pub fn fraction_collateral_to_liquidity(&self, collateral_amount: Fraction) -> Fraction {
        collateral_amount / self.0
    }

    pub fn liquidity_to_collateral(&self, liquidity_amount: u64) -> u64 {
        (self.0 * u128::from(liquidity_amount)).to_floor()
    }
}

impl From<CollateralExchangeRate> for Fraction {
    fn from(exchange_rate: CollateralExchangeRate) -> Self {
        exchange_rate.0
    }
}

pub struct NewReserveCollateralParams {
    pub mint_pubkey: Pubkey,
    pub supply_vault: Pubkey,
}

static_assertions::const_assert_eq!(RESERVE_CONFIG_SIZE, std::mem::size_of::<ReserveConfig>());
static_assertions::const_assert_eq!(0, std::mem::size_of::<ReserveConfig>() % 8);
#[derive(AnchorSerialize, AnchorDeserialize, PartialEq, Eq, Derivative, Default)]
#[derivative(Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[zero_copy]
#[repr(C)]
pub struct ReserveConfig {
    pub status: u8,
    pub asset_tier: u8,
    #[cfg_attr(feature = "serde", serde(skip_serializing, default))]
    #[derivative(Debug = "ignore")]
    pub reserved_0: [u8; 2],
    pub multiplier_side_boost: [u8; 2],
    pub multiplier_tag_boost: [u8; 8],
    pub protocol_take_rate_pct: u8,
    pub protocol_liquidation_fee_pct: u8,
    pub loan_to_value_pct: u8,
    pub liquidation_threshold_pct: u8,
    pub min_liquidation_bonus_bps: u16,
    pub max_liquidation_bonus_bps: u16,
    pub bad_debt_liquidation_bonus_bps: u16,
    pub deleveraging_margin_call_period_secs: u64,
    pub deleveraging_threshold_slots_per_bps: u64,
    pub fees: ReserveFees,
    pub borrow_rate_curve: BorrowRateCurve,
    pub borrow_factor_pct: u64,

    pub deposit_limit: u64,
    pub borrow_limit: u64,
    pub token_info: TokenInfo,

    pub deposit_withdrawal_cap: WithdrawalCaps,
    pub debt_withdrawal_cap: WithdrawalCaps,

    pub elevation_groups: [u8; 20],
    #[cfg_attr(feature = "serde", serde(skip_serializing, default))]
    #[derivative(Debug = "ignore")]
    pub reserved_1: [u8; 4],
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
}

#[repr(u8)]
#[derive(TryFromPrimitive, IntoPrimitive, PartialEq, Eq, Debug, Clone, Copy)]
pub enum ReserveStatus {
    Active = 0,
    Obsolete = 1,
    Hidden = 2,
}

#[derive(AnchorSerialize, AnchorDeserialize, PartialEq, Eq, Default, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Deserialize, serde::Serialize))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[zero_copy]
#[repr(C)]
pub struct WithdrawalCaps {
    pub config_capacity: i64,
    pub current_total: i64,
    pub last_interval_start_timestamp: u64,
    pub config_interval_length_seconds: u64,
}

#[derive(AnchorSerialize, AnchorDeserialize, Default, PartialEq, Eq, Derivative)]
#[derivative(Debug)]
#[zero_copy]
#[repr(C)]
pub struct ReserveFees {
    pub borrow_fee_sf: u64,
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
                BorrowFee,
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
                    let borrow_fee_sf = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(0, &self))?;
                    let flash_loan_fee_sf = seq
                        .next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(1, &self))?;
                    Ok(ReserveFees {
                        borrow_fee_sf,
                        flash_loan_fee_sf,
                        padding: [0; 8],
                    })
                }

                fn visit_map<V>(self, mut map: V) -> Result<ReserveFees, V::Error>
                where
                    V: MapAccess<'de>,
                {
                    let mut borrow_fee_f: Option<Fraction> = None;
                    let mut flash_loan_fee_f: Option<Fraction> = None;
                    while let Some(key) = map.next_key()? {
                        match key {
                            Field::BorrowFee => {
                                if borrow_fee_f.is_some() {
                                    return Err(de::Error::duplicate_field("borrow_fee"));
                                }
                                borrow_fee_f = Some(map.next_value()?);
                            }
                            Field::FlashLoanFee => {
                                if flash_loan_fee_f.is_some() {
                                    return Err(de::Error::duplicate_field("flash_loan_fee"));
                                }
                                flash_loan_fee_f = Some(map.next_value()?);
                            }
                        }
                    }
                    let borrow_fee_f =
                        borrow_fee_f.ok_or_else(|| de::Error::missing_field("borrow_fee"))?;
                    let flash_loan_fee_f = flash_loan_fee_f
                        .ok_or_else(|| de::Error::missing_field("flash_loan_fee"))?;
                    Ok(ReserveFees {
                        borrow_fee_sf: u64::try_from(borrow_fee_f.to_bits())
                            .map_err(|_| de::Error::custom("borrow_fee does not fit in u64"))?,
                        flash_loan_fee_sf: u64::try_from(flash_loan_fee_f.to_bits())
                            .map_err(|_| de::Error::custom("flash_loan_fee does not fit in u64"))?,
                        padding: [0; 8],
                    })
                }
            }

            const FIELDS: &[&str] = &["borrow_fee", "flash_loan_fee"];
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
                borrow_fee: Fraction,
                flash_loan_fee: Fraction,
            }

            let borrow_fee_f = Fraction::from_bits(self.borrow_fee_sf.into());
            let flash_loan_fee_f = Fraction::from_bits(self.flash_loan_fee_sf.into());

            let fees = ReserveFeesSerde {
                borrow_fee: borrow_fee_f,
                flash_loan_fee: flash_loan_fee_f,
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
    ) -> Result<(u64, u64)> {
        self.calculate_fees(
            borrow_amount,
            self.borrow_fee_sf,
            fee_calculation,
            referral_fee_bps,
        )
    }

    pub fn calculate_flash_loan_fees(
        &self,
        flash_loan_amount_f: Fraction,
        referral_fee_bps: u16,
    ) -> Result<(u64, u64)> {
        let (total_fees, referral_fee) = self.calculate_fees(
            flash_loan_amount_f,
            self.flash_loan_fee_sf,
            FeeCalculation::Exclusive,
            referral_fee_bps,
        )?;

        Ok((total_fees, referral_fee))
    }

    fn calculate_fees(
        &self,
        amount: Fraction,
        fee_sf: u64,
        fee_calculation: FeeCalculation,
        referral_fee_bps: u16,
    ) -> Result<(u64, u64)> {
        let borrow_fee_rate = Fraction::from_bits(fee_sf.into());
        let referral_fee_rate = Fraction::from_bps(referral_fee_bps);
        if borrow_fee_rate > Fraction::ZERO && amount > Fraction::ZERO {
            let need_to_assess_referral_fee = referral_fee_rate > Fraction::ZERO;
            let minimum_fee = if need_to_assess_referral_fee {
                2u64
            } else {
                1u64
            };

            let borrow_fee_amount = match fee_calculation {
                FeeCalculation::Exclusive => amount.mul(borrow_fee_rate),
                FeeCalculation::Inclusive => {
                    let borrow_fee_rate = borrow_fee_rate.div(borrow_fee_rate.add(Fraction::ONE));
                    amount.mul(borrow_fee_rate)
                }
            };

            let borrow_fee_f = borrow_fee_amount.max(minimum_fee.into());
            if borrow_fee_f >= amount {
                msg!("Borrow amount is too small to receive liquidity after fees");
                return err!(LendingError::BorrowTooSmall);
            }

            let borrow_fee = borrow_fee_f.to_round();
            let referral_fee = if need_to_assess_referral_fee {
                let referal_fee_f = borrow_fee_f * referral_fee_rate;
                referal_fee_f.to_round::<u64>().max(1u64)
            } else {
                0
            };

            Ok((borrow_fee, referral_fee))
        } else {
            Ok((0, 0))
        }
    }
}

pub enum FeeCalculation {
    Exclusive,
    Inclusive,
}

#[derive(Debug, PartialEq, Eq, num_enum::IntoPrimitive, num_enum::TryFromPrimitive)]
#[repr(u8)]
pub enum AssetTier {
    Regular = 0,
    IsolatedCollateral = 1,
    IsolatedDebt = 2,
}

fn approximate_compounded_interest(rate: Fraction, elapsed_slots: u64) -> Fraction {
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
