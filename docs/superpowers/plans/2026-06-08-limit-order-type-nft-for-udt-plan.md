# Limit Order Type NFT-for-UDT Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the tests-only `limit-order-type` fixture so a Cobuild OTX can consume an NFT order and verify a real `test-udt` payment to the maker.

**Architecture:** Keep asset custody outside the order type: the offered NFT is unlocked by a tests-only input-type proxy lock, while `limit-order-type` verifies only the current OTX-scoped payment. The fixture builder owns business scenarios; framework helpers own generic cell, OTX, and transaction layout mechanics.

**Tech Stack:** Rust 2024, `ckb-std`, `cobuild-core`, `ckb-testtool`, tests-only contracts under `tests/contracts`, offline Cargo, CKB contract Makefiles.

---

## Source Requirements

Read these before executing any task:

- `docs/superpowers/specs/2026-06-07-cobuild-otx-test-type-scripts-vision.md`
- `docs/superpowers/specs/2026-06-07-limit-order-test-type-script-spec.md`
- `tests/contracts/limit-order/src/entry.rs`
- `tests/contracts/limit-order/src/types.rs`
- `tests/contracts/limit-order/src/error.rs`
- `tests/tests/limit_order.rs`
- `tests/src/fixtures/limit_order.rs`
- `tests/src/framework/*.rs`
- `tests/src/fixtures/udt.rs`
- `tests/contracts/test-udt/src/entry.rs`
- `tests/contracts/test-nft/src/entry.rs`

Start execution with:

```bash
git status --short
```

Expected: no output. If the worktree is dirty, inspect the changes and do not overwrite unrelated user work.

## File Structure

- Rename directory: `tests/contracts/limit-order` -> `tests/contracts/limit-order-type`
- Modify: `tests/contracts/limit-order-type/Cargo.toml`
  - Change package name from `limit-order` to `limit-order-type`.
- Modify: `tests/contracts/limit-order-type/README.md`
  - Change heading to `limit-order-type`.
- Modify: `Cargo.toml`
  - Replace workspace member `tests/contracts/limit-order` with `tests/contracts/limit-order-type`.
- Modify: `tests/tests/contract_template_layout.rs`
  - Assert `limit-order-type` lives under `tests/contracts`.
- Modify: `tests/tests/makefile_layout.rs`
  - Include `tests/contracts/limit-order-type` in the root Makefile dry-run expectations.
- Modify: `tests/src/framework/mod.rs`
  - Update framework self-test deployment from `limit-order` to `limit-order-type`.
- Modify: `tests/src/framework/contracts.rs`
  - No planned change; existing `deploy_data2_script(context, name, args)` already supports lock and type args.
- Modify: `tests/src/framework/cobuild.rs`
  - Add append input permission builder support.
- Modify: `tests/src/framework/tx.rs`
  - Add generic support for base outputs, append inputs, and tx-level remainder outputs.
- Modify: `tests/src/framework/cells.rs`
  - No planned change; existing `typed_output`, `live_input`, and `live_resolved_typed_input` are sufficient.
- Use: `tests/vendor/ckb-proxy-locks/contracts/input-type-proxy-lock`
  - Minimal tests-only lock contract that unlocks when any transaction input type hash equals the first 32 bytes of its args.
- Modify: root `Cargo.toml`
  - Add `tests/vendor/ckb-proxy-locks` as a git submodule; do not add its crates to this workspace.
- Modify: `tests/tests/workspace_layout.rs`
  - Include `ckb-proxy-locks` under tests/vendor as the source of proxy lock contracts.
- Modify: `tests/contracts/limit-order-type/src/types.rs`
  - Add pure UDT payment parsing and validation support.
- Modify: `tests/contracts/limit-order-type/src/entry.rs`
  - Load output type hashes and count supported settlement cells inside the current OTX scope.
- Modify: `tests/contracts/limit-order-type/src/error.rs`
  - Add only necessary tests-only error variants for UDT payment parsing.
- Modify: `tests/src/fixtures/limit_order.rs`
  - Keep limit-order business DSL here.
  - Add NFT-for-UDT scenario builders.
  - Add helper variants for insufficient UDT, wrong UDT, wrong owner, and tx-level remainder.
- Modify: `tests/tests/limit_order.rs`
  - Keep thin tests only: names, fixture calls, assertions.

Do not modify:

- `contracts/cobuild-otx-lock`
- `crates/cobuild-core`
- `crates/cobuild-types`
- production contract directories outside `tests/contracts`

## Red/Green Log Discipline

For every task below, append a short note to this file under the task's **Red/Green Record** section:

```text
Red: <command> -> <observed failing test/error>
Green: <command> -> <observed pass>
```

Do not skip red runs. For expected-failure integration tests, verify `failed_txs` is not changed unless `COBUILD_TEST_DUMP_EXPECTED_FAILURES=1`.

## Task 1: Rename Contract to `limit-order-type`

