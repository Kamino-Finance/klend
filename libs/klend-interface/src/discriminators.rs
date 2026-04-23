use sha2::{Digest, Sha256};

/// Compute the 8-byte Anchor discriminator for a given instruction name.
/// Uses the convention: `sha256("global:<snake_case_name>")[..8]`
pub fn compute_discriminator(name: &str) -> [u8; 8] {
    let mut hasher = Sha256::new();
    hasher.update(format!("global:{name}"));
    let hash = hasher.finalize();
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&hash[..8]);
    disc
}

macro_rules! disc {
    ($name:ident, $ix:expr) => {
        pub static $name: [u8; 8] = {
            const PREIMAGE: &[u8] = concat!("global:", $ix).as_bytes();
            sha256_first8(PREIMAGE)
        };
    };
}

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

/// Compress a single 64-byte block into the state, returning the new state.
/// Uses value semantics to avoid `&mut` (not available in const fn on Rust 1.74).
const fn compress_block(h: [u32; 8], block: [u8; 64]) -> [u32; 8] {
    let mut w = [0u32; 64];
    let mut i = 0;
    while i < 16 {
        w[i] = ((block[i * 4] as u32) << 24)
            | ((block[i * 4 + 1] as u32) << 16)
            | ((block[i * 4 + 2] as u32) << 8)
            | (block[i * 4 + 3] as u32);
        i += 1;
    }
    i = 16;
    while i < 64 {
        let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
        let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16]
            .wrapping_add(s0)
            .wrapping_add(w[i - 7])
            .wrapping_add(s1);
        i += 1;
    }

    let (mut a, mut b, mut c, mut d, mut e, mut f, mut g, mut hh) =
        (h[0], h[1], h[2], h[3], h[4], h[5], h[6], h[7]);

    i = 0;
    while i < 64 {
        let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
        let ch = (e & f) ^ ((!e) & g);
        let temp1 = hh
            .wrapping_add(s1)
            .wrapping_add(ch)
            .wrapping_add(SHA256_K[i])
            .wrapping_add(w[i]);
        let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
        let maj = (a & b) ^ (a & c) ^ (b & c);
        let temp2 = s0.wrapping_add(maj);

        hh = g;
        g = f;
        f = e;
        e = d.wrapping_add(temp1);
        d = c;
        c = b;
        b = a;
        a = temp1.wrapping_add(temp2);
        i += 1;
    }

    [
        h[0].wrapping_add(a),
        h[1].wrapping_add(b),
        h[2].wrapping_add(c),
        h[3].wrapping_add(d),
        h[4].wrapping_add(e),
        h[5].wrapping_add(f),
        h[6].wrapping_add(g),
        h[7].wrapping_add(hh),
    ]
}

/// Const-compatible SHA-256 returning only the first 8 bytes.
/// Supports messages up to 119 bytes (two 64-byte blocks).
pub const fn sha256_first8(msg: &[u8]) -> [u8; 8] {
    let h: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let bit_len = (msg.len() as u64) * 8;
    let padded_len = if msg.len() + 9 <= 64 { 64 } else { 128 };

    let mut padded = [0u8; 128];
    let mut i = 0;
    while i < msg.len() {
        padded[i] = msg[i];
        i += 1;
    }
    padded[msg.len()] = 0x80;
    padded[padded_len - 8] = (bit_len >> 56) as u8;
    padded[padded_len - 7] = (bit_len >> 48) as u8;
    padded[padded_len - 6] = (bit_len >> 40) as u8;
    padded[padded_len - 5] = (bit_len >> 32) as u8;
    padded[padded_len - 4] = (bit_len >> 24) as u8;
    padded[padded_len - 3] = (bit_len >> 16) as u8;
    padded[padded_len - 2] = (bit_len >> 8) as u8;
    padded[padded_len - 1] = bit_len as u8;

    // First block
    let mut block0 = [0u8; 64];
    i = 0;
    while i < 64 {
        block0[i] = padded[i];
        i += 1;
    }
    let h = compress_block(h, block0);

    // Second block (if needed)
    let h = if padded_len > 64 {
        let mut block1 = [0u8; 64];
        i = 0;
        while i < 64 {
            block1[i] = padded[64 + i];
            i += 1;
        }
        compress_block(h, block1)
    } else {
        h
    };

    [
        (h[0] >> 24) as u8,
        (h[0] >> 16) as u8,
        (h[0] >> 8) as u8,
        h[0] as u8,
        (h[1] >> 24) as u8,
        (h[1] >> 16) as u8,
        (h[1] >> 8) as u8,
        h[1] as u8,
    ]
}

