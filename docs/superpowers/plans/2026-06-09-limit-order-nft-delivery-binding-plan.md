# Limit Order NFT Delivery Binding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make tests-only limit-order Fill validation prove seller UDT payment and buyer NFT delivery, while shrinking FillOrder action data to only fields not already committed by order state.

**Architecture:** Migrate both tests-only contracts from the current 45-byte Fill action to a shared 37-byte action: `tag`, `payment_output_index`, and `buyer_lock_hash`. Rename order amount fields from `min_requested_amount` to `requested_amount`, keep explicit payment output uniqueness checks across same-OTX type+lock Fill actions, and add OTX settlement-output scanning for an NFT output locked to the buyer.

**Tech Stack:** Rust 2024, `ckb-std`, `cobuild-core`, tests-only CKB contracts, `ckb-testtool`, offline Cargo, contract Makefiles.

---

## Source Requirements

Read these before executing:

- `docs/superpowers/specs/2026-06-09-limit-order-nft-delivery-binding-design.md`
- `docs/superpowers/specs/2026-06-09-limit-order-payment-output-binding-design.md`
- `crates/cobuild-core/src/engine.rs`
- `crates/cobuild-core/src/plan.rs`
- `crates/cobuild-core/src/view.rs`
- `tests/contracts/limit-order-type/src/types.rs`
- `tests/contracts/limit-order-type/src/entry.rs`
- `tests/contracts/limit-order-type/src/error.rs`
- `tests/contracts/limit-order-lock/src/types.rs`
- `tests/contracts/limit-order-lock/src/entry.rs`
- `tests/contracts/limit-order-lock/src/error.rs`
- `tests/src/fixtures/limit_order.rs`
- `tests/src/fixtures/limit_order/type_nft_for_udt.rs`
- `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`
- `tests/src/framework/cobuild.rs`
- `tests/src/framework/tx.rs`
- `tests/src/framework/assertions.rs`
- `tests/tests/limit_order_type.rs`
- `tests/tests/limit_order_lock.rs`

Start execution with:

```bash
git status --short
```

Expected: no output. If dirty, inspect first and do not overwrite unrelated changes.

## File Structure

Modify:

- `tests/contracts/limit-order-type/src/types.rs`
  - Rename order amount field to `requested_amount`.
  - Keep CreateOrder ABI width but rename local field.
  - Change Fill ABI to 37 bytes.
  - Remove Fill action `requested_asset_id` and amount fields.
  - Validate payment against order state only.
- `tests/contracts/limit-order-lock/src/types.rs`
  - Same order field rename and 37-byte Fill ABI as type contract.
- `tests/contracts/limit-order-type/src/entry.rs`
  - Update duplicate payment parsing to 37-byte Fill actions.
  - Add current-OTX NFT delivery scan for `buyer_lock_hash + offered_nft_type_hash`.
  - Keep exact UDT payment output validation.
- `tests/contracts/limit-order-lock/src/entry.rs`
  - Same entry behavior as type contract for lock orders.
- `tests/src/fixtures/limit_order.rs`
  - Rename fixture state/builder amount names.
  - Change `limit_order_fill` builder to `payment_output_index + buyer_lock_hash`.
- `tests/src/fixtures/limit_order/type_nft_for_udt.rs`
  - Update action construction and scenarios for 37-byte Fill.
  - Add missing/wrong NFT delivery fixture cases.
- `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`
  - Same fixture updates for lock and mixed cases.
- `tests/src/framework/mod.rs`
  - Minimal unit-test sync for the renamed order field and new Fill action helper signature.
- `tests/tests/limit_order_type.rs`
  - Add assertions for missing/wrong NFT delivery and keep payment binding coverage.
- `tests/tests/limit_order_lock.rs`
  - Same for lock and mixed cases.
- `docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md`
  - Record Red/Green command results per task before each task commit.

Do not modify:

- `contracts/cobuild-otx-lock`
- `crates/cobuild-types`
- public production schemas
- production order-book, partial-fill, cancel, or compatibility behavior

## Red/Green Log Discipline

