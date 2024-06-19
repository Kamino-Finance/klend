use anchor_lang::{
    prelude::{AccountInfo, CpiContext},
    Result,
};
use anchor_spl::token_interface;

use super::spltoken;

pub fn deposit_obligation_collateral_transfer<'a>(
    from: AccountInfo<'a>,
    to: AccountInfo<'a>,
    authority: AccountInfo<'a>,
    token_program: AccountInfo<'a>,
    collateral_amount: u64,
) -> Result<()> {
    #[allow(deprecated)]
    token_interface::transfer(
        CpiContext::new(
            token_program,
            token_interface::Transfer {
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
    liquidity_mint: AccountInfo<'a>,
    liquidity_token_program: AccountInfo<'a>,
    collateral_mint: AccountInfo<'a>,
    collateral_token_program: AccountInfo<'a>,
    destination_collateral: AccountInfo<'a>,
    mint_authority: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    liquidity_deposit_amount: u64,
    liquidity_decimals: u8,
    collateral_mint_amount: u64,
) -> Result<()> {
    token_interface::transfer_checked(
        CpiContext::new(
            liquidity_token_program.clone(),
            token_interface::TransferChecked {
                from: source_liquidity_deposit,
                to: destination_liquidity_deposit,
                authority: user_authority,
                mint: liquidity_mint,
            },
        ),
        liquidity_deposit_amount,
        liquidity_decimals,
    )?;

    spltoken::mint(
        collateral_token_program,
        collateral_mint,
        mint_authority,
        destination_collateral,
        authority_signer_seeds,
        collateral_mint_amount,
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn deposit_reserve_liquidity_and_obligation_collateral_transfer<'a>(
    source_liquidity_deposit: AccountInfo<'a>,
    destination_liquidity_deposit: AccountInfo<'a>,
    user_authority: AccountInfo<'a>,
    liquidity_mint: AccountInfo<'a>,
    liquidity_token_program: AccountInfo<'a>,
    collateral_mint: AccountInfo<'a>,
    collateral_supply_vault: AccountInfo<'a>,
    collateral_token_program: AccountInfo<'a>,
    mint_authority: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    liquidity_deposit_amount: u64,
    liquidity_decimals: u8,
    collateral_mint_amount: u64,
) -> Result<()> {
    deposit_reserve_liquidity_transfer(
        source_liquidity_deposit,
        destination_liquidity_deposit,
        user_authority,
        liquidity_mint,
        liquidity_token_program,
        collateral_mint,
        collateral_token_program,
        collateral_supply_vault,
        mint_authority,
        authority_signer_seeds,
        liquidity_deposit_amount,
        liquidity_decimals,
        collateral_mint_amount,
    )
}

pub fn withdraw_obligation_collateral_transfer<'a>(
    token_program: AccountInfo<'a>,
    destination_collateral: AccountInfo<'a>,
    source_collateral: AccountInfo<'a>,
    lending_market_authority: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    withdraw_amount: u64,
) -> Result<()> {
    #[allow(deprecated)]
    token_interface::transfer(
        CpiContext::new_with_signer(
            token_program,
            token_interface::Transfer {
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
    collateral_token_program: AccountInfo<'a>,
    liquidity_token_program: AccountInfo<'a>,
    reserve_liquidity_mint: AccountInfo<'a>,
    reserve_collateral_mint: AccountInfo<'a>,
    burn_source_collateral: AccountInfo<'a>,
    user_authority: AccountInfo<'a>,
    reserve_liquidity_supply: AccountInfo<'a>,
    destination_liquidity: AccountInfo<'a>,
    lending_market_authority: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    collateral_amount: u64,
    liquidity_amount: u64,
    liquidity_decimals: u8,
) -> Result<()> {
    spltoken::burn(
        reserve_collateral_mint,
        burn_source_collateral,
        user_authority,
        collateral_token_program.clone(),
        collateral_amount,
    )?;

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            liquidity_token_program,
            token_interface::TransferChecked {
                from: reserve_liquidity_supply,
                to: destination_liquidity,
                authority: lending_market_authority,
                mint: reserve_liquidity_mint,
            },
            &[authority_signer_seeds],
        ),
        liquidity_amount,
        liquidity_decimals,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn withdraw_and_redeem_reserve_collateral_transfer<'a>(
    collateral_token_program: AccountInfo<'a>,
    liquidity_token_program: AccountInfo<'a>,
    reserve_liquidity_mint: AccountInfo<'a>,
    reserve_collateral_mint: AccountInfo<'a>,
    burn_reserve_source_collateral: AccountInfo<'a>,
    reserve_liquidity_supply: AccountInfo<'a>,
    user_destination_liquidity: AccountInfo<'a>,
    lending_market_authority: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    collateral_amount: u64,
    liquidity_amount: u64,
    liquidity_decimals: u8,
) -> Result<()> {
    spltoken::burn_with_signer(
        reserve_collateral_mint,
        burn_reserve_source_collateral,
        lending_market_authority.clone(),
        collateral_token_program.clone(),
        collateral_amount,
        &[authority_signer_seeds],
    )?;

    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            liquidity_token_program,
            token_interface::TransferChecked {
                from: reserve_liquidity_supply,
                to: user_destination_liquidity,
                authority: lending_market_authority,
                mint: reserve_liquidity_mint,
            },
            &[authority_signer_seeds],
        ),
        liquidity_amount,
        liquidity_decimals,
    )?;
    Ok(())
}

pub fn repay_obligation_liquidity_transfer<'a>(
    token_program: AccountInfo<'a>,
    liquidity_mint: AccountInfo<'a>,
    user_liquidity: AccountInfo<'a>,
    reserve_liquidity: AccountInfo<'a>,
    user_authority: AccountInfo<'a>,
    repay_amount: u64,
    decimals: u8,
) -> Result<()> {
    token_interface::transfer_checked(
        CpiContext::new(
            token_program,
            token_interface::TransferChecked {
                from: user_liquidity,
                to: reserve_liquidity,
                authority: user_authority,
                mint: liquidity_mint,
            },
        ),
        repay_amount,
        decimals,
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn borrow_obligation_liquidity_transfer<'a>(
    token_program: AccountInfo<'a>,
    liquidity_mint: AccountInfo<'a>,
    reserve_liquidity: AccountInfo<'a>,
    user_liquidity: AccountInfo<'a>,
    lending_market_authority: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    liquidity_amount: u64,
    liquidity_decimals: u8,
) -> Result<()> {
    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            token_program,
            token_interface::TransferChecked {
                from: reserve_liquidity,
                to: user_liquidity,
                authority: lending_market_authority,
                mint: liquidity_mint,
            },
            &[authority_signer_seeds],
        ),
        liquidity_amount,
        liquidity_decimals,
    )?;

    Ok(())
}

pub fn pay_borrowing_fees_transfer<'a>(
    token_program: AccountInfo<'a>,
    liquidity_mint: AccountInfo<'a>,
    user_liquidity: AccountInfo<'a>,
    fee_collector: AccountInfo<'a>,
    user_authority: AccountInfo<'a>,
    fee: u64,
    decimals: u8,
) -> Result<()> {
    token_interface::transfer_checked(
        CpiContext::new(
            token_program,
            token_interface::TransferChecked {
                from: user_liquidity,
                to: fee_collector,
                authority: user_authority,
                mint: liquidity_mint,
            },
        ),
        fee,
        decimals,
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn send_origination_fees_transfer<'a>(
    token_program: AccountInfo<'a>,
    reserve_liquidity_mint: AccountInfo<'a>,
    reserve_liquidity: AccountInfo<'a>,
    fee_receiver: AccountInfo<'a>,
    lending_market_authority: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    fee: u64,
    decimals: u8,
) -> Result<()> {
    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            token_program,
            token_interface::TransferChecked {
                to: fee_receiver,
                from: reserve_liquidity,
                authority: lending_market_authority,
                mint: reserve_liquidity_mint,
            },
            &[authority_signer_seeds],
        ),
        fee,
        decimals,
    )?;

    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn withdraw_fees_from_reserve<'a>(
    token_program: AccountInfo<'a>,
    reserve_liquidity_mint: AccountInfo<'a>,
    reserve_supply_liquidity: AccountInfo<'a>,
    fee_receiver: AccountInfo<'a>,
    lending_market_authority: AccountInfo<'a>,
    authority_signer_seeds: &[&[u8]],
    withdraw_amount: u64,
    mint_decimals: u8,
) -> Result<()> {
    token_interface::transfer_checked(
        CpiContext::new_with_signer(
            token_program,
            token_interface::TransferChecked {
                from: reserve_supply_liquidity,
                to: fee_receiver,
                authority: lending_market_authority,
                mint: reserve_liquidity_mint,
            },
            &[authority_signer_seeds],
        ),
        withdraw_amount,
        mint_decimals,
    )?;

    Ok(())
}
