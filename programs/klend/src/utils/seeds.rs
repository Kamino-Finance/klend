pub const LENDING_MARKET_AUTH: &[u8] = b"lma";
pub const RESERVE_LIQ_SUPPLY: &[u8] = b"reserve_liq_supply";
pub const FEE_RECEIVER: &[u8] = b"fee_receiver";
pub const RESERVE_COLL_MINT: &[u8] = b"reserve_coll_mint";
pub const RESERVE_COLL_SUPPLY: &[u8] = b"reserve_coll_supply";
pub const BASE_SEED_REFERRER_TOKEN_STATE: &[u8] = b"referrer_acc";
pub const BASE_SEED_USER_METADATA: &[u8] = b"user_meta";
pub const BASE_SEED_REFERRER_STATE: &[u8] = b"ref_state";
pub const BASE_SEED_SHORT_URL: &[u8] = b"short_url";

pub mod pda {
    use anchor_lang::prelude::Pubkey;

    use super::*;
    use crate::ID;

    pub fn lending_market_auth(lending_market: &Pubkey) -> Pubkey {
        lending_market_auth_program_id(&ID, lending_market)
    }

    pub fn lending_market_auth_program_id(program_id: &Pubkey, lending_market: &Pubkey) -> Pubkey {
        let (lending_market_authority, _market_authority_bump) = Pubkey::find_program_address(
            &[LENDING_MARKET_AUTH, lending_market.as_ref()],
            program_id,
        );
        lending_market_authority
    }

    pub struct InitReservePdas {
        pub liquidity_supply_vault: Pubkey,
        pub collateral_ctoken_mint: Pubkey,
        pub collateral_supply_vault: Pubkey,
        pub fee_vault: Pubkey,
    }

    pub fn init_reserve_pdas(market: &Pubkey, mint: &Pubkey) -> InitReservePdas {
        init_reserve_pdas_program_id(&ID, market, mint)
    }

    pub fn init_reserve_pdas_program_id(
        program_id: &Pubkey,
        market: &Pubkey,
        mint: &Pubkey,
    ) -> InitReservePdas {
        let (fee_vault, _fee_vault_bump) = Pubkey::find_program_address(
            &[FEE_RECEIVER, market.as_ref(), mint.as_ref()],
            program_id,
        );
        let (liquidity_supply_vault, _liquidity_supply_vault_bump) = Pubkey::find_program_address(
            &[RESERVE_LIQ_SUPPLY, market.as_ref(), mint.as_ref()],
            program_id,
        );
        let (collateral_ctoken_mint, _collateral_ctoken_mint_bump) = Pubkey::find_program_address(
            &[RESERVE_COLL_MINT, market.as_ref(), mint.as_ref()],
            program_id,
        );
        let (collateral_supply_vault, _collateral_supply_vault_bump) = Pubkey::find_program_address(
            &[RESERVE_COLL_SUPPLY, market.as_ref(), mint.as_ref()],
            program_id,
        );

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
}
