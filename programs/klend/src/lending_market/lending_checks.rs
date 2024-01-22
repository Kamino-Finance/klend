use anchor_lang::{
    err,
    prelude::{msg, AccountLoader, Context, Pubkey},
    Key, Result,
};

use crate::{
    handlers::*,
    state::{
        DepositObligationCollateralAccounts, RedeemReserveCollateralAccounts,
        WithdrawObligationCollateralAccounts,
    },
    utils::{seeds::BASE_SEED_REFERRER_TOKEN_STATE, PROGRAM_VERSION},
    LendingError, Obligation, ReferrerTokenState, ReserveStatus,
};

pub fn borrow_obligation_liquidity_checks(ctx: &Context<BorrowObligationLiquidity>) -> Result<()> {
    let borrow_reserve = &ctx.accounts.borrow_reserve.load()?;

    if borrow_reserve.liquidity.supply_vault == ctx.accounts.user_destination_liquidity.key() {
        msg!(
            "Borrow reserve liquidity supply cannot be used as the destination liquidity provided"
        );
        return err!(LendingError::InvalidAccountInput);
    }

    if borrow_reserve.config.status() == ReserveStatus::Obsolete {
        msg!("Reserve is not active");
        return err!(LendingError::ReserveObsolete);
    }

    if borrow_reserve.version != PROGRAM_VERSION as u64 {
        msg!("Reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    Ok(())
}

pub fn deposit_obligation_collateral_checks(
    accounts: &DepositObligationCollateralAccounts,
) -> Result<()> {
    let deposit_reserve = &accounts.deposit_reserve.load()?;

    if deposit_reserve.collateral.supply_vault == accounts.user_source_collateral.key() {
        msg!("Deposit reserve collateral supply cannot be used as the source collateral provided");
        return err!(LendingError::InvalidAccountInput);
    }

    if deposit_reserve.config.status() == ReserveStatus::Obsolete {
        msg!("Reserve is not active");
        return err!(LendingError::ReserveObsolete);
    }

    if deposit_reserve.version != PROGRAM_VERSION as u64 {
        msg!("Reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    Ok(())
}

pub fn deposit_reserve_liquidity_checks(
    accounts: &crate::state::nested_accounts::DepositReserveLiquidityAccounts,
) -> Result<()> {
    let reserve = accounts.reserve.load()?;

    if reserve.liquidity.supply_vault == accounts.user_source_liquidity.key() {
        msg!("Reserve liquidity supply cannot be used as the source liquidity provided");
        return err!(LendingError::InvalidAccountInput);
    }
    if reserve.collateral.supply_vault == accounts.user_destination_collateral.key() {
        msg!("Reserve collateral supply cannot be used as the destination collateral provided");
        return err!(LendingError::InvalidAccountInput);
    }

    if reserve.config.status() == ReserveStatus::Obsolete {
        msg!("Reserve is not active");
        return err!(LendingError::ReserveObsolete);
    }

    if reserve.version != PROGRAM_VERSION as u64 {
        msg!("Reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    Ok(())
}

pub fn liquidate_obligation_checks(
    ctx: &Context<LiquidateObligationAndRedeemReserveCollateral>,
) -> Result<()> {
    let repay_reserve = ctx.accounts.repay_reserve.load()?;
    let withdraw_reserve = ctx.accounts.withdraw_reserve.load()?;

    if repay_reserve.liquidity.supply_vault == ctx.accounts.user_source_liquidity.key() {
        msg!("Repay reserve liquidity supply cannot be used as the source liquidity provided");
        return err!(LendingError::InvalidAccountInput);
    }
    if repay_reserve.collateral.supply_vault == ctx.accounts.user_destination_collateral.key() {
        msg!(
            "Repay reserve collateral supply cannot be used as the destination collateral provided"
        );
        return err!(LendingError::InvalidAccountInput);
    }

    if repay_reserve.version != PROGRAM_VERSION as u64 {
        msg!("Withdraw reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    if withdraw_reserve.liquidity.supply_vault == ctx.accounts.user_source_liquidity.key() {
        msg!("Withdraw reserve liquidity supply cannot be used as the source liquidity provided");
        return err!(LendingError::InvalidAccountInput);
    }
    if withdraw_reserve.collateral.supply_vault == ctx.accounts.user_destination_collateral.key() {
        msg!("Withdraw reserve collateral supply cannot be used as the destination collateral provided");
        return err!(LendingError::InvalidAccountInput);
    }

    if withdraw_reserve.version != PROGRAM_VERSION as u64 {
        msg!("Withdraw reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    Ok(())
}

pub fn redeem_reserve_collateral_checks(accounts: &RedeemReserveCollateralAccounts) -> Result<()> {
    let reserve = &accounts.reserve.load()?;

    if reserve.collateral.supply_vault == accounts.user_source_collateral.key() {
        msg!("Reserve collateral supply cannot be used as the source collateral provided");
        return err!(LendingError::InvalidAccountInput);
    }
    if reserve.liquidity.supply_vault == accounts.user_destination_liquidity.key() {
        msg!("Reserve liquidity supply cannot be used as the destination liquidity provided");
        return err!(LendingError::InvalidAccountInput);
    }

    if reserve.version != PROGRAM_VERSION as u64 {
        msg!("Reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    Ok(())
}

pub fn repay_obligation_liquidity_checks(ctx: &Context<RepayObligationLiquidity>) -> Result<()> {
    let repay_reserve = ctx.accounts.repay_reserve.load()?;

    if repay_reserve.liquidity.supply_vault == ctx.accounts.user_source_liquidity.key() {
        msg!("Repay reserve liquidity supply cannot be used as the source liquidity provided");
        return err!(LendingError::InvalidAccountInput);
    }

    if repay_reserve.version != PROGRAM_VERSION as u64 {
        msg!("Reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    Ok(())
}

pub fn withdraw_obligation_collateral_checks(
    accounts: &WithdrawObligationCollateralAccounts,
) -> Result<()> {
    let withdraw_reserve = accounts.withdraw_reserve.load()?;

    if withdraw_reserve.version != PROGRAM_VERSION as u64 {
        msg!("Reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    if withdraw_reserve.collateral.supply_vault == accounts.user_destination_collateral.key() {
        msg!("Withdraw reserve collateral supply cannot be used as the destination collateral provided");
        return err!(LendingError::InvalidAccountInput);
    }

    Ok(())
}

pub fn flash_borrow_reserve_liquidity_checks(
    ctx: &Context<FlashBorrowReserveLiquidity>,
) -> Result<()> {
    let reserve = ctx.accounts.reserve.load()?;

    if reserve.liquidity.supply_vault == ctx.accounts.user_destination_liquidity.key() {
        msg!(
            "Borrow reserve liquidity supply cannot be used as the destination liquidity provided"
        );
        return err!(LendingError::InvalidAccountInput);
    }

    if reserve.version != PROGRAM_VERSION as u64 {
        msg!("Reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    if reserve.config.status() != ReserveStatus::Active {
        msg!("Reserve is not active");
        return err!(LendingError::ReserveObsolete);
    }

    if reserve.config.fees.flash_loan_fee_sf == u64::MAX {
        msg!("Flash loans are disabled for this reserve");
        return err!(LendingError::FlashLoansDisabled);
    }

    Ok(())
}

pub fn flash_repay_reserve_liquidity_checks(
    ctx: &Context<FlashRepayReserveLiquidity>,
) -> Result<()> {
    let reserve = ctx.accounts.reserve.load()?;

    if reserve.liquidity.supply_vault == ctx.accounts.user_source_liquidity.key() {
        msg!("Reserve liquidity supply cannot be used as the source liquidity provided");
        return err!(LendingError::InvalidAccountInput);
    }

    Ok(())
}

pub fn refresh_obligation_farms_for_reserve_checks(
    ctx: &Context<RefreshObligationFarmsForReserve>,
) -> Result<()> {
    if !ctx.accounts.obligation.data_is_empty() {
        let obligation_account: AccountLoader<Obligation> =
            AccountLoader::try_from(&ctx.accounts.obligation).unwrap();
        let obligation = obligation_account.load()?;

        if obligation.lending_market != ctx.accounts.lending_market.key() {
            msg!("Obligation lending market does not match the lending market provided");
            return err!(LendingError::InvalidAccountInput);
        }
    }

    let reserve = ctx.accounts.reserve.load()?;

    if reserve.config.status() == ReserveStatus::Obsolete {
        msg!("Reserve is not active");
        return err!(LendingError::ReserveObsolete);
    }

    if reserve.version != PROGRAM_VERSION as u64 {
        msg!("Reserve version does not match the program version");
        return err!(LendingError::ReserveDeprecated);
    }

    Ok(())
}

pub fn validate_referrer_token_state(
    referrer_token_state: &ReferrerTokenState,
    referrer_token_state_key: Pubkey,
    mint: Pubkey,
    owner_referrer: Pubkey,
    reserve_key: Pubkey,
) -> anchor_lang::Result<()> {
    if referrer_token_state.mint == Pubkey::default()
        || referrer_token_state.referrer == Pubkey::default()
    {
        return err!(LendingError::ReferrerAccountNotInitialized);
    }

    if referrer_token_state.mint != mint {
        return err!(LendingError::ReferrerAccountMintMissmatch);
    }

    let referrer_token_state_valid_pda = Pubkey::create_program_address(
        &[
            BASE_SEED_REFERRER_TOKEN_STATE,
            referrer_token_state.referrer.as_ref(),
            reserve_key.as_ref(),
            &[referrer_token_state.bump.try_into().unwrap()],
        ],
        &crate::ID,
    )
    .unwrap();

    if referrer_token_state_key != referrer_token_state_valid_pda {
        return err!(LendingError::ReferrerAccountWrongAddress);
    }

    if referrer_token_state.referrer != owner_referrer {
        return err!(LendingError::ReferrerAccountReferrerMissmatch);
    }

    Ok(())
}
