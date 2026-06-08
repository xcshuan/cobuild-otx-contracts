# Limit Order Lock Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a tests-only `limit-order-lock` contract that represents an NFT-for-UDT order directly in lock args and validates OTX-scoped fill settlement when the NFT input is unlocked.

**Architecture:** Add a new test contract crate under `tests/contracts/limit-order-lock` and a thin integration fixture under `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`. The contract parses fixed-width lock args, requires a single current lock input with the offered NFT type, uses `plan_lock_validation()` to find one OTX-level `FillOrder` input-lock action, and counts only same-OTX `test-udt` outputs to the maker.

**Tech Stack:** Rust 2024, `ckb-std` 1.1, `cobuild-core`, `cobuild-types` action readers through `cobuild-core`, `ckb-testtool`, tests-only RISC-V contract Makefiles, offline Cargo.

---

## Source Requirements

Read these before executing:

- `docs/superpowers/specs/2026-06-08-limit-order-lock-design.md`
- `docs/superpowers/specs/2026-06-08-limit-order-create-order-design.md`
- `tests/contracts/limit-order-type/src/types.rs`
- `tests/contracts/limit-order-type/src/entry.rs`
- `tests/contracts/limit-order-type/src/error.rs`
- `tests/contracts/limit-order-type/Makefile`
- `tests/contracts/test-nft/src/entry.rs`
- `tests/contracts/test-udt/src/entry.rs`
- `tests/src/fixtures/limit_order.rs`
- `tests/src/fixtures/limit_order/type_nft_for_udt.rs`
- `tests/src/framework/tx.rs`
- `tests/src/framework/cobuild.rs`
- `tests/src/framework/assertions.rs`
- `tests/src/framework/fixture.rs`
- `tests/src/framework/contracts.rs`
- `tests/src/framework/cells.rs`
- `tests/tests/limit_order_type.rs`
- `crates/cobuild-core/src/plan.rs`
- `crates/cobuild-core/src/engine.rs`

Start execution with:

```bash
git status --short
```

Expected: no output. If dirty, inspect first and do not overwrite unrelated changes.

## File Structure

Create:

- `tests/contracts/limit-order-lock/Cargo.toml`
  - Contract crate manifest, modeled after `limit-order-type` without `type-id`.
- `tests/contracts/limit-order-lock/Makefile`
  - Contract build wrapper, modeled after `test-udt` / `test-nft`.
- `tests/contracts/limit-order-lock/src/main.rs`
  - `ckb_std::entry!` adapter returning `i8`.
- `tests/contracts/limit-order-lock/src/lib.rs`
  - `no_std` crate root and public modules.
- `tests/contracts/limit-order-lock/src/error.rs`
  - Local tests-only error enum and conversions.
- `tests/contracts/limit-order-lock/src/types.rs`
  - Lock args, action, UDT payment parser, and pure validation helpers.
- `tests/contracts/limit-order-lock/src/entry.rs`
  - Syscall entry validation, Cobuild plan integration, OTX scope checks.
- `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`
  - Scenario builders for the lock-shaped NFT-for-UDT order.
- `tests/tests/limit_order_lock.rs`
  - Thin integration tests that call fixture scenarios and assertions.

Modify:

- `Cargo.toml`
  - Add workspace member `tests/contracts/limit-order-lock`.
- `tests/src/fixtures/limit_order.rs`
  - Re-export lock fixture helpers; reuse the existing `limit_order_fill`
    action-data builder for tag `2`.
- `tests/src/fixtures/mod.rs`
  - No required change because `limit_order` is already public.
- `docs/superpowers/plans/2026-06-08-limit-order-lock-plan.md`
  - Record red/green results after every task.

Do not modify:

- `contracts/cobuild-otx-lock`
- `crates/cobuild-core`
- `crates/cobuild-types`
- public action schemas
- production contracts outside `tests/contracts`

## Red/Green Log Discipline

Each task has a **Red/Green Record** section. During execution, replace the instruction line with the exact command and observed result:

```text
Red: <command> -> <failing output summary>
Green: <command> -> <passing output summary>
```

Expected-failure integration tests must not dump tracked `tests/failed_txs` unless `COBUILD_TEST_DUMP_EXPECTED_FAILURES=1`.

## Task 1: Scaffold Contract Crate And Pure Parsers

**Files:**
- Create: `tests/contracts/limit-order-lock/Cargo.toml`
- Create: `tests/contracts/limit-order-lock/Makefile`
- Create: `tests/contracts/limit-order-lock/src/main.rs`
- Create: `tests/contracts/limit-order-lock/src/lib.rs`
- Create: `tests/contracts/limit-order-lock/src/error.rs`
- Create: `tests/contracts/limit-order-lock/src/types.rs`
- Modify: `Cargo.toml`
- Modify: `docs/superpowers/plans/2026-06-08-limit-order-lock-plan.md`

**Red/Green Record:**

Scaffold: `cargo test -p limit-order-lock --offline` -> PASS; compiled
`limit-order-lock`, ran 0 lib tests, 0 main tests, 0 doc tests. Initial
placeholder `types.rs` emitted one unused-import warning before parser
implementation.

Red: `cargo test -p limit-order-lock --offline` -> FAIL as expected with
unresolved parser symbols: `parse_order_args`, `parse_fill_order_action`, and
`parse_udt_payment`.

Green: `cargo test -p limit-order-lock --offline` -> PASS; 6 parser unit tests
passed, 0 failed. `cargo fmt` -> PASS. `git diff --check` -> PASS with no
output. `git status --short` -> showed only Task 1 changes after restoring the
generated `Cargo.lock` package entry.

- [x] **Step 1: Add minimal crate scaffold**

Add `tests/contracts/limit-order-lock` to workspace members in `Cargo.toml` after `tests/contracts/limit-order-type`.

Create `tests/contracts/limit-order-lock/Cargo.toml`:

```toml
[package]
name = "limit-order-lock"
version = "0.1.0"
edition = "2024"

[dependencies]
ckb-std = { version = "1.1", default-features = false, features = ["allocator", "ckb-types", "dummy-atomic"] }
cobuild-core = { path = "../../../crates/cobuild-core" }

[features]
library = []
native-simulator = ["library", "ckb-std/native-simulator"]
```

Create `tests/contracts/limit-order-lock/Makefile` by copying the simple contract Makefile style from `tests/contracts/test-udt/Makefile`. Keep `CONTRACT_FEATURES :=` empty because this lock does not need `type-id`.

Create `src/lib.rs`:

```rust
#![cfg_attr(not(feature = "library"), no_std)]
#![allow(special_module_name)]
#![allow(unused_attributes)]
#[cfg(feature = "library")]
mod main;
#[cfg(feature = "library")]
pub use main::program_entry;

extern crate alloc;

pub mod entry;
pub mod error;
pub mod types;
```

Create `src/main.rs`:

```rust
#![cfg_attr(not(any(feature = "library", test)), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(any(feature = "library", test))]
extern crate alloc;

#[cfg(not(any(feature = "library", test)))]
ckb_std::entry!(program_entry);
#[cfg(not(any(feature = "library", test)))]
ckb_std::default_alloc!(16384, 1258306, 64);

pub fn program_entry() -> i8 {
    match limit_order_lock::entry::main() {
        Ok(()) => 0,
        Err(err) => err.into(),
    }
}
```

Create `src/entry.rs` with a temporary body:

```rust
use crate::error::Error;

pub fn main() -> Result<(), Error> {
    Err(Error::InvalidCobuild)
}
```

Create `src/error.rs`:

```rust
use ckb_std::error::SysError;
use cobuild_core::error::CoreError;

#[repr(i8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    IndexOutOfBound = 1,
    ItemMissing = 2,
    LengthNotEnough = 3,
    Encoding = 4,
    InvalidArgs = 5,
    InvalidActionData = 6,
    UnsupportedAction = 7,
    InvalidNftInput = 8,
    ActionMismatch = 9,
    InsufficientPayment = 10,
    AmountOverflow = 11,
    InvalidCobuild = 12,
    UnexpectedSyscall = 13,
}

impl From<SysError> for Error {
    fn from(err: SysError) -> Self {
        match err {
            SysError::IndexOutOfBound => Self::IndexOutOfBound,
            SysError::ItemMissing => Self::ItemMissing,
            SysError::LengthNotEnough(_) => Self::LengthNotEnough,
            SysError::Encoding => Self::Encoding,
            SysError::Unknown(_) => Self::UnexpectedSyscall,
            SysError::WaitFailure
            | SysError::InvalidFd
            | SysError::OtherEndClosed
            | SysError::MaxVmsSpawned
            | SysError::MaxFdsCreated => Self::UnexpectedSyscall,
        }
    }
}

impl From<CoreError> for Error {
    fn from(_: CoreError) -> Self {
        Self::InvalidCobuild
    }
}

impl From<Error> for i8 {
    fn from(err: Error) -> Self {
        err as i8
    }
}
```

Create `src/types.rs` with only imports and constants:

```rust
use crate::error::Error;

pub const ORDER_ARGS_LEN: usize = 104;
pub const UDT_PAYMENT_DATA_LEN: usize = 16;
pub const FILL_ORDER_TAG: u8 = 2;
pub const FILL_ORDER_DATA_LEN: usize = 41;
```

- [x] **Step 2: Verify scaffold compiles**

Run:

```bash
cargo test -p limit-order-lock --offline
```

Expected: PASS with zero or few tests.

- [x] **Step 3: Write failing parser tests**

Append to `tests/contracts/limit-order-lock/src/types.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec::Vec;

    const OWNER_LOCK_HASH: [u8; 32] = [2; 32];
    const NFT_TYPE_HASH: [u8; 32] = [3; 32];
    const REQUESTED_ASSET_ID: [u8; 32] = [4; 32];

    fn order_args(min_requested_amount: u64) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&OWNER_LOCK_HASH);
        data.extend_from_slice(&NFT_TYPE_HASH);
        data.extend_from_slice(&REQUESTED_ASSET_ID);
        data.extend_from_slice(&min_requested_amount.to_le_bytes());
        data
    }

    fn fill_action_data(asset_id: [u8; 32], min_requested_amount: u64) -> Vec<u8> {
        let mut data = Vec::new();
        data.push(FILL_ORDER_TAG);
        data.extend_from_slice(&asset_id);
        data.extend_from_slice(&min_requested_amount.to_le_bytes());
        data
    }

    #[test]
    fn parse_order_args_reads_fixed_width_fields() {
        let args = parse_order_args(&order_args(30)).expect("order args");

        assert_eq!(args.owner_lock_hash, OWNER_LOCK_HASH);
        assert_eq!(args.offered_nft_type_hash, NFT_TYPE_HASH);
        assert_eq!(args.requested_asset_id, REQUESTED_ASSET_ID);
        assert_eq!(args.min_requested_amount, 30);
    }

    #[test]
    fn parse_order_args_rejects_short_and_long_data() {
        let mut short = order_args(30);
        short.pop();
        let mut long = order_args(30);
        long.push(0);

        assert_eq!(parse_order_args(&short), Err(Error::InvalidArgs));
        assert_eq!(parse_order_args(&long), Err(Error::InvalidArgs));
    }

    #[test]
    fn parse_fill_action_accepts_tag_two() {
        let action = parse_fill_order_action(&fill_action_data(REQUESTED_ASSET_ID, 30))
            .expect("fill action");

        assert_eq!(action.requested_asset_id, REQUESTED_ASSET_ID);
        assert_eq!(action.min_requested_amount, 30);
    }

    #[test]
    fn parse_fill_action_rejects_unknown_tag_and_bad_lengths() {
        assert_eq!(parse_fill_order_action(&[]), Err(Error::InvalidActionData));

        let mut unknown = fill_action_data(REQUESTED_ASSET_ID, 30);
        unknown[0] = 1;
        assert_eq!(parse_fill_order_action(&unknown), Err(Error::UnsupportedAction));

        let mut short = fill_action_data(REQUESTED_ASSET_ID, 30);
        short.pop();
        let mut long = fill_action_data(REQUESTED_ASSET_ID, 30);
        long.push(0);
        assert_eq!(parse_fill_order_action(&short), Err(Error::InvalidActionData));
        assert_eq!(parse_fill_order_action(&long), Err(Error::InvalidActionData));
    }

    #[test]
    fn parse_udt_payment_accepts_u64_compatible_u128() {
        assert_eq!(parse_udt_payment(&30u128.to_le_bytes()), Ok(30));
    }

    #[test]
    fn parse_udt_payment_rejects_bad_length_and_overflow() {
        assert_eq!(parse_udt_payment(&[0u8; 15]), Err(Error::InvalidActionData));
        assert_eq!(
            parse_udt_payment(&(u128::from(u64::MAX) + 1).to_le_bytes()),
            Err(Error::AmountOverflow)
        );
    }
}
```

- [x] **Step 4: Run red**

Run:

```bash
cargo test -p limit-order-lock --offline
```

Expected: FAIL with unresolved items such as `parse_order_args`, `parse_fill_order_action`, `parse_udt_payment`, `OrderArgs`, and `FillOrderAction`.

- [x] **Step 5: Implement minimal parser types**

Replace the top of `tests/contracts/limit-order-lock/src/types.rs` with:

```rust
use crate::error::Error;

pub const ORDER_ARGS_LEN: usize = 104;
pub const UDT_PAYMENT_DATA_LEN: usize = 16;
pub const FILL_ORDER_TAG: u8 = 2;
pub const FILL_ORDER_DATA_LEN: usize = 41;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderArgs {
    pub owner_lock_hash: [u8; 32],
    pub offered_nft_type_hash: [u8; 32],
    pub requested_asset_id: [u8; 32],
    pub min_requested_amount: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FillOrderAction {
    pub requested_asset_id: [u8; 32],
    pub min_requested_amount: u64,
}

pub fn parse_order_args(data: &[u8]) -> Result<OrderArgs, Error> {
    if data.len() != ORDER_ARGS_LEN {
        return Err(Error::InvalidArgs);
    }

    Ok(OrderArgs {
        owner_lock_hash: read_bytes32(data, 0),
        offered_nft_type_hash: read_bytes32(data, 32),
        requested_asset_id: read_bytes32(data, 64),
        min_requested_amount: read_u64(data, 96),
    })
}

pub fn parse_fill_order_action(data: &[u8]) -> Result<FillOrderAction, Error> {
    let Some((&tag, _)) = data.split_first() else {
        return Err(Error::InvalidActionData);
    };
    if tag != FILL_ORDER_TAG {
        return Err(Error::UnsupportedAction);
    }
    if data.len() != FILL_ORDER_DATA_LEN {
        return Err(Error::InvalidActionData);
    }

    Ok(FillOrderAction {
        requested_asset_id: read_bytes32(data, 1),
        min_requested_amount: read_u64(data, 33),
    })
}

pub fn parse_udt_payment(data: &[u8]) -> Result<u64, Error> {
    if data.len() != UDT_PAYMENT_DATA_LEN {
        return Err(Error::InvalidActionData);
    }

    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(data);
    u64::try_from(u128::from_le_bytes(bytes)).map_err(|_| Error::AmountOverflow)
}

fn read_bytes32(data: &[u8], offset: usize) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&data[offset..offset + 32]);
    out
}

fn read_u64(data: &[u8], offset: usize) -> u64 {
    let mut out = [0u8; 8];
    out.copy_from_slice(&data[offset..offset + 8]);
    u64::from_le_bytes(out)
}
```

Keep the tests added in Step 3 below this code.

- [x] **Step 6: Run green**

Run:

```bash
cargo test -p limit-order-lock --offline
```

Expected: PASS for parser tests.

- [x] **Step 7: Format and commit**

Run:

```bash
cargo fmt
cargo test -p limit-order-lock --offline
git diff --check
git status --short
```

Expected: formatter succeeds, parser tests pass, diff check has no output, status shows only Task 1 files.

Commit:

```bash
git add Cargo.toml tests/contracts/limit-order-lock docs/superpowers/plans/2026-06-08-limit-order-lock-plan.md
git commit -m "test: scaffold limit order lock parser"
```

## Task 2: Add Pure Fill Validation And OTX Layout Helpers

**Files:**
- Modify: `tests/contracts/limit-order-lock/src/types.rs`
- Modify: `tests/contracts/limit-order-lock/src/entry.rs`
- Modify: `docs/superpowers/plans/2026-06-08-limit-order-lock-plan.md`

**Red/Green Record:**

Red: `cargo test -p limit-order-lock --offline validate_fill -- --nocapture` -> FAIL as expected with unresolved `UdtPayment` type/struct and unresolved `validate_fill` function in `types.rs` validation tests.

Green: `cargo test -p limit-order-lock --offline` -> PASS; 13 unit tests passed, including parser, pure fill validation, and OTX helper tests. `cargo fmt` -> PASS. `cargo test -p limit-order-lock --offline` -> PASS; 13 passed, 0 failed. `git diff --check` -> PASS with no output.

- [ ] **Step 1: Write failing pure validation tests**

Append to the `#[cfg(test)] mod tests` in `types.rs`:

```rust
    fn order(min_requested_amount: u64) -> OrderArgs {
        parse_order_args(&order_args(min_requested_amount)).expect("order args")
    }

    fn action(asset_id: [u8; 32], min_requested_amount: u64) -> FillOrderAction {
        parse_fill_order_action(&fill_action_data(asset_id, min_requested_amount))
            .expect("fill action")
    }

    fn payment(owner_lock_hash: [u8; 32], asset_id: [u8; 32], amount: u64) -> UdtPayment {
        UdtPayment {
            owner_lock_hash,
            asset_id,
            amount,
        }
    }

    #[test]
    fn validate_fill_accepts_exact_and_over_payment() {
        assert_eq!(
            validate_fill(
                &order(30),
                &action(REQUESTED_ASSET_ID, 30),
                &[payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 30)]
            ),
            Ok(())
        );

        assert_eq!(
            validate_fill(
                &order(30),
                &action(REQUESTED_ASSET_ID, 31),
                &[payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 40)]
            ),
            Ok(())
        );
    }

    #[test]
    fn validate_fill_rejects_action_mismatch_and_amount_below_order_minimum() {
        assert_eq!(
            validate_fill(&order(30), &action([9; 32], 30), &[]),
            Err(Error::ActionMismatch)
        );
        assert_eq!(
            validate_fill(&order(30), &action(REQUESTED_ASSET_ID, 29), &[]),
            Err(Error::InsufficientPayment)
        );
    }

    #[test]
    fn validate_fill_counts_only_matching_owner_and_asset() {
        assert_eq!(
            validate_fill(
                &order(30),
                &action(REQUESTED_ASSET_ID, 30),
                &[
                    payment([9; 32], REQUESTED_ASSET_ID, 30),
                    payment(OWNER_LOCK_HASH, [8; 32], 30),
                    payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 29),
                ],
            ),
            Err(Error::InsufficientPayment)
        );
    }

    #[test]
    fn validate_fill_detects_payment_sum_overflow() {
        assert_eq!(
            validate_fill(
                &order(30),
                &action(REQUESTED_ASSET_ID, 30),
                &[
                    payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, u64::MAX),
                    payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 1),
                ],
            ),
            Err(Error::AmountOverflow)
        );
    }
```

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-lock --offline validate_fill -- --nocapture
```

Expected: FAIL with unresolved `UdtPayment` and `validate_fill`.

- [ ] **Step 3: Implement pure validation**

Add to `types.rs` above helper readers:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UdtPayment {
    pub owner_lock_hash: [u8; 32],
    pub asset_id: [u8; 32],
    pub amount: u64,
}

pub fn validate_fill(
    order: &OrderArgs,
    action: &FillOrderAction,
    payments: &[UdtPayment],
) -> Result<(), Error> {
    if action.requested_asset_id != order.requested_asset_id {
        return Err(Error::ActionMismatch);
    }
    if action.min_requested_amount < order.min_requested_amount {
        return Err(Error::InsufficientPayment);
    }

    let paid = payments.iter().try_fold(0u64, |paid, payment| {
        if payment.owner_lock_hash == order.owner_lock_hash
            && payment.asset_id == order.requested_asset_id
        {
            paid.checked_add(payment.amount)
                .ok_or(Error::AmountOverflow)
        } else {
            Ok(paid)
        }
    })?;

    if paid < action.min_requested_amount {
        return Err(Error::InsufficientPayment);
    }

    Ok(())
}
```

