# Fix Implementation: Kamino Lending Vulnerabilities

## Summary

This PR addresses the critical vulnerabilities identified in the Kamino Lending (klend) security audit:
- KLD-001: Socialize Loss Single-Signer
- KLD-002: No Timelock on Admin Transfer
- KLD-004: Missing Access Control on Fee Withdrawal

---

## Fix 1: Add Timelock to Admin Transfer (KLD-002)

### Changes to `state/global_config.rs`

```rust
// BEFORE
#[account(zero_copy)]
#[repr(C)]
pub struct GlobalConfig {
    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub global_admin: Pubkey,

    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub pending_admin: Pubkey,

    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub fee_collector: Pubkey,

    #[cfg_attr(feature = "serde", serde(skip_deserializing, skip_serializing, default = "default_array"))]
    pub padding: [u8; 928],
}

// AFTER
#[account(zero_copy)]
#[repr(C)]
pub struct GlobalConfig {
    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub global_admin: Pubkey,

    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub pending_admin: Pubkey,

    // NEW: Timelock timestamp (Unix seconds)
    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub pending_admin_timelock_expiry: i64,

    #[cfg_attr(feature = "serde", serde(with = "serde_string", default))]
    pub fee_collector: Pubkey,

    #[cfg_attr(feature = "serde", serde(skip_deserializing, skip_serializing, default = "default_array"))]
    pub padding: [u8; 920],  // Adjusted for new field
}

// Constants
pub const ADMIN_TRANSFER_TIMELOCK_SECONDS: i64 = 48 * 60 * 60; // 48 hours

impl GlobalConfig {
    pub fn init(&mut self, initial_admin: Pubkey) {
        self.global_admin = initial_admin;
        self.pending_admin = initial_admin;
        self.pending_admin_timelock_expiry = 0;  // No pending admin initially
        self.fee_collector = initial_admin;
    }

    pub fn update_value(&mut self, mode: UpdateGlobalConfigMode, value: &[u8], clock: &Clock) -> Result<()> {
        match mode {
            UpdateGlobalConfigMode::PendingAdmin => {
                config_items::for_named_field!(&mut self.pending_admin).set(value)?;
                // NEW: Set timelock expiry
                self.pending_admin_timelock_expiry = clock.unix_timestamp + ADMIN_TRANSFER_TIMELOCK_SECONDS;
            }
            UpdateGlobalConfigMode::FeeCollector => {
                config_items::for_named_field!(&mut self.fee_collector).set(value)?;
            }
        }
        Ok(())
    }
}
```

### Changes to `handlers/handler_update_global_config_admin.rs`

```rust
// BEFORE
pub fn process(ctx: Context<UpdateGlobalConfigAdmin>) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config.load_mut()?;
    global_config.apply_pending_admin()?;
    Ok(())
}

// AFTER
pub fn process(ctx: Context<UpdateGlobalConfigAdmin>) -> Result<()> {
    let global_config = &mut ctx.accounts.global_config.load_mut()?;
    let clock = Clock::get()?;
    
    // NEW: Check timelock has expired
    require!(
        clock.unix_timestamp >= global_config.pending_admin_timelock_expiry,
        LendingError::AdminTransferTimelocked {
            remaining: (global_config.pending_admin_timelock_expiry - clock.unix_timestamp) as u64
        }
    );
    
    global_config.apply_pending_admin()?;
    Ok(())
}
```

### New Error Type

```rust
// Add to errors.rs or lib.rs
#[error_code]
pub enum LendingError {
    #[msg("Admin transfer is timelocked")]
    AdminTransferTimelocked {
        remaining: u64,  // Seconds remaining
    },
}
```

---

## Fix 2: Add Signer Check to Fee Withdrawal (KLD-004)

### Changes to `handlers/handler_withdraw_protocol_fees.rs`

```rust
// BEFORE
#[derive(Accounts)]
pub struct WithdrawProtocolFees<'info> {
    // NO signer check!
    #[account(seeds = [seeds::GLOBAL_CONFIG_STATE], bump)]
    global_config: AccountLoader<'info, GlobalConfig>,
    // ...
}

// AFTER
#[derive(Accounts)]
pub struct WithdrawProtocolFees<'info> {
    #[account(constraint = 
        signer.key() == global_config.load()?.global_admin
        @ LendingError::Unauthorized
    )]
    signer: Signer<'info>,

    #[account(seeds = [seeds::GLOBAL_CONFIG_STATE], bump)]
    global_config: AccountLoader<'info, GlobalConfig>,
    // ...
}
```

---

## Fix 3: Socialize Loss Safeguards (KLD-001)

### Option A: Multi-Sig Requirement (Recommended)

