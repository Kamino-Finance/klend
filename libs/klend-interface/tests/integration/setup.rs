use std::path::PathBuf;

use klend_interface::{
    discriminators::compute_discriminator, pda, FARMS_PROGRAM_ID, KLEND_PROGRAM_ID,
    SYSTEM_PROGRAM_ID, SYSVAR_RENT_ID, TOKEN_PROGRAM_ID,
};
use litesvm::LiteSVM;
use solana_sdk::{
    account::Account,
    clock::Clock,
    instruction::{AccountMeta, Instruction},
    program_pack::Pack,
    pubkey::Pubkey,
    signature::Keypair,
    signer::Signer,
    transaction::Transaction,
};
use solana_system_interface::instruction as system_instruction;
use spl_token::instruction as spl_ix;

use super::pyth;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// LendingMarket account size: 8 (disc) + 4656 (data).
const LENDING_MARKET_SIZE: usize = 8 + 4656;

/// Reserve account size: 8 (disc) + 8616 (data).
const RESERVE_SIZE: usize = 8 + 8616;

/// GlobalConfig account size: 8 (disc) + 1024 (data).
const GLOBAL_CONFIG_SIZE: usize = 8 + 1024;

/// Default min initial deposit (matches the program constant).
const MIN_INITIAL_DEPOSIT: u64 = 100_000;

// ---------------------------------------------------------------------------
// TestEnv
// ---------------------------------------------------------------------------

pub struct TestEnv {
    pub svm: LiteSVM,
    pub admin: Keypair,
    pub lending_market: Keypair,
    pub reserve: Keypair,
    pub liquidity_mint: Pubkey,
    pub pyth_oracle: Pubkey,
}

// ---------------------------------------------------------------------------
// Full environment setup
// ---------------------------------------------------------------------------

pub fn setup_full_env() -> TestEnv {
    let mut svm = LiteSVM::new()
        .with_transaction_history(0) // Disable transaction history (allow transactions to be replayed)
        .with_lamports((100_000.0_f64 * 1_000_000_000.0) as u64);
    svm.airdrop(&Pubkey::new_unique(), 100_000_000_000).unwrap();

    // Load the klend program .so
    let so_path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/deploy/kamino_lending.so");
    let program_bytes = std::fs::read(&so_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {e}", so_path.display()));
    svm.add_program(KLEND_PROGRAM_ID, &program_bytes).unwrap();

    // Load the klend .so as the farms program too — it won't be invoked
    // (farms accounts are optional/None), but Anchor checks executability.
    svm.add_program(FARMS_PROGRAM_ID, &program_bytes).unwrap();

    let admin = Keypair::new();
    svm.airdrop(&admin.pubkey(), 100_000_000_000).unwrap();

    // Inject GlobalConfig PDA so update_reserve_config can read global_admin
    inject_global_config(&mut svm, &admin.pubkey());

    // Create lending market
    let lending_market = Keypair::new();
    create_lending_market(&mut svm, &admin, &lending_market);

    // Create SPL token mint for liquidity
    let liquidity_mint = create_mint(&mut svm, &admin, 6);

    // Create mock pyth oracle at price = 1.0
    let pyth_oracle = pyth::create_pyth_price_account(&mut svm, 1.0);

    // Init reserve
    let reserve = Keypair::new();
    init_reserve(&mut svm, &admin, &lending_market, &reserve, &liquidity_mint);

    // Configure the reserve to be usable
    configure_reserve(
        &mut svm,
        &admin,
        &lending_market.pubkey(),
        &reserve.pubkey(),
        &pyth_oracle,
    );

    TestEnv {
        svm,
        admin,
        lending_market,
        reserve,
        liquidity_mint,
        pyth_oracle,
    }
}

// ---------------------------------------------------------------------------
// GlobalConfig injection
// ---------------------------------------------------------------------------

fn inject_global_config(svm: &mut LiteSVM, admin: &Pubkey) {
    let (gc_key, _) = pda::global_config(&KLEND_PROGRAM_ID);

    // Discriminator: sha256("account:GlobalConfig")[..8]
    let disc = {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(b"account:GlobalConfig");
        let hash = h.finalize();
        let mut d = [0u8; 8];
        d.copy_from_slice(&hash[..8]);
        d
    };

    let mut data = vec![0u8; GLOBAL_CONFIG_SIZE];
    data[..8].copy_from_slice(&disc);
    // global_admin at offset 8 (32 bytes)
    data[8..40].copy_from_slice(admin.as_ref());
    // pending_admin at offset 40 (32 bytes)
    data[40..72].copy_from_slice(admin.as_ref());
    // fee_collector at offset 72 (32 bytes)
    data[72..104].copy_from_slice(admin.as_ref());

    let account = Account {
        lamports: u32::MAX as u64,
        data,
        owner: KLEND_PROGRAM_ID,
        executable: false,
        rent_epoch: 0,
    };
    svm.set_account(gc_key, account).unwrap();
}

// ---------------------------------------------------------------------------
// Admin instruction builders
// ---------------------------------------------------------------------------

fn build_init_lending_market_ix(owner: &Pubkey, lending_market: &Pubkey) -> Instruction {
    let disc = compute_discriminator("init_lending_market");
    let quote_currency: [u8; 32] =
        *b"USD\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0";

    let mut data = disc.to_vec();
    data.extend_from_slice(&quote_currency);

    let (lma, _) = pda::lending_market_authority(&KLEND_PROGRAM_ID, lending_market);

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*owner, true),
            AccountMeta::new(*lending_market, false),
            AccountMeta::new_readonly(lma, false),
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
            AccountMeta::new_readonly(SYSVAR_RENT_ID, false),
        ],
        data,
    }
}

