// CPI Example: Deposit collateral and borrow from an Anchor program
//
// This file is a reference example — it is NOT compiled as part of
// klend-interface. It demonstrates how an on-chain Anchor program can
// use klend-interface instruction builders to CPI into Kamino Lending.
//
// Scenario: your program holds Token A in a vault, deposits it as
// collateral into a Klend obligation, then borrows Token B.
//
// To adapt this to your project:
// 1. Add `klend-interface` and `anchor-lang` to your Cargo.toml
// 2. Copy the relevant structs and handler logic below
// 3. Adjust PDA seeds and account constraints to match your program

use anchor_lang::prelude::*;
use klend_interface::{
    instructions::{borrow, deposit, obligation},
    pda as klend_pda,
    types::InitObligationArgs,
    KLEND_PROGRAM_ID,
};

declare_id!("YourProgram1111111111111111111111111111111111");

/// PDA seeds for the authority that owns the vaults and the obligation.
const AUTHORITY_SEED: &[u8] = b"authority";

#[program]
pub mod cpi_example {
    use super::*;

    /// Deposit `deposit_amount` of Token A as collateral, then borrow
    /// `borrow_amount` of Token B from Klend.
    ///
    /// The PDA `authority` acts as the obligation owner and signs all
    /// CPI calls via `invoke_signed`.
    pub fn deposit_and_borrow(
        ctx: Context<DepositAndBorrow>,
        deposit_amount: u64,
        borrow_amount: u64,
    ) -> Result<()> {
        let lending_market_key = ctx.accounts.lending_market.key();
        let authority_seeds: &[&[u8]] = &[
            AUTHORITY_SEED,
            lending_market_key.as_ref(),
            &[ctx.bumps.authority],
        ];

        // -----------------------------------------------------------------
        // 1. Init obligation (skip if already initialized)
        // -----------------------------------------------------------------
        let init_ix = obligation::init_obligation(
            obligation::InitObligationAccounts {
                obligation_owner: ctx.accounts.authority.key(),
                fee_payer: ctx.accounts.payer.key(),
                obligation: ctx.accounts.obligation.key(),
                lending_market: ctx.accounts.lending_market.key(),
                seed1_account: Pubkey::default(),
                seed2_account: Pubkey::default(),
                owner_user_metadata: ctx.accounts.owner_user_metadata.key(),
            },
            InitObligationArgs { tag: 0, id: 0 },
        );

        solana_program::program::invoke_signed(
            &init_ix,
            &[
                ctx.accounts.authority.to_account_info(),
                ctx.accounts.payer.to_account_info(),
                ctx.accounts.obligation.to_account_info(),
                ctx.accounts.lending_market.to_account_info(),
                // seed1_account and seed2_account are Pubkey::default() —
                // we still need to pass *some* AccountInfo. In practice you
                // would pass a dummy account or the system program. Here we
                // re-use system_program for both since the seeds are unused.
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
                ctx.accounts.owner_user_metadata.to_account_info(),
                ctx.accounts.rent.to_account_info(),
                ctx.accounts.system_program.to_account_info(),
            ],
            &[authority_seeds],
        )?;

        // -----------------------------------------------------------------
        // 2. Deposit Token A as collateral
        // -----------------------------------------------------------------
        let deposit_ix =
            deposit::deposit_reserve_liquidity_and_obligation_collateral_v2(
                deposit::DepositReserveLiquidityAndObligationCollateralV2Accounts {
                    owner: ctx.accounts.authority.key(),
                    obligation: ctx.accounts.obligation.key(),
                    lending_market: ctx.accounts.lending_market.key(),
                    lending_market_authority: ctx.accounts.lending_market_authority.key(),
                    reserve: ctx.accounts.deposit_reserve.key(),
                    reserve_liquidity_mint: ctx.accounts.deposit_reserve_liquidity_mint.key(),
                    reserve_liquidity_supply: ctx.accounts.deposit_reserve_liquidity_supply.key(),
                    reserve_collateral_mint: ctx.accounts.deposit_reserve_collateral_mint.key(),
                    reserve_destination_deposit_collateral: ctx
                        .accounts
                        .deposit_reserve_collateral_supply
                        .key(),
                    user_source_liquidity: ctx.accounts.vault_a.key(),
                    placeholder_user_destination_collateral: None,
                    liquidity_token_program: ctx.accounts.token_program.key(),
                    obligation_farm_user_state: None,
                    reserve_farm_state: None,
                },
                deposit_amount,
            );

        solana_program::program::invoke_signed(
            &deposit_ix,
            &[
                ctx.accounts.authority.to_account_info(),
                ctx.accounts.obligation.to_account_info(),
                ctx.accounts.lending_market.to_account_info(),
                ctx.accounts.lending_market_authority.to_account_info(),
                ctx.accounts.deposit_reserve.to_account_info(),
                ctx.accounts.deposit_reserve_liquidity_mint.to_account_info(),
                ctx.accounts.deposit_reserve_liquidity_supply.to_account_info(),
                ctx.accounts.deposit_reserve_collateral_mint.to_account_info(),
                ctx.accounts.deposit_reserve_collateral_supply.to_account_info(),
                ctx.accounts.vault_a.to_account_info(),
                ctx.accounts.klend_program.to_account_info(), // placeholder (None)
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.token_program.to_account_info(), // liquidity_token_program
                ctx.accounts.instructions_sysvar.to_account_info(),
                ctx.accounts.klend_program.to_account_info(), // farm user state (None)
                ctx.accounts.klend_program.to_account_info(), // reserve farm state (None)
                ctx.accounts.farms_program.to_account_info(),
            ],
            &[authority_seeds],
        )?;

        // -----------------------------------------------------------------
        // 3. Borrow Token B
        // -----------------------------------------------------------------
        //
        // remaining_accounts must include the deposit reserve so Klend can
        // verify collateral. Each deposit reserve is passed as:
        //   AccountMeta { pubkey, is_signer: false, is_writable: true }
        let remaining = vec![AccountMeta {
            pubkey: ctx.accounts.deposit_reserve.key(),
            is_signer: false,
            is_writable: true,
        }];

        let borrow_ix = borrow::borrow_obligation_liquidity_v2(
            borrow::BorrowObligationLiquidityV2Accounts {
                owner: ctx.accounts.authority.key(),
                obligation: ctx.accounts.obligation.key(),
                lending_market: ctx.accounts.lending_market.key(),
                lending_market_authority: ctx.accounts.lending_market_authority.key(),
                borrow_reserve: ctx.accounts.borrow_reserve.key(),
                borrow_reserve_liquidity_mint: ctx
                    .accounts
                    .borrow_reserve_liquidity_mint
                    .key(),
                reserve_source_liquidity: ctx
                    .accounts
                    .borrow_reserve_liquidity_supply
                    .key(),
                borrow_reserve_liquidity_fee_receiver: ctx
                    .accounts
                    .borrow_reserve_fee_receiver
                    .key(),
                user_destination_liquidity: ctx.accounts.vault_b.key(),
                referrer_token_state: None,
                token_program: ctx.accounts.token_program.key(),
                obligation_farm_user_state: None,
                reserve_farm_state: None,
            },
            borrow_amount,
            remaining,
        );

        solana_program::program::invoke_signed(
            &borrow_ix,
            &[
                ctx.accounts.authority.to_account_info(),
                ctx.accounts.obligation.to_account_info(),
                ctx.accounts.lending_market.to_account_info(),
                ctx.accounts.lending_market_authority.to_account_info(),
                ctx.accounts.borrow_reserve.to_account_info(),
                ctx.accounts.borrow_reserve_liquidity_mint.to_account_info(),
                ctx.accounts.borrow_reserve_liquidity_supply.to_account_info(),
                ctx.accounts.borrow_reserve_fee_receiver.to_account_info(),
                ctx.accounts.vault_b.to_account_info(),
                ctx.accounts.klend_program.to_account_info(), // referrer (None)
                ctx.accounts.token_program.to_account_info(),
                ctx.accounts.instructions_sysvar.to_account_info(),
                ctx.accounts.klend_program.to_account_info(), // farm user state (None)
                ctx.accounts.klend_program.to_account_info(), // reserve farm state (None)
                ctx.accounts.farms_program.to_account_info(),
                // remaining: deposit reserve (for collateral check)
                ctx.accounts.deposit_reserve.to_account_info(),
            ],
            &[authority_seeds],
        )?;

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Account validation struct
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct DepositAndBorrow<'info> {
    // -- Program-owned accounts --
    /// PDA authority that owns the vaults and the Klend obligation.
    /// Seeds: `[b"authority", lending_market.key()]`
    #[account(
        seeds = [AUTHORITY_SEED, lending_market.key().as_ref()],
        bump,
    )]
    pub authority: SystemAccount<'info>,

    /// Token A vault — holds the collateral token, owned by `authority`.
    #[account(mut, token::authority = authority)]
    pub vault_a: Account<'info, TokenAccount>,

    /// Token B vault — receives the borrowed token, owned by `authority`.
    #[account(mut, token::authority = authority)]
    pub vault_b: Account<'info, TokenAccount>,

    /// Fee payer for obligation init (can be the caller).
    #[account(mut)]
    pub payer: Signer<'info>,

    // -- Klend accounts (passed through for CPI) --
    /// CHECK: Klend lending market account.
    pub lending_market: UncheckedAccount<'info>,

    /// CHECK: Klend lending market authority PDA.
    /// Derived as `klend_pda::lending_market_authority(&KLEND_PROGRAM_ID, &lending_market)`.
    pub lending_market_authority: UncheckedAccount<'info>,

    /// CHECK: Obligation PDA owned by `authority`.
    /// Derived as `klend_pda::obligation(&KLEND_PROGRAM_ID, 0, 0, &authority, &lending_market, ...)`.
    #[account(mut)]
    pub obligation: UncheckedAccount<'info>,

    /// CHECK: User metadata PDA for `authority`.
    /// Derived as `klend_pda::user_metadata(&KLEND_PROGRAM_ID, &authority)`.
    pub owner_user_metadata: UncheckedAccount<'info>,

    // -- Deposit reserve (Token A) accounts --
    /// CHECK: Klend reserve for Token A.
    #[account(mut)]
    pub deposit_reserve: UncheckedAccount<'info>,

    /// CHECK: Token A mint.
    pub deposit_reserve_liquidity_mint: UncheckedAccount<'info>,

    /// CHECK: Reserve's Token A supply vault.
    /// Derived as `klend_pda::reserve_liquidity_supply(&KLEND_PROGRAM_ID, &deposit_reserve)`.
    #[account(mut)]
    pub deposit_reserve_liquidity_supply: UncheckedAccount<'info>,

    /// CHECK: Reserve's cToken mint.
    /// Derived as `klend_pda::reserve_collateral_mint(&KLEND_PROGRAM_ID, &deposit_reserve)`.
    #[account(mut)]
    pub deposit_reserve_collateral_mint: UncheckedAccount<'info>,

    /// CHECK: Reserve's cToken supply vault.
    /// Derived as `klend_pda::reserve_collateral_supply(&KLEND_PROGRAM_ID, &deposit_reserve)`.
    #[account(mut)]
    pub deposit_reserve_collateral_supply: UncheckedAccount<'info>,

    // -- Borrow reserve (Token B) accounts --
    /// CHECK: Klend reserve for Token B.
    #[account(mut)]
    pub borrow_reserve: UncheckedAccount<'info>,

    /// CHECK: Token B mint.
    pub borrow_reserve_liquidity_mint: UncheckedAccount<'info>,

    /// CHECK: Borrow reserve's Token B supply vault.
    /// Derived as `klend_pda::reserve_liquidity_supply(&KLEND_PROGRAM_ID, &borrow_reserve)`.
    #[account(mut)]
    pub borrow_reserve_liquidity_supply: UncheckedAccount<'info>,

    /// CHECK: Borrow reserve's fee receiver vault.
    /// Derived as `klend_pda::reserve_fee_receiver(&KLEND_PROGRAM_ID, &borrow_reserve)`.
    #[account(mut)]
    pub borrow_reserve_fee_receiver: UncheckedAccount<'info>,

    // -- Programs and sysvars --
    /// The Klend program.
    /// CHECK: Validated by address.
    #[account(address = KLEND_PROGRAM_ID)]
    pub klend_program: UncheckedAccount<'info>,

    /// CHECK: Kamino Farms program.
    pub farms_program: UncheckedAccount<'info>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,

    /// CHECK: Sysvar instructions (required by Klend).
    #[account(address = solana_program::sysvar::instructions::ID)]
    pub instructions_sysvar: UncheckedAccount<'info>,
}
