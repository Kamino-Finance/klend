use borsh::BorshSerialize;
use solana_instruction::{AccountMeta, Instruction};
use solana_pubkey::Pubkey;

use crate::{discriminators, util::*, KLEND_PROGRAM_ID};

// ---------------------------------------------------------------------------
// refresh_reserve
// ---------------------------------------------------------------------------

pub struct RefreshReserveAccounts {
    pub reserve: Pubkey,
    pub lending_market: Pubkey,
    pub pyth_oracle: Option<Pubkey>,
    pub switchboard_price_oracle: Option<Pubkey>,
    pub switchboard_twap_oracle: Option<Pubkey>,
    pub scope_prices: Option<Pubkey>,
}

pub fn refresh_reserve(accounts: RefreshReserveAccounts) -> Instruction {
    let data = discriminators::REFRESH_RESERVE.to_vec();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            writable(accounts.reserve),
            readonly(accounts.lending_market),
            optional_account(&KLEND_PROGRAM_ID, accounts.pyth_oracle, false),
            optional_account(&KLEND_PROGRAM_ID, accounts.switchboard_price_oracle, false),
            optional_account(&KLEND_PROGRAM_ID, accounts.switchboard_twap_oracle, false),
            optional_account(&KLEND_PROGRAM_ID, accounts.scope_prices, false),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// refresh_reserves_batch
// ---------------------------------------------------------------------------

pub fn refresh_reserves_batch(
    skip_price_updates: bool,
    remaining_accounts: Vec<AccountMeta>,
) -> Instruction {
    #[derive(BorshSerialize)]
    struct Args {
        skip_price_updates: bool,
    }

    let args = Args { skip_price_updates };
    let mut data = discriminators::REFRESH_RESERVES_BATCH.to_vec();
    args.serialize(&mut data).unwrap();

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: remaining_accounts,
        data,
    }
}

// ---------------------------------------------------------------------------
// refresh_obligation
// ---------------------------------------------------------------------------

pub struct RefreshObligationAccounts {
    pub lending_market: Pubkey,
    pub obligation: Pubkey,
}

pub fn refresh_obligation(
    accounts: RefreshObligationAccounts,
    remaining_accounts: Vec<AccountMeta>,
) -> Instruction {
    let data = discriminators::REFRESH_OBLIGATION.to_vec();

    let mut account_metas = vec![
        readonly(accounts.lending_market),
        writable(accounts.obligation),
    ];
    account_metas.extend(remaining_accounts);

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: account_metas,
        data,
    }
}