- [ ] **Step 4: Write failing OTX helper tests**

Replace `tests/contracts/limit-order-lock/src/entry.rs` with helper skeleton and tests:

```rust
use ckb_std::ckb_constants::Source;
use cobuild_core::{
    layout::Range,
    plan::{ActionOrigin, OtxMessageLayout},
};

use crate::error::Error;

pub fn main() -> Result<(), Error> {
    Err(Error::InvalidCobuild)
}

pub fn otx_fill_layout(origin: &ActionOrigin, input_index: usize) -> Result<OtxMessageLayout, Error> {
    let ActionOrigin::Otx { layout, .. } = origin else {
        return Err(Error::InvalidCobuild);
    };
    if !range_contains(layout.base_inputs, input_index)? {
        return Err(Error::InvalidCobuild);
    }
    Ok(*layout)
}

fn range_contains(range: Range, index: usize) -> Result<bool, Error> {
    let end = range
        .start
        .checked_add(range.count)
        .ok_or(Error::InvalidCobuild)?;
    Ok(index >= range.start as usize && index < end as usize)
}

#[allow(dead_code)]
fn _source_marker(_: Source) {}

#[cfg(test)]
mod tests {
    use super::*;

    fn layout() -> OtxMessageLayout {
        OtxMessageLayout {
            base_inputs: Range { start: 1, count: 1 },
            append_inputs: Range { start: 2, count: 1 },
            base_outputs: Range { start: 0, count: 1 },
            append_outputs: Range { start: 1, count: 1 },
            base_cell_deps: Range { start: 0, count: 0 },
            append_cell_deps: Range { start: 0, count: 0 },
            base_header_deps: Range { start: 0, count: 0 },
            append_header_deps: Range { start: 0, count: 0 },
        }
    }

    #[test]
    fn otx_fill_layout_accepts_current_input_in_base_scope() {
        let origin = ActionOrigin::Otx {
            witness_index: 0,
            otx_index: 0,
            layout: layout(),
        };

        assert_eq!(
            otx_fill_layout(&origin, 1).map(|layout| layout.append_outputs),
            Ok(Range { start: 1, count: 1 })
        );
    }

    #[test]
    fn otx_fill_layout_rejects_tx_level_action() {
        assert_eq!(
            otx_fill_layout(&ActionOrigin::TxLevel { witness_index: 0 }, 1),
            Err(Error::InvalidCobuild)
        );
    }

    #[test]
    fn otx_fill_layout_rejects_append_only_current_input() {
        let origin = ActionOrigin::Otx {
            witness_index: 0,
            otx_index: 0,
            layout: layout(),
        };

        assert_eq!(otx_fill_layout(&origin, 2), Err(Error::InvalidCobuild));
    }
}
```

- [ ] **Step 5: Run green**

Run:

```bash
cargo test -p limit-order-lock --offline
```

Expected: PASS for parser, pure validation, and OTX helper tests.

- [ ] **Step 6: Commit**

Run:

```bash
cargo fmt
cargo test -p limit-order-lock --offline
git diff --check
```

Expected: all pass.

Commit:

```bash
git add tests/contracts/limit-order-lock docs/superpowers/plans/2026-06-08-limit-order-lock-plan.md
git commit -m "test: add limit order lock fill validation"
```

## Task 3: Implement Contract Entry Validation

**Files:**
- Modify: `tests/contracts/limit-order-lock/src/entry.rs`
- Modify: `docs/superpowers/plans/2026-06-08-limit-order-lock-plan.md`

**Red/Green Record:**

Red: `cargo test -p limit-order-lock --offline range_contains -- --nocapture`
-> PASS before full entry implementation; 2 `range_contains` tests passed
because the existing helper already used checked addition.

Green: `cargo test -p limit-order-lock --offline` -> PASS; 15 unit tests
passed, 0 failed. Initial contract build
`make -e -C tests/contracts/limit-order-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts/.worktrees/limit-order-lock BUILD_DIR=build/debug CARGO_ARGS=--offline`
compiled but failed copying because `build/debug` did not exist. After creating
the ignored local build directory, the same make command -> PASS and copied
`limit-order-lock`. Final `cargo fmt` -> PASS; final
`cargo test -p limit-order-lock --offline` -> PASS with 15 passed, 0 failed;
final same make build -> PASS; `git diff --check` -> PASS with no output.

Review fix: `cargo test -p limit-order-lock --offline payment_output_matches_order_identity_requires_owner_and_asset -- --nocapture`
-> RED first with unresolved `payment_output_matches_order`, then PASS after
filtering payment outputs by order owner and requested asset before parsing
amount data. `cargo fmt` -> PASS. `cargo test -p limit-order-lock --offline`
-> PASS with 16 passed, 0 failed. `git diff --check` -> PASS with no output.

- [x] **Step 1: Write failing entry unit tests for input index helper**

Add these tests to `entry.rs` test module:

```rust
    #[test]
    fn range_contains_accepts_start_and_last_index() {
        assert_eq!(range_contains(Range { start: 3, count: 2 }, 3), Ok(true));
        assert_eq!(range_contains(Range { start: 3, count: 2 }, 4), Ok(true));
        assert_eq!(range_contains(Range { start: 3, count: 2 }, 5), Ok(false));
    }

    #[test]
    fn range_contains_rejects_overflowing_range() {
        assert_eq!(
            range_contains(
                Range {
                    start: usize::MAX,
                    count: 1,
                },
                usize::MAX
            ),
            Err(Error::InvalidCobuild)
        );
    }
```

If `Range.start` and `Range.count` are `u32` in this checkout, use `u32::MAX as usize` in the expected overflow test and adjust the range literal to `Range { start: u32::MAX, count: 1 }`.

- [x] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-lock --offline range_contains -- --nocapture
```

Expected: FAIL if the overflow test exposes an incorrect cast or if helper visibility/signature needs correction.

- [x] **Step 3: Implement entry validation**

Replace `entry.rs` with:

```rust
use alloc::vec::Vec;

use ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, prelude::*},
    high_level::{
        QueryIter, load_cell_data, load_cell_lock_hash, load_cell_type_hash, load_script,
        load_script_hash,
    },
};
use cobuild_core::{
    context::CurrentScript,
    engine::CobuildContext,
    layout::Range,
    plan::{ActionOrigin, OtxMessageLayout},
    reader::cursor_bytes,
};

use crate::{
    error::Error,
    types::{
        OrderArgs, UdtPayment, parse_fill_order_action, parse_order_args, parse_udt_payment,
        validate_fill,
    },
};

