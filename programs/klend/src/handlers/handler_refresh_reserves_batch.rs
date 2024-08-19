use anchor_lang::{prelude::*, Accounts, Result};

use crate::{
    fraction::FractionExtra,
    lending_market::lending_operations,
    state::Reserve,
    utils::{prices::get_price, FatAccountLoader, PROGRAM_VERSION},
    LendingError, LendingMarket,
};

fn maybe_price_account<'a, 'info>(
    price_account: Option<&'a AccountInfo<'info>>,
) -> Result<Option<&'a AccountInfo<'info>>> {
    if let Some(price_account) = price_account {
        if price_account.key == &crate::ID {
            Ok(None)
        } else {
            Ok(Some(price_account))
        }
    } else {
        msg!("Missing price account");
        err!(LendingError::InvalidAccountInput)
    }
}

pub fn process(ctx: Context<RefreshReservesBatch>, skip_price_updates: bool) -> Result<()> {
    let clock = &Clock::get()?;
    let mut remaining_accounts_it = ctx.remaining_accounts.iter();
    loop {
        let Some(reserve_acc) = remaining_accounts_it.next() else {
            break;
        };
        let Some(lending_market_acc) = remaining_accounts_it.next() else {
            msg!("Missing lending market account");
            return err!(LendingError::InvalidAccountInput);
        };

        let reserve_loader = FatAccountLoader::<Reserve>::try_from(reserve_acc)?;
        let reserve = &mut reserve_loader.load_mut()?;

        require_keys_eq!(
            reserve.lending_market,
            *lending_market_acc.key,
            LendingError::InvalidAccountInput
        );

        let lending_market_loader =
            FatAccountLoader::<LendingMarket>::try_from(lending_market_acc)?;
        let lending_market = &lending_market_loader.load()?;

        require!(
            lending_market.emergency_mode == false as u8,
            LendingError::GlobalEmergencyMode
        );

        require!(
            reserve.version == PROGRAM_VERSION as u64,
            LendingError::ReserveDeprecated
        );

        let price_res = if !skip_price_updates {
            let pyth_oracle = maybe_price_account(remaining_accounts_it.next())?;
            let switchboard_price_oracle = maybe_price_account(remaining_accounts_it.next())?;
            let switchboard_twap_oracle = maybe_price_account(remaining_accounts_it.next())?;
            let scope_prices = maybe_price_account(remaining_accounts_it.next())?;

            if lending_operations::is_price_refresh_needed(
                reserve,
                lending_market,
                clock.unix_timestamp,
            ) {
                reserve.config.token_info.validate_token_info_config(
                    pyth_oracle,
                    switchboard_price_oracle,
                    switchboard_twap_oracle,
                    scope_prices,
                )?;

                get_price(
                    &reserve.config.token_info,
                    pyth_oracle,
                    switchboard_price_oracle,
                    switchboard_twap_oracle,
                    scope_prices,
                    clock.unix_timestamp,
                )?
            } else {
                None
            }
        } else {
            None
        };

        lending_operations::refresh_reserve(
            reserve,
            clock,
            price_res,
            lending_market.referral_fee_bps,
        )?;
        lending_operations::refresh_reserve_limit_timestamps(reserve, clock.slot)?;

        if !skip_price_updates {
            msg!(
                "Token: {} Price: {}",
                &reserve.config.token_info.symbol(),
                reserve.liquidity.get_market_price_f().to_display()
            );
        }
    }

    Ok(())
}

#[derive(Accounts)]
pub struct RefreshReservesBatch {}
