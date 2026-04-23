use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::{
    discriminators, types::UpdateObligationConfigMode, util::*, FARMS_PROGRAM_ID, KLEND_PROGRAM_ID,
    SYSTEM_PROGRAM_ID, SYSVAR_INSTRUCTIONS_ID, SYSVAR_RENT_ID,
};

// ---------------------------------------------------------------------------
// init_obligation
// ---------------------------------------------------------------------------

pub struct InitObligationAccounts {
    pub obligation_owner: Pubkey,
    pub fee_payer: Pubkey,
    pub obligation: Pubkey,
    pub lending_market: Pubkey,
    pub seed1_account: Pubkey,
    pub seed2_account: Pubkey,
    pub owner_user_metadata: Pubkey,
}

pub fn init_obligation(
    accounts: InitObligationAccounts,
    args: crate::types::InitObligationArgs,
) -> Instruction {
    let mut data = discriminators::INIT_OBLIGATION.to_vec();
    args.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.obligation_owner),
            signer_writable(accounts.fee_payer),
            writable(accounts.obligation),
            readonly(accounts.lending_market),
            readonly(accounts.seed1_account),
            readonly(accounts.seed2_account),
            readonly(accounts.owner_user_metadata),
            readonly(SYSVAR_RENT_ID),
            readonly(SYSTEM_PROGRAM_ID),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// init_obligation_farms_for_reserve
// ---------------------------------------------------------------------------

pub struct InitObligationFarmsForReserveAccounts {
    pub payer: Pubkey,
    pub owner: Pubkey,
    pub obligation: Pubkey,
    pub lending_market_authority: Pubkey,
    pub reserve: Pubkey,
    pub reserve_farm_state: Pubkey,
    pub obligation_farm: Pubkey,
    pub lending_market: Pubkey,
}

pub fn init_obligation_farms_for_reserve(
    accounts: InitObligationFarmsForReserveAccounts,
    mode: u8,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        mode: u8,
    }

    let mut data = discriminators::INIT_OBLIGATION_FARMS_FOR_RESERVE.to_vec();
    Args { mode }.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer_writable(accounts.payer),
            readonly(accounts.owner),
            writable(accounts.obligation),
            readonly(accounts.lending_market_authority),
            writable(accounts.reserve),
            writable(accounts.reserve_farm_state),
            writable(accounts.obligation_farm),
            readonly(accounts.lending_market),
            readonly(FARMS_PROGRAM_ID),
            readonly(SYSVAR_RENT_ID),
            readonly(SYSTEM_PROGRAM_ID),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// refresh_obligation_farms_for_reserve
// ---------------------------------------------------------------------------

pub struct RefreshObligationFarmsForReserveAccounts {
    pub crank: Pubkey,
    pub obligation: Pubkey,
    pub lending_market_authority: Pubkey,
    pub reserve: Pubkey,
    pub reserve_farm_state: Pubkey,
    pub obligation_farm_user_state: Pubkey,
    pub lending_market: Pubkey,
}

pub fn refresh_obligation_farms_for_reserve(
    accounts: RefreshObligationFarmsForReserveAccounts,
    mode: u8,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        mode: u8,
    }

    let mut data = discriminators::REFRESH_OBLIGATION_FARMS_FOR_RESERVE.to_vec();
    Args { mode }.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.crank),
            readonly(accounts.obligation),
            readonly(accounts.lending_market_authority),
            readonly(accounts.reserve),
            writable(accounts.reserve_farm_state),
            writable(accounts.obligation_farm_user_state),
            readonly(accounts.lending_market),
            readonly(FARMS_PROGRAM_ID),
            readonly(SYSVAR_RENT_ID),
            readonly(SYSTEM_PROGRAM_ID),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// request_elevation_group
// ---------------------------------------------------------------------------

pub struct RequestElevationGroupAccounts {
    pub owner: Pubkey,
    pub obligation: Pubkey,
    pub lending_market: Pubkey,
}

