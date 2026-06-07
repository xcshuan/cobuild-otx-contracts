# Limit Order Test Type Script Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the first Cobuild OTX test type script, `limit-order`, as a small test-only contract that validates full-fill append settlement.

**Architecture:** The contract lives under `tests/contracts/limit-order` and calls `cobuild-core` to build a type validation plan. Pure parsing and validation helpers are unit tested first; the entry point wires those helpers to CKB syscalls and OTX action layout.

**Tech Stack:** Rust 2024, `ckb-std`, `cobuild-core`, test-only contract template, `ckb-testtool` integration tests.

---

## File Structure

- Create `docs/superpowers/specs/2026-06-07-limit-order-test-type-script-spec.md`: single-use spec for the first fixture.
- Create `docs/superpowers/plans/2026-06-07-limit-order-test-type-script.md`: this execution plan.
- Create `tests/contracts/limit-order`: script-template generated test contract.
- Modify `Cargo.toml`: add `tests/contracts/limit-order` as a workspace member.
- Create `tests/contracts/limit-order/src/types.rs`: fixed-width order, settlement, and fill action parsing plus pure settlement checks.
- Modify `tests/contracts/limit-order/src/error.rs`: test-only error mapping for parsing, Cobuild Core, and state failures.
- Modify `tests/contracts/limit-order/src/entry.rs`: CKB entry point using `CobuildContext::plan_type_validation`.
- Modify `tests/tests/contract_template_layout.rs`: assert the new test contract is under `tests/contracts`, not `contracts`.
- Create or modify a Limit Order integration test under `tests/tests`: after pure contract tests pass, add a CKB VM fixture for one valid fill and one insufficient settlement fill.

## Task 1: Scaffold Contract Location Test

**Files:**
- Modify: `tests/tests/contract_template_layout.rs`

- [ ] **Step 1: Write the failing layout test**

Add an assertion that the workspace contains `tests/contracts/limit-order` and does not contain `contracts/limit-order`.

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test --workspace --offline contract_template_layout -- --nocapture
```

Expected: fail because the new workspace member does not exist yet.

- [ ] **Step 3: Generate contract with script-template**

Run:

```bash
make generate CRATE=limit-order DESTINATION=tests/contracts
```

If template generation leaves `Cargo.toml.new`, keep the generated contract directory, add the workspace member manually, and remove the empty temporary manifest.

- [ ] **Step 4: Run layout test again**

Run:

```bash
cargo test --workspace --offline contract_template_layout -- --nocapture
```

Expected: pass.

## Task 2: Pure ABI Tests

**Files:**
- Create: `tests/contracts/limit-order/src/types.rs`
- Modify: `tests/contracts/limit-order/src/lib.rs`

- [ ] **Step 1: Write failing tests**

Add unit tests for:

- decoding a 152-byte order state;
- rejecting truncated order data;
- decoding a `FillOrder` action variant;
- rejecting unsupported action variants;
- computing required requested amount;
- rejecting multiplication overflow.

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p limit-order --offline
```

Expected: fail because `types` helpers are not implemented.

- [ ] **Step 3: Implement minimal parsing helpers**

Implement fixed-width readers with no heap-heavy abstraction:

```rust
pub const ORDER_DATA_LEN: usize = 152;
pub const SETTLEMENT_DATA_LEN: usize = 40;
pub const FILL_ORDER_TAG: u8 = 1;

pub struct OrderState { ... }
pub struct SettlementCell { ... }
pub struct FillOrderAction { ... }

pub fn parse_order_state(data: &[u8]) -> Result<OrderState, Error>;
pub fn parse_settlement_cell(data: &[u8]) -> Result<SettlementCell, Error>;
pub fn parse_fill_order_action(data: &[u8]) -> Result<FillOrderAction, Error>;
pub fn required_requested_amount(order: &OrderState) -> Result<u64, Error>;
```

- [ ] **Step 4: Run tests to verify pass**

Run:

```bash
cargo test -p limit-order --offline
```

Expected: pass.

## Task 3: Pure Settlement Validation Tests

**Files:**
- Modify: `tests/contracts/limit-order/src/types.rs`

- [ ] **Step 1: Write failing tests**

Add tests that validate:

- exact payment passes;
- overpayment passes;
- insufficient payment fails;
- wrong owner lock hash fails;
- wrong asset id fails.

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p limit-order --offline
```

Expected: fail because settlement aggregation is missing.

- [ ] **Step 3: Implement settlement validation**

Add a helper that sums only matching settlement cells:

```rust
pub fn validate_fill(
    order: &OrderState,
    action: &FillOrderAction,
    settlements: &[SettlementCell],
) -> Result<(), Error>;
```

- [ ] **Step 4: Run tests to verify pass**

Run:

```bash
cargo test -p limit-order --offline
```

Expected: pass.

## Task 4: Cobuild Entry Point

**Files:**
- Modify: `tests/contracts/limit-order/Cargo.toml`
- Modify: `tests/contracts/limit-order/src/error.rs`
- Modify: `tests/contracts/limit-order/src/entry.rs`

- [ ] **Step 1: Write failing entry-level unit test where possible**

Add tests for error conversions and action-origin rejection without syscalls.

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cargo test -p limit-order --offline
```

Expected: fail until error mapping and origin helper exist.

- [ ] **Step 3: Implement entry point**

The entry point should:

- load current type script hash;
- build `CobuildContext::build(CurrentScript::Type(hash))`;
- call `plan_type_validation()`;
- require one group input and zero group outputs;
- require exactly one related action;
- reject tx-level action origin;
- reject OTX relation that does not include current type in base inputs;
- scan OTX base and append output ranges for settlement cells;
- call `validate_fill`.

- [ ] **Step 4: Run tests to verify pass**

Run:

```bash
cargo test -p limit-order --offline
```

Expected: pass.

## Task 5: Integration Tests

**Files:**
- Create or modify: `tests/tests/limit_order.rs`
- Modify shared fixture helpers only when they are useful for more than this one test.

- [ ] **Step 1: Write failing valid-fill integration test**

Build a transaction with:

- one base order input using the Limit Order type;
- one OTX witness containing `FillOrder`;
- one append output locked to the owner with settlement data at the required amount.

Expected: pass once the contract is implemented and built.

- [ ] **Step 2: Write failing insufficient-payment integration test**

Use the same fixture with settlement amount one unit below required.

Expected: fail with the Limit Order payment error.

- [ ] **Step 3: Build the contract binary**

Run:

```bash
make -e -C tests/contracts/limit-order build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
```

- [ ] **Step 4: Run targeted integration tests**

Run:

```bash
cargo test -p tests --offline limit_order -- --nocapture
```

Expected: valid fill passes and insufficient payment fails as expected.

## Task 6: Final Verification and Commit

**Files:**
- All files changed above.

- [ ] **Step 1: Format**

Run:

```bash
cargo fmt
```

- [ ] **Step 2: Verify workspace tests**

Run:

```bash
cargo test --workspace --offline
```

- [ ] **Step 3: Verify contract build**

Run:

```bash
make -e -C tests/contracts/limit-order build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
```

- [ ] **Step 4: Check diff hygiene**

Run:

```bash
git diff --check
git status --short
```

- [ ] **Step 5: Commit**

Run:

```bash
git add docs/superpowers/specs/2026-06-07-limit-order-test-type-script-spec.md docs/superpowers/plans/2026-06-07-limit-order-test-type-script.md Cargo.toml tests
git commit -m "test: add limit order cobuild type fixture"
```