**Files:**
- Rename: `tests/contracts/limit-order` -> `tests/contracts/limit-order-type`
- Modify: `tests/contracts/limit-order-type/Cargo.toml`
- Modify: `tests/contracts/limit-order-type/README.md`
- Modify: `Cargo.toml`
- Modify: `tests/tests/contract_template_layout.rs`
- Modify: `tests/tests/makefile_layout.rs`
- Modify: `tests/src/framework/mod.rs`
- Modify: `tests/src/fixtures/limit_order.rs`
- Modify: `tests/tests/limit_order.rs`
  - No planned change in this task; test names can remain `limit_order`.

- [ ] **Step 1: Write the failing layout assertions**

Change `tests/tests/contract_template_layout.rs` so the first test expects:

```rust
#[test]
fn limit_order_type_fixture_contract_lives_under_tests() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let workspace_manifest =
        fs::read_to_string(workspace_root.join("Cargo.toml")).expect("workspace manifest");

    assert!(
        workspace_root
            .join("tests/contracts/limit-order-type")
            .is_dir(),
        "limit-order-type must be a test fixture contract under tests/contracts"
    );
    assert!(
        workspace_manifest.contains("\"tests/contracts/limit-order-type\""),
        "limit-order-type must be compiled as a workspace test contract"
    );
    assert!(
        !workspace_root.join("contracts/limit-order-type").exists(),
        "limit-order-type fixture must not be placed under production contracts"
    );
}
```

Change `tests/tests/makefile_layout.rs` so the dry-run expected test-only contracts include:

```rust
for contract in [
    "tests/contracts/limit-order-type",
    "tests/contracts/test-udt",
    "tests/contracts/test-nft",
] {
    assert!(
        stdout.contains(contract),
        "root Makefile must build test-only contract {contract}"
    );
}
```

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p tests --test contract_template_layout --offline limit_order_type_fixture_contract_lives_under_tests -- --nocapture
cargo test -p tests --test makefile_layout --offline root_makefile_builds_test_only_contracts -- --nocapture
```

Expected: fail because the directory and workspace member are still `limit-order`.

- [ ] **Step 3: Rename and update references**

Run the directory rename:

```bash
mv tests/contracts/limit-order tests/contracts/limit-order-type
```

Edit `tests/contracts/limit-order-type/Cargo.toml`:

```toml
[package]
name = "limit-order-type"
version = "0.1.0"
edition = "2024"
```

Edit root `Cargo.toml` member:

```toml
"tests/contracts/limit-order-type",
```

Edit `tests/contracts/limit-order-type/README.md` heading:

```markdown
# limit-order-type
```

Update deployments from:

```rust
deploy_data2_script(self.context_mut(), "limit-order", Vec::new())
```

to:

```rust
deploy_data2_script(self.context_mut(), "limit-order-type", Vec::new())
```

Update `tests/src/framework/mod.rs` contract helper self-test to deploy `"limit-order-type"`.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p tests --test contract_template_layout --offline limit_order_type_fixture_contract_lives_under_tests -- --nocapture
cargo test -p tests --test makefile_layout --offline root_makefile_builds_test_only_contracts -- --nocapture
cargo test -p limit-order-type --offline
```

Expected: all pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add Cargo.toml tests/contracts/limit-order-type tests/tests/contract_template_layout.rs tests/tests/makefile_layout.rs tests/src/framework/mod.rs tests/src/fixtures/limit_order.rs
git add -u tests/contracts/limit-order
git commit -m "test: rename limit order fixture contract"
```

**Red/Green Record:**

```text
Red: cargo test -p tests --test contract_template_layout --offline limit_order_type_fixture_contract_lives_under_tests -- --nocapture -> failed: missing tests/contracts/limit-order-type
Red: cargo test -p tests --test makefile_layout --offline root_makefile_builds_test_only_contracts -- --nocapture -> failed: root Makefile dry-run did not include tests/contracts/limit-order-type
Green: cargo test -p tests --test contract_template_layout --offline limit_order_type_fixture_contract_lives_under_tests -- --nocapture -> passed
Green: cargo test -p tests --test makefile_layout --offline root_makefile_builds_test_only_contracts -- --nocapture -> passed
Green: cargo test -p limit-order-type --offline -> passed after updating src/main.rs crate path
```

## Task 2: Vendor ckb-proxy-locks for Input Type Proxy Lock

**Files:**
- Use: `tests/vendor/ckb-proxy-locks/contracts/input-type-proxy-lock`
- Create: `.gitmodules`
- Add submodule gitlink: `tests/vendor/ckb-proxy-locks`
- Modify: `tests/tests/workspace_layout.rs`
- Modify: `docs/superpowers/plans/2026-06-08-limit-order-type-nft-for-udt-plan.md`
- Delete: `tests/contracts/test-input-type-proxy-lock`
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`

- [ ] **Step 1: Write failing vendor layout test**

In `tests/tests/workspace_layout.rs`, add:

```rust
#[test]
fn proxy_locks_live_under_tests_vendor_submodule() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let vendor_dir = workspace_root.join("tests/vendor/ckb-proxy-locks");
    let input_type_proxy_lock = vendor_dir.join("contracts/input-type-proxy-lock");

    assert!(
        vendor_dir.join(".git").exists() || vendor_dir.join(".git").is_file(),
        "ckb-proxy-locks must be checked out as a tests/vendor submodule"
    );
    assert!(
        input_type_proxy_lock.join("Cargo.toml").is_file(),
        "missing vendored input-type-proxy-lock manifest"
    );
    assert!(
        input_type_proxy_lock.join("Makefile").is_file(),
        "missing vendored input-type-proxy-lock Makefile"
    );
    assert!(
        !workspace_root
            .join("tests/contracts/test-input-type-proxy-lock")
            .exists(),
        "input-type-proxy-lock must be reused from tests/vendor/ckb-proxy-locks"
    );
}
```

Remove `"tests/contracts/test-input-type-proxy-lock"` from root workspace layout tests and Makefile dry-run expectations.

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p tests --test workspace_layout --offline proxy_locks_live_under_tests_vendor_submodule -- --nocapture
```

Expected: fail until the submodule is present and the in-repo `tests/contracts/test-input-type-proxy-lock` fixture is removed.

- [ ] **Step 3: Add submodule and remove in-repo proxy lock**

Run:

```bash
git submodule add https://github.com/ckb-devrel/ckb-proxy-locks tests/vendor/ckb-proxy-locks
```

If `tests/vendor/ckb-proxy-locks` already exists as a clean checkout, use:

```bash
git submodule add --force https://github.com/ckb-devrel/ckb-proxy-locks tests/vendor/ckb-proxy-locks
```

Remove the in-repo test contract:

```bash
git rm -r tests/contracts/test-input-type-proxy-lock
```

Remove the root workspace member:

```toml
"tests/contracts/test-input-type-proxy-lock",
```

After removing the member, run:

```bash
cargo check -p tests --offline
```

Expected: `Cargo.lock` no longer contains a `test-input-type-proxy-lock` package entry.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p tests --test workspace_layout --offline proxy_locks_live_under_tests_vendor_submodule -- --nocapture
cargo test -p tests --test workspace_layout --offline workspace_declares_clean_cobuild_members -- --nocapture
cargo test -p tests --test makefile_layout --offline root_makefile_builds_test_only_contracts -- --nocapture
```

Expected: all pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add .gitmodules Cargo.toml Cargo.lock docs/superpowers/plans/2026-06-08-limit-order-type-nft-for-udt-plan.md tests/vendor/ckb-proxy-locks tests/tests/workspace_layout.rs tests/tests/makefile_layout.rs
git add -u tests/contracts/test-input-type-proxy-lock
git commit -m "test: vendor ckb proxy locks"
```

**Red/Green Record:**

```text
Red: cargo test -p tests --test workspace_layout --offline test_asset_contracts_live_under_tests_directory -- --nocapture -> failed: missing test-only contract manifest for input-type-proxy-lock
Red: cargo test -p tests --test makefile_layout --offline root_makefile_builds_test_only_contracts -- --nocapture -> failed: root Makefile dry-run did not include tests/vendor/ckb-proxy-locks/contracts/input-type-proxy-lock
Green: cargo test -p input-type-proxy-lock --offline -> passed
Green: cargo test -p tests --test workspace_layout --offline test_asset_contracts_live_under_tests_directory -- --nocapture -> passed
Green: cargo test -p tests --test makefile_layout --offline root_makefile_builds_test_only_contracts -- --nocapture -> passed
Superseded: in-repo tests/contracts/test-input-type-proxy-lock was replaced by tests/vendor/ckb-proxy-locks submodule after user direction.
```

## Task 3: Extend Generic OTX Transaction Builder Layout

**Files:**
- Modify: `tests/src/framework/cobuild.rs`
- Modify: `tests/src/framework/tx.rs`
- Modify: `tests/src/framework/mod.rs` tests

- [ ] **Step 1: Write failing framework tests**

Add tests in `tests/src/framework/mod.rs` for:

```rust
#[test]
fn otx_builder_allows_append_inputs_and_outputs() {
    let otx = OtxBuilder::new()
        .base_input_cells(2)
        .base_output_cells(1)
        .append_input_cells(1)
        .append_output_cells(2)
        .allow_append_inputs()
        .allow_append_outputs()
        .build_with_layout();

    assert_eq!(otx.base_input_cells, 2);
    assert_eq!(otx.base_output_cells, 1);
    assert_eq!(otx.append_input_cells, 1);
    assert_eq!(otx.append_output_cells, 2);
    assert_eq!(otx.otx.append_permissions().as_slice(), &[0b0011]);
}
```

Add a transaction builder test that constructs:

- two base inputs;
- one append input;
- one base output;
- two append outputs;
- one tx-level remainder output;
- one OTX with matching counts.

Assert:

```rust
assert_eq!(tx.inputs().len(), 3);
assert_eq!(tx.outputs().len(), 4);
assert_eq!(tx.witnesses().len(), 2);
```

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p tests --lib --offline otx_builder_allows_append_inputs_and_outputs -- --nocapture
cargo test -p tests --lib --offline otx_transaction_builder_supports_base_append_and_remainder_outputs -- --nocapture
```

