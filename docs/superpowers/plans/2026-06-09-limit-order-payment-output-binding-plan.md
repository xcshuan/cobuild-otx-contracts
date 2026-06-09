# Limit Order Payment Output Binding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make tests-only `limit-order-type` and `limit-order-lock` Fill actions bind to one explicit UDT payment output and reject same-OTX payment output reuse.

**Architecture:** Change both local FillOrder ABIs from 41 to 45 bytes by adding `payment_output_index: u32`, then replace scan-and-sum settlement validation with exact output validation. Use `CobuildContext::otx_actions(otx_index)` to inspect same-OTX actions and reject duplicate payment indexes across type and lock limit-order Fill actions without touching production schemas.

**Tech Stack:** Rust 2024, `ckb-std`, `cobuild-core`, tests-only CKB contracts, `ckb-testtool`, offline Cargo, contract Makefiles.

---

## Source Requirements

Read these before executing:

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
  - New 45-byte Fill ABI, `payment_output_index`, single-payment validation helpers, duplicate-index pure helper tests.
- `tests/contracts/limit-order-type/src/entry.rs`
  - Retain `CobuildContext`, extract `otx_index`, verify output index range, call `otx_actions`, load exact UDT payment output.
- `tests/contracts/limit-order-lock/src/types.rs`
  - Same 45-byte Fill ABI and pure helper behavior as the type contract.
- `tests/contracts/limit-order-lock/src/entry.rs`
  - Same exact-payment binding flow for lock orders.
- `tests/src/fixtures/limit_order.rs`
  - Update `limit_order_fill` builder to include `payment_output_index`.
- `tests/src/fixtures/limit_order/type_nft_for_udt.rs`
  - Update existing scenarios and add type multi-order scenarios.
- `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`
  - Update existing scenarios and add lock multi-order and mixed type+lock scenarios.
- `tests/src/framework/cobuild.rs`
  - Add multi-action message builder support for duplicate tests.
- `tests/tests/limit_order_type.rs`
  - Add/adjust integration assertions for bound payment indexes and duplicate type fills.
- `tests/tests/limit_order_lock.rs`
  - Add/adjust integration assertions for bound payment indexes, duplicate lock fills, mixed type+lock duplicate fills.
- `docs/superpowers/plans/2026-06-09-limit-order-payment-output-binding-plan.md`
  - Record Red/Green command results per task before each task commit.

Do not modify:

- `contracts/cobuild-otx-lock`
- `crates/cobuild-types`
- public production schemas
- production order-book behavior

## Red/Green Log Discipline

Each task has a **Red/Green Record** section. During execution, replace the instruction text with exact command summaries before committing:

```text
Red: <command> -> FAIL as expected: <specific failing test or compiler error>
Green: <command> -> PASS: <specific passing test count or command result>
Review: <brief diff review result>
Commit: <hash> <subject>
```

If any command fails unexpectedly, use `superpowers:systematic-debugging` before changing code.

## Task 1: Update `limit-order-type` Pure ABI And Payment Helpers

**Files:**
- Modify: `tests/contracts/limit-order-type/src/types.rs`
- Modify: `docs/superpowers/plans/2026-06-09-limit-order-payment-output-binding-plan.md`

**Red/Green Record:**
Red: `cargo test -p limit-order-type --offline` -> FAIL as expected: `FillOrderAction` missing `payment_output_index` and `validate_fill` expected `&[SettlementCell]` but tests passed `SettlementCell`.
Green: `cargo test -p limit-order-type --offline` -> PASS: 30 unit tests passed, 0 failed; main/doc tests 0 passed, 0 failed.
Review: `git diff --check` -> PASS with no output; diff limited to Task 1 owned files and keeps `entry.rs` compiling until Task 2 binds explicit output loading.
Commit: `6bcbbee` `test: bind type fill action to payment index`.

- [ ] **Step 1: Write failing parser and validation tests**

In `tests/contracts/limit-order-type/src/types.rs`, update the test helper signature to:

```rust
fn fill_action_data(min_requested_amount: u64, payment_output_index: u32) -> Vec<u8> {
    let mut data = Vec::new();
    data.push(FILL_ORDER_TAG);
    data.extend_from_slice(&REQUESTED_ASSET_ID);
    data.extend_from_slice(&min_requested_amount.to_le_bytes());
    data.extend_from_slice(&payment_output_index.to_le_bytes());
    data
}
```

Add tests:

```rust
#[test]
fn parse_fill_order_action_reads_payment_output_index_little_endian() {
    let action = parse_limit_order_action(&fill_action_data(30, 0x0403_0201))
        .expect("fill action");

    assert_eq!(
        action,
        LimitOrderAction::Fill(FillOrderAction {
            requested_asset_id: REQUESTED_ASSET_ID,
            min_requested_amount: 30,
            payment_output_index: 0x0403_0201,
        })
    );
}

#[test]
fn parse_fill_order_action_rejects_legacy_41_byte_payload() {
    let mut data = Vec::new();
    data.push(FILL_ORDER_TAG);
    data.extend_from_slice(&REQUESTED_ASSET_ID);
    data.extend_from_slice(&30u64.to_le_bytes());

    assert_eq!(
        parse_limit_order_action(&data).unwrap_err(),
        Error::InvalidActionData
    );
}

#[test]
fn validate_fill_accepts_bound_payment() {
    let payment = settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 30);

    assert_eq!(
        validate_fill(&order_state(30), &fill_action(30, 1), payment),
        Ok(())
    );
}

#[test]
fn validate_fill_rejects_bound_payment_to_wrong_owner() {
    let payment = settlement([9; 32], REQUESTED_ASSET_ID, 30);

    assert_eq!(
        validate_fill(&order_state(30), &fill_action(30, 1), payment),
        Err(Error::InsufficientPayment)
    );
}

#[test]
fn validate_fill_rejects_bound_payment_with_wrong_asset_id() {
    let payment = settlement(OWNER_LOCK_HASH, [9; 32], 30);

    assert_eq!(
        validate_fill(&order_state(30), &fill_action(30, 1), payment),
        Err(Error::InsufficientPayment)
    );
}
```

