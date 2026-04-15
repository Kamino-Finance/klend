use solana_pubkey::Pubkey;

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
pub const EVENT_AUTHORITY: &[u8] = b"__event_authority";

/// All 4 reserve-related PDAs.
pub struct ReservePdas {
    pub liquidity_supply_vault: Pubkey,
    pub fee_vault: Pubkey,
    pub collateral_mint: Pubkey,
    pub collateral_supply_vault: Pubkey,
}

impl ReservePdas {
    pub fn derive(program_id: &Pubkey, reserve: &Pubkey) -> Self {
        Self {
            liquidity_supply_vault: reserve_liquidity_supply(program_id, reserve).0,
            fee_vault: reserve_fee_receiver(program_id, reserve).0,
            collateral_mint: reserve_collateral_mint(program_id, reserve).0,
            collateral_supply_vault: reserve_collateral_supply(program_id, reserve).0,
        }
    }
}

pub fn lending_market_authority(program_id: &Pubkey, lending_market: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[LENDING_MARKET_AUTH, lending_market.as_ref()], program_id)
}

pub fn obligation(
    program_id: &Pubkey,
    tag: u8,
    id: u8,
    owner: &Pubkey,
    lending_market: &Pubkey,
    seed1: &Pubkey,
    seed2: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            &[tag],
            &[id],
            owner.as_ref(),
            lending_market.as_ref(),
            seed1.as_ref(),
            seed2.as_ref(),
        ],
        program_id,
    )
}

pub fn reserve_liquidity_supply(program_id: &Pubkey, reserve: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[RESERVE_LIQ_SUPPLY, reserve.as_ref()], program_id)
}

pub fn reserve_fee_receiver(program_id: &Pubkey, reserve: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[FEE_RECEIVER, reserve.as_ref()], program_id)
}

pub fn reserve_collateral_mint(program_id: &Pubkey, reserve: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[RESERVE_COLL_MINT, reserve.as_ref()], program_id)
}

pub fn reserve_collateral_supply(program_id: &Pubkey, reserve: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[RESERVE_COLL_SUPPLY, reserve.as_ref()], program_id)
}

pub fn referrer_token_state(
    program_id: &Pubkey,
    referrer: &Pubkey,
    reserve: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            BASE_SEED_REFERRER_TOKEN_STATE,
            referrer.as_ref(),
            reserve.as_ref(),
        ],
        program_id,
    )
}

pub fn user_metadata(program_id: &Pubkey, owner: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[BASE_SEED_USER_METADATA, owner.as_ref()], program_id)
}

pub fn referrer_state(program_id: &Pubkey, referrer: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[BASE_SEED_REFERRER_STATE, referrer.as_ref()], program_id)
}

pub fn short_url(program_id: &Pubkey, url: &str) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[BASE_SEED_SHORT_URL, url.as_bytes()], program_id)
}

pub fn global_config(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[GLOBAL_CONFIG_STATE], program_id)
}

pub fn withdraw_ticket(
    program_id: &Pubkey,
    reserve: &Pubkey,
    sequence_number: u64,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            WITHDRAW_TICKET,
            reserve.as_ref(),
            &sequence_number.to_le_bytes(),
        ],
        program_id,
    )
}

pub fn owner_queued_collateral_vault(
    program_id: &Pubkey,
    reserve: &Pubkey,
    owner: &Pubkey,
) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[
            OWNER_QUEUED_COLLATERAL_VAULT,
            reserve.as_ref(),
            owner.as_ref(),
        ],
        program_id,
    )
}

pub fn event_authority(program_id: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(&[EVENT_AUTHORITY], program_id)
}

/// Farms user state PDA for an obligation on a specific farm.
///
/// Seeds: `[b"user", farm_state, obligation]` on [`crate::FARMS_PROGRAM_ID`].
pub fn farms_user_state(farm_state: &Pubkey, obligation: &Pubkey) -> (Pubkey, u8) {
    Pubkey::find_program_address(
        &[b"user", farm_state.as_ref(), obligation.as_ref()],
        &crate::FARMS_PROGRAM_ID,
    )
}
