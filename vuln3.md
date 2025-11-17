# Vulnerability #3: Potential Integer Overflow in Same-Reserve Liquidation

## Severity
**MEDIUM**

## Location
- **File**: `programs/klend/src/handlers/handler_liquidate_obligation_and_redeem_reserve_collateral.rs`
- **Lines**: 215-227, 240-248
- **Function**: `process_impl`

## Description
The liquidation handler has complex logic to handle cases where the withdraw_reserve and repay_reserve are the same. This involves converting `u64` values to `i64` for signed arithmetic. When `withdraw_liquidity_amount` and `repay_amount` are very large (near `u64::MAX`), the conversion to `i64` could overflow, leading to incorrect calculations.

### Vulnerable Code
```rust
let net_withdrawal_amount = if accounts.withdraw_reserve_liquidity_supply...key
    == accounts.repay_reserve_liquidity_supply...key
{
    withdraw_liquidity_amount as i64 - repay_amount as i64  // Potential overflow
} else {
    withdraw_liquidity_amount as i64
};
```

## Impact
- **Arithmetic Overflow**: If either `withdraw_liquidity_amount` or `repay_amount` exceeds `i64::MAX` (2^63-1), the cast will overflow
- **Incorrect Calculations**: Could lead to wrong net withdrawal amounts
- **Bypass Security Checks**: Might bypass post-transfer balance validation if the overflow results in unexpected values

## Exploit Scenario
1. Attacker engineers a liquidation where both `withdraw_liquidity_amount` and `repay_amount` are large values
2. Either value is greater than `i64::MAX` (9,223,372,036,854,775,807)
3. The cast from `u64` to `i64` overflows (e.g., `9,223,372,036,854,775,808u64 as i64` becomes `-9,223,372,036,854,775,808i64`)
4. The subtraction produces an incorrect `net_withdrawal_amount`
5. Post-transfer checks might pass with incorrect values

## Verification
**PARTIALLY VERIFIED**:
- The code does cast `u64` to `i64` which can overflow
- However, the protocol has multiple layers of protection:
  - Liquidations are bounded by the obligation's debt and collateral values
  - Post-transfer balance checks validate final state (line 429)
  - In practice, token amounts rarely approach `i64::MAX` for most tokens

**Likelihood**: Low - most token amounts won't reach values that cause `i64` overflow, but mathematically possible.

## Recommendation
Use checked arithmetic for the conversion and subtraction:

```rust
let net_withdrawal_amount = if accounts.withdraw_reserve_liquidity_supply...key
    == accounts.repay_reserve_liquidity_supply...key
{
    let withdraw_i64 = i64::try_from(withdraw_liquidity_amount)
        .map_err(|_| LendingError::IntegerOverflow)?;
    let repay_i64 = i64::try_from(repay_amount)
        .map_err(|_| LendingError::IntegerOverflow)?;
    withdraw_i64.checked_sub(repay_i64)
        .ok_or(LendingError::MathOverflow)?
} else {
    i64::try_from(withdraw_liquidity_amount)
        .map_err(|_| LendingError::IntegerOverflow)?
};
```

## Risk Assessment
- **Exploitability**: Low (requires extreme token amounts and same-reserve liquidation)
- **Impact**: Medium (could bypass checks if exploited)
- **Overall Severity**: MEDIUM

## Additional Context
The post-transfer balance check at line 429 does use proper signed arithmetic checking:
```rust
lending_checks::post_transfer_vault_balance_liquidity_reserve_checks_signed(...)
```

This provides a secondary layer of defense, but the cast should still be made safe.

## Status
- **Discovery Date**: 2025-11-17
- **Verification Status**: Confirmed via code review, mitigated by secondary checks
- **Reported**: Yes