fn build_init_reserve_ix(
    signer: &Pubkey,
    lending_market: &Pubkey,
    reserve: &Pubkey,
    liquidity_mint: &Pubkey,
    initial_liq_source: &Pubkey,
) -> Instruction {
    let disc = compute_discriminator("init_reserve");
    let data = disc.to_vec();

    let (lma, _) = pda::lending_market_authority(&KLEND_PROGRAM_ID, lending_market);
    let (liq_supply, _) = pda::reserve_liquidity_supply(&KLEND_PROGRAM_ID, reserve);
    let (fee_vault, _) = pda::reserve_fee_receiver(&KLEND_PROGRAM_ID, reserve);
    let (coll_mint, _) = pda::reserve_collateral_mint(&KLEND_PROGRAM_ID, reserve);
    let (coll_supply, _) = pda::reserve_collateral_supply(&KLEND_PROGRAM_ID, reserve);

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new(*signer, true),                   // signer (mut)
            AccountMeta::new_readonly(*lending_market, false), // lending_market
            AccountMeta::new_readonly(lma, false),             // lending_market_authority
            AccountMeta::new(*reserve, false),                 // reserve (zero)
            AccountMeta::new_readonly(*liquidity_mint, false), // reserve_liquidity_mint
            AccountMeta::new(liq_supply, false),               // reserve_liquidity_supply
            AccountMeta::new(fee_vault, false),                // fee_receiver
            AccountMeta::new(coll_mint, false),                // reserve_collateral_mint
            AccountMeta::new(coll_supply, false),              // reserve_collateral_supply
            AccountMeta::new(*initial_liq_source, false),      // initial_liquidity_source
            AccountMeta::new_readonly(SYSVAR_RENT_ID, false),
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false), // liquidity_token_program
            AccountMeta::new_readonly(TOKEN_PROGRAM_ID, false), // collateral_token_program (always spl_token)
            AccountMeta::new_readonly(SYSTEM_PROGRAM_ID, false),
        ],
        data,
    }
}

/// Borsh index for `UpdateConfigMode` variants.
/// Borsh 0.10 uses sequential u8 indices (0, 1, 2, ...) regardless of `#[repr(u64)]` values.
fn update_config_borsh_index(repr_value: u8) -> u8 {
    // The enum starts at repr=1 and is mostly contiguous.
    // Map repr discriminant -> borsh sequential index.
    const REPR_VALUES: &[u8] = &[
        1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25,
        26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36, 37, 38, 39, 40, 41, 42, 43, 44, 45, 46, 47, 48,
        49, 50, 51, 52, 53, 54, 55,
    ];
    REPR_VALUES
        .iter()
        .position(|&v| v == repr_value)
        .unwrap_or_else(|| panic!("Unknown UpdateConfigMode repr value: {repr_value}")) as u8
}