pub fn main() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();
    let order = parse_order_args(&args)?;

    let current_lock_hash = load_script_hash()?;
    let input_index = single_group_input_index(current_lock_hash)?;
    verify_offered_nft_input(input_index, order.offered_nft_type_hash)?;

    let plan = CobuildContext::build(CurrentScript::InputLock(current_lock_hash))?
        .plan_lock_validation()?;
    if plan.related_actions.len() != 1 {
        return Err(Error::InvalidCobuild);
    }

    let related = &plan.related_actions[0];
    let layout = otx_fill_layout(&related.origin, input_index)?;
    let action_data = cursor_bytes(&related.action.data)?;
    let action = parse_fill_order_action(&action_data)?;
    let payments = collect_payments(layout)?;

    validate_fill(&order, &action, &payments)
}

fn single_group_input_index(current_lock_hash: [u8; 32]) -> Result<usize, Error> {
    let group_count = QueryIter::new(load_cell_data, Source::GroupInput).count();
    if group_count != 1 {
        return Err(Error::InvalidNftInput);
    }

    QueryIter::new(load_cell_lock_hash, Source::Input)
        .enumerate()
        .find_map(|(index, lock_hash)| (lock_hash == current_lock_hash).then_some(index))
        .ok_or(Error::InvalidNftInput)
}

fn verify_offered_nft_input(
    input_index: usize,
    offered_nft_type_hash: [u8; 32],
) -> Result<(), Error> {
    let Some(type_hash) = load_cell_type_hash(input_index, Source::GroupInput)? else {
        return Err(Error::InvalidNftInput);
    };
    if type_hash != offered_nft_type_hash {
        return Err(Error::InvalidNftInput);
    }
    Ok(())
}

pub fn otx_fill_layout(origin: &ActionOrigin, input_index: usize) -> Result<OtxMessageLayout, Error> {
    let ActionOrigin::Otx { layout, .. } = origin else {
        return Err(Error::InvalidCobuild);
    };
    if !range_contains(layout.base_inputs, input_index)? {
        return Err(Error::InvalidCobuild);
    }
    Ok(*layout)
}

fn collect_payments(layout: OtxMessageLayout) -> Result<Vec<UdtPayment>, Error> {
    let mut payments = Vec::new();
    collect_payments_from_range(layout.base_outputs, &mut payments)?;
    collect_payments_from_range(layout.append_outputs, &mut payments)?;
    Ok(payments)
}

fn collect_payments_from_range(range: Range, payments: &mut Vec<UdtPayment>) -> Result<(), Error> {
    let end = range
        .start
        .checked_add(range.count)
        .ok_or(Error::InvalidCobuild)?;

    for index in range.start..end {
        let data = load_cell_data(index, Source::Output)?;
        if data.len() != crate::types::UDT_PAYMENT_DATA_LEN {
            continue;
        }
        let Some(asset_id) = load_cell_type_hash(index, Source::Output)? else {
            continue;
        };
        let owner_lock_hash = load_cell_lock_hash(index, Source::Output)?;
        payments.push(UdtPayment {
            owner_lock_hash,
            asset_id,
            amount: parse_udt_payment(&data)?,
        });
    }

    Ok(())
}

fn range_contains(range: Range, index: usize) -> Result<bool, Error> {
    let start = range.start as usize;
    let count = range.count as usize;
    let end = start.checked_add(count).ok_or(Error::InvalidCobuild)?;
    Ok(index >= start && index < end)
}
```

Keep the existing `#[cfg(test)] mod tests` and adjust imports if `cargo fmt` moves them.

- [x] **Step 4: Run green**

Run:

```bash
cargo test -p limit-order-lock --offline
```

Expected: PASS.

- [x] **Step 5: Build contract binary**

Run:

```bash
make -e -C tests/contracts/limit-order-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
```

Expected: PASS and copies `build/debug/limit-order-lock`.

- [x] **Step 6: Commit**

Run:

```bash
cargo fmt
cargo test -p limit-order-lock --offline
make -e -C tests/contracts/limit-order-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
git diff --check
```

Expected: all pass.

Commit:

```bash
git add tests/contracts/limit-order-lock docs/superpowers/plans/2026-06-08-limit-order-lock-plan.md
git commit -m "test: validate limit order lock entry"
```

## Task 4: Add Happy Path Integration Fixture

**Files:**
- Create: `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`
- Modify: `tests/src/fixtures/limit_order.rs`
- Create: `tests/tests/limit_order_lock.rs`
- Modify: `docs/superpowers/plans/2026-06-08-limit-order-lock-plan.md`

**Red/Green Record:**

Red: `cargo test -p tests --test limit_order_lock --offline limit_order_lock_accepts_nft_for_udt_otx_fill -- --nocapture` -> FAIL as expected with `error[E0583]: file not found for module 'lock_for_udt'` at `tests/src/fixtures/limit_order.rs:12:1`.

Green: `make -e -C tests/contracts/limit-order-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts/.worktrees/limit-order-lock BUILD_DIR=build/debug CARGO_ARGS=--offline` -> PASS; built and copied `limit-order-lock` debug binary. `make -e -C tests/contracts/test-udt build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts/.worktrees/limit-order-lock BUILD_DIR=build/debug CARGO_ARGS=--offline` -> PASS; built and copied `test-udt` debug binary. `make -e -C tests/contracts/test-nft build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts/.worktrees/limit-order-lock BUILD_DIR=build/debug CARGO_ARGS=--offline` -> PASS; built and copied `test-nft` debug binary. `cargo test -p tests --test limit_order_lock --offline limit_order_lock_accepts_nft_for_udt_otx_fill -- --nocapture` -> PASS; 1 test passed, 0 failed.

Final verification: `cargo fmt` -> PASS. `cargo test -p limit-order-lock --offline` -> PASS; 16 unit tests passed, 0 failed, plus 0 main/doc tests. `cargo test -p tests --test limit_order_lock --offline limit_order_lock_accepts_nft_for_udt_otx_fill -- --nocapture` -> PASS; 1 test passed, 0 failed. `git diff --check` -> PASS with no output.

- [x] **Step 1: Write failing integration test**

Create `tests/tests/limit_order_lock.rs`:

```rust
use tests::fixtures::limit_order::{limit_order_lock_nft_for_udt_case, failed_txs_count};

#[test]
fn limit_order_lock_accepts_nft_for_udt_otx_fill() {
    let (fixture, tx) = limit_order_lock_nft_for_udt_case();

    fixture.assert_pass(&tx);
}
```

In `tests/src/fixtures/limit_order.rs`, add:

```rust
#[cfg(not(test))]
mod lock_nft_for_udt;

#[cfg(not(test))]
pub use lock_nft_for_udt::{
    LimitOrderLockFillCase, limit_order_lock_nft_for_udt_case,
    limit_order_lock_nft_for_udt_case_with,
};
```

- [x] **Step 2: Run red**

Run:

```bash
cargo test -p tests --test limit_order_lock --offline limit_order_lock_accepts_nft_for_udt_otx_fill -- --nocapture
```

Expected: FAIL because `lock_for_udt.rs` or exported fixture functions do not exist.

