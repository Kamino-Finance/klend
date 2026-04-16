use borsh::BorshSerialize;
use solana_instruction::Instruction;
use solana_pubkey::Pubkey;

use crate::{discriminators, util::*, KLEND_PROGRAM_ID, SYSTEM_PROGRAM_ID, SYSVAR_RENT_ID};

// ---------------------------------------------------------------------------
// init_referrer_token_state
// ---------------------------------------------------------------------------

pub struct InitReferrerTokenStateAccounts {
    pub payer: Pubkey,
    pub lending_market: Pubkey,
    pub reserve: Pubkey,
    pub referrer: Pubkey,
    pub referrer_token_state: Pubkey,
}

pub fn init_referrer_token_state(accounts: InitReferrerTokenStateAccounts) -> Instruction {
    let data = discriminators::INIT_REFERRER_TOKEN_STATE.to_vec();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer_writable(accounts.payer),
            readonly(accounts.lending_market),
            readonly(accounts.reserve),
            readonly(accounts.referrer),
            writable(accounts.referrer_token_state),
            readonly(SYSVAR_RENT_ID),
            readonly(SYSTEM_PROGRAM_ID),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// init_user_metadata
// ---------------------------------------------------------------------------

pub struct InitUserMetadataAccounts {
    pub owner: Pubkey,
    pub fee_payer: Pubkey,
    pub user_metadata: Pubkey,
    pub referrer_user_metadata: Option<Pubkey>,
}

pub fn init_user_metadata(
    accounts: InitUserMetadataAccounts,
    user_lookup_table: Pubkey,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        user_lookup_table: Pubkey,
    }

    let mut data = discriminators::INIT_USER_METADATA.to_vec();
    Args { user_lookup_table }.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer(accounts.owner),
            signer_writable(accounts.fee_payer),
            writable(accounts.user_metadata),
            optional_account(&KLEND_PROGRAM_ID, accounts.referrer_user_metadata, false),
            readonly(SYSVAR_RENT_ID),
            readonly(SYSTEM_PROGRAM_ID),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// withdraw_referrer_fees
// ---------------------------------------------------------------------------

pub struct WithdrawReferrerFeesAccounts {
    pub referrer: Pubkey,
    pub referrer_token_state: Pubkey,
    pub reserve: Pubkey,
    pub reserve_liquidity_mint: Pubkey,
    pub reserve_supply_liquidity: Pubkey,
    pub referrer_token_account: Pubkey,
    pub lending_market: Pubkey,
    pub lending_market_authority: Pubkey,
    pub token_program: Pubkey,
}

pub fn withdraw_referrer_fees(accounts: WithdrawReferrerFeesAccounts) -> Instruction {
    let data = discriminators::WITHDRAW_REFERRER_FEES.to_vec();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer_writable(accounts.referrer),
            writable(accounts.referrer_token_state),
            writable(accounts.reserve),
            readonly(accounts.reserve_liquidity_mint),
            writable(accounts.reserve_supply_liquidity),
            writable(accounts.referrer_token_account),
            readonly(accounts.lending_market),
            readonly(accounts.lending_market_authority),
            readonly(accounts.token_program),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// init_referrer_state_and_short_url
// ---------------------------------------------------------------------------

pub struct InitReferrerStateAndShortUrlAccounts {
    pub referrer: Pubkey,
    pub referrer_state: Pubkey,
    pub referrer_short_url: Pubkey,
    pub referrer_user_metadata: Pubkey,
}

pub fn init_referrer_state_and_short_url(
    accounts: InitReferrerStateAndShortUrlAccounts,
    short_url: String,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        short_url: String,
    }

    let mut data = discriminators::INIT_REFERRER_STATE_AND_SHORT_URL.to_vec();
    Args { short_url }.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer_writable(accounts.referrer),
            writable(accounts.referrer_state),
            writable(accounts.referrer_short_url),
            readonly(accounts.referrer_user_metadata),
            readonly(SYSVAR_RENT_ID),
            readonly(SYSTEM_PROGRAM_ID),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// delete_referrer_state_and_short_url
// ---------------------------------------------------------------------------

pub struct DeleteReferrerStateAndShortUrlAccounts {
    pub referrer: Pubkey,
    pub referrer_state: Pubkey,
    pub short_url: Pubkey,
}

pub fn delete_referrer_state_and_short_url(
    accounts: DeleteReferrerStateAndShortUrlAccounts,
) -> Instruction {
    let data = discriminators::DELETE_REFERRER_STATE_AND_SHORT_URL.to_vec();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            signer_writable(accounts.referrer),
            writable(accounts.referrer_state),
            writable(accounts.short_url),
            readonly(SYSVAR_RENT_ID),
            readonly(SYSTEM_PROGRAM_ID),
        ],
        data,
    }
}