Each task has a **Red/Green Record** section. During execution, replace the instruction text with exact command summaries before committing:

```text
Red: <command> -> FAIL as expected: <specific failing test or compiler error>
Green: <command> -> PASS: <specific passing test count or command result>
Review: <brief diff review result>
Commit: <hash> <subject>
```

If any command fails unexpectedly, use `superpowers:systematic-debugging` before changing code.

## Task 1: Update `limit-order-type` Pure Types To 37-Byte Fill ABI

**Files:**
- Modify: `tests/contracts/limit-order-type/src/types.rs`
- Modify: `docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md`

**Red/Green Record:**
Red: `cargo test -p limit-order-type --offline` -> FAIL as expected: missing `requested_amount` fields, stale `FillOrderAction` fields, and stale 3-argument `validate_fill` signature in `tests/contracts/limit-order-type/src/types.rs`.
Green: `cargo test -p limit-order-type --offline` -> PASS: 33 passed, 0 failed; included minimal `tests/contracts/limit-order-type/src/entry.rs` compile sync because the crate compiles entry tests.
Review: `git diff --check` -> PASS; diff reviewed and limited to type Fill ABI/validation changes plus approved entry compile sync.
Commit: `5fe870e` test: shrink type fill action abi

- [ ] **Step 1: Write failing type-contract unit tests**

In `tests/contracts/limit-order-type/src/types.rs`, update or add tests with these names and expectations:

```rust
#[test]
fn parse_order_state_reads_requested_amount() {
    let order = parse_order_state(&order_data(30)).expect("order data");
    assert_eq!(order.requested_amount, 30);
}

#[test]
fn parse_create_order_action_reads_requested_amount() {
    let action = parse_create_order_action(&create_action_data(30)).expect("create action");
    assert_eq!(action.requested_amount, 30);
}

#[test]
fn parse_fill_order_action_accepts_payment_index_and_buyer_lock_hash() {
    let action = parse_limit_order_action(&fill_action_data(0x0403_0201, [7; 32]))
        .expect("fill action");

    assert_eq!(
        action,
        LimitOrderAction::Fill(FillOrderAction {
            payment_output_index: 0x0403_0201,
            buyer_lock_hash: [7; 32],
        })
    );
}

#[test]
fn parse_fill_order_action_rejects_old_41_and_45_byte_payloads() {
    assert_eq!(
        parse_limit_order_action(&legacy_41_byte_fill_action_data()).unwrap_err(),
        Error::InvalidActionData
    );
    assert_eq!(
        parse_limit_order_action(&legacy_45_byte_fill_action_data()).unwrap_err(),
        Error::InvalidActionData
    );
}

#[test]
fn validate_fill_uses_order_requested_amount() {
    let payment = settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 30);
    assert_eq!(validate_fill(&order_state(30), payment), Ok(()));
}

#[test]
fn validate_fill_rejects_payment_below_order_requested_amount() {
    let payment = settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 29);
    assert_eq!(
        validate_fill(&order_state(30), payment),
        Err(Error::InsufficientPayment)
    );
}
```

Keep existing wrong-owner and wrong-asset payment tests, but update them to call `validate_fill(&order, payment)` without an action argument.

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-type --offline
```

Expected: FAIL with missing `requested_amount`, stale `FillOrderAction` fields, stale 45-byte Fill parser, and stale `validate_fill` signature.

- [ ] **Step 3: Implement minimal type pure changes**

In `tests/contracts/limit-order-type/src/types.rs`:

- Keep `ORDER_DATA_LEN = 104`, `CREATE_ORDER_DATA_LEN = 105`.
- Change `FILL_ORDER_DATA_LEN` to `37`.
- Rename `OrderState::min_requested_amount` to `requested_amount`.
- Rename `CreateOrderAction::min_requested_amount` to `requested_amount`.
- Change `FillOrderAction` to:

```rust
pub struct FillOrderAction {
    pub payment_output_index: u32,
    pub buyer_lock_hash: [u8; 32],
}
```

- Parse order amount from offset `96` into `requested_amount`.
- Parse create amount from offset `97` into `requested_amount`.
- Parse Fill as:

```rust
Ok(FillOrderAction {
    payment_output_index: read_u32(data, 1),
    buyer_lock_hash: read_bytes32(data, 5),
})
```

- Update `validate_create` to compare `requested_amount`.
- Replace `validate_fill` with:

```rust
pub fn validate_fill(order: &OrderState, payment: SettlementCell) -> Result<(), Error> {
    if payment.owner_lock_hash != order.owner_lock_hash
        || payment.asset_id != order.requested_asset_id
        || payment.amount < order.requested_amount
    {
        return Err(Error::InsufficientPayment);
    }
    Ok(())
}
```

- Update helper/test names from `min_requested_amount` to `requested_amount`.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p limit-order-type --offline
```