fn build_update_reserve_config_ix(
    signer: &Pubkey,
    lending_market: &Pubkey,
    reserve: &Pubkey,
    mode: u8,
    value: Vec<u8>,
    skip_validation: bool,
) -> Instruction {
    let disc = compute_discriminator("update_reserve_config");

    let mut data = disc.to_vec();
    // mode: borsh enum tag (u8, sequential index)
    data.push(update_config_borsh_index(mode));
    // value: Vec<u8> (borsh: 4-byte len + bytes)
    data.extend_from_slice(&(value.len() as u32).to_le_bytes());
    data.extend_from_slice(&value);
    // skip_config_integrity_validation: bool
    data.push(skip_validation as u8);

    let (gc_key, _) = pda::global_config(&KLEND_PROGRAM_ID);

    Instruction {
        program_id: KLEND_PROGRAM_ID,
        accounts: vec![
            AccountMeta::new_readonly(*signer, true),
            AccountMeta::new_readonly(gc_key, false),
            AccountMeta::new_readonly(*lending_market, false),
            AccountMeta::new(*reserve, false),
        ],
        data,
    }
}

// ---------------------------------------------------------------------------
// Market & reserve creation
// ---------------------------------------------------------------------------

fn create_lending_market(svm: &mut LiteSVM, admin: &Keypair, market: &Keypair) {
    // Pre-create the market account owned by klend
    let create_ix = system_instruction::create_account(
        &admin.pubkey(),
        &market.pubkey(),
        svm.minimum_balance_for_rent_exemption(LENDING_MARKET_SIZE),
        LENDING_MARKET_SIZE as u64,
        &KLEND_PROGRAM_ID,
    );
    let init_ix = build_init_lending_market_ix(&admin.pubkey(), &market.pubkey());

    let tx = Transaction::new_signed_with_payer(
        &[create_ix, init_ix],
        Some(&admin.pubkey()),
        &[admin, market],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();
}

pub fn init_reserve(
    svm: &mut LiteSVM,
    admin: &Keypair,
    market: &Keypair,
    reserve: &Keypair,
    liquidity_mint: &Pubkey,
) {
    // Admin needs a token account with initial liquidity
    let admin_ta = create_token_account(svm, admin, liquidity_mint, &admin.pubkey());
    mint_to(svm, admin, liquidity_mint, &admin_ta, MIN_INITIAL_DEPOSIT);

    // Pre-create the reserve account owned by klend
    let create_ix = system_instruction::create_account(
        &admin.pubkey(),
        &reserve.pubkey(),
        svm.minimum_balance_for_rent_exemption(RESERVE_SIZE),
        RESERVE_SIZE as u64,
        &KLEND_PROGRAM_ID,
    );
    let init_ix = build_init_reserve_ix(
        &admin.pubkey(),
        &market.pubkey(),
        &reserve.pubkey(),
        liquidity_mint,
        &admin_ta,
    );

    let tx = Transaction::new_signed_with_payer(
        &[create_ix, init_ix],
        Some(&admin.pubkey()),
        &[admin, reserve],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();
}

pub fn configure_reserve(
    svm: &mut LiteSVM,
    admin: &Keypair,
    lending_market: &Pubkey,
    reserve: &Pubkey,
    pyth_oracle: &Pubkey,
) {
    // All updates use skip_validation=true since the reserve is in Hidden/initialization phase.
    let updates: Vec<(u8, Vec<u8>)> = vec![
        // Mode 21: UpdatePythPrice — value is a Pubkey
        (21, pyth_oracle.to_bytes().to_vec()),
        // Mode 18: UpdateTokenInfoPriceMaxAge — value is u64
        (18, 1_000_000u64.to_le_bytes().to_vec()),
        // Mode 24: UpdateBorrowRateCurve — 11 CurvePoints (8 bytes each = 88 bytes)
        (24, build_flat_borrow_rate_curve(100)),
        // Mode 1: UpdateLoanToValuePct — value is u8
        (1, vec![75u8]),
        // Mode 3: UpdateLiquidationThresholdPct — value is u8
        (3, vec![80u8]),
        // Mode 33: UpdateBorrowFactor — value is u64
        (33, 100u64.to_le_bytes().to_vec()),
        // Mode 39: UpdateReserveStatus — 0 = Active (must be last with skip=true)
        (39, vec![0u8]),
    ];

    for (mode, value) in updates {
        let ix = build_update_reserve_config_ix(
            &admin.pubkey(),
            lending_market,
            reserve,
            mode,
            value,
            true,
        );
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&admin.pubkey()),
            &[admin],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).unwrap();
    }

    // Set deposit/borrow limits after activation (skip_validation=false)
    let post_activation: Vec<(u8, Vec<u8>)> = vec![
        // Mode 9: UpdateDepositLimit
        (9, u64::MAX.to_le_bytes().to_vec()),
        // Mode 10: UpdateBorrowLimit
        (10, u64::MAX.to_le_bytes().to_vec()),
        // Mode 45: UpdateBorrowLimitOutsideElevationGroup
        (45, u64::MAX.to_le_bytes().to_vec()),
    ];
    for (mode, value) in post_activation {
        let ix = build_update_reserve_config_ix(
            &admin.pubkey(),
            lending_market,
            reserve,
            mode,
            value,
            false,
        );
        let tx = Transaction::new_signed_with_payer(
            &[ix],
            Some(&admin.pubkey()),
            &[admin],
            svm.latest_blockhash(),
        );
        svm.send_transaction(tx).unwrap();
    }
}