Update existing tests that call `fill_action` to pass an index:

```rust
fn fill_action(min_requested_amount: u64, payment_output_index: u32) -> FillOrderAction {
    parse_fill_order_action(&fill_action_data(min_requested_amount, payment_output_index))
        .expect("fill action")
}
```

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-type --offline
```

Expected: FAIL with missing `payment_output_index` field and `validate_fill` signature/type mismatches.

- [ ] **Step 3: Implement the minimal type-contract pure changes**

In `tests/contracts/limit-order-type/src/types.rs`:

```rust
const FILL_ORDER_DATA_LEN: usize = 45;
```

Change `FillOrderAction`:

```rust
pub struct FillOrderAction {
    pub requested_asset_id: [u8; 32],
    pub min_requested_amount: u64,
    pub payment_output_index: u32,
}
```

Change parser:

```rust
Ok(FillOrderAction {
    requested_asset_id: read_bytes32(data, 1),
    min_requested_amount: read_u64(data, 33),
    payment_output_index: read_u32(data, 41),
})
```

Add:

```rust
fn read_u32(data: &[u8], offset: usize) -> u32 {
    let mut out = [0u8; 4];
    out.copy_from_slice(&data[offset..offset + 4]);
    u32::from_le_bytes(out)
}
```

Replace `validate_fill` with single-payment validation:

```rust
pub fn validate_fill(
    order: &OrderState,
    action: &FillOrderAction,
    payment: SettlementCell,
) -> Result<(), Error> {
    if action.requested_asset_id != order.requested_asset_id {
        return Err(Error::ActionMismatch);
    }
    if action.min_requested_amount < order.min_requested_amount {
        return Err(Error::InsufficientPayment);
    }
    if payment.owner_lock_hash != order.owner_lock_hash
        || payment.asset_id != order.requested_asset_id
        || payment.amount < action.min_requested_amount
    {
        return Err(Error::InsufficientPayment);
    }
    Ok(())
}
```

Remove overflow-sum tests because Fill no longer sums multiple payments.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p limit-order-type --offline
```

Expected: PASS for `limit-order-type` unit tests.

- [ ] **Step 5: Review and commit**

Run:

```bash
git diff -- tests/contracts/limit-order-type/src/types.rs
git diff --check
```

Record results in this plan, then:

```bash
git add tests/contracts/limit-order-type/src/types.rs docs/superpowers/plans/2026-06-09-limit-order-payment-output-binding-plan.md
git commit -m "test: bind type fill action to payment index"
```

## Task 2: Update `limit-order-lock` Pure ABI And Payment Helpers

**Files:**
- Modify: `tests/contracts/limit-order-lock/src/types.rs`
- Modify: `docs/superpowers/plans/2026-06-09-limit-order-payment-output-binding-plan.md`

**Red/Green Record:**
Red: `cargo test -p limit-order-lock --offline` -> FAIL as expected: `FillOrderAction` missing `payment_output_index` and `validate_fill` expected `&[UdtPayment]` but tests passed `UdtPayment`.
Green: `cargo test -p limit-order-lock --offline` -> PASS: 18 unit tests passed, 0 failed; main/doc tests 0 passed, 0 failed.
Review: `git diff --check` -> PASS with no output; diff limited to Task 2 owned files. Used the same temporary `BoundPayment` compatibility adapter as Task 1 so unchanged `entry.rs` can keep passing `&Vec<UdtPayment>` until later entry-binding tasks remove old collection flow.
Commit: `6bfa4c0` `test: bind lock fill action to payment index`.

- [ ] **Step 1: Write failing parser and validation tests**

In `tests/contracts/limit-order-lock/src/types.rs`, update `fill_action_data`:

```rust
fn fill_action_data(
    asset_id: [u8; 32],
    min_requested_amount: u64,
    payment_output_index: u32,
) -> Vec<u8> {
    let mut data = Vec::new();
    data.push(FILL_ORDER_TAG);
    data.extend_from_slice(&asset_id);
    data.extend_from_slice(&min_requested_amount.to_le_bytes());
    data.extend_from_slice(&payment_output_index.to_le_bytes());
    data
}
```

Add tests:

```rust
#[test]
fn parse_fill_action_reads_payment_output_index_little_endian() {
    let action = parse_fill_order_action(&fill_action_data(
        REQUESTED_ASSET_ID,
        30,
        0x0403_0201,
    ))
    .expect("fill action");

    assert_eq!(action.requested_asset_id, REQUESTED_ASSET_ID);
    assert_eq!(action.min_requested_amount, 30);
    assert_eq!(action.payment_output_index, 0x0403_0201);
}

#[test]
fn parse_fill_action_rejects_legacy_41_byte_payload() {
    let mut data = Vec::new();
    data.push(FILL_ORDER_TAG);
    data.extend_from_slice(&REQUESTED_ASSET_ID);
    data.extend_from_slice(&30u64.to_le_bytes());

    assert_eq!(
        parse_fill_order_action(&data),
        Err(Error::InvalidActionData)
    );
}

#[test]
fn validate_fill_accepts_bound_payment() {
    assert_eq!(
        validate_fill(
            &order(30),
            &action(REQUESTED_ASSET_ID, 30, 1),
            payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 30),
        ),
        Ok(())
    );
}

#[test]
fn validate_fill_rejects_bound_payment_wrong_owner_or_asset() {
    assert_eq!(
        validate_fill(
            &order(30),
            &action(REQUESTED_ASSET_ID, 30, 1),
            payment([9; 32], REQUESTED_ASSET_ID, 30),
        ),
        Err(Error::InsufficientPayment)
    );
    assert_eq!(
        validate_fill(
            &order(30),
            &action(REQUESTED_ASSET_ID, 30, 1),
            payment(OWNER_LOCK_HASH, [9; 32], 30),
        ),
        Err(Error::InsufficientPayment)
    );
}
```