Expected: fail because `allow_append_inputs`, `base_output`, `append_input`, and remainder output builder methods do not exist.

- [ ] **Step 3: Implement builder support**

In `tests/src/framework/cobuild.rs`, add:

```rust
pub fn allow_append_inputs(mut self) -> Self {
    self.append_permissions |= 0b0001;
    self
}
```

In `tests/src/framework/tx.rs`, extend `OtxTransactionBuilder`:

```rust
#[derive(Clone, Debug, Default)]
pub struct OtxTransactionBuilder {
    cell_deps: Vec<CellDep>,
    base_inputs: Vec<CellInput>,
    append_inputs: Vec<CellInput>,
    base_outputs: Vec<TestCellOutput>,
    append_outputs: Vec<TestCellOutput>,
    remainder_outputs: Vec<TestCellOutput>,
    otxs: Vec<BuiltOtx>,
}
```

Add methods:

```rust
pub fn append_input(mut self, input: CellInput) -> Self {
    self.append_inputs.push(input);
    self
}

pub fn base_output(mut self, output: TestCellOutput) -> Self {
    self.base_outputs.push(output);
    self
}

pub fn remainder_output(mut self, output: TestCellOutput) -> Self {
    self.remainder_outputs.push(output);
    self
}
```

Update assertions:

```rust
assert!(
    total_base_outputs as usize <= self.base_outputs.len(),
    "OTX base output range exceeds transaction outputs"
);
assert!(
    total_append_inputs as usize <= self.append_inputs.len(),
    "OTX append input range exceeds transaction inputs"
);
```

Build transaction in layout order:

```rust
for input in self.base_inputs {
    builder = builder.input(input);
}
for input in self.append_inputs {
    builder = builder.input(input);
}
for output in self.base_outputs {
    builder = builder.output(output.cell).output_data(output.data.pack());
}
for output in self.append_outputs {
    builder = builder.output(output.cell).output_data(output.data.pack());
}
for output in self.remainder_outputs {
    builder = builder.output(output.cell).output_data(output.data.pack());
}
```

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p tests --lib --offline otx_builder_allows_append_inputs_and_outputs -- --nocapture
cargo test -p tests --lib --offline otx_transaction_builder_supports_base_append_and_remainder_outputs -- --nocapture
cargo test -p tests --lib --offline
```

Expected: all pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add tests/src/framework
git commit -m "test: extend otx fixture transaction layout"
```

**Red/Green Record:**

```text
Red: cargo test -p tests --lib --offline otx_builder_allows_append_inputs_and_outputs -- --nocapture -> failed to compile: missing OtxBuilder::allow_append_inputs
Red: cargo test -p tests --lib --offline otx_transaction_builder_supports_base_append_and_remainder_outputs -- --nocapture -> failed to compile: missing OtxBuilder::allow_append_inputs and OtxTransactionBuilder::append_input
Green: cargo test -p tests --lib --offline otx_builder_allows_append_inputs_and_outputs -- --nocapture -> passed
Green: cargo test -p tests --lib --offline otx_transaction_builder_supports_base_append_and_remainder_outputs -- --nocapture -> passed
Green: make -e -C tests/contracts/limit-order-type build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline -> rebuilt renamed fixture binary required by lib tests
Green: cargo test -p tests --lib --offline -> passed
```

## Task 4: Add UDT Payment Parsing to `limit-order-type`

**Files:**
- Modify: `tests/contracts/limit-order-type/src/types.rs`
- Modify: `tests/contracts/limit-order-type/src/error.rs`

- [ ] **Step 1: Write failing pure tests**

In `types.rs`, add tests for:

```rust
#[test]
fn parse_udt_payment_reads_16_byte_amount() {
    let payment = parse_udt_payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, &30u128.to_le_bytes())
        .expect("udt payment");

    assert_eq!(payment.owner_lock_hash, OWNER_LOCK_HASH);
    assert_eq!(payment.asset_id, REQUESTED_ASSET_ID);
    assert_eq!(payment.amount, 30);
}

#[test]
fn parse_udt_payment_rejects_malformed_amount() {
    assert_eq!(
        parse_udt_payment(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, &[0u8; 15]),
        Err(Error::InvalidSettlementData)
    );
}

#[test]
fn validate_fill_rejects_payment_sum_overflow() {
    let settlements = [
        settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, u64::MAX),
        settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 1),
    ];

    assert_eq!(
        validate_fill(&order_state(), &fill_action(30), &settlements),
        Err(Error::AmountOverflow)
    );
}
```

Add action mismatch tests if missing:

```rust
#[test]
fn validate_fill_rejects_requested_asset_mismatch() {
    let mut action = fill_action(30);
    action.requested_asset_id = [9; 32];

    assert_eq!(
        validate_fill(&order_state(), &action, &[]),
        Err(Error::ActionMismatch)
    );
}

#[test]
fn validate_fill_rejects_offered_amount_mismatch() {
    let mut action = fill_action(30);
    action.offered_amount = 9;

    assert_eq!(
        validate_fill(&order_state(), &action, &[]),
        Err(Error::ActionMismatch)
    );
}

#[test]
fn validate_fill_rejects_action_min_below_required_even_if_paid() {
    let settlements = [settlement(OWNER_LOCK_HASH, REQUESTED_ASSET_ID, 30)];

    assert_eq!(
        validate_fill(&order_state(), &fill_action(29), &settlements),
        Err(Error::InsufficientPayment)
    );
}
```

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-type --offline parse_udt_payment -- --nocapture
cargo test -p limit-order-type --offline validate_fill_rejects_payment_sum_overflow -- --nocapture
```

Expected: fail because `parse_udt_payment` is not implemented.

- [ ] **Step 3: Implement UDT payment parsing**

Keep `SettlementCell` as the normalized payment object, but add:

```rust
pub const UDT_PAYMENT_DATA_LEN: usize = 16;

pub fn parse_udt_payment(
    owner_lock_hash: [u8; 32],
    asset_id: [u8; 32],
    data: &[u8],
) -> Result<SettlementCell, Error> {
    if data.len() != UDT_PAYMENT_DATA_LEN {
        return Err(Error::InvalidSettlementData);
    }
    let amount = read_u128_as_u64(data, 0)?;
    Ok(SettlementCell {
        owner_lock_hash,
        asset_id,
        amount,
    })
}

fn read_u128_as_u64(data: &[u8], offset: usize) -> Result<u64, Error> {
    let mut out = [0u8; 16];
    out.copy_from_slice(&data[offset..offset + 16]);
    let amount = u128::from_le_bytes(out);
    u64::try_from(amount).map_err(|_| Error::AmountOverflow)
}
```

Do not change the action ABI in this task.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p limit-order-type --offline
```

Expected: pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add tests/contracts/limit-order-type/src/types.rs tests/contracts/limit-order-type/src/error.rs
git commit -m "test: add udt payment parsing to limit order type"
```

**Red/Green Record:**

```text
Red:
Green:
```

## Task 5: Count UDT Payment Outputs in `limit-order-type`

**Files:**
- Modify: `tests/contracts/limit-order-type/src/entry.rs`
- Modify: `tests/contracts/limit-order-type/src/types.rs` if helper signatures need adjustment

- [ ] **Step 1: Write failing entry-level unit tests where possible**

Add a pure helper in `entry.rs`:

```rust
pub fn otx_fill_layout(
    origin: &ActionOrigin,
    relation: Option<OtxTypeRelation>,
) -> Result<OtxMessageLayout, Error>
```

Existing tests already cover tx-level rejection and base input relation. Add a unit test for append-only relation:

```rust
#[test]
fn otx_fill_context_rejects_append_input_relation_only() {
    let origin = ActionOrigin::Otx {
        witness_index: 0,
        otx_index: 0,
        layout: layout(),
    };
    let mut relation = relation(false);
    relation.input_type_in_append = true;

    assert_eq!(
        otx_fill_layout(&origin, Some(relation)),
        Err(crate::error::Error::InvalidCobuild)
    );
}
```

The UDT output counting itself will be covered by integration tests in later tasks because it depends on CKB syscalls.

- [ ] **Step 2: Run red if the new relation test fails**

Run:

```bash
cargo test -p limit-order-type --offline otx_fill_context_rejects_append_input_relation_only -- --nocapture
```

Expected: pass if existing implementation already rejects it, or fail until the relation check is strict. Record the observed result.

- [ ] **Step 3: Implement output type-aware settlement collection**

Update imports:

```rust
use ckb_std::high_level::{
    QueryIter, load_cell_data, load_cell_lock_hash, load_cell_type_hash, load_script_hash,
};
```

Update `collect_settlements_from_range` logic:

```rust
let lock_hash = load_cell_lock_hash(index, Source::Output)?;
let type_hash = load_cell_type_hash(index, Source::Output)?;

if data.len() == SETTLEMENT_DATA_LEN {
    settlements.push(parse_settlement_cell(lock_hash, &data)?);
    continue;
}

if let Some(type_hash) = type_hash {
    if data.len() == UDT_PAYMENT_DATA_LEN {
        settlements.push(parse_udt_payment(lock_hash, type_hash, &data)?);
    }
}
```

This deliberately skips ordinary cells with malformed lengths and skips typed cells whose data is not a supported UDT payment length. Wrong UDT type is handled by `validate_fill` not counting it.

- [ ] **Step 4: Run green**

Run:

```bash
cargo test -p limit-order-type --offline
```

Expected: pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add tests/contracts/limit-order-type/src/entry.rs tests/contracts/limit-order-type/src/types.rs
git commit -m "test: count udt payment outputs in limit order type"
```

**Red/Green Record:**