Expected: PASS for `limit-order-type` unit tests.

- [ ] **Step 5: Review and commit**

Run:

```bash
git diff --check
git diff -- tests/contracts/limit-order-type/src/types.rs
```

Expected: no whitespace errors; diff limited to pure type ABI/validation changes.

Commit:

```bash
git add tests/contracts/limit-order-type/src/types.rs docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md
git commit -m "test: shrink type fill action abi"
```

## Task 2: Update `limit-order-lock` Pure Types To 37-Byte Fill ABI

**Files:**
- Modify: `tests/contracts/limit-order-lock/src/types.rs`
- Modify: `docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md`

**Red/Green Record:**
Red: `cargo test -p limit-order-lock --offline` -> FAIL as expected: missing `OrderArgs::requested_amount`, missing `FillOrderAction::buyer_lock_hash`, and stale 3-argument `validate_fill` signature in `tests/contracts/limit-order-lock/src/types.rs`.
Green: `cargo test -p limit-order-lock --offline` -> PASS: 22 passed, 0 failed; included minimal `tests/contracts/limit-order-lock/src/entry.rs` compile sync because the crate compiles entry tests.
Review: `git diff --check` -> PASS; diff reviewed and limited to lock Fill ABI/validation changes plus requested entry compile sync.
Commit: `10c49d4` test: shrink lock fill action abi

- [ ] **Step 1: Write failing lock-contract unit tests**

In `tests/contracts/limit-order-lock/src/types.rs`, update or add tests with these names and expectations:

```rust
#[test]
fn parse_order_args_reads_requested_amount() {
    let args = parse_order_args(&order_args(30)).expect("order args");
    assert_eq!(args.requested_amount, 30);
}

#[test]
fn parse_fill_action_accepts_payment_index_and_buyer_lock_hash() {
    let action = parse_fill_order_action(&fill_action_data(0x0403_0201, [7; 32]))
        .expect("fill action");
    assert_eq!(action.payment_output_index, 0x0403_0201);
    assert_eq!(action.buyer_lock_hash, [7; 32]);
}

#[test]
fn parse_fill_action_rejects_old_41_and_45_byte_payloads() {
    assert_eq!(
        parse_fill_order_action(&legacy_41_byte_fill_action_data()),
        Err(Error::InvalidActionData)
    );
    assert_eq!(
        parse_fill_order_action(&legacy_45_byte_fill_action_data()),
        Err(Error::InvalidActionData)
    );
}

#[test]
fn validate_fill_uses_order_requested_amount() {
    assert_eq!(
        validate_fill(
            &order(30),
            payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 30)
        ),
        Ok(())
    );
}
```