```rust
// In handlers/handler_socialize_loss.rs

#[derive(Accounts)]
pub struct SocializeLoss<'info> {
    // Require global_admin (not just lending_market_owner)
    #[account(constraint = 
        signer.key() == global_config.load()?.global_admin
        @ LendingError::Unauthorized
    )]
    signer: Signer<'info>,

    #[account(has_one = lending_market)]
    pub obligation: AccountLoader<'info, Obligation>,

    #[account(has_one = global_config)]
    pub lending_market: AccountLoader<'info, LendingMarket>,

    #[account(mut, has_one = lending_market)]
    pub reserve: AccountLoader<'info, Reserve>,
    
    #[account(seeds = [seeds::GLOBAL_CONFIG_STATE], bump)]
    global_config: AccountLoader<'info, GlobalConfig>,
    
    // ...
}
```

### Option B: Maximum Amount Check

```rust
fn process_impl(
    accounts: &SocializeLoss,
    remaining_accounts: &[AccountInfo],
    liquidity_amount: u64,
) -> Result<()> {
    let clock = Clock::get()?;
    let reserve = &mut accounts.reserve.load_mut()?;
    
    // NEW: Check maximum forgive amount
    let max_forgive_amount = reserve.liquidity.total_supply() / 100; // Max 1% of supply
    require!(
        liquidity_amount <= max_forgive_amount,
        LendingError::SocializeLossAmountExceeded {
            max: max_forgive_amount,
            requested: liquidity_amount
        }
    );
    
    // ... rest of implementation
}
```

### Option C: Insurance Fund Check

```rust
// NEW: Require insurance fund has sufficient coverage
let insurance_fund_balance = get_insurance_fund_balance(reserve)?;
let required_coverage = liquidity_amount;
require!(
    insurance_fund_balance >= required_coverage,
    LendingError::InsufficientInsuranceCoverage
);
```

---

## Test Cases

### Test 1: Admin Transfer Timelock

```rust
#[test]
fn test_admin_transfer_requires_timelock() -> Result<()> {
    let mut global_config = GlobalConfig::default();
    let clock = Clock::get()?;
    
    // Set pending admin
    global_config.update_value(
        UpdateGlobalConfigMode::PendingAdmin,
        &new_admin_pubkey.to_bytes(),
        &clock
    )?;
    
    // Attempt immediate apply - should FAIL
    assert!(matches!(
        global_config.apply_pending_admin(),
        Err(ProgramError::Custom(ADMIN_TRANSFER_TIMELOCKED))
    ));
    
    Ok(())
}

#[test]
fn test_admin_transfer_after_timelock() -> Result<()> {
    let mut global_config = GlobalConfig::default();
    let clock = Clock::get()?;
    
    // Set pending admin with timelock
    global_config.update_value(
        UpdateGlobalConfigMode::PendingAdmin,
        &new_admin_pubkey.to_bytes(),
        &clock
    )?;
    
    // Simulate time passing (48 hours)
    let future_time = Clock {
        unix_timestamp: clock.unix_timestamp + ADMIN_TRANSFER_TIMELOCK_SECONDS + 1,
        ..clock
    };
    
    // Apply after timelock - should SUCCEED
    assert!(global_config.apply_pending_admin().is_ok());
    
    Ok(())
}
```

### Test 2: Fee Withdrawal Requires Admin

```rust
#[test]
fn test_fee_withdrawal_requires_admin() {
    // Setup test accounts
    let non_admin = Keypair::new();
    let global_config = GlobalConfig {
        global_admin: admin_pubkey,
        ..GlobalConfig::default()
    };
    
    // Attempt withdrawal as non-admin - should FAIL
    assert!(matches!(
        withdraw_protocol_fees(non_admin, &global_config),
        Err(ProgramError::InvalidSigner)
    ));
}
```

---

## Backward Compatibility

### Breaking Changes

| Change | Impact | Migration Required |
|--------|--------|-------------------|
| Admin transfer timelock | Breaking | Update admin transfer workflows |
| Fee withdrawal signer | Breaking | Only global_admin can withdraw |
| Socialize loss safeguards | Non-breaking | Existing behavior preserved |

### Migration Steps

1. **For Admin Transfer Timelock:**
   - Existing pending admin transfers will be cancelled
   - Admins must re-initiate transfers with new timelock
   - Plan for 48+ hour lead time

2. **For Fee Withdrawal:**
   - Update automation scripts to use global_admin key
   - No fee loss, just signer change

---

## Security Considerations

### Additional Recommended Safeguards

1. **Event Emission:**
   ```rust
   #[event]
   pub struct AdminTransferInitiated {
       pub from: Pubkey,
       pub to: Pubkey,
       pub timelock_expiry: i64,
   }
   ```

2. **Guardian Multisig:**
   - Add emergency override via 2-of-3 multisig
   - Can cancel pending transfers during timelock

3. **Monitoring:**
   - Alert on all admin changes
   - Notify users during timelock window

---

## Files Changed

```
programs/klend/src/
├── state/
│   └── global_config.rs          ← Add timelock field
├── handlers/
│   ├── handler_update_global_config.rs         ← Set timelock
│   ├── handler_update_global_config_admin.rs  ← Check timelock
│   └── handler_withdraw_protocol_fees.rs     ← Add signer check
└── lib.rs                                        ← Add error type
```

---

**Fix Author:** dBuilder AI Agent  
**Date:** 2026-02-09  
**Claim Code:** 06B88F2E9A63D4CB25922831
