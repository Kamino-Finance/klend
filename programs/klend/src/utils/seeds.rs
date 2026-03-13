pub const LENDING_MARKET_AUTH: &[u8] = b"lma";
pub const RESERVE_LIQ_SUPPLY: &[u8] = b"reserve_liq_supply";
pub const FEE_RECEIVER: &[u8] = b"fee_receiver";
pub const RESERVE_COLL_MINT: &[u8] = b"reserve_coll_mint";
pub const RESERVE_COLL_SUPPLY: &[u8] = b"reserve_coll_supply";
pub const BASE_SEED_REFERRER_TOKEN_STATE: &[u8] = b"referrer_acc";
pub const BASE_SEED_USER_METADATA: &[u8] = b"user_meta";
pub const BASE_SEED_REFERRER_STATE: &[u8] = b"ref_state";
pub const BASE_SEED_SHORT_URL: &[u8] = b"short_url";
pub const GLOBAL_CONFIG_STATE: &[u8] = b"global_config";
pub const WITHDRAW_TICKET: &[u8] = b"withdraw_ticket";
pub const OWNER_QUEUED_COLLATERAL_VAULT: &[u8] = b"owner_queued_collateral_vault";
pub const KVAULT_BASE_AUTHORITY: &[u8] = b"authority";
pub const EVENT_AUTHORITY: &[u8] = b"__event_authority";

pub mod pda {
    use anchor_lang::prelude::Pubkey;

    use super::*;
    use crate::utils::CORRESPONDING_KAMINO_VAULT_PROGRAM_ID;

    pub fn program_data() -> Pubkey {
        program_data_program_id(&crate::ID)
    }

    pub fn program_data_program_id(program_id: &Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[program_id.as_ref()],
            &solana_program::bpf_loader_upgradeable::ID,
        )
        .0
    }

    pub fn lending_market_auth(lending_market: &Pubkey) -> Pubkey {
        lending_market_auth_program_id(&crate::ID, lending_market)
    }

    pub fn lending_market_auth_program_id(program_id: &Pubkey, lending_market: &Pubkey) -> Pubkey {
        let (lending_market_authority, _market_authority_bump) = Pubkey::find_program_address(
            &[LENDING_MARKET_AUTH, lending_market.as_ref()],
            program_id,
        );
        lending_market_authority
    }

    pub fn global_config() -> Pubkey {
        global_config_program_id(&crate::ID)
    }

    pub fn global_config_program_id(program_id: &Pubkey) -> Pubkey {
        Pubkey::find_program_address(&[GLOBAL_CONFIG_STATE], program_id).0
    }

    pub struct InitReservePdas {
        pub liquidity_supply_vault: Pubkey,
        pub collateral_ctoken_mint: Pubkey,
        pub collateral_supply_vault: Pubkey,
        pub fee_vault: Pubkey,
    }

    pub fn init_reserve_pdas(reserve: &Pubkey) -> InitReservePdas {
        init_reserve_pdas_program_id(&crate::ID, reserve)
    }

    pub fn init_reserve_pdas_program_id(program_id: &Pubkey, reserve: &Pubkey) -> InitReservePdas {
        let (fee_vault, _fee_vault_bump) =
            Pubkey::find_program_address(&[FEE_RECEIVER, reserve.as_ref()], program_id);
        let (liquidity_supply_vault, _liquidity_supply_vault_bump) =
            Pubkey::find_program_address(&[RESERVE_LIQ_SUPPLY, reserve.as_ref()], program_id);
        let (collateral_ctoken_mint, _collateral_ctoken_mint_bump) =
            Pubkey::find_program_address(&[RESERVE_COLL_MINT, reserve.as_ref()], program_id);
        let (collateral_supply_vault, _collateral_supply_vault_bump) =
            Pubkey::find_program_address(&[RESERVE_COLL_SUPPLY, reserve.as_ref()], program_id);

        InitReservePdas {
            liquidity_supply_vault,
            collateral_ctoken_mint,
            collateral_supply_vault,
            fee_vault,
        }
    }

    pub fn referrer_token_state(referrer: Pubkey, reserve: Pubkey) -> (Pubkey, u8) {
        Pubkey::find_program_address(
            &[
                BASE_SEED_REFERRER_TOKEN_STATE,
                referrer.as_ref(),
                reserve.as_ref(),
            ],
            &crate::ID,
        )
    }

    pub fn event_authority() -> Pubkey {
        let (event_authority, _) = Pubkey::find_program_address(&[EVENT_AUTHORITY], &crate::ID);

        event_authority
    }

    pub fn withdraw_ticket(reserve: Pubkey, sequence_number: u64) -> Pubkey {
        Pubkey::find_program_address(
            &[
                WITHDRAW_TICKET,
                reserve.as_ref(),
                &sequence_number.to_le_bytes(),
            ],
            &crate::ID,
        )
        .0
    }

    pub fn owner_queued_collateral_vault(reserve: Pubkey, owner: Pubkey) -> Pubkey {
        Pubkey::find_program_address(
            &[
                OWNER_QUEUED_COLLATERAL_VAULT,
                reserve.as_ref(),
                owner.as_ref(),
            ],
            &crate::ID,
        )
        .0
    }


    pub mod kvault {
        use super::*;


        pub fn base_authority(vault: Pubkey) -> Pubkey {
            Pubkey::find_program_address(
                &[KVAULT_BASE_AUTHORITY, vault.as_ref()],
                &CORRESPONDING_KAMINO_VAULT_PROGRAM_ID,
            )
            .0
        }
    }
}

