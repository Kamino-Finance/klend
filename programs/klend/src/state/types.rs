use crate::utils::Fraction;
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalculateBorrowResult {
    pub borrow_amount_f: Fraction,
    pub receive_amount: u64,
    pub borrow_fee: u64,
    pub referrer_fee: u64,
}

#[derive(Debug)]
pub struct CalculateRepayResult {
    pub settle_amount_f: Fraction,
    pub repay_amount: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CalculateLiquidationResult {
    pub settle_amount_f: Fraction,
    pub repay_amount: u64,
    pub withdraw_amount: u64,
    pub liquidation_bonus_rate: Fraction,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidateObligationResult {
    pub settle_amount_f: Fraction,
    pub repay_amount: u64,
    pub withdraw_amount: u64,
    pub withdraw_collateral_amount: u64,
    pub liquidation_bonus_rate: Fraction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiquidateAndRedeemResult {
    pub repay_amount: u64,
    pub withdraw_amount: u64,
    pub withdraw_collateral_amount: u64,
    pub total_withdraw_liquidity_amount: Option<(u64, u64)>,
}

pub struct LiquidationParams {
    pub user_ltv: Fraction,
    pub liquidation_bonus_rate: Fraction,
}

pub struct RefreshObligationDepositsResult {
    pub lowest_deposit_ltv_accumulator: u8,
    pub num_of_obsolete_reserves: u8,
    pub deposited_value_f: Fraction,
    pub allowed_borrow_value_f: Fraction,
    pub unhealthy_borrow_value_f: Fraction,
}

pub struct RefreshObligationBorrowsResult {
    pub borrow_factor_adjusted_debt_value_f: Fraction,
    pub borrowed_assets_market_value_f: Fraction,
}