- [x] **Step 3: Add lock fixture scenario**

Create `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`:

```rust
use ckb_testtool::ckb_types::{
    bytes::Bytes,
    core::{ScriptHashType, TransactionView},
    prelude::*,
};

use crate::framework::{
    cells::{TestCellOutput, live_input, typed_output},
    contracts::{DeployedScript, cell_dep_for_script, deploy_data2_script},
    fixture::CobuildTestFixture,
    scripts::script_hash,
};

use super::{LimitOrderCobuildMessageExt, NFT_TYPE_ARGS};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitOrderLockFillCase {
    Valid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct LockOrder {
    owner_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
    requested_asset_id: [u8; 32],
    min_requested_amount: u64,
}

pub fn limit_order_lock_nft_for_udt_case() -> (CobuildTestFixture, TransactionView) {
    limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::Valid)
}

pub fn limit_order_lock_nft_for_udt_case_with(
    case: LimitOrderLockFillCase,
) -> (CobuildTestFixture, TransactionView) {
    assert_eq!(case, LimitOrderLockFillCase::Valid);

    let mut fixture = CobuildTestFixture::new();
    let limit_order_lock_code = deploy_data2_script(fixture.context_mut(), "limit-order-lock", Vec::new());
    let always_success = fixture.deploy_always_success();
    let owner_lock = always_success.script.clone();
    let buyer_lock = always_success.script.clone();
    let issuer_lock_hash = script_hash(&always_success.script);
    let nft = deploy_test_nft(&mut fixture, NFT_TYPE_ARGS);
    let udt = deploy_test_udt_with_owner(&mut fixture, issuer_lock_hash);

    let order = LockOrder {
        owner_lock_hash: script_hash(&owner_lock),
        offered_nft_type_hash: nft.script_hash,
        requested_asset_id: udt.script_hash,
        min_requested_amount: 30,
    };
    let order_lock = fixture
        .context_mut()
        .build_script_with_hash_type(
            &limit_order_lock_code.out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(&lock_args(order)),
        )
        .expect("build limit order lock");
    let order_lock_hash = script_hash(&order_lock);

    let nft_payload = nft_data(b"lock-order-nft", [1, 2, 3, 4], 1_717_171_717);
    let nft_input = live_input(
        fixture.context_mut(),
        typed_output(order_lock.clone(), nft.script.clone(), 100_000_000_000),
        nft_payload.clone(),
    );
    let udt_input = live_input(
        fixture.context_mut(),
        typed_output(buyer_lock.clone(), udt.script.clone(), 100_000_000_000),
        udt_amount_data(30),
    );
    let nft_output = TestCellOutput::new(
        typed_output(buyer_lock, nft.script.clone(), 90_000_000_000),
        nft_payload,
    );
    let udt_payment_output = TestCellOutput::new(
        typed_output(owner_lock, udt.script.clone(), 90_000_000_000),
        udt_amount_data(30),
    );

    let message = fixture
        .cobuild()
        .input_lock_action(order_lock_hash)
        .limit_order_fill(udt.script_hash, 30)
        .build();
    let otx = fixture
        .otx()
        .base_input_cells(1)
        .base_output_cells(1)
        .append_input_cells(1)
        .append_output_cells(1)
        .allow_append_inputs()
        .allow_append_outputs()
        .message(message)
        .build_with_layout();
    let tx = fixture
        .tx()
        .cell_dep(cell_dep_for_script(&limit_order_lock_code))
        .cell_dep(cell_dep_for_script(&always_success))
        .cell_dep(cell_dep_for_script(&nft))
        .cell_dep(cell_dep_for_script(&udt))
        .base_input(nft_input)
        .append_input(udt_input)
        .base_output(nft_output)
        .append_output(udt_payment_output)
        .otx(otx)
        .build();

    (fixture, tx)
}

fn lock_args(order: LockOrder) -> Vec<u8> {
    let mut data = Vec::with_capacity(104);
    data.extend_from_slice(&order.owner_lock_hash);
    data.extend_from_slice(&order.offered_nft_type_hash);
    data.extend_from_slice(&order.requested_asset_id);
    data.extend_from_slice(&order.min_requested_amount.to_le_bytes());
    data
}

fn deploy_test_udt_with_owner(
    fixture: &mut CobuildTestFixture,
    owner_lock_hash: [u8; 32],
) -> DeployedScript {
    deploy_data2_script(fixture.context_mut(), "test-udt", owner_lock_hash.to_vec())
}

fn deploy_test_nft(fixture: &mut CobuildTestFixture, args: [u8; 32]) -> DeployedScript {
    deploy_data2_script(fixture.context_mut(), "test-nft", args.to_vec())
}

fn nft_data(name: &[u8], attributes: [u8; 4], created_at: u64) -> Vec<u8> {
    let mut data = Vec::with_capacity(1 + name.len() + 4 + 8);
    data.push(name.len() as u8);
    data.extend_from_slice(name);
    data.extend_from_slice(&attributes);
    data.extend_from_slice(&created_at.to_le_bytes());
    data
}

fn udt_amount_data(amount: u128) -> Vec<u8> {
    amount.to_le_bytes().to_vec()
}
```

- [x] **Step 4: Add input-lock action builder**

If `tests/src/framework/cobuild.rs` does not already support input-lock targets, add:

```rust
    pub fn input_lock_action(mut self, script_hash: [u8; 32]) -> Self {
        self.script_hash = script_hash;
        self.script_role = 0;
        self
    }
```

to `impl CobuildMessageBuilder`, using the same role value as `cobuild_core::protocol::ScriptRole::InputLock`.

- [x] **Step 5: Build contracts and run green**

Run:

```bash
make -e -C tests/contracts/limit-order-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-udt build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-nft build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
cargo test -p tests --test limit_order_lock --offline limit_order_lock_accepts_nft_for_udt_otx_fill -- --nocapture
```

Expected: all build commands PASS and happy path integration test PASS.

- [x] **Step 6: Commit**

Run:

```bash
cargo fmt
cargo test -p limit-order-lock --offline
cargo test -p tests --test limit_order_lock --offline limit_order_lock_accepts_nft_for_udt_otx_fill -- --nocapture
git diff --check
```

Expected: all pass.

Commit:

```bash
git add tests/src/fixtures/limit_order.rs tests/src/fixtures/limit_order/lock_nft_for_udt.rs tests/src/framework/cobuild.rs tests/tests/limit_order_lock.rs docs/superpowers/plans/2026-06-08-limit-order-lock-plan.md
git commit -m "test: add limit order lock happy path"
```

## Task 5: Add Integration Failure Matrix

**Files:**
- Modify: `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`
- Modify: `tests/tests/limit_order_lock.rs`
- Modify: `docs/superpowers/plans/2026-06-08-limit-order-lock-plan.md`

