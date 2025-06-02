use anchor_lang::{prelude::*, Accounts};

use crate::{order_operations, LendingMarket, Obligation, ObligationOrder};

pub fn process(ctx: Context<SetObligationOrder>, index: u8, order: ObligationOrder) -> Result<()> {
    let lending_market = &ctx.accounts.lending_market.load()?;
    let obligation = &mut ctx.accounts.obligation.load_mut()?;
    order_operations::set_order_on_obligation(lending_market, obligation, index, order)?;
    Ok(())
}

#[derive(Accounts)]
pub struct SetObligationOrder<'info> {
    pub owner: Signer<'info>,

    #[account(mut, has_one = lending_market, has_one = owner)]
    pub obligation: AccountLoader<'info, Obligation>,

    pub lending_market: AccountLoader<'info, LendingMarket>,
}
