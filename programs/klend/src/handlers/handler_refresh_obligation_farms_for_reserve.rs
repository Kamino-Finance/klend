use anchor_lang::{prelude::*, Accounts};
use farms::{program::Farms, state::UserState as FarmsUserState};

use crate::{
    fraction::FractionExtra,
    lending_market::{farms_ixs, lending_checks},
    state::{obligation::Obligation, LendingMarket},
    utils::{constraints, seeds, FatAccountLoader},
    LendingError, Reserve, ReserveFarmKind,
};

pub fn process_refresh_obligation_farms_for_reserve(
    ctx: Context<RefreshObligationFarmsForReserve>,
    mode: u8,
) -> Result<()> {
    constraints::check_remaining_accounts(&ctx)?;
    let farm_kind: ReserveFarmKind = mode.try_into().unwrap();
    process_impl_refresh_obligation_farms_for_reserve(&ctx.accounts.base_accounts, farm_kind)
}

pub(crate) fn process_impl_refresh_obligation_farms_for_reserve(
    account_ctx: &RefreshObligationFarmsForReserveBase,
    farm_kind: ReserveFarmKind,
) -> Result<()> {
    lending_checks::refresh_obligation_farms_for_reserve_checks(account_ctx)?;
    require_keys_eq!(
        account_ctx.obligation_farm_user_state.load()?.delegatee,
        account_ctx.obligation.key(),
        LendingError::InvalidAccountInput
    );

    msg!("RefreshObligationFarmsForReserve {:?}", farm_kind);
    let reserve = &account_ctx.reserve.load()?;
    let reserve_address: Pubkey = *account_ctx.reserve.to_account_info().key;

   
    let farm_address = reserve.get_farm(farm_kind);
    if farm_address == Pubkey::default() {
        return Err(LendingError::NoFarmForReserve.into());
    }
    require_keys_eq!(
        farm_address,
        account_ctx.reserve_farm_state.key(),
        LendingError::InvalidAccountInput
    );

    let amount = if account_ctx.obligation.data_is_empty() {
        0
    } else {
        let obligation_account: FatAccountLoader<Obligation> =
            FatAccountLoader::try_from(&account_ctx.obligation).unwrap();
        let obligation = obligation_account.load()?;

        amount_for_obligation(&obligation, &reserve_address, farm_kind)
    };

    msg!(
        "RefreshObligationFarmsForReserve amount {} slot {}",
        amount,
        Clock::get()?.slot,
    );

    if account_ctx
        .obligation_farm_user_state
        .load()?
        .active_stake_scaled
        != u128::from(amount)
    {
        farms_ixs::cpi_set_stake_delegated(account_ctx, reserve, farm_kind, amount)?;
    } else {
        msg!("Farm stake is unchanged, skipping update");
    }

    Ok(())
}

#[derive(Accounts)]
pub struct RefreshObligationFarmsForReserve<'info> {
    #[account()]
    pub crank: Signer<'info>,

    pub base_accounts: RefreshObligationFarmsForReserveBase<'info>,

    pub farms_program: Program<'info, Farms>,
    pub rent: Sysvar<'info, Rent>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct RefreshObligationFarmsForReserveBase<'info> {
    /// CHECK: Obligation is checked against the lending market in lending_checks
    /// CHECK: The obligation must match the `delegatee` in the farm state (checked in handler)
    pub obligation: AccountInfo<'info>,

    /// CHECK: Seed checked
    #[account(
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(has_one = lending_market)]
    pub reserve: AccountLoader<'info, Reserve>,

    /// CHECK: Checked against the reserve's stored farm account + CPI checks
    #[account(mut)]
    pub reserve_farm_state: AccountInfo<'info>,

    /// CHECK: Checked against the farm state account in CPI
    #[account(mut)]
    pub obligation_farm_user_state: AccountLoader<'info, FarmsUserState>,

    pub lending_market: AccountLoader<'info, LendingMarket>,
}


#[derive(Accounts)]
pub struct OptionalObligationFarmsAccounts<'info> {
    /// CHECK: `delegatee` is checked as equal to the `obligation` in `lending_checks
    #[account(mut)]
    pub obligation_farm_user_state: Option<AccountLoader<'info, FarmsUserState>>,
    /// CHECK: Checked against the reserve's stored farm account + CPI checks
    #[account(mut)]
    pub reserve_farm_state: Option<AccountInfo<'info>>,
}

fn amount_for_obligation(
    obligation: &Obligation,
    reserve_address: &Pubkey,
    farm_kind: ReserveFarmKind,
) -> u64 {
    match farm_kind {
        ReserveFarmKind::Collateral => {
            let collateral = obligation.find_collateral_in_deposits(*reserve_address);
            if let Ok(obligation_collateral) = collateral {
                obligation_collateral.deposited_amount
            } else {
                0
            }
        }

        ReserveFarmKind::Debt => {
            let liquidity = obligation.find_liquidity_in_borrows(*reserve_address);
            if let Ok((obligation_liquidity, _)) = liquidity {
                obligation_liquidity.borrowed_amount().to_floor::<u64>()
            } else {
                0
            }
        }
    }
}
