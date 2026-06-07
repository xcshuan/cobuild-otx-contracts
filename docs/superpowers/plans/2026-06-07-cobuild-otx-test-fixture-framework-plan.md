# Cobuild OTX Test Fixture Framework Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a reusable tests-only Cobuild OTX fixture framework and refactor the Limit Order integration test onto it without changing behavior.

**Architecture:** Add focused modules under `tests/src/framework/` for contracts, scripts, cells, Cobuild witness builders, tx assembly, assertions, and the top-level fixture. `tests/tests/limit_order.rs` should become a scenario-level test that composes these helpers instead of hand-building Molecule witnesses and transactions.

**Tech Stack:** Rust 2024, `ckb-testtool`, `cobuild-types` Molecule entity builders, existing `tests::Loader`, existing `Context`.

---

### Task 1: Add Framework Module Skeleton and Helper Tests

**Files:**
- Modify: `tests/src/lib.rs`
- Create: `tests/src/framework/mod.rs`
- Create: `tests/src/framework/cells.rs`
- Create: `tests/src/framework/cobuild.rs`
- Create: `tests/src/framework/assertions.rs`

- [ ] **Step 1: Write failing tests**

Add tests in `tests/src/framework/mod.rs` that exercise the desired helper surface:

```rust
#[cfg(test)]
mod tests {
    use super::{
        cells::{LimitOrderState, order_data, settlement_data},
        cobuild::{CobuildMessageBuilder, OtxBuilder},
    };

    #[test]
    fn limit_order_helpers_encode_fixed_width_order_and_settlement_data() {
        let order = LimitOrderState {
            order_id: [1; 32],
            owner_lock_hash: [2; 32],
            offered_asset_id: [3; 32],
            requested_asset_id: [4; 32],
            offered_remaining: 10,
            min_requested_per_offered: 3,
            nonce: 9,
        };

        let data = order_data(order);
        let settlement = settlement_data([4; 32], 30);

        assert_eq!(data.len(), 152);
        assert_eq!(&data[0..32], &[1; 32]);
        assert_eq!(&data[32..64], &[2; 32]);
        assert_eq!(&data[64..96], &[3; 32]);
        assert_eq!(&data[96..128], &[4; 32]);
        assert_eq!(&data[128..136], &10u64.to_le_bytes());
        assert_eq!(&data[136..144], &3u64.to_le_bytes());
        assert_eq!(&data[144..152], &9u64.to_le_bytes());
        assert_eq!(settlement.len(), 40);
        assert_eq!(&settlement[0..32], &[4; 32]);
        assert_eq!(&settlement[32..40], &30u64.to_le_bytes());
    }

    #[test]
    fn cobuild_helpers_encode_limit_order_fill_action_and_default_otx_layout() {
        let message = CobuildMessageBuilder::new()
            .input_type_action([9; 32])
            .limit_order_fill([1; 32], [4; 32], 10, 30)
            .build();

        let otx = OtxBuilder::new()
            .message(message)
            .base_input_cells(1)
            .append_output_cells(1)
            .allow_append_outputs()
            .build();

        assert_eq!(otx.append_permissions().as_slice(), &[0b0010]);
    }
}
```

- [ ] **Step 2: Verify red**

Run:

```bash
cargo test -p tests framework --offline
```

Expected: FAIL because framework modules and builders do not exist yet.

- [ ] **Step 3: Implement minimal cells and Cobuild builders**

Implement:

- `LimitOrderState`
- `order_data()`
- `settlement_data()`
- `CobuildMessageBuilder`
- `OtxBuilder`

Use `cobuild_types::entity::core::{Action, ActionVec, Message, Otx, SealPairVec}`. The default `OtxBuilder` values must match the current Limit Order integration test:

- `base_input_cells = 1`
- `base_input_masks = vec![0]`
- `base_output_cells = 0`
- `base_output_masks = vec![]`
- `append_input_cells = 0`
- `append_output_cells = 1`
- `append_permissions = 0b0010` after `allow_append_outputs()`
- empty seals

- [ ] **Step 4: Verify green**

Run:

```bash
cargo test -p tests framework --offline
```

Expected: PASS for the new framework helper tests.

### Task 2: Add Contract, Script, Tx, and Assertion Helpers

**Files:**
- Create: `tests/src/framework/contracts.rs`
- Create: `tests/src/framework/scripts.rs`
- Create: `tests/src/framework/tx.rs`
- Modify: `tests/src/framework/assertions.rs`
- Modify: `tests/src/framework/mod.rs`

- [ ] **Step 1: Write failing tests for deploy and assertion helpers**

Add framework tests that:

- deploy `limit-order` and `always-success`
- confirm `DeployedScript.script_hash == script_hash(&script)`
- construct a pure expected script error with `ScriptError::ValidationFailure("by convention".to_owned(), 11).input_type_script(0).into()` and assert it through `assert_type_script_exit_result(Err(error), 0, 11)`
- verify the assertion rejects the wrong input index or wrong exit code

- [ ] **Step 2: Verify red**

Run:

```bash
cargo test -p tests framework --offline
```

Expected: FAIL because deployment, script, tx, and assertion helpers are incomplete.

- [ ] **Step 3: Implement helpers**

Implement:

- `DeployedScript`
- `deploy_data2_script(context, name, args)`
- `deploy_always_success(context, args)`
- `cell_dep_for_script(&DeployedScript)`
- `script_hash(&Script) -> [u8; 32]`
- `normal_output(lock, capacity)`
- `typed_output(lock, type_script, capacity)`
- `TestCellOutput { cell, data }`
- `live_input(context, output, data)`
- `OtxTransactionBuilder` that emits cell deps, inputs, outputs, output data, `OtxStart`, and `Otx` witnesses
- `assert_pass(context, tx)` using dump-on-unexpected-failure
- `assert_type_script_exit_result(result, input_index, code)` for unit testing the error matching logic
- `assert_type_script_exit(context, tx, input_index, code)` with no default dump for expected failure

`OtxTransactionBuilder` must:

- keep deployed contract deps outside OTX scope by default
- compute `OtxStart.start_cell_deps` as the number of already-added non-OTX deps
- keep `TestCellOutput.cell` and `TestCellOutput.data` aligned when adding outputs and output data
- reject OTX builds with no OTX witnesses or zero base inputs
- append `OtxStart` and OTX witnesses contiguously

- [ ] **Step 4: Verify green**

Run:

```bash
cargo test -p tests framework --offline
```

Expected: PASS.

### Task 3: Add Top-Level CobuildTestFixture

**Files:**
- Create: `tests/src/framework/fixture.rs`
- Modify: `tests/src/framework/mod.rs`

- [ ] **Step 1: Write failing fixture API test**

Add a test that creates `CobuildTestFixture::new()`, deploys `limit-order` and `always-success`, creates an owner lock, and starts an OTX transaction builder.

- [ ] **Step 2: Verify red**

Run:

```bash
cargo test -p tests framework --offline
```

Expected: FAIL because `CobuildTestFixture` is not implemented.

- [ ] **Step 3: Implement fixture facade**

`CobuildTestFixture` should own `Context` and expose:

- `new()`
- `context()` and `context_mut()` if needed
- `deploy_limit_order()`
- `deploy_always_success()`
- `limit_order()`
- `cobuild()`
- `otx()`
- `tx()`
- `assert_pass(tx)`
- `assert_type_script_exit(tx, input_index, code)`

The fixture should expose only generic deployment and current Limit Order helpers in this task. Do not add UDT/NFT data encoders or NFT-for-UDT swap helpers.

- [ ] **Step 4: Verify green**

Run:

```bash
cargo test -p tests framework --offline
```

Expected: PASS.

### Task 4: Refactor Limit Order Integration Test

**Files:**
- Modify: `tests/tests/limit_order.rs`

- [ ] **Step 1: Write the refactored scenario first**

Rewrite the two tests so they use the framework:

- valid append settlement uses settlement amount `30`
- insufficient append settlement uses settlement amount `29`
- both tests share a small scenario helper, but no longer hand-write `Otx::new_builder()`, `OtxStart::new_builder()`, or `TransactionBuilder::default()`

- [ ] **Step 2: Verify red or compile failure**

Run:

```bash
cargo test -p tests --test limit_order --offline
```

Expected: FAIL until missing framework API pieces are filled.

- [ ] **Step 3: Fill missing API gaps only**

Implement only the API needed by the refactored test. Keep broader business helpers for Crowdfunding, NFT Minter, and AMM out of this task.

- [ ] **Step 4: Verify green**

Run:

```bash
cargo test -p tests --test limit_order --offline
```

Expected: both Limit Order tests pass; the failing case reports exit code `11` without writing a new `tests/failed_txs/*.json` unless `COBUILD_TEST_DUMP_EXPECTED_FAILURES=1`.

Before and after the negative test, check the number of files in `tests/failed_txs` in the test process or via a focused assertion-helper test so expected failures do not silently reintroduce unconditional dumping.

### Task 5: Full Verification

**Files:**
- No new files unless formatting changes are required.

- [ ] **Step 1: Run workspace tests**

Run:

```bash
cargo test --workspace --offline
```

Expected: PASS.

- [ ] **Step 2: Run Limit Order debug build**

Run:

```bash
make -e -C tests/contracts/limit-order build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
```

Expected: PASS.

- [ ] **Step 3: Run formatting check**

Run:

```bash
cargo fmt --check
```

Expected: PASS.

- [ ] **Step 4: Run whitespace check**

Run:

```bash
git diff --check
```

Expected: PASS.

### Task 6: Commit

**Files:**
- Stage only tests/framework, refactored Limit Order test, docs created for this task, and necessary test manifest changes.

- [ ] **Step 1: Review diff**

Run:

```bash
git diff --stat
git diff -- tests/src tests/tests/limit_order.rs docs/superpowers/specs/2026-06-07-cobuild-otx-test-fixture-framework-design.md docs/superpowers/plans/2026-06-07-cobuild-otx-test-fixture-framework-plan.md
```

- [ ] **Step 2: Commit**

Run:

```bash
git add tests/src tests/tests/limit_order.rs docs/superpowers/specs/2026-06-07-cobuild-otx-test-fixture-framework-design.md docs/superpowers/plans/2026-06-07-cobuild-otx-test-fixture-framework-plan.md
git commit -m "test: add cobuild otx test fixture framework"
```
