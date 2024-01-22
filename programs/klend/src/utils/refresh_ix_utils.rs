use anchor_lang::{prelude::*, Discriminator};
use solana_program::log::sol_log_compute_units;

use crate::{
    instruction::{RefreshObligation, RefreshObligationFarmsForReserve, RefreshReserve},
    lending_market::ix_utils::{BpfInstructionLoader, InstructionLoader},
    LendingError, Reserve, ReserveFarmKind,
};

#[derive(Debug, Clone)]
pub enum RequiredIxType {
    RefreshReserve,
    RefreshFarmsForObligationForReserve,
    RefreshObligation,
}

#[derive(Debug, Clone)]
pub struct RequiredIx {
    pub kind: RequiredIxType,
    pub accounts: Vec<(Pubkey, usize)>,
}

impl RequiredIx {
    pub fn discriminator(&self) -> [u8; 8] {
        match self.kind {
            RequiredIxType::RefreshReserve => RefreshReserve::DISCRIMINATOR,
            RequiredIxType::RefreshFarmsForObligationForReserve => {
                RefreshObligationFarmsForReserve::DISCRIMINATOR
            }
            RequiredIxType::RefreshObligation => RefreshObligation::DISCRIMINATOR,
        }
    }
}

pub fn check_refresh(
    instruction_sysvar_account_info: &AccountInfo,
    reserves: &[(Pubkey, &Reserve)],
    obligation_address: &Pubkey,
    modes: &[ReserveFarmKind],
) -> Result<()> {
    msg!("Beginning check_refresh");
    sol_log_compute_units();

    let ix_loader = BpfInstructionLoader {
        instruction_sysvar_account_info,
    };

    #[cfg(not(feature = "staging"))]
    if ix_loader.is_forbidden_cpi_call()? {
        msg!("Instruction was called via CPI!");
        return err!(LendingError::CpiDisabled);
    }
    let current_idx: usize = ix_loader.load_current_index().unwrap().into();
    let check_ixns = |required_ixns: Vec<RequiredIx>, ix_type: AppendedIxType| -> Result<()> {
        for (i, required_ix) in required_ixns.iter().enumerate() {
            let offset = match ix_type {
                AppendedIxType::PreIxs => current_idx.checked_sub(i + 1).ok_or_else(|| {
                    msg!(
                        "current_idx: {}, i: {}, required_ix {:?}",
                        current_idx,
                        i,
                        required_ix
                    );
                    error!(LendingError::IncorrectInstructionInPosition)
                })?,
                AppendedIxType::PostIxs => current_idx.checked_add(i + 1).ok_or_else(|| {
                    msg!(
                        "current_idx: {}, i: {}, required_ix {:?}",
                        current_idx,
                        i,
                        required_ix
                    );
                    LendingError::IncorrectInstructionInPosition
                })?,
            };

            let ix = ix_loader
                .load_instruction_at(offset)
                .map_err(|_| LendingError::IncorrectInstructionInPosition)?;

            let ix_discriminator: [u8; 8] = ix.data[0..8].try_into().unwrap();

            require_keys_eq!(ix.program_id, crate::id());

            let ix_discriminator_matches = ix_discriminator == required_ix.discriminator();
            if !ix_discriminator_matches {
                for (i, ix) in required_ixns.iter().enumerate() {
                    msg!("Required ix: {} {:?}", i, ix);
                }
            }

            require!(
                ix_discriminator_matches,
                LendingError::IncorrectInstructionInPosition
            );

            for (key, index) in required_ix.accounts.iter() {
                require_keys_eq!(
                    ix.accounts
                        .get(*index)
                        .ok_or(LendingError::IncorrectInstructionInPosition)?
                        .pubkey,
                    *key
                );
            }
        }

        Ok(())
    };

    let refresh_reserve_ixs = if reserves.len() == 2 && reserves[0].0 == reserves[1].0 {
        reserves.len() - 1
    } else {
        reserves.len()
    };

    let mut required_pre_ixs = Vec::with_capacity(refresh_reserve_ixs + 1 + refresh_reserve_ixs);
    let mut required_post_ixs = Vec::with_capacity(refresh_reserve_ixs);
    for reserve in reserves.iter().take(refresh_reserve_ixs) {
        required_pre_ixs.push(RequiredIx {
            kind: RequiredIxType::RefreshReserve,
            accounts: vec![(reserve.0, 0)],
        });
    }

    required_pre_ixs.push(RequiredIx {
        kind: RequiredIxType::RefreshObligation,
        accounts: vec![(*obligation_address, 1)],
    });

    reserves
        .iter()
        .zip(modes)
        .for_each(|((reserve_address, reserve), farm_type)| {
            if reserve.get_farm(*farm_type) != Pubkey::default() {
                let required_ix = RequiredIx {
                    kind: RequiredIxType::RefreshFarmsForObligationForReserve,
                    accounts: vec![
                        (*reserve_address, 3),
                        (*obligation_address, 1),
                        (reserve.get_farm(*farm_type), 4),
                    ],
                };
                required_pre_ixs.push(required_ix.clone());
                required_post_ixs.push(required_ix);
            }
        });

    required_pre_ixs.reverse();
    check_ixns(required_pre_ixs, AppendedIxType::PreIxs)?;
    check_ixns(required_post_ixs, AppendedIxType::PostIxs)?;

    msg!("Finished check_refresh");
    sol_log_compute_units();

    Ok(())
}

enum AppendedIxType {
    PreIxs,
    PostIxs,
}

fn _discriminator_to_ix(discriminator: [u8; 8]) -> &'static str {
    match discriminator {
        x if x == RefreshReserve::discriminator() => "RefreshReserve",
        x if x == RefreshObligation::discriminator() => "RefreshObligation",
        x if x == RefreshObligationFarmsForReserve::discriminator() => {
            "RefreshObligationFarmsForReserve"
        }
        _ => "unknown",
    }
}