Keep existing wrong-owner, wrong-asset, and insufficient-payment tests, but update them to call `validate_fill(&order, payment)` without an action argument.

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-lock --offline
```

Expected: FAIL with missing `requested_amount`, stale 45-byte Fill parser, stale action fields, and stale `validate_fill` signature.

- [ ] **Step 3: Implement minimal lock pure changes**

In `tests/contracts/limit-order-lock/src/types.rs`:

- Keep `ORDER_ARGS_LEN = 104`.
- Change `FILL_ORDER_DATA_LEN` to `37`.
- Rename `OrderArgs::min_requested_amount` to `requested_amount`.
- Change `FillOrderAction` to:

```rust
pub struct FillOrderAction {
    pub payment_output_index: u32,
    pub buyer_lock_hash: [u8; 32],
}
```

- Parse order amount from offset `96` into `requested_amount`.
- Parse Fill as `payment_output_index` at offset `1` and `buyer_lock_hash` at offset `5`.
- Replace `validate_fill(order, action, payment)` with `validate_fill(order, payment)` and compare payment amount to `order.requested_amount`.
- Remove action requested-asset and action amount checks.
- Update helper/test names from `min_requested_amount` to `requested_amount`.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p limit-order-lock --offline
```

Expected: PASS for `limit-order-lock` unit tests.

- [ ] **Step 5: Review and commit**

Run:

```bash
git diff --check
git diff -- tests/contracts/limit-order-lock/src/types.rs
```

Expected: no whitespace errors; diff limited to pure lock ABI/validation changes.

Commit:

```bash
git add tests/contracts/limit-order-lock/src/types.rs docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md
git commit -m "test: shrink lock fill action abi"
```

## Task 3: Add NFT Delivery Scanning To `limit-order-type` Entry

**Files:**
- Modify: `tests/contracts/limit-order-type/src/entry.rs`
- Modify: `docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md`

**Red/Green Record:**
Red: `cargo test -p limit-order-type --offline` -> FAIL as expected: `error[E0425]: cannot find function nft_delivery_matches in this scope` for the three new entry predicate tests.
Green: `cargo test -p limit-order-type --offline` -> PASS: 36 passed, 0 failed; includes NFT delivery predicate tests and existing payment/output-index entry coverage.
Review: `git diff --check` -> PASS; diff reviewed and limited to `limit-order-type` entry NFT delivery scan plus this Task 3 record.
Commit: `40e4999` feat: require type order nft delivery

- [ ] **Step 1: Write failing entry unit tests**

In `tests/contracts/limit-order-type/src/entry.rs`:

- Update local test `fill_data(payment_output_index)` helper to emit 37 bytes:

```rust
fn fill_data(payment_output_index: u32) -> Vec<u8> {
    let mut data = Vec::new();
    data.push(crate::types::FILL_ORDER_TAG);
    data.extend_from_slice(&payment_output_index.to_le_bytes());
    data.extend_from_slice(&[9; 32]);
    data
}
```

- Add pure helper tests for an output-range scanner that can be unit-tested without syscalls by factoring the predicate:

```rust
#[test]
fn nft_delivery_match_accepts_buyer_lock_and_offered_nft_type() {
    assert!(nft_delivery_matches([7; 32], Some([8; 32]), [7; 32], [8; 32]));
}

#[test]
fn nft_delivery_match_rejects_wrong_buyer_lock() {
    assert!(!nft_delivery_matches([6; 32], Some([8; 32]), [7; 32], [8; 32]));
}

#[test]
fn nft_delivery_match_rejects_wrong_or_missing_nft_type() {
    assert!(!nft_delivery_matches([7; 32], Some([9; 32]), [7; 32], [8; 32]));
    assert!(!nft_delivery_matches([7; 32], None, [7; 32], [8; 32]));
}
```

