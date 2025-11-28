# Vulnerability #1: Potential DoS via Panic in Borrow Handler

## Severity
**MEDIUM**

## Location
- **File**: `programs/klend/src/handlers/handler_borrow_obligation_liquidity.rs`
- **Lines**: 70-72 (v1), 147-149 (v2)
- **Function**: `process_v1` and `process_v2`

## Description
The borrow handler uses `.unwrap()` when converting remaining accounts to `FatAccountLoader<Reserve>` for deposit reserves. If an attacker passes a malformed or invalid account in the `remaining_accounts` slice, the program will panic rather than return a proper error.

### Vulnerable Code
```rust
let deposit_reserves_iter = remaining_accounts
    .iter()
    .map(|account_info| FatAccountLoader::<Reserve>::try_from(account_info).unwrap());
```

## Impact
- **Denial of Service (DoS)**: Attackers can cause the program to panic by providing invalid accounts
- **Ungraceful Failure**: Transactions fail with a panic instead of a descriptive error message
- **User Experience**: Makes debugging difficult for legitimate users who accidentally provide wrong accounts

## Exploit Scenario
1. Attacker calls `borrow_obligation_liquidity` with valid primary accounts
2. Attacker includes an invalid or malformed account in `remaining_accounts`
3. The `.unwrap()` call panics when `try_from` returns an error
4. Transaction fails with panic, potentially causing confusion or DoS

## Verification
**VERIFIED**: The code uses `.unwrap()` which will panic if `try_from()` fails. This is confirmed by reading the source code at the specified lines.

## Recommendation
Replace `.unwrap()` with proper error handling:

```rust
let deposit_reserves_iter = remaining_accounts
    .iter()
    .map(|account_info| {
        FatAccountLoader::<Reserve>::try_from(account_info)
            .expect("Remaining account is not a valid deposit reserve")
    });
```

Or better yet, use the `?` operator to propagate errors:

```rust
let deposit_reserves: Result<Vec<_>> = remaining_accounts
    .iter()
    .map(|account_info| FatAccountLoader::<Reserve>::try_from(account_info))
    .collect();
let deposit_reserves_iter = deposit_reserves?.into_iter();
```

## Risk Assessment
- **Exploitability**: Low (requires intentional malformed input)
- **Impact**: Medium (DoS only, no fund loss)
- **Overall Severity**: MEDIUM

## Status
- **Discovery Date**: 2025-11-17
- **Verification Status**: Confirmed via code review
- **Reported**: Yes