Update helper:

```rust
fn action(
    asset_id: [u8; 32],
    min_requested_amount: u64,
    payment_output_index: u32,
) -> FillOrderAction {
    parse_fill_order_action(&fill_action_data(
        asset_id,
        min_requested_amount,
        payment_output_index,
    ))
    .expect("fill action")
}
```

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-lock --offline
```

Expected: FAIL with missing `payment_output_index` field and `validate_fill` signature/type mismatches.

- [ ] **Step 3: Implement the minimal lock-contract pure changes**

In `tests/contracts/limit-order-lock/src/types.rs`:

```rust
pub const FILL_ORDER_DATA_LEN: usize = 45;
```

Change `FillOrderAction`:

```rust
pub struct FillOrderAction {
    pub requested_asset_id: [u8; 32],
    pub min_requested_amount: u64,
    pub payment_output_index: u32,
}
```

Parse:

```rust
Ok(FillOrderAction {
    requested_asset_id: read_bytes32(data, 1),
    min_requested_amount: read_u64(data, 33),
    payment_output_index: read_u32(data, 41),
})
```

Add:

```rust
fn read_u32(data: &[u8], offset: usize) -> u32 {
    let mut out = [0u8; 4];
    out.copy_from_slice(&data[offset..offset + 4]);
    u32::from_le_bytes(out)
}
```

Replace `validate_fill`:

```rust
pub fn validate_fill(
    order: &OrderArgs,
    action: &FillOrderAction,
    payment: UdtPayment,
) -> Result<(), Error> {
    if action.requested_asset_id != order.requested_asset_id {
        return Err(Error::ActionMismatch);
    }
    if action.min_requested_amount < order.min_requested_amount {
        return Err(Error::InsufficientPayment);
    }
    if payment.owner_lock_hash != order.owner_lock_hash
        || payment.asset_id != order.requested_asset_id
        || payment.amount < action.min_requested_amount
    {
        return Err(Error::InsufficientPayment);
    }
    Ok(())
}
```

Remove overflow-sum tests because Fill no longer sums multiple payments.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p limit-order-lock --offline
```

Expected: PASS for `limit-order-lock` unit tests.

- [ ] **Step 5: Review and commit**

Run:

```bash
git diff -- tests/contracts/limit-order-lock/src/types.rs
git diff --check
```

Record results in this plan, then:

```bash
git add tests/contracts/limit-order-lock/src/types.rs docs/superpowers/plans/2026-06-09-limit-order-payment-output-binding-plan.md
git commit -m "test: bind lock fill action to payment index"
```

## Task 3: Update Fixture Action ABI And Existing Single-Order Scenarios

**Files:**
- Modify: `tests/src/fixtures/limit_order.rs`
- Modify: `tests/src/fixtures/limit_order/type_nft_for_udt.rs`
- Modify: `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`
- Modify: `tests/src/framework/cobuild.rs`
- Modify: `docs/superpowers/plans/2026-06-09-limit-order-payment-output-binding-plan.md`

**Red/Green Record:**
Red: `cargo test -p tests --test limit_order_type --offline` -> FAIL as expected at compile: `tests/src/fixtures/limit_order/type_nft_for_udt.rs:200:10` and `tests/src/fixtures/limit_order.rs:100:10` called `.limit_order_fill(asset, amount)` but the trait now requires `payment_output_index: u32`.
Green: `cargo test -p tests --test limit_order_type --offline` -> COMPILED, then runtime FAIL expected for later entry-binding tasks: 1 passed, 18 failed; failures include type script validation error 7 and expected-exit mismatches after fixtures emit 45-byte fill actions.
Green: `cargo test -p tests --test limit_order_lock --offline` -> COMPILED, then runtime FAIL expected for later entry-binding tasks: 7 passed, 8 failed; failures include lock script validation error 6 after fixtures emit 45-byte fill actions.
Green: `cargo test -p tests --lib --offline` -> PASS: 24 passed, 0 failed; warning only for unused `LimitOrderCobuildMessageExt` import in `tests/src/framework/mod.rs`.
Review: `git diff --check` -> PASS with no output; diff reviewed and limited to Task 3 owned files, preserving single-action message builder behavior while adding `push_action`. Follow-up review removed the two-argument compatibility `CobuildMessageBuilder::limit_order_fill` method so fixture fill calls require an explicit `payment_output_index`.
Commit: pending `test: update limit order fill fixtures`.

- [ ] **Step 1: Write failing fixture compile change**

Change the trait in `tests/src/fixtures/limit_order.rs`:

```rust
pub trait LimitOrderCobuildMessageExt {
    fn limit_order_create(self, order: LimitOrderState) -> Self;
    fn limit_order_fill(
        self,
        requested_asset_id: [u8; 32],
        min_requested_amount: u64,
        payment_output_index: u32,
    ) -> Self;
}
```