// Refresh
disc!(REFRESH_RESERVE, "refresh_reserve");
disc!(REFRESH_RESERVES_BATCH, "refresh_reserves_batch");
disc!(REFRESH_OBLIGATION, "refresh_obligation");

// Deposit
disc!(DEPOSIT_RESERVE_LIQUIDITY, "deposit_reserve_liquidity");
disc!(
    DEPOSIT_OBLIGATION_COLLATERAL_V2,
    "deposit_obligation_collateral_v2"
);
disc!(
    DEPOSIT_RESERVE_LIQUIDITY_AND_OBLIGATION_COLLATERAL_V2,
    "deposit_reserve_liquidity_and_obligation_collateral_v2"
);

// Withdraw
disc!(REDEEM_RESERVE_COLLATERAL, "redeem_reserve_collateral");
disc!(
    WITHDRAW_OBLIGATION_COLLATERAL_V2,
    "withdraw_obligation_collateral_v2"
);
disc!(
    WITHDRAW_OBLIGATION_COLLATERAL_AND_REDEEM_RESERVE_COLLATERAL_V2,
    "withdraw_obligation_collateral_and_redeem_reserve_collateral_v2"
);

// Borrow
disc!(
    BORROW_OBLIGATION_LIQUIDITY_V2,
    "borrow_obligation_liquidity_v2"
);

// Repay
disc!(
    REPAY_OBLIGATION_LIQUIDITY_V2,
    "repay_obligation_liquidity_v2"
);
disc!(
    REPAY_AND_WITHDRAW_AND_REDEEM,
    "repay_and_withdraw_and_redeem"
);

// Compound
disc!(DEPOSIT_AND_WITHDRAW, "deposit_and_withdraw");

// Liquidate
disc!(
    LIQUIDATE_OBLIGATION_AND_REDEEM_RESERVE_COLLATERAL_V2,
    "liquidate_obligation_and_redeem_reserve_collateral_v2"
);

// Flash
disc!(
    FLASH_BORROW_RESERVE_LIQUIDITY,
    "flash_borrow_reserve_liquidity"
);
disc!(
    FLASH_REPAY_RESERVE_LIQUIDITY,
    "flash_repay_reserve_liquidity"
);

// Obligation lifecycle
disc!(INIT_OBLIGATION, "init_obligation");
disc!(
    INIT_OBLIGATION_FARMS_FOR_RESERVE,
    "init_obligation_farms_for_reserve"
);
disc!(
    REFRESH_OBLIGATION_FARMS_FOR_RESERVE,
    "refresh_obligation_farms_for_reserve"
);
disc!(REQUEST_ELEVATION_GROUP, "request_elevation_group");

// Orders
disc!(SET_OBLIGATION_ORDER, "set_obligation_order");
disc!(SET_BORROW_ORDER, "set_borrow_order");
disc!(FILL_BORROW_ORDER, "fill_borrow_order");

// Referrer
disc!(INIT_REFERRER_TOKEN_STATE, "init_referrer_token_state");
disc!(INIT_USER_METADATA, "init_user_metadata");
disc!(WITHDRAW_REFERRER_FEES, "withdraw_referrer_fees");
disc!(
    INIT_REFERRER_STATE_AND_SHORT_URL,
    "init_referrer_state_and_short_url"
);
disc!(
    DELETE_REFERRER_STATE_AND_SHORT_URL,
    "delete_referrer_state_and_short_url"
);

// Withdraw queue
disc!(ENQUEUE_TO_WITHDRAW, "enqueue_to_withdraw");
disc!(WITHDRAW_QUEUED_LIQUIDITY, "withdraw_queued_liquidity");
disc!(
    RECOVER_INVALID_TICKET_COLLATERAL,
    "recover_invalid_ticket_collateral"
);
disc!(CANCEL_WITHDRAW_TICKET, "cancel_withdraw_ticket");

// Rollover / obligation config
disc!(ROLLOVER_FIXED_TERM_BORROW, "rollover_fixed_term_borrow");
disc!(UPDATE_OBLIGATION_CONFIG, "update_obligation_config");

// Admin
disc!(CLONE_RESERVE_CONFIG, "clone_reserve_config");

