use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use crate::{discriminators, types::ReserveConfigCustomizationArgs, util::*, KLEND_PROGRAM_ID};

// ---------------------------------------------------------------------------
// clone_reserve_config
// ---------------------------------------------------------------------------

pub struct CloneReserveConfigAccounts {
    pub signer: Pubkey,
    pub target_lending_market: Pubkey,
    pub source_reserve: Pubkey,
    pub target_reserve: Pubkey,
}

pub fn clone_reserve_config(
    accounts: CloneReserveConfigAccounts,
    customizations: ReserveConfigCustomizationArgs,
) -> Instruction {
    use borsh::BorshSerialize;

    let mut data = discriminators::CLONE_RESERVE_CONFIG.to_vec();
    customizations.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.signer),
            readonly(accounts.target_lending_market),
            readonly(accounts.source_reserve),
            writable(accounts.target_reserve),
        ],
        data,
    }
}