Do not update call sites yet.

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p tests --test limit_order_type --offline
```

Expected: FAIL to compile because existing `.limit_order_fill(asset, amount)` call sites now need `payment_output_index`.

- [ ] **Step 3: Implement fixture ABI update**

Update `limit_order_fill` in `tests/src/fixtures/limit_order.rs`:

```rust
fn limit_order_fill(
    self,
    requested_asset_id: [u8; 32],
    min_requested_amount: u64,
    payment_output_index: u32,
) -> Self {
    let mut data = Vec::with_capacity(45);
    data.push(FILL_ORDER_TAG);
    data.extend_from_slice(&requested_asset_id);
    data.extend_from_slice(&min_requested_amount.to_le_bytes());
    data.extend_from_slice(&payment_output_index.to_le_bytes());
    self.action_data(data)
}
```

Update single-order type fixture calls:

```rust
.limit_order_fill(REQUESTED_ASSET_ID, 30, 0)
```

for `limit_order_case`, where the only OTX append settlement output is absolute output index `0`.

Update NFT-for-UDT type fixture calls:

```rust
.limit_order_fill(action_requested_asset, action_requested_amount, payment_output_index)
```

where:

```rust
let payment_output_index = match scenario.action_case {
    Some(FillActionCase::PaymentInAnotherOtx) => 2,
    _ => 1,
};
```

Update lock fixture `fill_action_data`:

```rust
fn fill_action_data(
    requested_asset_id: [u8; 32],
    amount: u64,
    payment_output_index: u32,
) -> Vec<u8> {
    let mut data = Vec::with_capacity(45);
    data.push(FILL_ORDER_TAG);
    data.extend_from_slice(&requested_asset_id);
    data.extend_from_slice(&amount.to_le_bytes());
    data.extend_from_slice(&payment_output_index.to_le_bytes());
    data
}
```

Use:

```rust
let payment_output_index = if case == LimitOrderLockFillCase::PaymentInAnotherOtx {
    2
} else {
    1
};
```

Update malformed action construction by popping from the new 45-byte payload.

Add multi-action support to `tests/src/framework/cobuild.rs` without changing existing single-action users:

```rust
#[derive(Clone, Debug)]
pub struct CobuildActionSpec {
    pub script_hash: [u8; 32],
    pub script_role: u8,
    pub action_data: Vec<u8>,
}
```

Add to `CobuildMessageBuilder`:

```rust
actions: Vec<CobuildActionSpec>,
```

Update `action_data` to push one action when building, and add:

```rust
pub fn push_action(mut self, script_role: u8, script_hash: [u8; 32], action_data: Vec<u8>) -> Self {
    self.actions.push(CobuildActionSpec {
        script_hash,
        script_role,
        action_data,
    });
    self
}
```

In `build`, if `self.actions` is empty, build the existing single action; otherwise build every action in `self.actions`.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p tests --test limit_order_type --offline
cargo test -p tests --test limit_order_lock --offline
```

Expected at this stage: tests may fail at runtime because entries still scan old payment data, but compilation should pass. If runtime failures occur due to old entry behavior and new ABI, record them as expected red for later tasks; run `cargo test -p tests --lib --offline` and require PASS for fixture library compilation.

- [ ] **Step 5: Review and commit**

Run:

```bash
git diff -- tests/src/fixtures/limit_order.rs tests/src/fixtures/limit_order/type_nft_for_udt.rs tests/src/fixtures/limit_order/lock_nft_for_udt.rs tests/src/framework/cobuild.rs
git diff --check
```

Record results in this plan, then:

```bash
git add tests/src/fixtures/limit_order.rs tests/src/fixtures/limit_order/type_nft_for_udt.rs tests/src/fixtures/limit_order/lock_nft_for_udt.rs tests/src/framework/cobuild.rs docs/superpowers/plans/2026-06-09-limit-order-payment-output-binding-plan.md
git commit -m "test: update limit order fill fixtures"
```

## Task 4: Bind `limit-order-type` Fill To Exact Payment Output

**Files:**
- Modify: `tests/contracts/limit-order-type/src/entry.rs`
- Modify: `tests/contracts/limit-order-type/src/types.rs`
- Modify: `tests/src/fixtures/limit_order/type_nft_for_udt.rs`
- Modify: `tests/tests/limit_order_type.rs`
- Modify: `docs/superpowers/plans/2026-06-09-limit-order-payment-output-binding-plan.md`

**Red/Green Record:** Execution agent must replace this line with exact Red, Green, Review, and Commit results.

- [ ] **Step 1: Write failing integration and range tests**

Add enum cases in `tests/src/fixtures/limit_order/type_nft_for_udt.rs`:

```rust
PaymentOutputOutOfRange,
PaymentOutputWrongUdt,
PaymentOutputWrongOwner,
PaymentOutputInsufficient,
```

Map them so the action points to:

- out of range: tx-level remainder output absolute index `2`;
- wrong UDT: append output index `1` with wrong UDT;
- wrong owner: append output index `1` with wrong owner;
- insufficient: append output index `1` with amount `29`.

Add tests in `tests/tests/limit_order_type.rs`:

```rust
#[test]
fn limit_order_type_rejects_payment_output_outside_current_otx() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::PaymentOutputOutOfRange);

    fixture.assert_type_script_exit(&tx, 0, 12);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}
```

Add these payment failure tests in `tests/tests/limit_order_type.rs`:

```rust
#[test]
fn limit_order_type_rejects_bound_payment_output_wrong_udt() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::PaymentOutputWrongUdt);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_bound_payment_output_wrong_owner() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::PaymentOutputWrongOwner);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_bound_payment_output_insufficient() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::PaymentOutputInsufficient);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}
```

In `tests/contracts/limit-order-type/src/entry.rs` unit tests, add:

```rust
#[test]
fn output_index_in_otx_outputs_accepts_base_and_append_outputs() {
    let layout = layout();

    assert_eq!(output_index_in_otx_outputs(layout, 0), Ok(true));
    assert_eq!(output_index_in_otx_outputs(layout, 1), Ok(true));
}

#[test]
fn output_index_in_otx_outputs_rejects_out_of_range_output() {
    assert_eq!(output_index_in_otx_outputs(layout(), 2), Ok(false));
}
```

