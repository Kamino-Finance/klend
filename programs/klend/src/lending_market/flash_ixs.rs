use anchor_lang::{
    err, prelude::*, solana_program::instruction::Instruction, Discriminator, Key, Result,
};

use super::ix_utils::{self, InstructionLoader};
use crate::{
    handlers::{
        handler_flash_borrow_reserve_liquidity::FlashBorrowReserveLiquidity,
        handler_flash_repay_reserve_liquidity::FlashRepayReserveLiquidity,
    },
    instruction::{
        FlashBorrowReserveLiquidity as FlashBorrowReserveLiquidityArgs,
        FlashRepayReserveLiquidity as FlashRepayReserveLiquidityArgs,
    },
    LendingError,
};

pub fn flash_repay_checks(
    ctx: &Context<FlashRepayReserveLiquidity>,
    borrow_instruction_index: u8,
    liquidity_amount: u64,
) -> anchor_lang::Result<()> {
    let instruction_loader = ix_utils::BpfInstructionLoader {
        instruction_sysvar_account_info: &ctx.accounts.sysvar_info,
    };
    let current_index: usize = instruction_loader.load_current_index()?.into();
    if instruction_loader.is_flash_forbidden_cpi_call()? {
        msg!("Flash Repay was called via CPI!");
        return err!(LendingError::FlashRepayCpi);
    }

    if (borrow_instruction_index as usize) > current_index {
        msg!(
            "Flash repay: borrow instruction index {} has to be less than current index {}",
            borrow_instruction_index,
            current_index
        );
        return err!(LendingError::InvalidFlashRepay);
    }

    let ixn = instruction_loader.load_instruction_at(borrow_instruction_index as usize)?;
    if ixn.program_id != *ctx.program_id {
        msg!(
            "Flash repay: supplied instruction index {} doesn't belong to program id {}",
            borrow_instruction_index,
            *ctx.program_id
        );
        return err!(LendingError::InvalidFlashRepay);
    }

    let discriminator = FlashBorrowReserveLiquidityArgs::DISCRIMINATOR;

    if ixn.data[..8] != discriminator {
        msg!("Flash repay: Supplied borrow instruction index is not a flash borrow");
        return err!(LendingError::InvalidFlashRepay);
    }
    let borrow_liquidity_amount = u64::from_le_bytes(ixn.data[8..16].try_into().unwrap());

    if ixn.accounts[3].pubkey != ctx.accounts.reserve.key() {
        msg!("Invalid reserve account on flash repay");
        return err!(LendingError::InvalidFlashRepay);
    }

    if liquidity_amount != borrow_liquidity_amount {
        msg!("Liquidity amount for flash repay doesn't match borrow");
        return err!(LendingError::InvalidFlashRepay);
    }

    Ok(())
}

pub fn flash_borrow_checks(
    ctx: &Context<FlashBorrowReserveLiquidity>,
    liquidity_amount: u64,
) -> Result<()> {
    let instruction_loader = ix_utils::BpfInstructionLoader {
        instruction_sysvar_account_info: &ctx.accounts.sysvar_info,
    };
    flash_borrow_checks_internal(liquidity_amount, &instruction_loader)
}

fn flash_borrow_checks_internal(
    liquidity_amount: u64,
    instruction_loader: &impl InstructionLoader,
) -> Result<()> {
    let current_index: usize = instruction_loader.load_current_index()?.into();
    if instruction_loader.is_flash_forbidden_cpi_call()? {
        msg!("Flash Borrow was called via CPI!");
        return err!(LendingError::FlashBorrowCpi);
    }

    let borrow_ix = instruction_loader.load_instruction_at(current_index)?;

    let ix_iterator = ix_utils::IxIterator::new_at(current_index + 1, instruction_loader);
    let mut found_repay_ix = false;

    let flash_repay_discriminator = FlashRepayReserveLiquidityArgs::DISCRIMINATOR;
    let flash_borrow_discriminator = FlashBorrowReserveLiquidityArgs::DISCRIMINATOR;

    for ixn in ix_iterator {
        let ixn = ixn?;
        if ixn.program_id != crate::ID {
            continue;
        }

        if ixn.data[..8] == flash_borrow_discriminator {
            msg!("Multiple flash borrows not allowed");
            return err!(LendingError::MultipleFlashBorrows);
        }

        if ixn.data[..8] == flash_repay_discriminator {
            if found_repay_ix {
                msg!("Multiple flash repays not allowed");
                return err!(LendingError::MultipleFlashBorrows);
            }
            flash_borrow_check_matching_repay(liquidity_amount, &borrow_ix, &ixn, current_index)?;

            found_repay_ix = true;
        }
    }

    if !found_repay_ix {
        msg!("No flash repay found");
        return err!(LendingError::NoFlashRepayFound);
    }

    Ok(())
}

fn flash_borrow_check_matching_repay(
    liquidity_amount: u64,
    borrow_ix: &Instruction,
    repay_ix: &Instruction,
    borrow_index: usize,
) -> Result<()> {
    let repay_ix_data = FlashRepayReserveLiquidityArgs::try_from_slice(&repay_ix.data[8..])?;

    let repay_liquidity_amount = repay_ix_data.liquidity_amount;
    let borrow_instruction_index = repay_ix_data.borrow_instruction_index;

    if repay_liquidity_amount != liquidity_amount {
        msg!("Liquidity amount for flash repay doesn't match borrow");
        return err!(LendingError::InvalidFlashRepay);
    }
    if (usize::from(borrow_instruction_index)) != borrow_index {
        msg!(
            "Borrow instruction index {borrow_instruction_index} for flash repay doesn't match current index {borrow_index}",
        );
        return err!(LendingError::InvalidFlashRepay);
    }

    if repay_ix.accounts.len() != borrow_ix.accounts.len() {
        msg!("Number of accounts mismatch between first and second ix of couple");
        return err!(LendingError::InvalidFlashRepay);
    }

    for (idx, (account_borrow, account_repay)) in borrow_ix
        .accounts
        .iter()
        .zip(repay_ix.accounts.iter())
        .enumerate()
    {
        let account_borrow_pk = &account_borrow.pubkey;
        let account_repay_pk = &account_repay.pubkey;
        if account_borrow_pk != account_repay_pk {
            msg!("Some accounts in flash tx couple differs. index: {idx}, borrow:{account_borrow_pk}, repay:{account_repay_pk}",);
            return err!(LendingError::InvalidFlashRepay);
        }
    }

    Ok(())
}
