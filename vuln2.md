# Vulnerability #2: Panic in Post-Transfer Balance Check

## Severity
**LOW**

## Location
- **File**: `programs/klend/src/handlers/handler_borrow_obligation_liquidity.rs`
- **Lines**: 146-153
- **Function**: `process_v1`

## Description
The post-transfer vault balance check uses `.unwrap()` on the token amount accessor, which could panic if the token account is in an invalid state. This prevents proper error handling and could cause unexpected transaction failures.

### Vulnerable Code
```rust
lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
    token_interface::accessor::amount(&accounts.reserve_source_liquidity.to_account_info())
        .unwrap(),  // <-- Could panic
    reserve.liquidity.available_amount,
    initial_reserve_token_balance,
    initial_reserve_available_liquidity,
    LendingAction::Subtractive(borrow_amount),
)?;
```

## Impact
- **Potential Panic**: If the token account state is corrupted or malformed
- **Error Handling**: The accessor::amount call could fail but `.unwrap()` causes a panic instead of proper error handling
- **Limited Exploitability**: This would only occur if the reserve's token account is already in a bad state

## Exploit Scenario
1. Reserve's source liquidity token account becomes corrupted (unlikely in normal operation)
2. User attempts to borrow from this reserve
3. The `accessor::amount` call fails during post-transfer check
4. `.unwrap()` causes a panic instead of returning a descriptive error

## Verification
**VERIFIED**: The code uses `.unwrap()` on `token_interface::accessor::amount()` which can fail if the account data is malformed.

## Recommendation
Use the `?` operator for proper error propagation:

```rust
lending_checks::post_transfer_vault_balance_liquidity_reserve_checks(
    token_interface::accessor::amount(&accounts.reserve_source_liquidity.to_account_info())?,
    reserve.liquidity.available_amount,
    initial_reserve_token_balance,
    initial_reserve_available_liquidity,
    LendingAction::Subtractive(borrow_amount),
)?;
```

## Risk Assessment
- **Exploitability**: Very Low (requires pre-existing account corruption)
- **Impact**: Low (transaction failure only, no fund loss)
- **Overall Severity**: LOW

## Notes
This same pattern appears in multiple handlers:
- `handler_borrow_obligation_liquidity.rs` (lines 146-153)
- `handler_liquidate_obligation_and_redeem_reserve_collateral.rs` (similar pattern)
- Other handlers with post-transfer checks

All instances should be updated for consistency and better error handling.

## Status
- **Discovery Date**: 2025-11-17
- **Verification Status**: Confirmed via code review
- **Reported**: Yes
