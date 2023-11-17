use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token::Token;
use farms::{program::Farms, state::UserState as FarmsUserState};

use crate::{
    fraction::FractionExtra,
    lending_market::{farms_ixs, lending_checks},
    state::{obligation::Obligation, LendingMarket},
    utils::{constraints, seeds, Fraction},
    LendingError, Reserve, ReserveFarmKind,
};

pub fn process(ctx: Context<RefreshObligationFarmsForReserve>, mode: u8) -> Result<()> {
    constraints::check_remaining_accounts(&ctx)?;
    lending_checks::refresh_obligation_farms_for_reserve_checks(&ctx)?;
    let farm_kind: ReserveFarmKind = mode.try_into().unwrap();

    msg!("RefreshObligationFarmsForReserve {:?}", farm_kind);
    let market = ctx.accounts.lending_market.load()?;
    let reserve = &mut ctx.accounts.reserve.load()?;
    let reserve_address: Pubkey = *ctx.accounts.reserve.to_account_info().key;

       let farm_address = reserve.get_farm(farm_kind);
    require!(
        farm_address == ctx.accounts.reserve_farm_state.key(),
        LendingError::InvalidAccountInput
    );
    if farm_address == Pubkey::default() {
        return Err(LendingError::NoFarmForReserve.into());
    }

    let (quantity, side_boost, asset_product_boost, market_product_boost) =
        if ctx.accounts.obligation.data_is_empty() {
            (0, 0, 0, 0)
        } else {
            let obligation_account: AccountLoader<Obligation> =
                AccountLoader::try_from(&ctx.accounts.obligation).unwrap();
            let obligation = obligation_account.load()?;

            let amount = amount_for_obligation(&obligation, &reserve_address, farm_kind);
            let (side_boost, tag_boost, product_boost) =
                boosts_for_obligation(reserve, &obligation, &market, mode);

            (amount, side_boost, tag_boost, product_boost)
        };

    let amount = quantity * side_boost * asset_product_boost * market_product_boost;

    msg!(
        "RefreshObligationFarmsForReserve amount {} slot {}",
        amount,
        Clock::get()?.slot,
    );

    if ctx
        .accounts
        .obligation_farm_user_state
        .load()?
        .active_stake_scaled
        != u128::from(amount)
    {
        farms_ixs::cpi_set_stake_delegated(&ctx, reserve, farm_kind, amount)?;
    } else {
        msg!("Farm stake is unchanged, skipping update");
    }

    Ok(())
}

#[derive(Accounts)]
pub struct RefreshObligationFarmsForReserve<'info> {
    #[account(mut)]
    pub crank: Signer<'info>,
       #[account(
        constraint = obligation_farm_user_state.load()?.delegatee == obligation.key() @ LendingError::InvalidAccountInput
    )]
    pub obligation: AccountInfo<'info>,

       #[account(
        mut,
        seeds = [seeds::LENDING_MARKET_AUTH, lending_market.key().as_ref()],
        bump = lending_market.load()?.bump_seed as u8,
    )]
    pub lending_market_authority: AccountInfo<'info>,

    #[account(has_one = lending_market)]
    pub reserve: AccountLoader<'info, Reserve>,

       #[account(mut)]
    pub reserve_farm_state: AccountInfo<'info>,

       #[account(mut,
        constraint = obligation_farm_user_state.load()?.delegatee == obligation.key() @ LendingError::InvalidAccountInput,
    )]
    pub obligation_farm_user_state: AccountLoader<'info, FarmsUserState>,

    pub lending_market: AccountLoader<'info, LendingMarket>,

    pub farms_program: Program<'info, Farms>,
    pub rent: Sysvar<'info, Rent>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

fn boosts_for_obligation(
    reserve: &Reserve,
    obligation: &Obligation,
    market: &LendingMarket,
    mode: u8,
) -> (u64, u64, u64) {
    let side: usize = mode.into();
    let tag: usize = obligation.tag.try_into().unwrap();

    let side_boost: u64 = reserve.config.multiplier_side_boost[side].into();
    let tag_boost: u64 = reserve.config.multiplier_tag_boost[tag].into();
    let product_boost: u64 = market.multiplier_points_tag_boost[tag].into();

    (side_boost, tag_boost, product_boost)
}

fn amount_for_obligation(
    obligation: &Obligation,
    reserve_address: &Pubkey,
    farm_kind: ReserveFarmKind,
) -> u64 {
    match farm_kind {
        ReserveFarmKind::Collateral => {
            let collateral = obligation.find_collateral_in_deposits(*reserve_address);
            if let Ok((obligation_collateral, _)) = collateral {
                obligation_collateral.deposited_amount
            } else {
                0
            }
        }

        ReserveFarmKind::Debt => {
            let liquidity = obligation.find_liquidity_in_borrows(*reserve_address);
            if let Ok((obligation_liquidity, _)) = liquidity {
                let fraction = Fraction::from_bits(obligation_liquidity.borrowed_amount_sf);
                fraction.to_floor::<u64>()
            } else {
                0
            }
        }
    }
}
