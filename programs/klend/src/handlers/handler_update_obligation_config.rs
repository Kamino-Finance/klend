use std::fmt::Debug;

use anchor_lang::{prelude::*, Accounts};

use crate::{
    lending_market::lending_operations, state::LendingMarket, Obligation,
    ObligationConfigUpdateSubject, Reserve, UpdateObligationConfigMode,
};

pub fn process(
    ctx: Context<UpdateObligationConfig>,
    mode: UpdateObligationConfigMode,
    value: &[u8],
) -> Result<()> {
    let mut obligation = ctx.accounts.obligation.load_mut()?;
    let market = ctx.accounts.lending_market.load()?;
    lending_operations::update_obligation_config(
        &mut obligation,
        &market,
        ObligationConfigUpdateSubject::resolve(
            ctx.accounts
                .deposit_reserve
                .as_ref()
                .map(|loader| loader.key()),
            ctx.accounts
                .borrow_reserve
                .as_ref()
                .map(|loader| loader.key()),
        )?,
        mode,
        value,
    )?;
    Ok(())
}

#[derive(Accounts)]
pub struct UpdateObligationConfig<'info> {

    pub owner: Signer<'info>,


    #[account(mut, has_one = lending_market, has_one = owner)]
    pub obligation: AccountLoader<'info, Obligation>,




    #[account(has_one = lending_market)]
    pub borrow_reserve: Option<AccountLoader<'info, Reserve>>,




    #[account(has_one = lending_market)]
    pub deposit_reserve: Option<AccountLoader<'info, Reserve>>,


    pub lending_market: AccountLoader<'info, LendingMarket>,
}