pub fn request_elevation_group(
    accounts: RequestElevationGroupAccounts,
    elevation_group: u8,
    remaining_accounts: Vec<AccountMeta>,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        elevation_group: u8,
    }

    let mut data = discriminators::REQUEST_ELEVATION_GROUP.to_vec();
    Args { elevation_group }.serialize(&mut data).unwrap();

    let mut account_metas = vec![
        signer(accounts.owner),
        writable(accounts.obligation),
        readonly(accounts.lending_market),
    ];
    account_metas.extend(remaining_accounts);

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: account_metas,
        data,
    }
}

// ---------------------------------------------------------------------------
// update_obligation_config
// ---------------------------------------------------------------------------

pub struct UpdateObligationConfigAccounts {
    pub owner: Pubkey,
    pub obligation: Pubkey,
    pub borrow_reserve: Option<Pubkey>,
    pub deposit_reserve: Option<Pubkey>,
    pub lending_market: Pubkey,
}

pub fn update_obligation_config(
    accounts: UpdateObligationConfigAccounts,
    mode: UpdateObligationConfigMode,
    value: Vec<u8>,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        mode: UpdateObligationConfigMode,
        value: Vec<u8>,
    }

    let mut data = discriminators::UPDATE_OBLIGATION_CONFIG.to_vec();
    Args { mode, value }.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.owner),
            writable(accounts.obligation),
            optional_account(&KLEND_PROGRAM_ID, accounts.borrow_reserve, false),
            optional_account(&KLEND_PROGRAM_ID, accounts.deposit_reserve, false),
            readonly(accounts.lending_market),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// initiate_obligation_ownership_transfer
// ---------------------------------------------------------------------------

pub struct InitiateObligationOwnershipTransferAccounts {
    pub owner: Pubkey,
    pub obligation: Pubkey,
}

pub fn initiate_obligation_ownership_transfer(
    accounts: InitiateObligationOwnershipTransferAccounts,
    new_owner: Pubkey,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        new_owner: Pubkey,
    }

    let mut data = discriminators::INITIATE_OBLIGATION_OWNERSHIP_TRANSFER.to_vec();
    Args { new_owner }.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.owner),
            writable(accounts.obligation),
            readonly(SYSVAR_INSTRUCTIONS_ID),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// approve_obligation_ownership_transfer
// ---------------------------------------------------------------------------

pub struct ApproveObligationOwnershipTransferAccounts {
    pub global_admin: Pubkey,
    pub global_config: Pubkey,
    pub obligation: Pubkey,
    pub pending_owner: Pubkey,
}

pub fn approve_obligation_ownership_transfer(
    accounts: ApproveObligationOwnershipTransferAccounts,
) -> Instruction {
    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.global_admin),
            readonly(accounts.global_config),
            writable(accounts.obligation),
            readonly(accounts.pending_owner),
        ],
        data: discriminators::APPROVE_OBLIGATION_OWNERSHIP_TRANSFER.to_vec(),
    }
}

// ---------------------------------------------------------------------------
// accept_obligation_ownership
// ---------------------------------------------------------------------------

pub struct AcceptObligationOwnershipAccounts {
    pub pending_owner: Pubkey,
    pub obligation: Pubkey,
}

pub fn accept_obligation_ownership(accounts: AcceptObligationOwnershipAccounts) -> Instruction {
    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.pending_owner),
            writable(accounts.obligation),
            readonly(SYSVAR_INSTRUCTIONS_ID),
        ],
        data: discriminators::ACCEPT_OBLIGATION_OWNERSHIP.to_vec(),
    }
}

// ---------------------------------------------------------------------------
// abort_obligation_ownership_transfer
// ---------------------------------------------------------------------------

pub struct AbortObligationOwnershipTransferAccounts {
    pub owner: Pubkey,
    pub obligation: Pubkey,
}

pub fn abort_obligation_ownership_transfer(
    accounts: AbortObligationOwnershipTransferAccounts,
) -> Instruction {
    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.owner),
            writable(accounts.obligation),
            readonly(SYSVAR_INSTRUCTIONS_ID),
        ],
        data: discriminators::ABORT_OBLIGATION_OWNERSHIP_TRANSFER.to_vec(),
    }
}