**Red/Green Record:**
- RED: `cargo test -p tests --test limit_order_lock --offline -- --nocapture` failed at compile time with 28 `E0599` errors: missing `LimitOrderLockFillCase` variants and missing `CobuildTestFixture::assert_lock_script_exit`.
- GREEN: `cargo test -p tests --test limit_order_lock --offline -- --nocapture` initially compiled and ran 15 tests with 13 passed / 2 failed: `OrderInputInAppendScope` reached `Inputs[0].Lock` exit 8 before cobuild layout validation, and `WrongNftType` initially failed from `Outputs[0].Type`. After fixture correction and expected append-scope exit update, the command passed with 15 passed / 0 failed.
- VERIFY: `cargo fmt` exit 0; `cargo test -p limit-order-lock --offline` passed 16 unit tests plus 0 main/doc tests; `cargo test -p tests --test limit_order_lock --offline -- --nocapture` passed 15 integration tests; `git diff --check` exit 0; `find tests/failed_txs -maxdepth 1 -type f 2>/dev/null | wc -l` printed `1`; `git status --short --ignored tests/failed_txs` printed `!! tests/failed_txs/`. The passing failure-matrix tests checked no new expected-failure dumps were added during each case.
- FOLLOW-UP FIX: `OrderInputInAppendScope` was corrected to use an always-success dummy base input and put the only limit-order-lock input in append scope with an append seal. `cargo test -p tests --test limit_order_lock --offline limit_order_lock_rejects_append_scope_input -- --nocapture` first observed `Inputs[1].Lock`; after updating the assertion to input index 1 and exit 12, it passed with 1 passed / 0 failed. `cargo test -p tests --test limit_order_lock --offline -- --nocapture` passed with 15 passed / 0 failed; `git diff --check` exit 0; failed_txs count remained `1` ignored file.

- [x] **Step 1: Write failing thin tests**

Extend `tests/tests/limit_order_lock.rs`:

```rust
use tests::fixtures::limit_order::{
    LimitOrderLockFillCase, failed_txs_count, limit_order_lock_nft_for_udt_case,
    limit_order_lock_nft_for_udt_case_with,
};

fn assert_no_expected_failure_dump(before: usize) {
    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), before);
    }
}

#[test]
fn limit_order_lock_rejects_malformed_lock_args() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::MalformedArgs);
    fixture.assert_lock_script_exit(&tx, 0, 5);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_wrong_nft_type() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::WrongNftType);
    fixture.assert_lock_script_exit(&tx, 0, 8);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_tx_level_fill_order() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::TxLevelFillOrder);
    fixture.assert_lock_script_exit(&tx, 0, 12);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_wrong_action_target() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::WrongActionTarget);
    fixture.assert_lock_script_exit(&tx, 0, 12);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_append_scope_input() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::OrderInputInAppendScope);
    fixture.assert_lock_script_exit(&tx, 1, 12);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_requested_asset_mismatch() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::RequestedAssetMismatch);
    fixture.assert_lock_script_exit(&tx, 0, 9);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_fill_amount_below_order_minimum() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::MinRequestedBelowRequired);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_insufficient_udt() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::InsufficientUdt);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_wrong_udt() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::WrongUdt);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_wrong_owner() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::WrongOwner);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_does_not_count_tx_level_remainder_payment() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::TxLevelRemainderOnly);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_does_not_count_payment_in_another_otx() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::PaymentInAnotherOtx);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_unknown_action_tag() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::UnknownActionTag);
    fixture.assert_lock_script_exit(&tx, 0, 7);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_malformed_action_payload() {
    let before = failed_txs_count();
    let (fixture, tx) = limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::MalformedAction);
    fixture.assert_lock_script_exit(&tx, 0, 6);
    assert_no_expected_failure_dump(before);
}
```

- [x] **Step 2: Run red**

Run:

```bash
cargo test -p tests --test limit_order_lock --offline -- --nocapture
```

Expected: FAIL with missing enum variants and possibly missing `assert_lock_script_exit`.

- [x] **Step 3: Add lock script assertion wrapper if missing**

If `CobuildTestFixture` does not expose lock script assertions, add to `tests/src/framework/fixture.rs`:

```rust
    pub fn assert_lock_script_exit(&self, tx: &TransactionView, input_index: usize, code: i8) {
        let result = self.context.verify_tx(tx, 50_000_000);
        assertions::assert_lock_script_exit_result(result, input_index, code);
    }
```

If `MAX_CYCLES` is private in `assertions.rs`, instead add a public helper in `assertions.rs` mirroring `assert_type_script_exit`:

```rust
pub fn assert_lock_script_exit(
    context: &Context,
    tx: &TransactionView,
    input_index: usize,
    code: i8,
) {
    let result = context.verify_tx(tx, MAX_CYCLES);
    if result.is_err() && dump_expected_failures() {
        let _ = verify_and_dump_failed_tx(context, tx, MAX_CYCLES);
    }
    assert_lock_script_exit_result(result, input_index, code);
}
```

and call it from `CobuildTestFixture::assert_lock_script_exit`.

- [x] **Step 4: Implement scenario variants**

Extend `LimitOrderLockFillCase`:

```rust
pub enum LimitOrderLockFillCase {
    Valid,
    MalformedArgs,
    WrongNftType,
    TxLevelFillOrder,
    WrongActionTarget,
    OrderInputInAppendScope,
    RequestedAssetMismatch,
    MinRequestedBelowRequired,
    InsufficientUdt,
    WrongUdt,
    WrongOwner,
    TxLevelRemainderOnly,
    PaymentInAnotherOtx,
    UnknownActionTag,
    MalformedAction,
}
```

Update the fixture builder with these concrete knobs:

- `MalformedArgs`: remove one byte from `lock_args(order)` before building the order lock.
- `WrongNftType`: deploy a second NFT type and use it on the input while args keep the original NFT hash.
- `TxLevelFillOrder`: put `FillOrder` in `tx_level_message`, use `empty_message()` in the OTX.
- `WrongActionTarget`: target `[8; 32]` with `.input_lock_action([8; 32])`.
- `OrderInputInAppendScope`: make the order NFT an append input and add a dummy always-success base input; OTX layout uses `base_input_cells(1).append_input_cells(2)`.
- `RequestedAssetMismatch`: action requested asset is wrong UDT hash.
- `MinRequestedBelowRequired`: action min requested amount is `29`.
- `InsufficientUdt`: payment amount is `29`.
- `WrongUdt`: payment cell type is wrong UDT.
- `WrongOwner`: payment lock is a second always-success deployment.
- `TxLevelRemainderOnly`: OTX append payment is `29`, tx remainder output pays `1` to owner.
- `PaymentInAnotherOtx`: current OTX append payment is `29`, a second OTX append output pays `1` to owner.
- `UnknownActionTag`: action data is `vec![1]` plus 40 bytes of payload.
- `MalformedAction`: action data is valid FillOrder data with the final byte removed.

