use anchor_lang::{
    prelude::{AccountInfo, CpiContext},
    Result,
};
use anchor_spl::token;

use super::spltoken;

pub fn deposit_obligation_collateral_transfer<'a>(
    from: AccountInfo<'a>,
    to: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    collateral_amount: u64,
) -> Result<()> {
    token::transfer(
        CpiContext::new(
            token_program,
            anchor_spl::token::Transfer {
                from,
                to,
                authority,
            },
        ),
        collateral_amount,
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn deposit_reserve_liquidity_transfer<'a>(
    source_liquidity_deposit: AccountInfo<'a>,
    destination_liquidity_deposit: AccountInfo<'a>,
    user_authority: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    collateral_mint: AccountInfo<'a>,
    destination_collateral: AccountInfo<'a>,
    mint_authority: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    liquidity_deposit_amount: u64,
    collateral_mint_amount: u64,
) -> Result<()> {
    token::transfer(
        CpiContext::new(
            token_program.clone(),
            anchor_spl::token::Transfer {
                from: source_liquidity_deposit,
                to: destination_liquidity_deposit,
                authority: user_authority,
            },
        ),
        liquidity_deposit_amount,
    )?;

    spltoken::mint(
        token_program,
        collateral_mint,
        mint_authority,
        destination_collateral,
        authority_signer_seeds,
        collateral_mint_amount,
    )?;

    Ok(())
}

pub fn withdraw_obligation_collateral_transfer<'a>(
    token_program: AccountInfo<'a>,
    destination_collateral: AccountInfo<'a>,
    source_collateral: AccountInfo<'a>,
    lending_market_authority: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    withdraw_amount: u64,
) -> Result<()> {
    token::transfer(
        CpiContext::new_with_signer(
            token_program,
            anchor_spl::token::Transfer {
                to: destination_collateral,
                from: source_collateral,
                authority: lending_market_authority,
            },
            &[authority_signer_seeds],
        ),
        withdraw_amount,
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn redeem_reserve_collateral_transfer<'a>(
    token_program: AccountInfo<'a>,
    reserve_collateral_mint: AccountInfo<'a>,
    burn_source_collateral: AccountInfo<'a>,
    user_authority: AccountInfo<'a>,
    reserve_liquidity_supply: AccountInfo<'a>,
    destination_liquidity: AccountInfo<'a>,
    lending_market_authority: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    collateral_amount: u64,
    liquidity_amount: u64,
) -> Result<()> {
    spltoken::burn(
        reserve_collateral_mint,
        burn_source_collateral,
        user_authority,
        token_program.clone(),
        collateral_amount,
    )?;

    token::transfer(
        CpiContext::new_with_signer(
            token_program,
            anchor_spl::token::Transfer {
                from: reserve_liquidity_supply,
                to: destination_liquidity,
                authority: lending_market_authority,
            },
            &[authority_signer_seeds],
        ),
        liquidity_amount,
    )?;
    Ok(())
}

pub fn repay_obligation_liquidity_transfer<'a>(
    token_program: AccountInfo<'a>,
    user_liquidity: AccountInfo<'a>,
    reserve_liquidity: AccountInfo<'a>,
    user_authority: AccountInfo<'a>,
    repay_amount: u64,
) -> Result<()> {
    token::transfer(
        CpiContext::new(
            token_program,
            anchor_spl::token::Transfer {
                from: user_liquidity,
                to: reserve_liquidity,
                authority: user_authority,
            },
        ),
        repay_amount,
    )?;

    Ok(())
}

pub fn borrow_obligation_liquidity_transfer<'a>(
    token_program: AccountInfo<'a>,
    reserve_liquidity: AccountInfo<'a>,
    user_liquidity: AccountInfo<'a>,
    lending_market_authority: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    liquidity_amount: u64,
) -> Result<()> {
    token::transfer(
        CpiContext::new_with_signer(
            token_program,
            anchor_spl::token::Transfer {
                from: reserve_liquidity,
                to: user_liquidity,
                authority: lending_market_authority,
            },
            &[authority_signer_seeds],
        ),
        liquidity_amount,
    )?;

    Ok(())
}

pub fn pay_borrowing_fees_transfer<'a>(
    token_program: AccountInfo<'a>,
    user_liquidity: AccountInfo<'a>,
    fee_collector: AccountInfo<'a>,
    user_authority: AccountInfo<'a>,
    fee: u64,
) -> Result<()> {
    token::transfer(
        CpiContext::new(
            token_program,
            anchor_spl::token::Transfer {
                from: user_liquidity,
                to: fee_collector,
                authority: user_authority,
            },
        ),
        fee,
    )?;

    Ok(())
}

pub fn send_origination_fees_transfer<'a>(
    token_program: AccountInfo<'a>,
    reserve_liquidity: AccountInfo<'a>,
    fee_receiver: AccountInfo<'a>,
    lending_market_authority: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    fee: u64,
) -> Result<()> {
    token::transfer(
        CpiContext::new_with_signer(
            token_program,
            anchor_spl::token::Transfer {
                to: fee_receiver,
                from: reserve_liquidity,
                authority: lending_market_authority,
            },
            &[authority_signer_seeds],
        ),
        fee,
    )?;

    Ok(())
}

pub fn withdraw_fees_from_reserve<'a>(
    token_program: AccountInfo<'a>,
    reserve_supply_liquidity: AccountInfo<'a>,
    fee_receiver: AccountInfo<'a>,
    lending_market_authority: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    withdraw_amount: u64,
) -> Result<()> {
    token::transfer(
        CpiContext::new_with_signer(
            token_program,
            anchor_spl::token::Transfer {
                from: reserve_supply_liquidity,
                to: fee_receiver,
                authority: lending_market_authority,
            },
            &[authority_signer_seeds],
        ),
        withdraw_amount,
    )?;

    Ok(())
}