Adjust test `layout()` so `base_outputs: Range { start: 0, count: 1 }` and `append_outputs: Range { start: 1, count: 1 }`.

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-type --offline
cargo test -p tests --test limit_order_type --offline
```

Expected: FAIL because `output_index_in_otx_outputs` does not exist and entry still scans settlements instead of validating the action-selected output.

- [ ] **Step 3: Implement exact payment binding for type entry**

In `tests/contracts/limit-order-type/src/entry.rs`, keep context:

```rust
let context = CobuildContext::build(CurrentScript::Type(current_type_hash))?;
let plan = context.plan_type_validation()?;
```

Change fill call:

```rust
OrderMode::Fill => validate_fill_entry(&context, &plan),
```

Change signature:

```rust
fn validate_fill_entry(
    context: &CobuildContext,
    plan: &TypeValidationPlan,
) -> Result<(), Error>
```

Extract `otx_index` with layout:

```rust
let (otx_index, layout) = otx_fill_layout(
    &related.action.origin,
    related.otx_type_scope.in_otx_scope(),
)?;
```

Change `otx_fill_layout` return type to `Result<(usize, OtxMessageLayout), Error>` and pattern:

```rust
let ActionOrigin::Otx {
    otx_index, layout, ..
} = origin
else {
    return Err(Error::InvalidCobuild);
};
```

Add:

```rust
fn output_index_in_otx_outputs(
    layout: OtxMessageLayout,
    output_index: usize,
) -> Result<bool, Error> {
    Ok(range_contains(layout.base_outputs, output_index)?
        || range_contains(layout.append_outputs, output_index)?)
}

fn range_contains(range: Range, index: usize) -> Result<bool, Error> {
    let end = range
        .start
        .checked_add(range.count)
        .ok_or(Error::InvalidCobuild)?;
    Ok(index >= range.start && index < end)
}
```

After parsing action:

```rust
let payment_output_index = action.payment_output_index as usize;
if !output_index_in_otx_outputs(layout, payment_output_index)? {
    return Err(Error::InvalidCobuild);
}
let payment = load_udt_payment_output(payment_output_index)?;
validate_fill(&order, &action, payment)
```

Add:

```rust
fn load_udt_payment_output(index: usize) -> Result<SettlementCell, Error> {
    let data = load_cell_data(index, Source::Output)?;
    let lock_hash = load_cell_lock_hash(index, Source::Output)?;
    let Some(type_hash) = load_cell_type_hash(index, Source::Output)? else {
        return Err(Error::InsufficientPayment);
    };
    parse_udt_payment(lock_hash, type_hash, &data)
}
```

Remove `collect_settlements` and `collect_settlements_from_range` once unused.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p limit-order-type --offline
make -e -C tests/contracts/limit-order-type build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
cargo test -p tests --test limit_order_type --offline
```

Expected: PASS for type unit tests, type contract build, and type integration tests except duplicate tests not added yet.

- [ ] **Step 5: Review and commit**

Run:

```bash
git diff -- tests/contracts/limit-order-type/src/entry.rs tests/contracts/limit-order-type/src/types.rs tests/src/fixtures/limit_order/type_nft_for_udt.rs tests/tests/limit_order_type.rs
git diff --check
```

Record results in this plan, then:

```bash
git add tests/contracts/limit-order-type/src/entry.rs tests/contracts/limit-order-type/src/types.rs tests/src/fixtures/limit_order/type_nft_for_udt.rs tests/tests/limit_order_type.rs docs/superpowers/plans/2026-06-09-limit-order-payment-output-binding-plan.md
git commit -m "fix: bind type orders to payment output"
```

## Task 5: Bind `limit-order-lock` Fill To Exact Payment Output

**Files:**
- Modify: `tests/contracts/limit-order-lock/src/entry.rs`
- Modify: `tests/contracts/limit-order-lock/src/types.rs`
- Modify: `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`
- Modify: `tests/tests/limit_order_lock.rs`
- Modify: `docs/superpowers/plans/2026-06-09-limit-order-payment-output-binding-plan.md`

**Red/Green Record:** Execution agent must replace this line with exact Red, Green, Review, and Commit results.

- [ ] **Step 1: Write failing integration and range tests**

Add enum cases in `LimitOrderLockFillCase`:

```rust
PaymentOutputOutOfRange,
PaymentOutputWrongUdt,
PaymentOutputWrongOwner,
PaymentOutputInsufficient,
```

Map them like the type fixture:

- out of range: tx-level remainder output absolute index `2`;
- wrong UDT: append output index `1` with wrong UDT;
- wrong owner: append output index `1` with wrong owner;
- insufficient: append output index `1` with amount `29`.

Add lock tests:

```rust
#[test]
fn limit_order_lock_rejects_payment_output_outside_current_otx() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::PaymentOutputOutOfRange);
    fixture.assert_lock_script_exit(&tx, 0, 12);
    assert_no_expected_failure_dump(before);
}
```

Add these payment failure tests in `tests/tests/limit_order_lock.rs`:

```rust
#[test]
fn limit_order_lock_rejects_bound_payment_output_wrong_udt() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::PaymentOutputWrongUdt);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_bound_payment_output_wrong_owner() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::PaymentOutputWrongOwner);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}

#[test]
fn limit_order_lock_rejects_bound_payment_output_insufficient() {
    let before = failed_txs_count();
    let (fixture, tx) =
        limit_order_lock_nft_for_udt_case_with(LimitOrderLockFillCase::PaymentOutputInsufficient);
    fixture.assert_lock_script_exit(&tx, 0, 10);
    assert_no_expected_failure_dump(before);
}
```

In `tests/contracts/limit-order-lock/src/entry.rs`, add unit tests for `output_index_in_otx_outputs` accepting base and append outputs and rejecting out-of-range.

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-lock --offline
cargo test -p tests --test limit_order_lock --offline
```

Expected: FAIL because `output_index_in_otx_outputs` does not exist and entry still scans all matching payments.

- [ ] **Step 3: Implement exact payment binding for lock entry**

In `tests/contracts/limit-order-lock/src/entry.rs`, keep context:

```rust
let context = CobuildContext::build(CurrentScript::InputLock(current_lock_hash))?;
let plan = context.plan_lock_validation()?;
```

After parsing action:

```rust
let payment_output_index = action.payment_output_index as usize;
if !output_index_in_otx_outputs(layout, payment_output_index)? {
    return Err(Error::InvalidCobuild);
}
let payment = load_udt_payment_output(payment_output_index)?;
validate_fill(&order, &action, payment)
```

Add:

```rust
fn output_index_in_otx_outputs(
    layout: OtxMessageLayout,
    output_index: usize,
) -> Result<bool, Error> {
    Ok(range_contains(layout.base_outputs, output_index)?
        || range_contains(layout.append_outputs, output_index)?)
}

