# klend-interface

Instruction builders and zero-copy account deserialization for
Kamino Lending (Klend). No `anchor-lang` dependency;
targets `solana-sdk` v2.x.

## Quick start

Add to your `Cargo.toml`:

```toml
[dependencies]
klend-interface = { git = "https://github.com/Kamino-Finance/klend" }
solana-pubkey = "2.1"
solana-instruction = "2.1"
solana-sdk = "~2.3"
solana-client = "2.1"
spl-associated-token-account = "6"
```

## Two-level API

The crate provides two levels of instruction building:

- **Low-level** ([`instructions`]): one function per Klend instruction, returning a single
  [`solana_instruction::Instruction`]. You supply every account address manually.

- **High-level** ([`helpers`]): workflow builders that return `Vec<Instruction>` with required
  `refresh_reserve` / `refresh_obligation` instructions prepended automatically. Uses
  [`ReserveInfo`] and [`ObligationContext`] to auto-derive PDAs.

## Core types

| Type | Purpose |
|------|---------|
| [`ReserveInfo`] | On-chain reserve metadata; built via [`ReserveInfo::from_account_data`]. |
| [`ObligationContext`] | Bundles an obligation with all its reserves; provides `.deposit()`, `.borrow()`, `.repay()`, `.withdraw()`. |
| [`state::Reserve`] | Zero-copy deserialization of the on-chain Reserve account (8616 bytes). |
| [`state::Obligation`] | Zero-copy deserialization of the on-chain Obligation account (3336 bytes). |
| [`state::LendingMarket`] | Zero-copy deserialization of the on-chain LendingMarket account (4656 bytes). |

## Typical flow with `ObligationContext`

```rust,no_run
use klend_interface::{ObligationContext, pda, KLEND_PROGRAM_ID};
use solana_pubkey::Pubkey;
# fn example(rpc: &solana_client::rpc_client::RpcClient, owner: Pubkey) -> Result<(), Box<dyn std::error::Error>> {

let lending_market: Pubkey = "7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF".parse()?;
let reserve: Pubkey = "D6q6wuQSrifJKZYpR1M8R4YawnLDtDsMmWM1NbBmgJ59".parse()?;

// 1. Derive the obligation PDA
let (obligation_pubkey, _) = pda::obligation(
    &KLEND_PROGRAM_ID, 0, 0, &owner, &lending_market,
    &Pubkey::default(), &Pubkey::default(),
);

// 2. Fetch the obligation, discover its reserves, fetch them
let obligation_data = rpc.get_account(&obligation_pubkey)?;
let reserve_addrs = ObligationContext::reserve_addresses_for_obligation(&obligation_data.data)?;
let reserve_accounts = rpc.get_multiple_accounts(&reserve_addrs)?;

// 3. Build the context
let reserves: Vec<(Pubkey, &[u8])> = reserve_addrs.iter()
    .zip(reserve_accounts.iter())
    .filter_map(|(addr, acc)| acc.as_ref().map(|a| (*addr, a.data.as_slice())))
    .collect();
let ctx = ObligationContext::from_account_data(obligation_pubkey, &obligation_data.data, &reserves)?;

// 4. Build instructions — refreshes are prepended automatically
let user_ata = spl_associated_token_account::get_associated_token_address(&owner, &"EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v".parse()?);
let ixs = ctx.deposit(owner, &reserve, user_ata, 1_000_000)?;
# Ok(())
# }
```

## Scaled fraction fields (`_sf`)

All on-chain fields ending in `_sf` (e.g. `market_price_sf`, `borrowed_amount_sf`) are
128-bit fixed-point numbers stored as raw `u128` bits. Use [`Fraction`] (`U68F60` from the
`fixed` crate) to interpret them:

```rust
use klend_interface::Fraction;

let raw_sf: u128 = 1_152_921_504_606_846_976; // 1.0 encoded
let value = Fraction::from_bits(raw_sf);
let float_value: f64 = value.to_num();
```

## Examples

The `examples/` directory contains runnable examples matching the
[Kamino developer documentation](https://kamino.com/docs/build/developers/overview):

| Example | Description |
|---------|-------------|
| `deposit_lending` | Deposit liquidity and receive cTokens directly (no obligation). |
| `deposit_borrowing` | Deposit as collateral into an obligation using `ObligationContext`. |
| `borrow` | Borrow liquidity against an obligation. |
| `repay` | Repay borrowed liquidity. |
| `withdraw_obligation` | Withdraw collateral from an obligation and redeem for liquidity. |
| `redeem_ctokens` | Redeem cTokens for the underlying liquidity (no obligation). |
| `flash_loan` | Flash-borrow and repay within a single transaction. |
| `market_data` | Fetch and inspect lending market and reserve data. |
| `user_position` | Read an obligation and display deposit/borrow positions. |
| `cpi_deposit_and_borrow` | CPI from an Anchor program: deposit collateral and borrow (reference only, not runnable). |

Full source for each example is included in the [crate documentation](https://docs.rs/klend-interface)
or in the public [GitHub repository](https://github.com/Kamino-Finance/klend).

## License

Licensed under Business Source License 1.1 with an Additional Use Grant
permitting unmodified use as a dependency. See the LICENSE file for full terms.