```text
Red:
Green:
```

## Task 6: Add NFT-for-UDT Fixture Builder and Passing Test

**Files:**
- Modify: `tests/src/fixtures/limit_order.rs`
- Modify: `tests/tests/limit_order.rs`

- [ ] **Step 1: Write failing thin integration test**

Add to `tests/tests/limit_order.rs`:

```rust
#[test]
fn limit_order_type_accepts_nft_for_udt_otx_fill() {
    let (fixture, tx) = limit_order_nft_for_udt_case();

    fixture.assert_pass(&tx);
}
```

Import:

```rust
use tests::fixtures::limit_order::{
    failed_txs_count, limit_order_case, limit_order_nft_for_udt_case,
};
```

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p tests --test limit_order --offline limit_order_type_accepts_nft_for_udt_otx_fill -- --nocapture
```

Expected: fail because `limit_order_nft_for_udt_case` does not exist.

- [ ] **Step 3: Implement fixture deployment helpers**

In `tests/src/fixtures/limit_order.rs`, add constants:

```rust
const NFT_ASSET_ID: [u8; 32] = [5; 32];
const UDT_ASSET_ARGS_SEED: [u8; 32] = [6; 32];
const NFT_DATA_NAME: &[u8] = b"order-nft";
```

Add local helpers:

```rust
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

Add private deployment helper functions:

```rust
fn deploy_test_udt_with_owner(
    fixture: &mut CobuildTestFixture,
    owner_lock_hash: [u8; 32],
) -> DeployedScript {
    deploy_data2_script(fixture.context_mut(), "test-udt", owner_lock_hash.to_vec())
}

fn deploy_test_nft(fixture: &mut CobuildTestFixture, args: [u8; 32]) -> DeployedScript {
    deploy_data2_script(fixture.context_mut(), "test-nft", args.to_vec())
}

fn deploy_input_type_proxy_lock(
    fixture: &mut CobuildTestFixture,
    owner_type_hash: [u8; 32],
) -> DeployedScript {
    deploy_data2_script(
        fixture.context_mut(),
        "input-type-proxy-lock",
        owner_type_hash.to_vec(),
    )
}
```

- [ ] **Step 4: Implement `limit_order_nft_for_udt_case`**

Build this transaction:

- Deploy:
  - `limit-order-type`
  - `input-type-proxy-lock` with args = `limit_order.script_hash`
  - `test-nft` with fixed 32-byte args
  - `test-udt` with owner args = issuer lock hash
  - `always_success` for maker, buyer, and issuer where needed
- Base inputs:
  - order input with `requested_asset_id = udt_type_hash`
  - NFT input with lock = proxy lock script and type = `test-nft`
- Append inputs:
  - buyer UDT funding input with amount `30`
- Base outputs:
  - NFT output to buyer with same `test-nft` type and same NFT data
- Append outputs:
  - UDT payment output to maker with amount `30`
- OTX:
  - `base_input_cells(2)`
  - `base_output_cells(1)`
  - `append_input_cells(1)`
  - `append_output_cells(1)`
  - `allow_append_inputs()`
  - `allow_append_outputs()`
  - `FillOrder` action target = `limit_order.script_hash`

Use `typed_output` and `live_input` from framework cells. For UDT funding input, use `live_input` with typed output and 16-byte amount data.

The order input lock can stay `always_success`. The NFT cell lock must be the proxy lock.

- [ ] **Step 5: Build contracts and run green**

Run:

```bash
make -e -C tests/contracts/limit-order-type build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/vendor/ckb-proxy-locks/contracts/input-type-proxy-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-udt build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-nft build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
cargo test -p tests --test limit_order --offline limit_order_type_accepts_nft_for_udt_otx_fill -- --nocapture
```

Expected: pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add tests/src/fixtures/limit_order.rs tests/tests/limit_order.rs
git commit -m "test: add nft for udt limit order fixture"
```

**Red/Green Record:**

```text
Red:
Green:
```

## Task 7: Add NFT-for-UDT Payment Failure Cases

**Files:**
- Modify: `tests/src/fixtures/limit_order.rs`
- Modify: `tests/tests/limit_order.rs`

- [ ] **Step 1: Add scenario enum to fixture**

In `tests/src/fixtures/limit_order.rs`, add:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NftForUdtPaymentCase {
    Valid,
    InsufficientUdt,
    WrongUdt,
    WrongOwner,
    TxLevelRemainderOnly,
}
```

Refactor `limit_order_nft_for_udt_case()` to call:

```rust
pub fn limit_order_nft_for_udt_case_with(
    case: NftForUdtPaymentCase,
) -> (CobuildTestFixture, TransactionView)
```

- [ ] **Step 2: Write failing thin tests**

Add to `tests/tests/limit_order.rs`:

```rust
#[test]
fn limit_order_type_rejects_nft_for_udt_insufficient_udt() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_nft_for_udt_case_with(NftForUdtPaymentCase::InsufficientUdt);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_nft_for_udt_wrong_udt() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_nft_for_udt_case_with(NftForUdtPaymentCase::WrongUdt);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_nft_for_udt_wrong_owner() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_nft_for_udt_case_with(NftForUdtPaymentCase::WrongOwner);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}
```