fn load_udt_payment_output(index: usize) -> Result<UdtPayment, Error> {
    let Some(asset_id) = load_cell_type_hash(index, Source::Output)? else {
        return Err(Error::InsufficientPayment);
    };
    let owner_lock_hash = load_cell_lock_hash(index, Source::Output)?;
    let data = load_cell_data(index, Source::Output)?;
    Ok(UdtPayment {
        owner_lock_hash,
        asset_id,
        amount: parse_udt_payment(&data)?,
    })
}
```

Remove `collect_payments`, `collect_payments_from_range`, and `payment_output_matches_order` once unused.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p limit-order-lock --offline
make -e -C tests/contracts/limit-order-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
cargo test -p tests --test limit_order_lock --offline
```

Expected: PASS for lock unit tests, lock contract build, and lock integration tests except duplicate tests not added yet.

- [ ] **Step 5: Review and commit**

Run:

```bash
git diff -- tests/contracts/limit-order-lock/src/entry.rs tests/contracts/limit-order-lock/src/types.rs tests/src/fixtures/limit_order/lock_nft_for_udt.rs tests/tests/limit_order_lock.rs
git diff --check
```

Record results in this plan, then:

```bash
git add tests/contracts/limit-order-lock/src/entry.rs tests/contracts/limit-order-lock/src/types.rs tests/src/fixtures/limit_order/lock_nft_for_udt.rs tests/tests/limit_order_lock.rs docs/superpowers/plans/2026-06-09-limit-order-payment-output-binding-plan.md
git commit -m "fix: bind lock orders to payment output"
```

## Task 6: Reject Duplicate Payment Indexes Within Same Contract Family

**Files:**
- Modify: `tests/contracts/limit-order-type/src/entry.rs`
- Modify: `tests/contracts/limit-order-lock/src/entry.rs`
- Modify: `tests/src/fixtures/limit_order/type_nft_for_udt.rs`
- Modify: `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`
- Modify: `tests/tests/limit_order_type.rs`
- Modify: `tests/tests/limit_order_lock.rs`
- Modify: `docs/superpowers/plans/2026-06-09-limit-order-payment-output-binding-plan.md`

**Red/Green Record:** Execution agent must replace this line with exact Red, Green, Review, and Commit results.

- [ ] **Step 1: Write failing duplicate helper and integration tests**

In both `entry.rs` test modules, add pure tests using `ActionView`:

```rust
#[test]
fn duplicate_payment_output_index_accepts_unique_indexes() {
    let actions = vec![
        test_action(cobuild_core::protocol::ScriptRole::InputType, [7; 32], fill_data(1)),
        test_action(cobuild_core::protocol::ScriptRole::InputType, [7; 32], fill_data(2)),
    ];

    assert_eq!(ensure_unique_payment_output_indexes(&actions, &[[7; 32]]), Ok(()));
}

#[test]
fn duplicate_payment_output_index_rejects_duplicate_indexes() {
    let actions = vec![
        test_action(cobuild_core::protocol::ScriptRole::InputType, [7; 32], fill_data(1)),
        test_action(cobuild_core::protocol::ScriptRole::InputType, [7; 32], fill_data(1)),
    ];

    assert_eq!(
        ensure_unique_payment_output_indexes(&actions, &[[7; 32]]),
        Err(Error::InvalidCobuild)
    );
}
```

Add local test helpers:

```rust
fn fill_data(payment_output_index: u32) -> Vec<u8> {
    let mut data = Vec::new();
    data.push(crate::types::FILL_ORDER_TAG);
    data.extend_from_slice(&[4; 32]);
    data.extend_from_slice(&30u64.to_le_bytes());
    data.extend_from_slice(&payment_output_index.to_le_bytes());
    data
}

fn test_action(
    script_role: cobuild_core::protocol::ScriptRole,
    script_hash: [u8; 32],
    data: Vec<u8>,
) -> cobuild_core::view::ActionView {
    cobuild_core::view::ActionView {
        index: 0,
        script_info_hash: [0; 32],
        script_role,
        script_hash,
        data: cobuild_core::reader::cursor_from_slice(&data),
    }
}
```

Add fixture scenarios:

```rust
TwoTypeOrdersReusePaymentOutput,
TwoTypeOrdersUseDistinctPaymentOutputs,
TwoLockOrdersReusePaymentOutput,
TwoLockOrdersUseDistinctPaymentOutputs,
```