// Obligation ownership transfer
disc!(
    INITIATE_OBLIGATION_OWNERSHIP_TRANSFER,
    "initiate_obligation_ownership_transfer"
);
disc!(
    APPROVE_OBLIGATION_OWNERSHIP_TRANSFER,
    "approve_obligation_ownership_transfer"
);
disc!(ACCEPT_OBLIGATION_OWNERSHIP, "accept_obligation_ownership");
disc!(
    ABORT_OBLIGATION_OWNERSHIP_TRANSFER,
    "abort_obligation_ownership_transfer"
);

/// Known Klend instruction types, identified by their 8-byte discriminator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum KlendInstruction {
    RefreshReserve,
    RefreshReservesBatch,
    RefreshObligation,
    DepositReserveLiquidity,
    DepositObligationCollateralV2,
    DepositReserveLiquidityAndObligationCollateralV2,
    RedeemReserveCollateral,
    WithdrawObligationCollateralV2,
    WithdrawObligationCollateralAndRedeemReserveCollateralV2,
    BorrowObligationLiquidityV2,
    RepayObligationLiquidityV2,
    RepayAndWithdrawAndRedeem,
    DepositAndWithdraw,
    LiquidateObligationAndRedeemReserveCollateralV2,
    FlashBorrowReserveLiquidity,
    FlashRepayReserveLiquidity,
    InitObligation,
    InitObligationFarmsForReserve,
    RefreshObligationFarmsForReserve,
    RequestElevationGroup,
    SetObligationOrder,
    SetBorrowOrder,
    FillBorrowOrder,
    InitReferrerTokenState,
    InitUserMetadata,
    WithdrawReferrerFees,
    InitReferrerStateAndShortUrl,
    DeleteReferrerStateAndShortUrl,
    EnqueueToWithdraw,
    WithdrawQueuedLiquidity,
    RecoverInvalidTicketCollateral,
    CancelWithdrawTicket,
    RolloverFixedTermBorrow,
    UpdateObligationConfig,
    CloneReserveConfig,
    InitiateObligationOwnershipTransfer,
    ApproveObligationOwnershipTransfer,
    AcceptObligationOwnership,
    AbortObligationOwnershipTransfer,
}

impl core::fmt::Display for KlendInstruction {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{self:?}")
    }
}