Use existing helpers from `type_nft_for_udt.rs` as a model, but keep all lock-order business scenarios in `lock_nft_for_udt.rs`.

- [x] **Step 5: Run green**

Run:

```bash
cargo test -p tests --test limit_order_lock --offline -- --nocapture
```

Expected: PASS for happy path and all failure cases.

- [x] **Step 6: Commit**

Run:

```bash
cargo fmt
cargo test -p limit-order-lock --offline
cargo test -p tests --test limit_order_lock --offline -- --nocapture
git diff --check
find tests/failed_txs -maxdepth 1 -type f 2>/dev/null | wc -l
git status --short --ignored tests/failed_txs
```

Expected: tests pass, diff check clean, failed tx output shows no new tracked files.

Commit:

```bash
git add tests/src/framework/assertions.rs tests/src/framework/fixture.rs tests/src/fixtures/limit_order/lock_nft_for_udt.rs tests/tests/limit_order_lock.rs docs/superpowers/plans/2026-06-08-limit-order-lock-plan.md
git commit -m "test: cover limit order lock failures"
```

## Task 6: Final Verification And Cleanup

**Files:**
- Modify: `docs/superpowers/plans/2026-06-08-limit-order-lock-plan.md`

**Red/Green Record:**

Verification setup:
`cargo test -p tests --lib --offline` initially failed because
`build/debug/limit-order-type` and `build/debug/cobuild-otx-lock` were missing
in the isolated worktree. Built both missing fixture binaries, then reran the
command successfully.

Workspace setup:
`cargo test --workspace --offline` initially failed because the existing
`limit_order` tests needed `build/debug/input-type-proxy-lock`. The vendored
`tests/vendor/ckb-proxy-locks` submodule was uninitialized in the worktree;
`git submodule update --init tests/vendor/ckb-proxy-locks` required escalation
because Git had to write module metadata under the shared `.git` directory.
After initializing the submodule, built `input-type-proxy-lock` with
`CARGO_TARGET_DIR=/home/xcshuan/contracts/ckb/cobuild-otx-contracts/.worktrees/limit-order-lock/target make -e -C tests/vendor/ckb-proxy-locks/contracts/input-type-proxy-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts/.worktrees/limit-order-lock BUILD_DIR=build/debug CUSTOM_RUSTFLAGS='-C debug-assertions' CARGO_ARGS=--offline`
-> PASS with one existing upstream dead_code warning, then rebuilt
`limit-order-type` so `generated_proxy_lock.rs` matched the vendored binary.

Final verification:
`cargo fmt` -> PASS.
`make -e -C tests/contracts/limit-order-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts/.worktrees/limit-order-lock BUILD_DIR=build/debug CARGO_ARGS=--offline` -> PASS.
`make -e -C tests/contracts/test-udt build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts/.worktrees/limit-order-lock BUILD_DIR=build/debug CARGO_ARGS=--offline` -> PASS.
`make -e -C tests/contracts/test-nft build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts/.worktrees/limit-order-lock BUILD_DIR=build/debug CARGO_ARGS=--offline` -> PASS.
`cargo test -p limit-order-lock --offline` -> PASS; 16 unit tests passed.
`cargo test -p tests --test limit_order_lock --offline` -> PASS; 15 integration tests passed.
`cargo test -p tests --lib --offline` -> PASS after building missing existing fixture binaries; 24 tests passed.
`cargo test --workspace --offline` -> PASS after initializing/building the vendored proxy lock prerequisite.
`cargo fmt --check` initially reported formatting for regenerated
`tests/contracts/limit-order-type/src/generated_proxy_lock.rs`; after
`cargo fmt`, `cargo fmt --check` -> PASS.
`git diff --check` -> PASS.
`git status --short` -> showed modified plan plus regenerated
`tests/contracts/limit-order-type/src/generated_proxy_lock.rs`.
`find tests/failed_txs -maxdepth 1 -type f 2>/dev/null | wc -l` -> `1`.
`git status --short --ignored tests/failed_txs` -> `!! tests/failed_txs/`;
no tracked `tests/failed_txs` files were added.

- [x] **Step 1: Run required verification**

Run:

```bash
cargo fmt
make -e -C tests/contracts/limit-order-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-udt build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-nft build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
cargo test -p limit-order-lock --offline
cargo test -p tests --test limit_order_lock --offline
cargo test -p tests --lib --offline
cargo test --workspace --offline
cargo fmt --check
git diff --check
git status --short
find tests/failed_txs -maxdepth 1 -type f 2>/dev/null | wc -l
git status --short --ignored tests/failed_txs
```

Expected: all commands pass; `git status --short` only shows the plan file while recording final results; no tracked `tests/failed_txs` files are added.

- [ ] **Step 2: Review implementation diff**

Run:

```bash
git diff --stat HEAD
git diff HEAD -- tests/contracts/limit-order-lock tests/src/fixtures/limit_order.rs tests/src/fixtures/limit_order/lock_nft_for_udt.rs tests/tests/limit_order_lock.rs tests/src/framework Cargo.toml
```

Expected: diff is limited to tests-only contract, fixtures, framework helpers if needed, workspace member, and this plan's records. Confirm no changes to `contracts/cobuild-otx-lock`, `crates/cobuild-core`, or `crates/cobuild-types`.

- [ ] **Step 3: Commit final plan records**

Commit:

```bash
git add docs/superpowers/plans/2026-06-08-limit-order-lock-plan.md
git commit -m "docs: record limit order lock verification"
```

- [ ] **Step 4: Prepare final delivery summary**

Collect:

```bash
git log --oneline --decorate -n 8
git status --short
git status --short --ignored tests/failed_txs
```

Final response must include:

- spec path: `docs/superpowers/specs/2026-06-08-limit-order-lock-design.md`;
- plan path: `docs/superpowers/plans/2026-06-08-limit-order-lock-plan.md`;
- changed file summary;
- each task's red/green/verification command results;
- whether `tests/failed_txs` gained tracked files;
- all new commit hashes.

## Self-Review Checklist

- Spec coverage:
  - Lock args ABI covered in Task 1.
  - FillOrder ABI covered in Task 1.
  - Pure settlement validation covered in Task 2.
  - Cobuild lock plan, OTX origin, and base-input scope covered in Task 3.
  - Happy path integration covered in Task 4.
  - Failure matrix covered in Task 5.
  - Required final verification covered in Task 6.
- Scope:
  - No task modifies `contracts/cobuild-otx-lock`.
  - No task modifies `crates/cobuild-types`.
  - No task modifies `crates/cobuild-core`.
  - No production protocol features are included.
- Type consistency:
  - Contract uses `OrderArgs`, `FillOrderAction`, and `UdtPayment`.
  - Fixture uses `LimitOrderLockFillCase`.
  - Action tag remains `FILL_ORDER_TAG = 2`.
  - Lock args length remains 104 bytes and action data length remains 41 bytes.