/// Build a flat borrow rate curve: 11 CurvePoints.
/// First point is (0, rate_bps), last point is (10000, rate_bps), rest are (0, 0).
/// Validation requires the last point to have utilization_rate_bps = 10000 (100%).
fn build_flat_borrow_rate_curve(rate_bps: u32) -> Vec<u8> {
    let mut buf = Vec::with_capacity(88);
    // Point 0: utilization=0, rate=rate_bps
    buf.extend_from_slice(&0u32.to_le_bytes());
    buf.extend_from_slice(&rate_bps.to_le_bytes());
    // Points 1-10: utilization=10000 (100%), rate=rate_bps
    // All must be >= previous utilization (sorted), last must be 10000.
    for _ in 1..11 {
        buf.extend_from_slice(&10000u32.to_le_bytes());
        buf.extend_from_slice(&rate_bps.to_le_bytes());
    }
    buf
}

// ---------------------------------------------------------------------------
// Token helpers
// ---------------------------------------------------------------------------

pub fn create_mint(svm: &mut LiteSVM, payer: &Keypair, decimals: u8) -> Pubkey {
    let mint = Keypair::new();
    let rent = svm.minimum_balance_for_rent_exemption(spl_token::state::Mint::LEN);

    let create_ix = system_instruction::create_account(
        &payer.pubkey(),
        &mint.pubkey(),
        rent,
        spl_token::state::Mint::LEN as u64,
        &spl_token::id(),
    );
    let init_ix = spl_ix::initialize_mint(
        &spl_token::id(),
        &mint.pubkey(),
        &payer.pubkey(),
        None,
        decimals,
    )
    .unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[create_ix, init_ix],
        Some(&payer.pubkey()),
        &[payer, &mint],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();
    mint.pubkey()
}

pub fn create_token_account(
    svm: &mut LiteSVM,
    payer: &Keypair,
    mint: &Pubkey,
    owner: &Pubkey,
) -> Pubkey {
    let ta = Keypair::new();
    let rent = svm.minimum_balance_for_rent_exemption(spl_token::state::Account::LEN);

    let create_ix = system_instruction::create_account(
        &payer.pubkey(),
        &ta.pubkey(),
        rent,
        spl_token::state::Account::LEN as u64,
        &spl_token::id(),
    );
    let init_ix = spl_ix::initialize_account(&spl_token::id(), &ta.pubkey(), mint, owner).unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[create_ix, init_ix],
        Some(&payer.pubkey()),
        &[payer, &ta],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();
    ta.pubkey()
}