- [ ] **Step 3: Run red**

Run each new test:

```bash
cargo test -p tests --test limit_order --offline limit_order_type_rejects_nft_for_udt_insufficient_udt -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_nft_for_udt_wrong_udt -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_nft_for_udt_wrong_owner -- --nocapture
```

Expected: fail because the enum/helper variants are not implemented.

- [ ] **Step 4: Implement payment variants**

In the fixture:

- `InsufficientUdt`: append UDT payment amount = `29`.
- `WrongUdt`: create a second UDT type and use it for the append payment output; keep order `requested_asset_id` set to the original UDT type hash.
- `WrongOwner`: append UDT payment lock = buyer lock or another always-success lock.
- `TxLevelRemainderOnly`: append payment amount = `29`; add a tx-level remainder UDT output to owner amount = `1` or `30`. The order type must fail because remainder is outside the current OTX.

Do not add a negative test where an ordinary settlement cell substitutes for the UDT payment. The spec forbids that as a fixture shape for NFT-for-UDT, but the current order ABI has no scenario marker that lets the script distinguish a malicious ordinary settlement cell from the legacy MVP settlement shape. Adding such a test requires a spec update and explicit action/state ABI change.

- [ ] **Step 5: Run green for core payment failures**

Run:

```bash
cargo test -p tests --test limit_order --offline limit_order_type_rejects_nft_for_udt_insufficient_udt -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_nft_for_udt_wrong_udt -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_nft_for_udt_wrong_owner -- --nocapture
```

Expected: all pass and no `failed_txs` are added unless `COBUILD_TEST_DUMP_EXPECTED_FAILURES=1`.

- [ ] **Step 6: Add and green tx-level remainder test**

Add:

```rust
#[test]
fn limit_order_type_does_not_count_tx_level_remainder_udt() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_nft_for_udt_case_with(NftForUdtPaymentCase::TxLevelRemainderOnly);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}
```

Run:

```bash
cargo test -p tests --test limit_order --offline limit_order_type_does_not_count_tx_level_remainder_udt -- --nocapture
```

Expected: pass.

- [ ] **Step 7: Commit**

Run:

```bash
git add tests/src/fixtures/limit_order.rs tests/tests/limit_order.rs
git commit -m "test: cover nft order udt payment failures"
```

**Red/Green Record:**

```text
Red:
Green:
```

## Task 8: Add Cobuild Action and Relation Failure Cases

**Files:**
- Modify: `tests/src/fixtures/limit_order.rs`
- Modify: `tests/tests/limit_order.rs`
- Modify: `tests/src/framework/cobuild.rs`
- Modify: `tests/src/framework/tx.rs`

- [ ] **Step 1: Add fixture variants**

Add:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FillActionCase {
    TxLevelFillOrder,
    OutputTypeTarget,
    OfferedAmountMismatch,
    RequestedAssetMismatch,
    MinRequestedBelowRequired,
    NoRelatedAction,
    MultipleRelatedActions,
    OrderTypeOnlyInAppendInputRelation,
    PaymentInAnotherOtx,
}
```

Add:

```rust
pub fn limit_order_action_failure_case(
    case: FillActionCase,
) -> (CobuildTestFixture, TransactionView)
```

- [ ] **Step 2: Write failing thin tests**

Add one test per high-value case:

```rust
#[test]
fn limit_order_type_rejects_tx_level_fill_order() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::TxLevelFillOrder);

    fixture.assert_type_script_exit(&tx, 0, 12);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_output_type_fill_order_target() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::OutputTypeTarget);

    fixture.assert_type_script_exit(&tx, 0, 12);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_offered_amount_mismatch() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::OfferedAmountMismatch);

    fixture.assert_type_script_exit(&tx, 0, 10);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_requested_asset_mismatch() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::RequestedAssetMismatch);

    fixture.assert_type_script_exit(&tx, 0, 10);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_min_requested_below_required() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::MinRequestedBelowRequired);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_payment_in_another_otx() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::PaymentInAnotherOtx);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}