Add integration tests asserting duplicate reuse fails with exit `12` and distinct indexes pass.

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-type --offline
cargo test -p limit-order-lock --offline
cargo test -p tests --test limit_order_type --offline
cargo test -p tests --test limit_order_lock --offline
```

Expected: FAIL because `ensure_unique_payment_output_indexes` and multi-order fixtures are not implemented.

- [ ] **Step 3: Implement duplicate checking in both entries**

In each `entry.rs`, import:

```rust
use cobuild_core::{protocol::ScriptRole, view::ActionView};
```

Add helper:

```rust
fn ensure_unique_payment_output_indexes(
    actions: &[ActionView],
    limit_order_targets: &[[u8; 32]],
) -> Result<(), Error> {
    let mut indexes = Vec::<u32>::new();
    for action in actions {
        if !matches!(
            action.script_role,
            ScriptRole::InputType | ScriptRole::OutputType | ScriptRole::InputLock
        ) {
            continue;
        }
        if !limit_order_targets.contains(&action.script_hash) {
            continue;
        }
        let data = cursor_bytes(&action.data)?;
        if data.first().copied() != Some(crate::types::FILL_ORDER_TAG) {
            continue;
        }
        let LimitOrderAction::Fill(fill) = parse_limit_order_action(&data)? else {
            return Err(Error::InvalidCobuild);
        };
        if indexes.contains(&fill.payment_output_index) {
            return Err(Error::InvalidCobuild);
        }
        indexes.push(fill.payment_output_index);
    }
    Ok(())
}
```

For `limit-order-lock`, use `parse_fill_order_action(&data)?` instead of `LimitOrderAction`.

In each fill entry after `otx_actions` is available:

```rust
let actions = context.otx_actions(otx_index)?;
ensure_unique_payment_output_indexes(&actions, &[related.action.action.script_hash])?;
```

For lock:

```rust
let actions = context.otx_actions(otx_index)?;
ensure_unique_payment_output_indexes(&actions, &[related.action.script_hash])?;
```

Implement two-order fixture builders using `CobuildMessageBuilder::push_action` and two payment outputs. For reuse scenarios, set both actions to `payment_output_index = 1`; for distinct scenarios, use `1` and `2`.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p limit-order-type --offline
cargo test -p limit-order-lock --offline
cargo test -p tests --test limit_order_type --offline
cargo test -p tests --test limit_order_lock --offline
```

Expected: PASS for unit and same-family duplicate integration tests.

- [ ] **Step 5: Review and commit**

Run:

```bash
git diff -- tests/contracts/limit-order-type/src/entry.rs tests/contracts/limit-order-lock/src/entry.rs tests/src/fixtures/limit_order/type_nft_for_udt.rs tests/src/fixtures/limit_order/lock_nft_for_udt.rs tests/tests/limit_order_type.rs tests/tests/limit_order_lock.rs
git diff --check
```

Record results in this plan, then:

```bash
git add tests/contracts/limit-order-type/src/entry.rs tests/contracts/limit-order-lock/src/entry.rs tests/src/fixtures/limit_order/type_nft_for_udt.rs tests/src/fixtures/limit_order/lock_nft_for_udt.rs tests/tests/limit_order_type.rs tests/tests/limit_order_lock.rs docs/superpowers/plans/2026-06-09-limit-order-payment-output-binding-plan.md
git commit -m "fix: reject duplicate limit order payments"
```

## Task 7: Cover Mixed Type And Lock Duplicate Payment Reuse

**Files:**
- Modify: `tests/contracts/limit-order-type/src/entry.rs`
- Modify: `tests/contracts/limit-order-lock/src/entry.rs`
- Modify: `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`
- Modify: `tests/tests/limit_order_lock.rs`
- Modify: `docs/superpowers/plans/2026-06-09-limit-order-payment-output-binding-plan.md`

**Red/Green Record:** Execution agent must replace this line with exact Red, Green, Review, and Commit results.

- [ ] **Step 1: Write failing mixed duplicate test**

Add fixture export in `tests/src/fixtures/limit_order.rs`:

```rust
pub use lock_nft_for_udt::{
    LimitOrderLockFillCase, limit_order_lock_nft_for_udt_case,
    limit_order_lock_nft_for_udt_case_with, mixed_limit_order_type_lock_duplicate_payment_case,
};
```

Add fixture function stub in `lock_nft_for_udt.rs`:

```rust
pub fn mixed_limit_order_type_lock_duplicate_payment_case() -> (CobuildTestFixture, TransactionView) {
    unimplemented!("Task 7 green step builds the mixed duplicate payment fixture")
}
```

Add test in `tests/tests/limit_order_lock.rs`:

```rust
#[test]
fn limit_order_mixed_type_and_lock_reject_duplicate_payment_output() {
    let before = failed_txs_count();
    let (fixture, tx) = mixed_limit_order_type_lock_duplicate_payment_case();
    fixture.assert_type_script_exit(&tx, 0, 12);
    assert_no_expected_failure_dump(before);
}
```

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p tests --test limit_order_lock --offline mixed_type_and_lock
```

Expected: FAIL because the mixed fixture is not implemented or because same-family target-only duplicate checking does not yet include both target hashes.

- [ ] **Step 3: Implement mixed target-set duplicate checking**

Update duplicate target selection in both entries so the target set includes every same-OTX tag `2` action with role `InputType`, `OutputType`, or `InputLock` that parses as the 45-byte Fill ABI and is part of the test fixture message.

Use this helper shape:

```rust
fn limit_order_target_hashes(actions: &[ActionView], current_target: [u8; 32]) -> Result<Vec<[u8; 32]>, Error> {
    let mut targets = Vec::new();
    targets.push(current_target);
    for action in actions {
        if !matches!(
            action.script_role,
            ScriptRole::InputType | ScriptRole::OutputType | ScriptRole::InputLock
        ) {
            continue;
        }
        let data = cursor_bytes(&action.data)?;
        if data.first().copied() != Some(crate::types::FILL_ORDER_TAG) {
            continue;
        }
        if data.len() != crate::types::FILL_ORDER_DATA_LEN {
            return Err(Error::InvalidActionData);
        }
        if !targets.contains(&action.script_hash) {
            targets.push(action.script_hash);
        }
    }
    Ok(targets)
}
```

Then:

```rust
let actions = context.otx_actions(otx_index)?;
let targets = limit_order_target_hashes(&actions, current_target)?;
ensure_unique_payment_output_indexes(&actions, &targets)?;
```

Keep unrelated non-tag-2 actions ignored. Any tag `2` action in the selected role set with malformed length fails closed, matching the tests-only fixture boundary.

Implement `mixed_limit_order_type_lock_duplicate_payment_case` with these exact construction steps:

1. Create `CobuildTestFixture::new()`.
2. Deploy:
   - `limit-order-type` with `fixture.deploy_limit_order()`;
   - `limit-order-lock` with `deploy_data2_script(fixture.context_mut(), "limit-order-lock", Vec::new())`;
   - `always_success`, wrong-owner lock if needed, `input-type-proxy-lock`, one `test-nft`, and one `test-udt`.
3. Use `owner_lock = always_success.script.clone()` and `buyer_lock = always_success.script.clone()`.
4. Build one type-order input with `fixture.limit_order().owner(owner_lock.clone()).offered_nft_type_hash(nft.script_hash).requested_asset_id(udt.script_hash).min_requested_amount(30).build_input(&limit_order_type.script)`.
5. Build one lock-order NFT input by constructing `limit-order-lock` args from:
   - `owner_lock_hash = script_hash(&owner_lock)`;
   - `offered_nft_type_hash = nft.script_hash`;
   - `requested_asset_id = udt.script_hash`;
   - `min_requested_amount = 30`.
   Then build the lock script with `ScriptHashType::Data2` and `Bytes::copy_from_slice(&lock_args)`, and create a live NFT input with `typed_output(order_lock.clone(), nft.script.clone(), 100_000_000_000)`.
6. Build one buyer UDT append input with amount `60`.
7. Build two base outputs:
   - a buyer NFT output for the type order's proxy-held NFT;
   - a buyer NFT output for the lock order's NFT.
8. Build one append output:
   - a shared owner UDT payment output using `typed_output(owner_lock.clone(), udt.script.clone(), 90_000_000_000)` and `udt_amount_data(30)`.
9. Set `let shared_payment_output_index = 2u32;` because the two base outputs occupy absolute indexes `0` and `1`, and the shared append UDT payment is absolute output index `2`.
10. Build one OTX with:
   - `base_input_cells(2)` for the type order input and lock NFT input;
   - `base_output_cells(2)` for the two NFT outputs;
   - `append_input_cells(1)` for the buyer UDT input;
   - `append_output_cells(1)` for the shared owner UDT payment;
   - append input/output permissions;
   - a seal pair for the lock order hash with base scope;
   - a message containing two actions via `push_action`:
     - `script_role = 1`, `script_hash = limit_order_type.script_hash`, action data for shared index `2`;
     - `script_role = 0`, `script_hash = order_lock_hash`, action data for shared index `2`.
11. Build the tx with all needed cell deps and the OTX.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p limit-order-type --offline
cargo test -p limit-order-lock --offline
cargo test -p tests --test limit_order_type --offline
cargo test -p tests --test limit_order_lock --offline
```

Expected: PASS including mixed duplicate rejection.

- [ ] **Step 5: Review and commit**

Run:

```bash
git diff -- tests/contracts/limit-order-type/src/entry.rs tests/contracts/limit-order-lock/src/entry.rs tests/src/fixtures/limit_order.rs tests/src/fixtures/limit_order/lock_nft_for_udt.rs tests/tests/limit_order_lock.rs
git diff --check
```

Record results in this plan, then:

```bash
git add tests/contracts/limit-order-type/src/entry.rs tests/contracts/limit-order-lock/src/entry.rs tests/src/fixtures/limit_order.rs tests/src/fixtures/limit_order/lock_nft_for_udt.rs tests/tests/limit_order_lock.rs docs/superpowers/plans/2026-06-09-limit-order-payment-output-binding-plan.md
git commit -m "test: cover mixed limit order payment reuse"
```

## Final Verification

After all tasks are committed, run:

```bash
cargo fmt
make -e -C tests/contracts/limit-order-type build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/limit-order-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-udt build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-nft build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
cargo test -p limit-order-type --offline
cargo test -p limit-order-lock --offline
cargo test -p cobuild-core --lib --offline
cargo test -p tests --test limit_order_type --offline
cargo test -p tests --test limit_order_lock --offline
cargo test -p tests --lib --offline
cargo test --workspace --offline
cargo fmt --check
git diff --check
git status --short
find tests/failed_txs -maxdepth 1 -type f 2>/dev/null | wc -l
git status --short --ignored tests/failed_txs
```

Record the exact results here before final response:

```text
Final Verification:
cargo fmt -> not run yet
limit-order-type contract build -> not run yet
limit-order-lock contract build -> not run yet
test-udt contract build -> not run yet
test-nft contract build -> not run yet
cargo test -p limit-order-type --offline -> not run yet
cargo test -p limit-order-lock --offline -> not run yet
cargo test -p cobuild-core --lib --offline -> not run yet
cargo test -p tests --test limit_order_type --offline -> not run yet
cargo test -p tests --test limit_order_lock --offline -> not run yet
cargo test -p tests --lib --offline -> not run yet
cargo test --workspace --offline -> not run yet
cargo fmt --check -> not run yet
git diff --check -> not run yet
git status --short -> not run yet
failed_txs tracked-file count -> not run yet
git status --short --ignored tests/failed_txs -> not run yet
```

## Plan Self-Review

Spec coverage:

- 45-byte shared ABI: Tasks 1, 2, 3.
- Rejection of old 41-byte ABI: Tasks 1, 2.
- Exact bound payment output validation: Tasks 4, 5.
- Base/append OTX output range validation: Tasks 4, 5.
- Same-family duplicate rejection: Task 6.
- Mixed type+lock duplicate rejection: Task 7.
- No `cobuild-types` schema changes: enforced by file boundaries.
- Final verification commands: listed above.

Placeholder scan:

- The plan intentionally contains execution record lines to be replaced during implementation.
- No implementation task contains unspecified code requirements.

Type consistency:

- `payment_output_index` is consistently `u32` in action data and converted to `usize` for syscalls.
- Both contracts use the same 45-byte Fill ABI.
- Integration fixture indexes are absolute transaction output indexes.
