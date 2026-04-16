use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use crate::{
    instructions::{
        obligation::InitObligationAccounts,
        referrer::{init_user_metadata, InitUserMetadataAccounts},
    },
    pda, types, KLEND_PROGRAM_ID,
};

/// Build instructions to initialize user metadata.
///
/// Prerequisite for creating obligations.
///
/// Returns: `[init_user_metadata]`
pub fn init_user(
    owner: Pubkey,
    fee_payer: Pubkey,
    user_lookup_table: Pubkey,
    referrer_user_metadata: Option<Pubkey>,
) -> Vec<Instruction> {
    let (user_metadata_pda, _) = pda::user_metadata(&KLEND_PROGRAM_ID, &owner);

    vec![init_user_metadata(
        InitUserMetadataAccounts {
            owner,
            fee_payer,
            user_metadata: user_metadata_pda,
            referrer_user_metadata,
        },
        user_lookup_table,
    )]
}

/// Build instructions to initialize an obligation.
///
/// The obligation PDA and user-metadata PDA are derived automatically.
/// For a vanilla obligation (tag=0) use `Pubkey::default()` for seed1/seed2.
///
/// Returns: `[init_obligation]`
pub fn init_obligation(
    owner: Pubkey,
    fee_payer: Pubkey,
    lending_market: Pubkey,
    tag: u8,
    id: u8,
    seed1: Pubkey,
    seed2: Pubkey,
) -> Vec<Instruction> {
    let (obligation_pda, _) = pda::obligation(
        &KLEND_PROGRAM_ID,
        tag,
        id,
        &owner,
        &lending_market,
        &seed1,
        &seed2,
    );
    let (user_metadata_pda, _) = pda::user_metadata(&KLEND_PROGRAM_ID, &owner);

    vec![crate::instructions::obligation::init_obligation(
        InitObligationAccounts {
            obligation_owner: owner,
            fee_payer,
            obligation: obligation_pda,
            lending_market,
            seed1_account: seed1,
            seed2_account: seed2,
            owner_user_metadata: user_metadata_pda,
        },
        types::InitObligationArgs { tag, id },
    )]
}