```

If `OutputTypeTarget` fails before the `limit-order-type` script runs, update the assertion to the exact observed Core/type failure and record the reason in the Red/Green Record.

- [ ] **Step 3: Run red**

Run:

```bash
cargo test -p tests --test limit_order --offline limit_order_type_rejects_tx_level_fill_order -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_output_type_fill_order_target -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_offered_amount_mismatch -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_requested_asset_mismatch -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_min_requested_below_required -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_payment_in_another_otx -- --nocapture
```

Expected: fail because fixture variants are missing or behavior not implemented.

- [ ] **Step 4: Implement variants minimally**

Implementation guidance:

- `TxLevelFillOrder`: add a framework method that appends a tx-level Cobuild message witness. Do not hand-build raw witnesses in `tests/tests/limit_order.rs`.
- Add to `OtxTransactionBuilder`:

```rust
tx_level_message: Option<cobuild_types::entity::core::Message>,
```

Add:

```rust
pub fn tx_level_message(mut self, message: cobuild_types::entity::core::Message) -> Self {
    self.tx_level_message = Some(message);
    self
}
```

In `build`, when `tx_level_message` is `Some(message)`, append this witness before OTX witnesses:

```rust
let witness = WitnessLayout::from(
    cobuild_types::entity::core::SighashAll::new_builder()
        .seal(Vec::<u8>::new())
        .message(message)
        .build(),
);
builder = builder.witness(Bytes::copy_from_slice(witness.as_slice()).pack());
```

Add imports for `SighashAll`, `WitnessLayout`, and `Entity` as needed in `tests/src/framework/tx.rs`.
- `OutputTypeTarget`: add `output_type_action(script_hash)` to `CobuildMessageBuilder`:

```rust
pub fn output_type_action(mut self, script_hash: [u8; 32]) -> Self {
    self.script_hash = script_hash;
    self.script_role = 2;
    self
}
```

- `OfferedAmountMismatch`: set action offered amount to `9`.
- `RequestedAssetMismatch`: set action requested asset to another UDT hash.
- `MinRequestedBelowRequired`: set action minimum to `29` while append payment is `30`.
- `PaymentInAnotherOtx`: build two OTXs with non-overlapping output ranges; current order's OTX append output pays `29`, another OTX append output pays `1` or `30` to owner. The order type must fail.

- [ ] **Step 5: Run green**

Run:

```bash
cargo test -p tests --test limit_order --offline limit_order_type_rejects_tx_level_fill_order -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_output_type_fill_order_target -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_offered_amount_mismatch -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_requested_asset_mismatch -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_min_requested_below_required -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_payment_in_another_otx -- --nocapture
```

Expected: all pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add tests/src/framework tests/src/fixtures/limit_order.rs tests/tests/limit_order.rs
git commit -m "test: cover limit order action failures"
```

**Red/Green Record:**

```text
Red:
Green:
```

## Task 9: Final Verification

**Files:**
- All files changed by prior tasks.

- [ ] **Step 1: Format**

Run:

```bash
cargo fmt
```

Expected: completes successfully.

- [ ] **Step 2: Build required contracts**

Run:

```bash
make -e -C tests/contracts/limit-order-type build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/vendor/ckb-proxy-locks/contracts/input-type-proxy-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-udt build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-nft build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
```

Expected: all build successfully.

- [ ] **Step 3: Run required tests**

Run:

```bash
cargo test -p tests --test limit_order --offline
cargo test -p tests --lib --offline
cargo test --workspace --offline
```

Expected: all pass.

- [ ] **Step 4: Check formatting and diff hygiene**

Run:

```bash
cargo fmt --check
git diff --check
git status --short
```

Expected:

- `cargo fmt --check` passes.
- `git diff --check` prints no output.
- `git status --short` shows only intended tracked changes before final commit, then no output after commit.

- [ ] **Step 5: Confirm failed_txs policy**

Run:

```bash
find tests/failed_txs -maxdepth 1 -type f 2>/dev/null | wc -l
```

Record whether the count changed. Expected failure tests should not dump new files unless `COBUILD_TEST_DUMP_EXPECTED_FAILURES=1`.

- [ ] **Step 6: Final commit**

If prior tasks committed everything, this may be unnecessary. If there are remaining changes:

```bash
git add docs/superpowers/plans/2026-06-08-limit-order-type-nft-for-udt-plan.md tests Cargo.toml
git commit -m "test: support nft for udt limit order fills"
```

**Verification Record:**

```text
cargo fmt:
make limit-order-type:
make vendor input-type-proxy-lock:
make test-udt:
make test-nft:
cargo test -p tests --test limit_order --offline:
cargo test -p tests --lib --offline:
cargo test --workspace --offline:
cargo fmt --check:
git diff --check:
failed_txs:
```

## Plan Self-Review

- Spec coverage:
  - `limit-order-type` rename: Task 1.
  - NFT custody via input-type proxy lock: Task 2 and Task 6.
  - Real `test-udt` payment cell for NFT-for-UDT: Tasks 4, 5, 6, 7.
  - OTX-scoped settlement and tx-level remainder exclusion: Tasks 3, 7, 8.
  - One OTX / one order MVP: existing script checks plus Tasks 7 and 8.
  - Thin `tests/tests/limit_order.rs`: Tasks 6-8 explicitly keep tests thin.
  - No production crates or cobuild-core changes: File Structure forbids them.
- Placeholder scan: no `TBD`/`TODO` placeholders are intentional task content.
- Type consistency:
  - Contract package and binary name are `limit-order-type`.
  - Proxy lock source is vendored at `tests/vendor/ckb-proxy-locks`.
  - Fixture public functions are `limit_order_nft_for_udt_case`, `limit_order_nft_for_udt_case_with`, and `limit_order_action_failure_case`.