- Update duplicate-payment tests so all test Fill data uses the new 37-byte ABI.

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-type --offline
```

Expected: FAIL because `nft_delivery_matches` does not exist, `validate_fill` call sites still pass an action, and entry duplicate parsing still expects 45-byte Fill data.

- [ ] **Step 3: Implement minimal entry changes**

In `tests/contracts/limit-order-type/src/entry.rs`:

- Change local `FILL_ORDER_DATA_LEN` to `37`.
- Update imports if `FillOrderAction` fields changed.
- Change `validate_fill(&order, &action, payment)` to `validate_fill(&order, payment)`.
- After payment validation, call:

```rust
if !has_nft_delivery_output(layout, action.buyer_lock_hash, order.offered_nft_type_hash)? {
    return Err(Error::InvalidCobuild);
}
```

- Add:

```rust
fn has_nft_delivery_output(
    layout: OtxMessageLayout,
    buyer_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<bool, Error> {
    for range in [layout.base_outputs, layout.append_outputs] {
        let end = range
            .start
            .checked_add(range.count)
            .ok_or(Error::InvalidCobuild)?;
        for index in range.start..end {
            let lock_hash = load_cell_lock_hash(index, Source::Output)?;
            let type_hash = load_cell_type_hash(index, Source::Output)?;
            if nft_delivery_matches(lock_hash, type_hash, buyer_lock_hash, offered_nft_type_hash) {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

fn nft_delivery_matches(
    lock_hash: [u8; 32],
    type_hash: Option<[u8; 32]>,
    buyer_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> bool {
    lock_hash == buyer_lock_hash && type_hash == Some(offered_nft_type_hash)
}
```

- Ensure `limit_order_target_hashes` fail-closed length check uses `37`.
- Ensure duplicate payment parsing still ignores unrelated action targets and rejects malformed in-scope tag `2`.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p limit-order-type --offline
```

Expected: PASS for `limit-order-type` unit tests.

- [ ] **Step 5: Review and commit**

Run:

```bash
git diff --check
git diff -- tests/contracts/limit-order-type/src/entry.rs
```

Expected: no whitespace errors; diff limited to type entry validation and unit tests.

Commit:

```bash
git add tests/contracts/limit-order-type/src/entry.rs docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md
git commit -m "feat: require type order nft delivery"
```

## Task 4: Add NFT Delivery Scanning To `limit-order-lock` Entry

**Files:**
- Modify: `tests/contracts/limit-order-lock/src/entry.rs`
- Modify: `docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md`

**Red/Green Record:**
Red: `cargo test -p limit-order-lock --offline` failed with 4x E0425 missing `nft_delivery_matches` in `entry.rs` predicate tests.
Green: `cargo test -p limit-order-lock --offline` passed; 25 unit tests, 0 failures; doc-tests 0 tests.
Review: `git diff --check` passed with no output; reviewed diff for `tests/contracts/limit-order-lock/src/entry.rs` and this plan record.
Commit: `d16e2b1` feat: require lock order nft delivery

- [ ] **Step 1: Write failing entry unit tests**

In `tests/contracts/limit-order-lock/src/entry.rs`:

- Update local `fill_data(payment_output_index)` helper to emit 37 bytes:

```rust
fn fill_data(payment_output_index: u32) -> Vec<u8> {
    let mut data = Vec::new();
    data.push(crate::types::FILL_ORDER_TAG);
    data.extend_from_slice(&payment_output_index.to_le_bytes());
    data.extend_from_slice(&[9; 32]);
    data
}
```

- Add the same `nft_delivery_matches` tests as Task 3, using lock entry's local helper.
- Update duplicate-payment tests to use 37-byte Fill data.

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-lock --offline
```

Expected: FAIL because `nft_delivery_matches` does not exist, `validate_fill` call sites still pass an action, and duplicate parsing still assumes the old Fill shape.

- [ ] **Step 3: Implement minimal lock entry changes**

In `tests/contracts/limit-order-lock/src/entry.rs`:

- Change `validate_fill(&order, &action, payment)` to `validate_fill(&order, payment)`.
- After payment validation, require `has_nft_delivery_output(layout, action.buyer_lock_hash, order.offered_nft_type_hash)?`.
- Add the same `has_nft_delivery_output` and `nft_delivery_matches` helpers as Task 3.
- Ensure duplicate payment scan parses the new 37-byte Fill action and still covers mixed `InputType`, `OutputType`, and `InputLock` target actions.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p limit-order-lock --offline
```

Expected: PASS for `limit-order-lock` unit tests.

- [ ] **Step 5: Review and commit**

Run:

```bash
git diff --check
git diff -- tests/contracts/limit-order-lock/src/entry.rs
```

Expected: no whitespace errors; diff limited to lock entry validation and unit tests.

Commit:

```bash
git add tests/contracts/limit-order-lock/src/entry.rs docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md
git commit -m "feat: require lock order nft delivery"
```

## Task 5: Update Shared Fixtures To New Names And Fill ABI

**Files:**
- Modify: `tests/src/fixtures/limit_order.rs`
- Modify: `tests/src/fixtures/limit_order/type_nft_for_udt.rs`
- Modify: `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`
- Modify: `docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md`

**Red/Green Record:**
Red: `git status --short` -> clean; `cargo test -p tests --lib --offline` -> PASS unexpectedly: 24 library tests passed because stale scenario modules are gated behind `#[cfg(not(test))]`.
Green: `cargo test -p tests --lib --offline` -> PASS after fixture migration and minimal `tests/src/framework/mod.rs` unit-test sync: 24 passed, 0 failed.
Review: `git diff --check` -> PASS; diff reviewed and limited to fixture ABI/name migration plus framework unit-test sync.
Commit: pending

- [ ] **Step 1: Write failing fixture compilation target**

Do not add implementation yet. First run:

```bash
cargo test -p tests --lib --offline
```

Expected: FAIL with compile errors from stale fixture calls to:

- `min_requested_amount`
- old `limit_order_fill(requested_asset_id, amount, payment_output_index)`
- old local `fill_action_data(requested_asset_id, amount, payment_output_index)`
- old action cases for requested asset mismatch and action amount below required

- [ ] **Step 2: Implement fixture ABI migration**

In `tests/src/fixtures/limit_order.rs`:

- Rename `LimitOrderState::min_requested_amount` to `requested_amount`.
- Rename builder storage field to `requested_amount`.
- Rename builder method to `requested_amount`.
- Update `order_data` and `create_order_action_data` to write `requested_amount`.
- Change `LimitOrderCobuildMessageExt::limit_order_fill` to:

```rust
fn limit_order_fill(self, payment_output_index: u32, buyer_lock_hash: [u8; 32]) -> Self;
```

- Encode Fill action as 37 bytes:

```rust
let mut data = Vec::with_capacity(37);
data.push(FILL_ORDER_TAG);
data.extend_from_slice(&payment_output_index.to_le_bytes());
data.extend_from_slice(&buyer_lock_hash);
self.action_data(data)
```

In `tests/src/fixtures/limit_order/type_nft_for_udt.rs` and `lock_nft_for_udt.rs`:

- Rename order state and helper fields to `requested_amount`.
- Replace builder calls `.min_requested_amount(30)` with `.requested_amount(30)`.
- Remove action cases that only mutate requested asset or action amount:
  - `RequestedAssetMismatch`
  - `MinRequestedBelowRequired`
- Keep payment negative cases by mutating the actual payment output.
- Update all Fill action builders to pass `payment_output_index` and `script_hash(&buyer_lock)`.
- Update local malformed action builders to truncate the new 37-byte data.
- Update unknown-tag builders to use 37-byte length.
- Ensure all happy-path fixtures still include buyer NFT outputs in the current OTX settlement range.

- [ ] **Step 3: Run green**

Run:

```bash
cargo test -p tests --lib --offline
```

Expected: PASS for the `tests` crate library compilation/tests.

- [ ] **Step 4: Review and commit**

Run:

```bash
git diff --check
git diff -- tests/src/fixtures/limit_order.rs tests/src/fixtures/limit_order/type_nft_for_udt.rs tests/src/fixtures/limit_order/lock_nft_for_udt.rs
```

Expected: no whitespace errors; diff limited to fixture ABI/name migration.

Commit:

```bash
git add tests/src/fixtures/limit_order.rs tests/src/fixtures/limit_order/type_nft_for_udt.rs tests/src/fixtures/limit_order/lock_nft_for_udt.rs docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md
git commit -m "test: migrate limit order fixtures to buyer delivery action"
```

## Task 6: Add `limit-order-type` NFT Delivery Integration Coverage

**Files:**
- Modify: `tests/src/fixtures/limit_order/type_nft_for_udt.rs`
- Modify: `tests/tests/limit_order_type.rs`
- Modify: `docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md`

**Red/Green Record:**
Red: pending
Green: pending
Review: pending
Commit: pending

- [ ] **Step 1: Write failing integration tests**

In `tests/src/fixtures/limit_order/type_nft_for_udt.rs`, add fill action cases:

```rust
MissingBuyerNftOutput,
BuyerNftWrongLock,
BuyerNftWrongType,
```

Wire those cases so:

- `MissingBuyerNftOutput`: omit the NFT output from OTX outputs.
- `BuyerNftWrongLock`: output the correct NFT type to a wrong lock.
- `BuyerNftWrongType`: output a different NFT type to the buyer lock.

In `tests/tests/limit_order_type.rs`, add tests:

```rust
#[test]
fn fill_nft_order_rejects_missing_buyer_nft_output() {
    let (mut fixture, tx) =
        limit_order_nft_for_udt_case_with(FillActionCase::MissingBuyerNftOutput);
    assert_script_error(&mut fixture, tx, "limit-order-type", Error::InvalidCobuild);
}

#[test]
fn fill_nft_order_rejects_buyer_nft_output_with_wrong_lock() {
    let (mut fixture, tx) =
        limit_order_nft_for_udt_case_with(FillActionCase::BuyerNftWrongLock);
    assert_script_error(&mut fixture, tx, "limit-order-type", Error::InvalidCobuild);
}

#[test]
fn fill_nft_order_rejects_buyer_nft_output_with_wrong_type() {
    let (mut fixture, tx) =
        limit_order_nft_for_udt_case_with(FillActionCase::BuyerNftWrongType);
    assert_script_error(&mut fixture, tx, "limit-order-type", Error::InvalidCobuild);
}
```

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p tests --test limit_order_type --offline
```

Expected: FAIL because the new fixture cases and/or entry validation are not fully wired into integration behavior.

- [ ] **Step 3: Implement minimal type fixture behavior**

Complete the fixture wiring from Step 1:

- Ensure `MissingBuyerNftOutput` still leaves payment output indexes stable.
- Ensure wrong NFT type uses a deployed `test-nft` script that is present in cell deps.
- Ensure expected errors match `InvalidCobuild`.
- Keep existing payment binding tests unchanged except for ABI/name updates.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p tests --test limit_order_type --offline
```

Expected: PASS for `limit_order_type` integration tests.

- [ ] **Step 5: Review and commit**

Run:

```bash
git diff --check
git diff -- tests/src/fixtures/limit_order/type_nft_for_udt.rs tests/tests/limit_order_type.rs
```

Expected: no whitespace errors; diff limited to type integration fixture/tests.

Commit:

```bash
git add tests/src/fixtures/limit_order/type_nft_for_udt.rs tests/tests/limit_order_type.rs docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md
git commit -m "test: cover type order nft delivery"
```

## Task 7: Add `limit-order-lock` And Mixed NFT Delivery Integration Coverage

**Files:**
- Modify: `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`
- Modify: `tests/tests/limit_order_lock.rs`
- Modify: `docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md`

**Red/Green Record:**
Red: pending
Green: pending
Review: pending
Commit: pending

- [ ] **Step 1: Write failing integration tests**

In `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`, add cases:

```rust
MissingBuyerNftOutput,
BuyerNftWrongLock,
BuyerNftWrongType,
```

Wire those cases with the same semantics as Task 6, but for the lock-order fixture.

In `tests/tests/limit_order_lock.rs`, add tests:

```rust
#[test]
fn fill_lock_order_rejects_missing_buyer_nft_output() {
    let (mut fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::MissingBuyerNftOutput);
    assert_script_error(&mut fixture, tx, "limit-order-lock", Error::InvalidCobuild);
}

#[test]
fn fill_lock_order_rejects_buyer_nft_output_with_wrong_lock() {
    let (mut fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::BuyerNftWrongLock);
    assert_script_error(&mut fixture, tx, "limit-order-lock", Error::InvalidCobuild);
}

#[test]
fn fill_lock_order_rejects_buyer_nft_output_with_wrong_type() {
    let (mut fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::BuyerNftWrongType);
    assert_script_error(&mut fixture, tx, "limit-order-lock", Error::InvalidCobuild);
}
```

Also keep or add mixed type+lock tests proving duplicate payment index still rejects with the new 37-byte action.

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p tests --test limit_order_lock --offline
```

Expected: FAIL because the new fixture cases and/or entry validation are not fully wired into integration behavior.

- [ ] **Step 3: Implement minimal lock and mixed fixture behavior**

Complete fixture wiring:

- Omit or mutate NFT delivery outputs for the three negative cases.
- Keep lock input validation unchanged.
- Keep two-lock-order and mixed type+lock duplicate payment scenarios using same `payment_output_index` rejection.
- Ensure distinct-payment multi-order scenarios include valid NFT outputs for each offered NFT.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p tests --test limit_order_lock --offline
```

Expected: PASS for `limit_order_lock` integration tests.

- [ ] **Step 5: Review and commit**

Run:

```bash
git diff --check
git diff -- tests/src/fixtures/limit_order/lock_nft_for_udt.rs tests/tests/limit_order_lock.rs
```

Expected: no whitespace errors; diff limited to lock/mixed integration fixture/tests.

Commit:

```bash
git add tests/src/fixtures/limit_order/lock_nft_for_udt.rs tests/tests/limit_order_lock.rs docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md
git commit -m "test: cover lock order nft delivery"
```

## Task 8: Final Verification And Documentation Record

**Files:**
- Modify: `docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md`

**Red/Green Record:**
Red: not applicable
Green: pending
Review: pending
Commit: pending

- [ ] **Step 1: Run formatting**

Run:

```bash
cargo fmt
```

Expected: completes successfully.

- [ ] **Step 2: Build tests-only contracts**

Run:

```bash
make -e -C tests/contracts/limit-order-type build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/limit-order-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-udt build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-nft build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
```

Expected: all four contract builds complete successfully.

- [ ] **Step 3: Run focused tests**

Run:

```bash
cargo test -p limit-order-type --offline
cargo test -p limit-order-lock --offline
cargo test -p cobuild-core --lib --offline
cargo test -p tests --test limit_order_type --offline
cargo test -p tests --test limit_order_lock --offline
cargo test -p tests --lib --offline
```

Expected: all commands pass.

- [ ] **Step 4: Run full workspace verification**

Run:

```bash
cargo test --workspace --offline
cargo clippy --workspace --offline --all-targets
cargo fmt --check
git diff --check
git status --short
find tests/failed_txs -maxdepth 1 -type f 2>/dev/null | wc -l
git status --short --ignored tests/failed_txs
```

Expected:

- workspace tests pass;
- clippy passes with no warnings requiring code changes;
- formatting and diff checks pass;
- `git status --short` shows only this plan file until committed;
- failed tx count is recorded;
- no new tracked files under `tests/failed_txs`.

- [ ] **Step 5: Record final verification and commit**

Replace this task's Red/Green Record with exact command summaries and failed-tx status.

Commit:

```bash
git add docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md
git commit -m "docs: record limit order nft delivery verification"
```

## Final Delivery Notes

Final response must include:

- Spec path:
  `docs/superpowers/specs/2026-06-09-limit-order-nft-delivery-binding-design.md`
- Plan path:
  `docs/superpowers/plans/2026-06-09-limit-order-nft-delivery-binding-plan.md`
- Final FillOrder ABI:
  `tag u8 = 2`, `payment_output_index u32`, `buyer_lock_hash [u8; 32]`, total 37 bytes.
- Order amount rename:
  `min_requested_amount` -> `requested_amount`.
- NFT delivery rule:
  scan current OTX base/append outputs for `buyer_lock_hash + offered_nft_type_hash`; no NFT output index or NFT output index uniqueness check.
- Payment duplicate scope:
  same-OTX type+lock mixed Fill actions remain covered.
- Red/green/verification command results for every task.
- Whether `tests/failed_txs` gained any tracked files.
- All new commit hashes.
