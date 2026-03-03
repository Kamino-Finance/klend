use anchor_lang::{prelude::*, Accounts};
use anchor_spl::token_interface::{Mint, TokenAccount};
use solana_program::sysvar::{instructions::Instructions as SysInstructions, SysvarId};

use crate::{
    borrow_order_operations, utils::ctx_event_emitter, BorrowOrderConfig, LendingError,
    LendingMarket, Obligation, Reserve,
};

pub fn process(
    ctx: Context<SetBorrowOrder>,
    order_config: BorrowOrderConfigArgs,
    min_expected_current_remaining_debt_amount: u64,
) -> Result<()> {
    let order_config = order_config.with_accounts(ctx.accounts);
    let lending_market = &ctx.accounts.lending_market.load()?;
    let reserve = &ctx.accounts.reserve.load()?;
    let obligation = &mut ctx.accounts.obligation.load_mut()?;
    let clock = Clock::get()?;

   
   
   
   
   
   

    require_gte!(
        obligation.borrow_order.remaining_debt_amount,
        min_expected_current_remaining_debt_amount,
        LendingError::ExpectationNotMet,
    );

    borrow_order_operations::set_borrow_order(
        lending_market,
        reserve,
        &mut obligation.borrow_order,
        order_config,
        &clock,
        ctx_event_emitter!(ctx),
    )?;
    Ok(())
}

#[event_cpi]
#[derive(Accounts)]
pub struct SetBorrowOrder<'info> {

    pub owner: Signer<'info>,


    #[account(mut, has_one = lending_market, has_one = owner)]
    pub obligation: AccountLoader<'info, Obligation>,


    pub lending_market: AccountLoader<'info, LendingMarket>,




    #[account(has_one = lending_market)]
    pub reserve: AccountLoader<'info, Reserve>,




   
   
   
   
   
   
    #[account(
        token::mint = debt_liquidity_mint,
        token::authority = owner,
        token::token_program = reserve.load()?.liquidity.token_program,
    )]
    pub filled_debt_destination: Box<InterfaceAccount<'info, TokenAccount>>,




   
    #[account(
        address = reserve.load()?.liquidity.mint_pubkey,
        mint::token_program = reserve.load()?.liquidity.token_program,
    )]
    pub debt_liquidity_mint: Box<InterfaceAccount<'info, Mint>>,

    /// CHECK: Sysvar Instruction allowing introspection, fixed address
    #[account(address = SysInstructions::id())]
    pub instruction_sysvar_account: AccountInfo<'info>,
}


#[derive(AnchorDeserialize, AnchorSerialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct BorrowOrderConfigArgs {
    pub remaining_debt_amount: u64,
    pub max_borrow_rate_bps: u32,
    pub min_debt_term_seconds: u64,
    pub fillable_until_timestamp: u64,
}

impl BorrowOrderConfigArgs {

    pub fn with_accounts(self, accounts: &SetBorrowOrder) -> Option<BorrowOrderConfig> {
        if self == Default::default() {
           
            return None;
        }
        let Self {
            remaining_debt_amount,
            max_borrow_rate_bps,
            min_debt_term_seconds,
            fillable_until_timestamp,
        } = self;
        Some(BorrowOrderConfig {
            debt_liquidity_mint: accounts.debt_liquidity_mint.key(),
            remaining_debt_amount,
            filled_debt_destination: accounts.filled_debt_destination.key(),
            max_borrow_rate_bps,
            min_debt_term_seconds,
            fillable_until_timestamp,
        })
    }
}
