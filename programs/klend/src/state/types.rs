use crate::{
    utils::Fraction, LendingMarket, Obligation, ObligationLiquidity, PriceStatusFlags, Reserve,
};


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DepositLiquidityResult {

    pub liquidity_amount: u64,

    pub collateral_amount: u64,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorrowSize {



    Exact(u64),



    AllAvailable,


    AtMost(u64),
}

impl BorrowSize {


    pub fn exact_or_all_available(liquidity_amount: u64) -> BorrowSize {
        if liquidity_amount == u64::MAX {
            BorrowSize::AllAvailable
        } else {
            BorrowSize::Exact(liquidity_amount)
        }
    }


    pub fn is_zero(&self) -> bool {
        match self {
            BorrowSize::Exact(amount_lte) | BorrowSize::AtMost(amount_lte) => *amount_lte == 0,
            BorrowSize::AllAvailable => false,
        }
    }
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalculateBorrowResult {

    pub borrow_amount_f: Fraction,

    pub receive_amount: u64,

    pub origination_fee: u64,

    pub referrer_fee: u64,
}


#[derive(Debug)]
pub struct CalculateRepayResult {

    pub settle_amount: Fraction,

    pub repay_amount: u64,
}


#[derive(Debug)]
pub struct RepayObligationLiquidityResult {

    pub repay_amount: u64,

    pub early_repay_penalty: u64,
}





#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixedTermRolloverResult {



    pub repaid_amount: Fraction,









    pub borrowed_amount: Fraction,


    pub tokens_to_transfer_over: u64,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalculateLiquidationResult {


    pub settle_amount: Fraction,

    pub repay_amount: u64,

    pub withdraw_amount: u64,

    pub liquidation_bonus_rate: Fraction,



    pub liquidation_reason: LiquidationReason,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LiquidationReason {


    LtvExceeded,


    IndividualDeleveraging,


    MarketWideDeleveraging,


    ReserveDebtMaturityReached,



    ObligationBorrowDebtTermReached(u64),
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidateObligationResult {


    pub settle_amount: Fraction,

    pub repay_amount: u64,

    pub withdraw_amount: u64,

    pub withdraw_collateral_amount: u64,

    pub liquidation_bonus_rate: Fraction,



    pub liquidation_reason: LiquidationReason,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepayAndWithdrawRedeemResult {

    pub repay_amount: u64,

    pub early_repay_penalty: u64,

    pub withdraw_obligation_amount: u64,

    pub withdraw_liquidity_amount: u64,

    pub obligation_closed: bool,
}


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DepositAndBorrowResult {

    pub receive_amount: u64,

    pub origination_fee: u64,


    pub referrer_fee: u64,

    pub deposit_liquidity_amount: u64,


    pub deposit_collateral_amount: u64,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidateAndRedeemResult {

    pub repay_amount: u64,

    pub withdraw_amount: u64,

    pub withdraw_collateral_amount: u64,

    pub total_withdraw_liquidity_amount: Option<(u64, u64)>,

    pub liquidation_reason: LiquidationReason,
}


#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExecuteObligationOrderResult {
    Deleverage(ExecuteDeleverageOrderResult),
    LeverUp(ExecuteLeverUpOrderResult),
}

impl ExecuteObligationOrderResult {


    pub fn executor_given_liquidity(&self) -> u64 {
        match self {
            Self::Deleverage(result) => result.repay_amount + result.early_repay_penalty,
            Self::LeverUp(result) => result.deposit_liquidity_amount,
        }
    }



    pub fn executor_received_liquidity(&self) -> u64 {
        match self {
            Self::Deleverage(result) => result.withdraw_liquidity_amount - result.protocol_fee,
            Self::LeverUp(result) => result.borrow_liquidity_amount - result.protocol_fee,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecuteDeleverageOrderResult {

    pub repay_amount: u64,

    pub early_repay_penalty: u64,


    pub withdraw_collateral_amount: u64,

    pub withdraw_liquidity_amount: u64,

    pub protocol_fee: u64,

    pub obligation_closed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecuteLeverUpOrderResult {

    pub borrow_liquidity_amount: u64,

    pub origination_fee: u64,

    pub deposit_liquidity_amount: u64,


    pub deposit_collateral_amount: u64,


    pub protocol_fee: u64,
}

pub struct LiquidationCheckInputs<'l> {
    pub lending_market: &'l LendingMarket,
    pub collateral_reserve: &'l Reserve,
    pub liquidity: &'l ObligationLiquidity,
    pub debt_reserve: &'l Reserve,
    pub obligation: &'l Obligation,
    pub timestamp: u64,
    pub max_allowed_ltv_override_pct_opt: Option<u64>,
}

// ..
pub struct LiquidationParams {
    pub liquidation_bonus_rate: Fraction,
    pub liquidation_reason: LiquidationReason,
}

pub struct RefreshObligationDepositsResult {
    pub lowest_deposit_liquidation_ltv_threshold_pct: u8,
    pub lowest_deposit_max_ltv_pct: u8,
    pub num_of_obsolete_reserves: u8,
    pub deposited_value_f: Fraction,
    pub allowed_borrow_value_f: Fraction,
    pub unhealthy_borrow_value_f: Fraction,

    pub prices_state: PriceStatusFlags,
    pub borrowing_disabled: bool,
}

pub struct RefreshObligationBorrowsResult {
    pub borrow_factor_adjusted_debt_value_f: Fraction,
    pub borrowed_assets_market_value_f: Fraction,
    pub prices_state: PriceStatusFlags,
    pub highest_borrow_factor_pct: u64,


    pub borrowed_amount_in_elevation_group: Option<u64>,
    pub num_of_obsolete_reserves: u8,
}

pub struct RedeemCollateralOptions {

    pub add_amount_to_withdrawal_caps: bool,



    pub use_withdraw_queue: bool,
}

impl RedeemCollateralOptions {




    pub const REGULAR: Self = Self {
        add_amount_to_withdrawal_caps: true,
        use_withdraw_queue: false,
    };


    pub const FOR_WITHDRAW_QUEUED_LIQUIDITY: Self = Self {
        add_amount_to_withdrawal_caps: true,
        use_withdraw_queue: true,
    };


    pub const FOR_LIQUIDATION: Self = Self {
        add_amount_to_withdrawal_caps: false,
        use_withdraw_queue: true,
    };
}

#[derive(Debug)]
pub struct TicketedWithdrawResult {

    pub collateral_amount_to_burn: u64,


    pub liquidity_amount_to_withdraw: u64,
}

pub enum LendingAction {
    Additive(u64),
    Subtractive(u64),
}

impl LendingAction {

    pub fn net_of(added: u64, subtracted: u64) -> Self {
        if added >= subtracted {
            Self::Additive(added - subtracted)
        } else {
            Self::Subtractive(subtracted - added)
        }
    }
}

#[derive(PartialEq)]
pub enum MaxReservesAsCollateralCheck {
    Perform,
    Skip,
}

#[derive(Copy, Clone, PartialEq, Eq)]
pub enum LtvMaxWithdrawalCheck {

    MaxLtv,

    LiquidationThreshold,
}