pub fn mint_to(
    svm: &mut LiteSVM,
    authority: &Keypair,
    mint: &Pubkey,
    destination: &Pubkey,
    amount: u64,
) {
    let ix = spl_ix::mint_to(
        &spl_token::id(),
        mint,
        destination,
        &authority.pubkey(),
        &[],
        amount,
    )
    .unwrap();

    let tx = Transaction::new_signed_with_payer(
        &[ix],
        Some(&authority.pubkey()),
        &[authority],
        svm.latest_blockhash(),
    );
    svm.send_transaction(tx).unwrap();
}

pub fn advance_clock_by_slots(svm: &mut LiteSVM, slots: u64) {
    let mut clock: Clock = svm.get_sysvar();
    clock.slot += slots;
    clock.unix_timestamp += slots as i64;
    svm.set_sysvar(&clock);
}

pub fn token_balance(svm: &mut LiteSVM, token_account: &Pubkey) -> u64 {
    let account = svm.get_account(token_account).unwrap();
    // SPL token account: amount is at offset 64, 8 bytes LE
    u64::from_le_bytes(account.data[64..72].try_into().unwrap())
}

// ---------------------------------------------------------------------------
// User + obligation helpers
// ---------------------------------------------------------------------------

pub fn create_user_and_obligation(env: &mut TestEnv) -> (Keypair, Pubkey) {
    let user = Keypair::new();
    env.svm.airdrop(&user.pubkey(), 10_000_000_000).unwrap();

    // init_user
    let user_ixs = klend_interface::helpers::init_user(
        user.pubkey(),
        user.pubkey(),
        Pubkey::new_unique(), // user_lookup_table (unused in practice)
        None,
    );
    let tx = Transaction::new_signed_with_payer(
        &user_ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    // init_obligation (vanilla: tag=0, id=0)
    let obligation_ixs = klend_interface::helpers::init_obligation(
        user.pubkey(),
        user.pubkey(),
        env.lending_market.pubkey(),
        0,
        0,
        Pubkey::default(),
        Pubkey::default(),
    );
    let obligation_pda = {
        let (pda, _) = pda::obligation(
            &KLEND_PROGRAM_ID,
            0,
            0,
            &user.pubkey(),
            &env.lending_market.pubkey(),
            &Pubkey::default(),
            &Pubkey::default(),
        );
        pda
    };
    let tx = Transaction::new_signed_with_payer(
        &obligation_ixs,
        Some(&user.pubkey()),
        &[&user],
        env.svm.latest_blockhash(),
    );
    env.svm.send_transaction(tx).unwrap();

    (user, obligation_pda)
}

pub fn build_reserve_info(env: &TestEnv) -> klend_interface::helpers::ReserveInfo {
    build_reserve_info_for(
        &env.reserve.pubkey(),
        &env.lending_market.pubkey(),
        &env.liquidity_mint,
        &env.pyth_oracle,
    )
}

pub fn build_reserve_info_for(
    reserve: &Pubkey,
    lending_market: &Pubkey,
    liquidity_mint: &Pubkey,
    pyth_oracle: &Pubkey,
) -> klend_interface::helpers::ReserveInfo {
    klend_interface::helpers::ReserveInfo {
        address: *reserve,
        lending_market: *lending_market,
        liquidity_mint: *liquidity_mint,
        liquidity_token_program: TOKEN_PROGRAM_ID,
        pyth_oracle: Some(*pyth_oracle),
        switchboard_price_oracle: None,
        switchboard_twap_oracle: None,
        scope_prices: None,
    }
}

pub fn build_obligation_info(
    obligation: &Pubkey,
    reserve: &Pubkey,
    has_deposit: bool,
    has_borrow: bool,
) -> klend_interface::helpers::ObligationInfo {
    klend_interface::helpers::ObligationInfo {
        address: *obligation,
        deposit_reserves: if has_deposit { vec![*reserve] } else { vec![] },
        borrow_reserves: if has_borrow { vec![*reserve] } else { vec![] },
        referrer: None,
    }
}
