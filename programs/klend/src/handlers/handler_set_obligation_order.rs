use anchor_lang::{prelude::*, Accounts};

use crate::{
    fraction::Fraction, obligation_order_operations, LendingMarket, Obligation, ObligationOrder,
};

pub fn process(
    ctx: Context<SetObligationOrder>,
    index: u8,
    order: ObligationOrder,
    min_expected_current_opportunity_parameter_sf: u128,
) -> Result<()> {
    let lending_market = &ctx.accounts.lending_market.load()?;
    let obligation = &mut ctx.accounts.obligation.load_mut()?;
    obligation_order_operations::set_order_on_obligation(
        lending_market,
        obligation,
        index,
        order,
        Fraction::from_bits(min_expected_current_opportunity_parameter_sf),
    )?;
    Ok(())
}

#[derive(Accounts)]
pub struct SetObligationOrder<'info> {
    pub owner: Signer<'info>,

    #[account(mut, has_one = lending_market, has_one = owner)]
    pub obligation: AccountLoader<'info, Obligation>,

    pub lending_market: AccountLoader<'info, LendingMarket>,
}