/// Identify a Klend instruction from its raw instruction data.
///
/// Returns `Some(variant)` if the first 8 bytes match a known discriminator,
/// `None` otherwise.
pub fn identify_instruction(data: &[u8]) -> Option<KlendInstruction> {
    if data.len() < 8 {
        return None;
    }
    let mut disc = [0u8; 8];
    disc.copy_from_slice(&data[..8]);

    match disc {
        d if d == REFRESH_RESERVE => Some(KlendInstruction::RefreshReserve),
        d if d == REFRESH_RESERVES_BATCH => Some(KlendInstruction::RefreshReservesBatch),
        d if d == REFRESH_OBLIGATION => Some(KlendInstruction::RefreshObligation),
        d if d == DEPOSIT_RESERVE_LIQUIDITY => Some(KlendInstruction::DepositReserveLiquidity),
        d if d == DEPOSIT_OBLIGATION_COLLATERAL_V2 => {
            Some(KlendInstruction::DepositObligationCollateralV2)
        }
        d if d == DEPOSIT_RESERVE_LIQUIDITY_AND_OBLIGATION_COLLATERAL_V2 => {
            Some(KlendInstruction::DepositReserveLiquidityAndObligationCollateralV2)
        }
        d if d == REDEEM_RESERVE_COLLATERAL => Some(KlendInstruction::RedeemReserveCollateral),
        d if d == WITHDRAW_OBLIGATION_COLLATERAL_V2 => {
            Some(KlendInstruction::WithdrawObligationCollateralV2)
        }
        d if d == WITHDRAW_OBLIGATION_COLLATERAL_AND_REDEEM_RESERVE_COLLATERAL_V2 => {
            Some(KlendInstruction::WithdrawObligationCollateralAndRedeemReserveCollateralV2)
        }
        d if d == BORROW_OBLIGATION_LIQUIDITY_V2 => {
            Some(KlendInstruction::BorrowObligationLiquidityV2)
        }
        d if d == REPAY_OBLIGATION_LIQUIDITY_V2 => {
            Some(KlendInstruction::RepayObligationLiquidityV2)
        }
        d if d == REPAY_AND_WITHDRAW_AND_REDEEM => {
            Some(KlendInstruction::RepayAndWithdrawAndRedeem)
        }
        d if d == DEPOSIT_AND_WITHDRAW => Some(KlendInstruction::DepositAndWithdraw),
        d if d == LIQUIDATE_OBLIGATION_AND_REDEEM_RESERVE_COLLATERAL_V2 => {
            Some(KlendInstruction::LiquidateObligationAndRedeemReserveCollateralV2)
        }
        d if d == FLASH_BORROW_RESERVE_LIQUIDITY => {
            Some(KlendInstruction::FlashBorrowReserveLiquidity)
        }
        d if d == FLASH_REPAY_RESERVE_LIQUIDITY => {
            Some(KlendInstruction::FlashRepayReserveLiquidity)
        }
        d if d == INIT_OBLIGATION => Some(KlendInstruction::InitObligation),
        d if d == INIT_OBLIGATION_FARMS_FOR_RESERVE => {
            Some(KlendInstruction::InitObligationFarmsForReserve)
        }
        d if d == REFRESH_OBLIGATION_FARMS_FOR_RESERVE => {
            Some(KlendInstruction::RefreshObligationFarmsForReserve)
        }
        d if d == REQUEST_ELEVATION_GROUP => Some(KlendInstruction::RequestElevationGroup),
        d if d == SET_OBLIGATION_ORDER => Some(KlendInstruction::SetObligationOrder),
        d if d == SET_BORROW_ORDER => Some(KlendInstruction::SetBorrowOrder),
        d if d == FILL_BORROW_ORDER => Some(KlendInstruction::FillBorrowOrder),
        d if d == INIT_REFERRER_TOKEN_STATE => Some(KlendInstruction::InitReferrerTokenState),
        d if d == INIT_USER_METADATA => Some(KlendInstruction::InitUserMetadata),
        d if d == WITHDRAW_REFERRER_FEES => Some(KlendInstruction::WithdrawReferrerFees),
        d if d == INIT_REFERRER_STATE_AND_SHORT_URL => {
            Some(KlendInstruction::InitReferrerStateAndShortUrl)
        }
        d if d == DELETE_REFERRER_STATE_AND_SHORT_URL => {
            Some(KlendInstruction::DeleteReferrerStateAndShortUrl)
        }
        d if d == ENQUEUE_TO_WITHDRAW => Some(KlendInstruction::EnqueueToWithdraw),
        d if d == WITHDRAW_QUEUED_LIQUIDITY => Some(KlendInstruction::WithdrawQueuedLiquidity),
        d if d == RECOVER_INVALID_TICKET_COLLATERAL => {
            Some(KlendInstruction::RecoverInvalidTicketCollateral)
        }
        d if d == CANCEL_WITHDRAW_TICKET => Some(KlendInstruction::CancelWithdrawTicket),
        d if d == ROLLOVER_FIXED_TERM_BORROW => Some(KlendInstruction::RolloverFixedTermBorrow),
        d if d == UPDATE_OBLIGATION_CONFIG => Some(KlendInstruction::UpdateObligationConfig),
        d if d == CLONE_RESERVE_CONFIG => Some(KlendInstruction::CloneReserveConfig),
        d if d == INITIATE_OBLIGATION_OWNERSHIP_TRANSFER => {
            Some(KlendInstruction::InitiateObligationOwnershipTransfer)
        }
        d if d == APPROVE_OBLIGATION_OWNERSHIP_TRANSFER => {
            Some(KlendInstruction::ApproveObligationOwnershipTransfer)
        }
        d if d == ACCEPT_OBLIGATION_OWNERSHIP => Some(KlendInstruction::AcceptObligationOwnership),
        d if d == ABORT_OBLIGATION_OWNERSHIP_TRANSFER => {
            Some(KlendInstruction::AbortObligationOwnershipTransfer)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! check_disc {
        ($name:expr, $constant:ident) => {
            assert_eq!(
                compute_discriminator($name),
                $constant,
                "Discriminator mismatch for {}",
                $name
            );
        };
    }

    #[test]
    fn verify_all_discriminators() {
        check_disc!("refresh_reserve", REFRESH_RESERVE);
        check_disc!("refresh_reserves_batch", REFRESH_RESERVES_BATCH);
        check_disc!("refresh_obligation", REFRESH_OBLIGATION);
        check_disc!("deposit_reserve_liquidity", DEPOSIT_RESERVE_LIQUIDITY);
        check_disc!(
            "deposit_obligation_collateral_v2",
            DEPOSIT_OBLIGATION_COLLATERAL_V2
        );
        check_disc!(
            "deposit_reserve_liquidity_and_obligation_collateral_v2",
            DEPOSIT_RESERVE_LIQUIDITY_AND_OBLIGATION_COLLATERAL_V2
        );
        check_disc!("redeem_reserve_collateral", REDEEM_RESERVE_COLLATERAL);
        check_disc!(
            "withdraw_obligation_collateral_v2",
            WITHDRAW_OBLIGATION_COLLATERAL_V2
        );
        check_disc!(
            "withdraw_obligation_collateral_and_redeem_reserve_collateral_v2",
            WITHDRAW_OBLIGATION_COLLATERAL_AND_REDEEM_RESERVE_COLLATERAL_V2
        );
        check_disc!(
            "borrow_obligation_liquidity_v2",
            BORROW_OBLIGATION_LIQUIDITY_V2
        );
        check_disc!(
            "repay_obligation_liquidity_v2",
            REPAY_OBLIGATION_LIQUIDITY_V2
        );
        check_disc!(
            "repay_and_withdraw_and_redeem",
            REPAY_AND_WITHDRAW_AND_REDEEM
        );
        check_disc!("deposit_and_withdraw", DEPOSIT_AND_WITHDRAW);
        check_disc!(
            "liquidate_obligation_and_redeem_reserve_collateral_v2",
            LIQUIDATE_OBLIGATION_AND_REDEEM_RESERVE_COLLATERAL_V2
        );
        check_disc!(
            "flash_borrow_reserve_liquidity",
            FLASH_BORROW_RESERVE_LIQUIDITY
        );
        check_disc!(
            "flash_repay_reserve_liquidity",
            FLASH_REPAY_RESERVE_LIQUIDITY
        );
        check_disc!("init_obligation", INIT_OBLIGATION);
        check_disc!(
            "init_obligation_farms_for_reserve",
            INIT_OBLIGATION_FARMS_FOR_RESERVE
        );
        check_disc!(
            "refresh_obligation_farms_for_reserve",
            REFRESH_OBLIGATION_FARMS_FOR_RESERVE
        );
        check_disc!("request_elevation_group", REQUEST_ELEVATION_GROUP);
        check_disc!("set_obligation_order", SET_OBLIGATION_ORDER);
        check_disc!("set_borrow_order", SET_BORROW_ORDER);
        check_disc!("fill_borrow_order", FILL_BORROW_ORDER);
        check_disc!("init_referrer_token_state", INIT_REFERRER_TOKEN_STATE);
        check_disc!("init_user_metadata", INIT_USER_METADATA);
        check_disc!("withdraw_referrer_fees", WITHDRAW_REFERRER_FEES);
        check_disc!(
            "init_referrer_state_and_short_url",
            INIT_REFERRER_STATE_AND_SHORT_URL
        );
        check_disc!(
            "delete_referrer_state_and_short_url",
            DELETE_REFERRER_STATE_AND_SHORT_URL
        );
        check_disc!("enqueue_to_withdraw", ENQUEUE_TO_WITHDRAW);
        check_disc!("withdraw_queued_liquidity", WITHDRAW_QUEUED_LIQUIDITY);
        check_disc!(
            "recover_invalid_ticket_collateral",
            RECOVER_INVALID_TICKET_COLLATERAL
        );
        check_disc!("cancel_withdraw_ticket", CANCEL_WITHDRAW_TICKET);
        check_disc!("rollover_fixed_term_borrow", ROLLOVER_FIXED_TERM_BORROW);
        check_disc!("update_obligation_config", UPDATE_OBLIGATION_CONFIG);
        check_disc!("clone_reserve_config", CLONE_RESERVE_CONFIG);
        check_disc!(
            "initiate_obligation_ownership_transfer",
            INITIATE_OBLIGATION_OWNERSHIP_TRANSFER
        );
        check_disc!(
            "approve_obligation_ownership_transfer",
            APPROVE_OBLIGATION_OWNERSHIP_TRANSFER
        );
        check_disc!("accept_obligation_ownership", ACCEPT_OBLIGATION_OWNERSHIP);
        check_disc!(
            "abort_obligation_ownership_transfer",
            ABORT_OBLIGATION_OWNERSHIP_TRANSFER
        );
    }
}
