# Limit Order CreateOrder Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the tests-only `limit-order-type` fixture with `CreateOrder` validation for type-id NFT escrow into the input-type proxy lock, while updating `FillOrder` to the new one-NFT ABI.

**Architecture:** `limit-order-type` becomes a dual-mode type script selected by current type group shape: `0 input / 1 output` validates `CreateOrder`, and `1 input / 0 output` validates `FillOrder`. Create validates type-id args, state/action equality, and a transaction output NFT locked by `input-type-proxy-lock(args = current_order_type_hash)`; fill keeps OTX-scoped payment validation only.

**Tech Stack:** Rust 2024, `ckb-std` 1.1 type-id checks, `cobuild-core`, `cobuild-types`, `ckb-testtool`, tests-only fixture contracts, offline Cargo, CKB contract Makefiles.

---

## Source Requirements

Read these before executing:

- `docs/superpowers/specs/2026-06-08-limit-order-create-order-design.md`
- `docs/superpowers/specs/2026-06-07-limit-order-test-type-script-spec.md`
- `tests/contracts/limit-order-type/src/types.rs`
- `tests/contracts/limit-order-type/src/entry.rs`
- `tests/contracts/limit-order-type/src/error.rs`
- `tests/src/fixtures/limit_order.rs`
- `tests/src/fixtures/limit_order/nft_for_udt.rs`
- `tests/src/framework/tx.rs`
- `tests/src/framework/cobuild.rs`
- `tests/src/framework/contracts.rs`
- `tests/src/framework/cells.rs`
- `tests/tests/limit_order.rs`
- `tests/vendor/ckb-proxy-locks/contracts/input-type-proxy-lock`
- `tests/contracts/test-nft/src/entry.rs`
- `tests/contracts/test-udt/src/entry.rs`

Start execution with:

```bash
git status --short
```

Expected: no output. If dirty, inspect first and do not overwrite unrelated changes.

## File Structure

Modify:

- `docs/superpowers/plans/2026-06-08-limit-order-create-order-plan.md`
  - Record red/green results after every task.
- `xtask/Cargo.toml`
  - Add `ckb-hash = "1.1"` and `hex = "0.4"` if not already available for generating proxy lock code hash source.
- `xtask/src/main.rs`
  - Add `proxy-lock-code-hash limit-order-type` command that hashes the vendored proxy lock binary and writes a Rust source constant.
- `tests/contracts/limit-order-type/Makefile`
  - Add a pre-build generation step for the proxy lock code hash constant.
- `tests/contracts/limit-order-type/src/lib.rs`
  - Expose generated proxy lock code hash module.
- `tests/contracts/limit-order-type/src/types.rs`
  - Replace legacy order/action ABI with CreateOrder tag `1`, FillOrder tag `2`, 104-byte order state, and new validation helpers.
- `tests/contracts/limit-order-type/src/entry.rs`
  - Split create/fill validation by group shape.
  - Use `ckb_std::type_id::check_type_id(0, 32)` for create mode.
  - Scan transaction outputs for NFT escrow into the expected proxy lock.
- `tests/contracts/limit-order-type/src/error.rs`
  - Add only necessary tests-only errors if existing variants cannot express create/type-id failures.
- `tests/src/framework/tx.rs`
  - Add a general tx-level transaction builder path that can build SighashAll transactions without OTX.
- `tests/src/framework/assertions.rs`
  - Add output type script exit assertion support for create-mode failures.
- `tests/src/framework/fixture.rs`
  - Expose output type script exit assertion on `CobuildTestFixture`.
- `tests/src/framework/mod.rs`
  - Add focused framework tests for tx-level message-only transactions and output type assertions.
- `tests/src/fixtures/limit_order.rs`
  - Update order data/action encoding helpers to new ABI.
  - Keep legacy ordinary settlement case working under new state shape.
- `tests/src/fixtures/limit_order/nft_for_udt.rs`
  - Add `CreateOrder` scenario builders.
  - Update fill scenarios to new ABI.
  - Keep this file under the fixture line-count limit.
- `tests/tests/limit_order.rs`
  - Add thin create tests and rename the obsolete offered-amount mismatch fill test.
- `tests/src/tests.rs`
  - No planned changes; run fixture boundary tests to enforce line-count and layering.

Do not modify:

- `contracts/cobuild-otx-lock`
- `crates/cobuild-core`
- `crates/cobuild-types`
- public action schemas
- production contracts outside `tests/contracts`

## Red/Green Log Discipline

For every task, update that task's **Red/Green Record** section:

```text
Red: <command> -> <expected failing output>
Green: <command> -> <passing output>
```

Expected-failure tests must not dump tracked `tests/failed_txs`. If `COBUILD_TEST_DUMP_EXPECTED_FAILURES` is not `1`, assert the ignored failed tx file count does not change.

## Task 1: Add Tx-Level Transaction Builder and Output Assertion Support

**Files:**
- Modify: `tests/src/framework/tx.rs`
- Modify: `tests/src/framework/assertions.rs`
- Modify: `tests/src/framework/fixture.rs`
- Modify: `tests/src/framework/mod.rs`

- [ ] **Step 1: Write failing framework test**

Add this test in `tests/src/framework/mod.rs`:

```rust
#[test]
fn tx_builder_supports_sighash_all_message_without_otx() {
    let mut fixture = CobuildTestFixture::new();
    let lock = fixture.deploy_always_success();
    let input = live_input(
        fixture.context_mut(),
        normal_output(lock.script.clone(), 1_000),
        Vec::new(),
    );
    let output = TestCellOutput::new(normal_output(lock.script, 900), Vec::new());
    let message = CobuildMessageBuilder::new()
        .output_type_action([9; 32])
        .action_data(vec![1])
        .build();

    let tx = OtxTransactionBuilder::new()
        .allow_no_otx()
        .base_input(input)
        .base_output(output)
        .tx_level_message(message)
        .build();

    assert_eq!(tx.inputs().len(), 1);
    assert_eq!(tx.outputs().len(), 1);
    assert_eq!(tx.witnesses().len(), 1);
}
```

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p tests --lib --offline tx_builder_supports_sighash_all_message_without_otx -- --nocapture
```

Expected: compile failure because `OtxTransactionBuilder::allow_no_otx` does not exist, or runtime panic because the builder requires an OTX.

- [ ] **Step 3: Add failing output type assertion test**

Add this test in `tests/src/framework/mod.rs`:

```rust
#[test]
fn output_type_script_exit_assertion_matches_index_and_exit_code() {
    let error = ScriptError::ValidationFailure("by convention".to_owned(), 14)
        .output_type_script(0)
        .into();

    super::assertions::assert_output_type_script_exit_result(Err(error), 0, 14);
}
```

Run:

```bash
cargo test -p tests --lib --offline output_type_script_exit_assertion_matches_index_and_exit_code -- --nocapture
```

Expected: compile failure because `assert_output_type_script_exit_result` does not exist.

- [ ] **Step 4: Implement minimal builder support**

In `tests/src/framework/tx.rs`, add:

```rust
allow_no_otx: bool,
```

to `OtxTransactionBuilder`.

Add:

```rust
pub fn allow_no_otx(mut self) -> Self {
    self.allow_no_otx = true;
    self
}
```

Change the start of `build` to:

```rust
assert!(
    self.allow_no_otx || !self.otxs.is_empty(),
    "OTX transaction requires one Otx unless allow_no_otx is set"
);
assert!(
    !self.base_inputs.is_empty(),
    "transaction requires non-zero base inputs"
);
if !self.allow_no_otx {
    assert!(
        self.otxs.iter().all(|otx| otx.base_input_cells > 0),
        "each OTX requires non-zero base inputs"
    );
}
```

Only append the `OtxStart` witness when `!self.otxs.is_empty()`:

```rust
if !self.otxs.is_empty() {
    builder = builder.witness(
        otx_start_witness(
            start_input_cell,
            start_output_cell,
            start_cell_deps,
            start_header_deps,
        )
        .pack(),
    );
}
```

Keep tx-level `SighashAll` witness before OTX witnesses.

- [ ] **Step 5: Implement output type assertions**

In `tests/src/framework/assertions.rs`, add:

```rust
pub fn assert_output_type_script_exit(
    context: &Context,
    tx: &TransactionView,
    output_index: usize,
    code: i8,
) {
    let result = context.verify_tx(tx, MAX_CYCLES);
    if result.is_err() && dump_expected_failures() {
        let _ = verify_and_dump_failed_tx(context, tx, MAX_CYCLES);
    }
    assert_output_type_script_exit_result(result, output_index, code);
}

pub fn assert_output_type_script_exit_result(
    result: Result<Cycle, Error>,
    output_index: usize,
    code: i8,
) {
    assert_script_exit_result(result, format!("Outputs[{output_index}].Type"), code);
}
```

In `tests/src/framework/fixture.rs`, add:

```rust
pub fn assert_output_type_script_exit(&self, tx: &TransactionView, output_index: usize, code: i8) {
    assertions::assert_output_type_script_exit(&self.context, tx, output_index, code);
}
```

- [ ] **Step 6: Run green**

Run:

```bash
cargo test -p tests --lib --offline tx_builder_supports_sighash_all_message_without_otx -- --nocapture
cargo test -p tests --lib --offline output_type_script_exit_assertion_matches_index_and_exit_code -- --nocapture
cargo test -p tests --lib --offline
```

Expected: both pass.

- [ ] **Step 7: Commit**

Run:

```bash
git add tests/src/framework/tx.rs tests/src/framework/assertions.rs tests/src/framework/fixture.rs tests/src/framework/mod.rs
git commit -m "test: support tx level fixture transactions"
```

**Red/Green Record:**

```text
Red:
cargo test -p tests --lib --offline tx_builder_supports_sighash_all_message_without_otx -- --nocapture -> failed to compile: missing OtxTransactionBuilder::allow_no_otx
cargo test -p tests --lib --offline output_type_script_exit_assertion_matches_index_and_exit_code -- --nocapture -> failed to compile: missing assert_output_type_script_exit_result, with allow_no_otx still missing from first red test
Green:
cargo test -p tests --lib --offline tx_builder_supports_sighash_all_message_without_otx -- --nocapture -> passed: 1 test
cargo test -p tests --lib --offline output_type_script_exit_assertion_matches_index_and_exit_code -- --nocapture -> passed: 1 test
cargo test -p tests --lib --offline -> passed: 23 tests
Review fix red:
cargo test -p tests --lib --offline tx_builder_still_rejects_zero_base_inputs_when_no_otx_is_allowed -- --nocapture -> failed: test did not panic, allow_no_otx bypassed OTX base input validation
Review fix green:
cargo test -p tests --lib --offline tx_builder_still_rejects_zero_base_inputs_when_no_otx_is_allowed -- --nocapture -> passed: 1 should-panic test
cargo test -p tests --lib --offline tx_builder_supports_sighash_all_message_without_otx -- --nocapture -> passed: 1 test
cargo test -p tests --lib --offline output_type_script_exit_assertion_matches_index_and_exit_code -- --nocapture -> passed: 1 test
cargo test -p tests --lib --offline -> passed: 24 tests
```

## Task 2: Generate Proxy Lock Code Hash Constant

**Files:**
- Modify: `xtask/Cargo.toml`
- Modify: `xtask/src/main.rs`
- Modify: `tests/contracts/limit-order-type/Makefile`
- Modify: `tests/contracts/limit-order-type/src/lib.rs`
- Create: `tests/contracts/limit-order-type/src/generated_proxy_lock.rs`

- [ ] **Step 1: Write failing generated-constant test**

Add this test in `tests/contracts/limit-order-type/src/types.rs` tests module:

```rust
#[test]
fn generated_proxy_lock_code_hash_is_32_bytes() {
    assert_eq!(crate::generated_proxy_lock::INPUT_TYPE_PROXY_LOCK_CODE_HASH.len(), 32);
    assert_ne!(
        crate::generated_proxy_lock::INPUT_TYPE_PROXY_LOCK_CODE_HASH,
        [0u8; 32]
    );
}
```

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-type --offline generated_proxy_lock_code_hash_is_32_bytes -- --nocapture
```

Expected: compile failure because `generated_proxy_lock` does not exist.

- [ ] **Step 3: Add xtask command**

In `xtask/Cargo.toml`, add:

```toml
ckb-hash = "1.1"
hex = "0.4"
```

In `xtask/src/main.rs`, add command matching:

```rust
[cmd, target] if cmd == "proxy-lock-code-hash" && target == "limit-order-type" => {
    write_limit_order_proxy_lock_hash()
}
```

Keep existing codegen command unchanged.

Add:

```rust
fn write_limit_order_proxy_lock_hash() -> Result<()> {
    let root = workspace_root()?;
    let binary = root.join("build/debug/input-type-proxy-lock");
    let output = root.join("tests/contracts/limit-order-type/src/generated_proxy_lock.rs");
    let data = fs::read(&binary)
        .with_context(|| format!("read proxy lock binary {}", binary.display()))?;
    let hash = ckb_hash::blake2b_256(&data);
    let source = format!(
        "pub const INPUT_TYPE_PROXY_LOCK_CODE_HASH: [u8; 32] = {};\n",
        rust_byte_array(&hash)
    );
    fs::write(&output, source)
        .with_context(|| format!("write generated proxy lock hash {}", output.display()))?;
    Ok(())
}

fn rust_byte_array(bytes: &[u8; 32]) -> String {
    let items = bytes
        .iter()
        .map(|byte| format!("0x{byte:02x}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{items}]")
}
```

Update usage error to:

```rust
bail!("usage: cargo run -p xtask -- codegen cobuild-types [--check] | proxy-lock-code-hash limit-order-type")
```

- [ ] **Step 4: Wire generated module**

In `tests/contracts/limit-order-type/src/lib.rs`, add:

```rust
pub mod generated_proxy_lock;
```

Create initial file:

```rust
pub const INPUT_TYPE_PROXY_LOCK_CODE_HASH: [u8; 32] = [0u8; 32];
```

In `tests/contracts/limit-order-type/Makefile`, add before `cargo build` in `build`:

```make
	@if [ -f "$(TOP)/build/debug/input-type-proxy-lock" ]; then \
		cargo run -p xtask -- proxy-lock-code-hash limit-order-type; \
	else \
		echo "Skipping proxy lock hash generation; $(TOP)/build/debug/input-type-proxy-lock is missing"; \
	fi
```

Do not make the contract build invoke network or build the vendored proxy itself.

- [ ] **Step 5: Build proxy lock and generate constant**

Run:

```bash
CARGO_TARGET_DIR=/home/xcshuan/contracts/ckb/cobuild-otx-contracts/target make -e -C tests/vendor/ckb-proxy-locks/contracts/input-type-proxy-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CUSTOM_RUSTFLAGS='-C debug-assertions' CARGO_ARGS=--offline
cargo run -p xtask --offline -- proxy-lock-code-hash limit-order-type
```

Expected: generated file changes from all-zero to a non-zero 32-byte constant.

- [ ] **Step 6: Run green**

Run:

```bash
cargo test -p limit-order-type --offline generated_proxy_lock_code_hash_is_32_bytes -- --nocapture
make -e -C tests/contracts/limit-order-type build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
```

Expected: both pass. The Makefile build should regenerate the same constant and leave no diff after generation.

- [ ] **Step 7: Commit**

Run:

```bash
git add xtask/Cargo.toml xtask/src/main.rs tests/contracts/limit-order-type/Makefile tests/contracts/limit-order-type/src/lib.rs tests/contracts/limit-order-type/src/generated_proxy_lock.rs tests/contracts/limit-order-type/src/types.rs Cargo.lock
git commit -m "test: generate proxy lock hash for limit order"
```

**Red/Green Record:**

```text
Red:
cargo test -p limit-order-type --offline generated_proxy_lock_code_hash_is_32_bytes -- --nocapture -> failed as expected with E0433: could not find `generated_proxy_lock` in the crate root.
Green:
CARGO_TARGET_DIR=/home/xcshuan/contracts/ckb/cobuild-otx-contracts/target make -e -C tests/vendor/ckb-proxy-locks/contracts/input-type-proxy-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CUSTOM_RUSTFLAGS='-C debug-assertions' CARGO_ARGS=--offline -> passed; built and copied input-type-proxy-lock with one existing dead_code warning.
cargo run -p xtask --offline -- proxy-lock-code-hash limit-order-type -> passed; generated non-zero 32-byte proxy lock code hash.
cargo test -p limit-order-type --offline generated_proxy_lock_code_hash_is_32_bytes -- --nocapture -> passed; 1 test passed, 22 filtered out.
make -e -C tests/contracts/limit-order-type build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline -> passed; regenerated stable constant and built limit-order-type.
Review fix red:
grep -n "cargo run --offline -p xtask -- proxy-lock-code-hash limit-order-type" tests/contracts/limit-order-type/Makefile -> failed: no match, Makefile xtask generation was not forced offline
Review fix green:
grep -n "cargo run --offline -p xtask -- proxy-lock-code-hash limit-order-type" tests/contracts/limit-order-type/Makefile -> passed: Makefile uses offline xtask invocation
make -e -C tests/contracts/limit-order-type build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline -> passed; offline xtask generation and limit-order-type build passed
```

## Task 3: Replace Order and Action ABI

**Files:**
- Modify: `tests/contracts/limit-order-type/src/types.rs`
- Modify: `tests/src/fixtures/limit_order.rs`
- Modify: `tests/src/framework/mod.rs`

- [ ] **Step 1: Write failing pure ABI tests**

In `tests/contracts/limit-order-type/src/types.rs`, replace the legacy tests for order and fill action with:

```rust
#[test]
fn parse_order_state_reads_nft_order_fields() {
    let order = parse_order_state(&order_data(30)).expect("order data");

    assert_eq!(order.owner_lock_hash, OWNER_LOCK_HASH);
    assert_eq!(order.offered_nft_type_hash, OFFERED_ASSET_ID);
    assert_eq!(order.requested_asset_id, REQUESTED_ASSET_ID);
    assert_eq!(order.min_requested_amount, 30);
}

#[test]
fn parse_create_order_action_reads_state_payload() {
    let action = parse_limit_order_action(&create_action_data(30)).expect("create action");

    assert_eq!(
        action,
        LimitOrderAction::Create(CreateOrderAction {
            owner_lock_hash: OWNER_LOCK_HASH,
            offered_nft_type_hash: OFFERED_ASSET_ID,
            requested_asset_id: REQUESTED_ASSET_ID,
            min_requested_amount: 30,
        })
    );
}

#[test]
fn parse_fill_order_action_reads_requested_asset_and_amount() {
    let action = parse_limit_order_action(&fill_action_data(30)).expect("fill action");

    assert_eq!(
        action,
        LimitOrderAction::Fill(FillOrderAction {
            requested_asset_id: REQUESTED_ASSET_ID,
            min_requested_amount: 30,
        })
    );
}

#[test]
fn validate_create_accepts_matching_state() {
    let order = order_state(30);
    let action = create_action(30);

    assert_eq!(validate_create(&order, &action), Ok(()));
}

#[test]
fn validate_create_rejects_state_mismatch() {
    let order = order_state(30);
    let mut action = create_action(30);
    action.min_requested_amount = 31;

    assert_eq!(validate_create(&order, &action), Err(Error::ActionMismatch));
}
```

Update test helpers in the same module to:

```rust
fn order_data(min_requested_amount: u64) -> Vec<u8> {
    let mut data = Vec::new();
    data.extend_from_slice(&OWNER_LOCK_HASH);
    data.extend_from_slice(&OFFERED_ASSET_ID);
    data.extend_from_slice(&REQUESTED_ASSET_ID);
    data.extend_from_slice(&min_requested_amount.to_le_bytes());
    data
}

fn create_action_data(min_requested_amount: u64) -> Vec<u8> {
    let mut data = Vec::new();
    data.push(CREATE_ORDER_TAG);
    data.extend_from_slice(&OWNER_LOCK_HASH);
    data.extend_from_slice(&OFFERED_ASSET_ID);
    data.extend_from_slice(&REQUESTED_ASSET_ID);
    data.extend_from_slice(&min_requested_amount.to_le_bytes());
    data
}

fn fill_action_data(min_requested_amount: u64) -> Vec<u8> {
    let mut data = Vec::new();
    data.push(FILL_ORDER_TAG);
    data.extend_from_slice(&REQUESTED_ASSET_ID);
    data.extend_from_slice(&min_requested_amount.to_le_bytes());
    data
}
```

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-type --offline parse_create_order_action_reads_state_payload -- --nocapture
cargo test -p limit-order-type --offline parse_fill_order_action_reads_requested_asset_and_amount -- --nocapture
```

Expected: compile failure because `CREATE_ORDER_TAG`, `LimitOrderAction`, `CreateOrderAction`, and `validate_create` do not exist or old parser expects legacy lengths.

- [ ] **Step 3: Implement new ABI**

In `types.rs`, replace constants and structs:

```rust
pub const ORDER_DATA_LEN: usize = 104;
pub const SETTLEMENT_DATA_LEN: usize = 40;
pub const UDT_PAYMENT_DATA_LEN: usize = 16;
pub const CREATE_ORDER_TAG: u8 = 1;
pub const FILL_ORDER_TAG: u8 = 2;
const CREATE_ORDER_DATA_LEN: usize = 105;
const FILL_ORDER_DATA_LEN: usize = 41;
```

Use:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OrderState {
    pub owner_lock_hash: [u8; 32],
    pub offered_nft_type_hash: [u8; 32],
    pub requested_asset_id: [u8; 32],
    pub min_requested_amount: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CreateOrderAction {
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LimitOrderAction {
    Create(CreateOrderAction),
    Fill(FillOrderAction),
}
```

Implement parsers:

```rust
pub fn parse_order_state(data: &[u8]) -> Result<OrderState, Error> {
    if data.len() != ORDER_DATA_LEN {
        return Err(Error::InvalidOrderData);
    }
    Ok(OrderState {
        owner_lock_hash: read_bytes32(data, 0),
        offered_nft_type_hash: read_bytes32(data, 32),
        requested_asset_id: read_bytes32(data, 64),
        min_requested_amount: read_u64(data, 96),
    })
}

pub fn parse_limit_order_action(data: &[u8]) -> Result<LimitOrderAction, Error> {
    let Some((&tag, _)) = data.split_first() else {
        return Err(Error::InvalidActionData);
    };
    match tag {
        CREATE_ORDER_TAG => parse_create_order_action(data).map(LimitOrderAction::Create),
        FILL_ORDER_TAG => parse_fill_order_action(data).map(LimitOrderAction::Fill),
        _ => Err(Error::UnsupportedAction),
    }
}

pub fn parse_create_order_action(data: &[u8]) -> Result<CreateOrderAction, Error> {
    if data.len() != CREATE_ORDER_DATA_LEN {
        return Err(Error::InvalidActionData);
    }
    Ok(CreateOrderAction {
        owner_lock_hash: read_bytes32(data, 1),
        offered_nft_type_hash: read_bytes32(data, 33),
        requested_asset_id: read_bytes32(data, 65),
        min_requested_amount: read_u64(data, 97),
    })
}

pub fn parse_fill_order_action(data: &[u8]) -> Result<FillOrderAction, Error> {
    if data.len() != FILL_ORDER_DATA_LEN {
        return Err(Error::InvalidActionData);
    }
    Ok(FillOrderAction {
        requested_asset_id: read_bytes32(data, 1),
        min_requested_amount: read_u64(data, 33),
    })
}
```

Update validation:

```rust
pub fn validate_create(order: &OrderState, action: &CreateOrderAction) -> Result<(), Error> {
    if order.owner_lock_hash != action.owner_lock_hash
        || order.offered_nft_type_hash != action.offered_nft_type_hash
        || order.requested_asset_id != action.requested_asset_id
        || order.min_requested_amount != action.min_requested_amount
    {
        return Err(Error::ActionMismatch);
    }
    Ok(())
}

pub fn validate_fill(
    order: &OrderState,
    action: &FillOrderAction,
    settlements: &[SettlementCell],
) -> Result<(), Error> {
    if action.requested_asset_id != order.requested_asset_id {
        return Err(Error::ActionMismatch);
    }
    if action.min_requested_amount < order.min_requested_amount {
        return Err(Error::InsufficientPayment);
    }
    // keep the existing paid summation, comparing owner_lock_hash and requested_asset_id
}
```

Remove `required_requested_amount`.

- [ ] **Step 4: Update fixture encoding helpers**

In `tests/src/fixtures/limit_order.rs`, update `LimitOrderState`:

```rust
pub struct LimitOrderState {
    pub owner_lock_hash: [u8; 32],
    pub offered_nft_type_hash: [u8; 32],
    pub requested_asset_id: [u8; 32],
    pub min_requested_amount: u64,
}
```

Update `order_data` to 104 bytes and add:

```rust
pub fn create_order_action_data(order: LimitOrderState) -> Vec<u8> {
    let mut data = Vec::with_capacity(105);
    data.push(CREATE_ORDER_TAG);
    data.extend_from_slice(&order.owner_lock_hash);
    data.extend_from_slice(&order.offered_nft_type_hash);
    data.extend_from_slice(&order.requested_asset_id);
    data.extend_from_slice(&order.min_requested_amount.to_le_bytes());
    data
}
```

Update `LimitOrderCobuildMessageExt`:

```rust
fn limit_order_create(self, order: LimitOrderState) -> Self;
fn limit_order_fill(self, requested_asset_id: [u8; 32], min_requested_amount: u64) -> Self;
```

Update implementation:

```rust
fn limit_order_fill(self, requested_asset_id: [u8; 32], min_requested_amount: u64) -> Self {
    let mut data = Vec::with_capacity(41);
    data.push(FILL_ORDER_TAG);
    data.extend_from_slice(&requested_asset_id);
    data.extend_from_slice(&min_requested_amount.to_le_bytes());
    self.action_data(data)
}
```

Update `LimitOrderBuilder` fields and setters:

- Remove `order_id`, `offered_remaining`, `min_requested_per_offered`, `nonce`.
- Rename `offered_asset_id` to `offered_nft_type_hash`.
- Add `min_requested_amount`.
- Keep compatibility setters only if tests still use them, but make them write the new fields.

- [ ] **Step 5: Update framework tests**

Update `tests/src/framework/mod.rs`:

```rust
let order = LimitOrderState {
    owner_lock_hash: [2; 32],
    offered_nft_type_hash: [3; 32],
    requested_asset_id: [4; 32],
    min_requested_amount: 30,
};

let data = order_data(order);
assert_eq!(data.len(), 104);
assert_eq!(&data[0..32], &[2; 32]);
assert_eq!(&data[32..64], &[3; 32]);
assert_eq!(&data[64..96], &[4; 32]);
assert_eq!(&data[96..104], &30u64.to_le_bytes());
```

Update the fill message test:

```rust
let message = CobuildMessageBuilder::new()
    .input_type_action([9; 32])
    .limit_order_fill([4; 32], 30)
    .build();
```

- [ ] **Step 6: Run green**

Run:

```bash
cargo test -p limit-order-type --offline
cargo test -p tests --lib --offline
```

Expected: both pass.

- [ ] **Step 7: Commit**

Run:

```bash
git add tests/contracts/limit-order-type/src/types.rs tests/src/fixtures/limit_order.rs tests/src/framework/mod.rs
git commit -m "test: update limit order action abi"
```

**Red/Green Record:**

```text
Red:
- cargo test -p limit-order-type --offline parse_create_order_action_reads_state_payload -- --nocapture -> failed to compile as expected; missing CREATE_ORDER_TAG, CreateOrderAction, LimitOrderAction, parse_limit_order_action, parse_create_order_action, validate_create, and new order/fill fields.
- cargo test -p limit-order-type --offline parse_fill_order_action_reads_requested_asset_and_amount -- --nocapture -> failed to compile as expected with the same missing new ABI symbols and old FillOrderAction fields.
Green:
- cargo test -p limit-order-type --offline -> passed; 23 unit tests, 0 doctests.
- cargo test -p tests --lib --offline -> passed; 24 unit tests.
```

## Task 4: Split Entry Validation by Create and Fill Group Shape

**Files:**
- Modify: `tests/contracts/limit-order-type/src/entry.rs`
- Modify: `tests/contracts/limit-order-type/src/error.rs`

- [ ] **Step 1: Write failing pure group-shape and type-id error tests**

In `entry.rs` tests module, add a pure helper:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OrderMode {
    Create,
    Fill,
}

pub fn order_mode(input_count: usize, output_count: usize) -> Result<OrderMode, Error> {
    match (input_count, output_count) {
        (0, 1) => Ok(OrderMode::Create),
        (1, 0) => Ok(OrderMode::Fill),
        _ => Err(Error::InvalidOrderData),
    }
}
```

Add tests:

```rust
#[test]
fn order_mode_accepts_create_shape() {
    assert_eq!(order_mode(0, 1), Ok(OrderMode::Create));
}

#[test]
fn order_mode_accepts_fill_shape() {
    assert_eq!(order_mode(1, 0), Ok(OrderMode::Fill));
}

#[test]
fn order_mode_rejects_update_or_empty_shapes() {
    assert_eq!(order_mode(1, 1), Err(Error::InvalidOrderData));
    assert_eq!(order_mode(0, 0), Err(Error::InvalidOrderData));
    assert_eq!(order_mode(2, 0), Err(Error::InvalidOrderData));
}

#[test]
fn type_id_sys_error_maps_to_stable_exit_code() {
    assert_eq!(Error::from(ckb_std::error::SysError::TypeIDError), Error::TypeId);
    assert_eq!(i8::from(Error::TypeId), 14);
}
```

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-type --offline order_mode -- --nocapture
```

Expected: compile failure because `order_mode`/`OrderMode` and `Error::TypeId` do not exist.

- [ ] **Step 3: Add TypeId error**

In `tests/contracts/limit-order-type/src/error.rs`, add:

```rust
TypeId = 14,
```

Map syscall errors:

```rust
SysError::TypeIDError => Self::TypeId,
```

- [ ] **Step 4: Implement group-shape split**

In `entry.rs`, replace `single_input_order()` and `require_no_order_output()` based flow with:

```rust
pub fn main() -> Result<(), Error> {
    let current_type_hash = load_script_hash()?;
    let plan =
        CobuildContext::build(CurrentScript::Type(current_type_hash))?.plan_type_validation()?;

    let input_count = QueryIter::new(load_cell_data, Source::GroupInput).count();
    let output_count = QueryIter::new(load_cell_data, Source::GroupOutput).count();

    match order_mode(input_count, output_count)? {
        OrderMode::Create => validate_create_entry(current_type_hash, &plan),
        OrderMode::Fill => validate_fill_entry(&plan),
    }
}
```

Create:

```rust
fn validate_fill_entry(plan: &TypeValidationPlan) -> Result<(), Error> {
    let order = single_group_order(Source::GroupInput)?;
    // existing FillOrder logic using parse_limit_order_action and LimitOrderAction::Fill
}

fn validate_create_entry(
    current_type_hash: [u8; 32],
    plan: &TypeValidationPlan,
) -> Result<(), Error> {
    let order = single_group_order(Source::GroupOutput)?;
    // parse CreateOrder action in Task 5
}
```

Import `TypeValidationPlan` from `cobuild_core::plan`.

Implement:

```rust
fn single_group_order(source: Source) -> Result<crate::types::OrderState, Error> {
    let mut cells = QueryIter::new(load_cell_data, source);
    let Some(data) = cells.next() else {
        return Err(Error::InvalidOrderData);
    };
    if cells.next().is_some() {
        return Err(Error::InvalidOrderData);
    }
    parse_order_state(&data)
}
```

In `validate_fill_entry`, require:

```rust
if plan.related_actions.len() != 1 {
    return Err(Error::InvalidCobuild);
}
let related = &plan.related_actions[0];
let layout = otx_fill_layout(&related.action.origin, related.otx_relation)?;
let action_data = cursor_bytes(&related.action.action.data)?;
let LimitOrderAction::Fill(action) = parse_limit_order_action(&action_data)? else {
    return Err(Error::UnsupportedAction);
};
let settlements = collect_settlements(layout)?;
validate_fill(&order, &action, &settlements)
```

Leave `validate_create_entry` returning `Err(Error::InvalidCobuild)` until Task 5 only if create integration tests are not yet added. Unit tests should pass.

- [ ] **Step 5: Run green**

Run:

```bash
cargo test -p limit-order-type --offline order_mode -- --nocapture
cargo test -p limit-order-type --offline type_id_sys_error_maps_to_stable_exit_code -- --nocapture
cargo test -p limit-order-type --offline
```

Expected: pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add tests/contracts/limit-order-type/src/entry.rs tests/contracts/limit-order-type/src/error.rs
git commit -m "test: split limit order entry modes"
```

**Red/Green Record:**

```text
Red:
Green:
```

## Task 5: Add CreateOrder Validation Helpers

**Files:**
- Modify: `tests/contracts/limit-order-type/src/entry.rs`
- Modify: `tests/contracts/limit-order-type/src/types.rs`
- Modify: `tests/contracts/limit-order-type/src/error.rs`

- [ ] **Step 1: Write failing pure tests for create validation**

In `entry.rs` tests module, add:

```rust
#[test]
fn expected_proxy_lock_hash_changes_with_order_type_hash() {
    let first = expected_proxy_lock_hash([1; 32]);
    let second = expected_proxy_lock_hash([2; 32]);

    assert_ne!(first, second);
}

#[test]
fn create_action_context_accepts_any_origin_with_single_create_action() {
    let action = crate::types::LimitOrderAction::Create(crate::types::CreateOrderAction {
        owner_lock_hash: [2; 32],
        offered_nft_type_hash: [3; 32],
        requested_asset_id: [4; 32],
        min_requested_amount: 30,
    });

    assert!(matches!(action, crate::types::LimitOrderAction::Create(_)));
}
```

The second test is intentionally small: it guards that create validation is not tied to `ActionOrigin::TxLevel` or OTX layout helpers.

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p limit-order-type --offline expected_proxy_lock_hash_changes_with_order_type_hash -- --nocapture
```

Expected: compile failure because `expected_proxy_lock_hash` does not exist.

- [ ] **Step 3: Implement expected proxy lock hash**

In `entry.rs`, add imports:

```rust
use ckb_std::ckb_types::{
    bytes::Bytes,
    core::ScriptHashType,
    packed::Script,
    prelude::*,
};
```

Add:

```rust
fn expected_proxy_lock_hash(order_type_hash: [u8; 32]) -> [u8; 32] {
    let script = Script::new_builder()
        .code_hash(crate::generated_proxy_lock::INPUT_TYPE_PROXY_LOCK_CODE_HASH.pack())
        .hash_type(ScriptHashType::Data2.into())
        .args(Bytes::copy_from_slice(&order_type_hash).pack())
        .build();
    script.calc_script_hash().unpack()
}
```

If `ScriptHashType::Data2.into()` does not compile in no_std, inspect existing generated blockchain types and use the equivalent packed byte form:

```rust
.hash_type(ckb_std::ckb_types::packed::Byte::new(2))
```

Do not add `ckb-hash` unless `calc_script_hash` is unavailable.

- [ ] **Step 4: Implement create action extraction**

Add:

```rust
fn single_create_action(plan: &TypeValidationPlan) -> Result<CreateOrderAction, Error> {
    if plan.related_actions.len() != 1 {
        return Err(Error::InvalidCobuild);
    }
    let action_data = cursor_bytes(&plan.related_actions[0].action.action.data)?;
    let LimitOrderAction::Create(action) = parse_limit_order_action(&action_data)? else {
        return Err(Error::UnsupportedAction);
    };
    Ok(action)
}
```

Add imports for `CreateOrderAction`, `LimitOrderAction`, `validate_create`.

- [ ] **Step 5: Implement NFT proxy output scan**

Add:

```rust
fn has_nft_proxy_output(
    offered_nft_type_hash: [u8; 32],
    proxy_lock_hash: [u8; 32],
) -> Result<bool, Error> {
    let output_count = QueryIter::new(load_cell_data, Source::Output).count();
    for index in 0..output_count {
        let lock_hash = load_cell_lock_hash(index, Source::Output)?;
        if lock_hash != proxy_lock_hash {
            continue;
        }
        let Some(type_hash) = load_cell_type_hash(index, Source::Output)? else {
            continue;
        };
        if type_hash == offered_nft_type_hash {
            return Ok(true);
        }
    }
    Ok(false)
}
```

- [ ] **Step 6: Complete `validate_create_entry`**

Use:

```rust
fn validate_create_entry(
    current_type_hash: [u8; 32],
    plan: &TypeValidationPlan,
) -> Result<(), Error> {
    ckb_std::type_id::check_type_id(0, 32).map_err(Error::from)?;
    let order = single_group_order(Source::GroupOutput)?;
    let action = single_create_action(plan)?;
    validate_create(&order, &action)?;

    let proxy_lock_hash = expected_proxy_lock_hash(current_type_hash);
    if !has_nft_proxy_output(order.offered_nft_type_hash, proxy_lock_hash)? {
        return Err(Error::InvalidCobuild);
    }
    Ok(())
}
```

- [ ] **Step 7: Run green**

Run:

```bash
cargo test -p limit-order-type --offline expected_proxy_lock_hash_changes_with_order_type_hash -- --nocapture
cargo test -p limit-order-type --offline
```

Expected: pass.

- [ ] **Step 8: Commit**

Run:

```bash
git add tests/contracts/limit-order-type/src/entry.rs tests/contracts/limit-order-type/src/types.rs tests/contracts/limit-order-type/src/error.rs
git commit -m "test: validate limit order creation helpers"
```

**Red/Green Record:**

```text
Red:
Green:
```

## Task 6: Add Type-ID CreateOrder Fixture Happy Path

**Files:**
- Modify: `tests/src/fixtures/limit_order/nft_for_udt.rs`
- Modify: `tests/tests/limit_order.rs`
- Modify: `tests/src/fixtures/limit_order.rs`

- [ ] **Step 1: Write failing thin create test**

Add to `tests/tests/limit_order.rs`:

```rust
#[test]
fn limit_order_type_accepts_create_order_with_nft_proxy_output() {
    let (fixture, tx) = limit_order_create_nft_order_case();

    fixture.assert_pass(&tx);
}
```

Import `limit_order_create_nft_order_case`.

- [ ] **Step 2: Run red**

Run:

```bash
cargo test -p tests --test limit_order --offline limit_order_type_accepts_create_order_with_nft_proxy_output -- --nocapture
```

Expected: compile failure because `limit_order_create_nft_order_case` does not exist.

- [ ] **Step 3: Add type-id helper in fixture**

In `tests/src/fixtures/limit_order/nft_for_udt.rs`, add:

```rust
use ckb_hash::new_blake2b;
use ckb_testtool::ckb_types::{
    bytes::Bytes,
    core::ScriptHashType,
    packed::{CellInput, CellOutput, OutPoint, Script},
    prelude::*,
};
```

Add:

```rust
fn type_id_args(first_input: &CellInput, output_index: u64) -> [u8; 32] {
    let mut blake2b = new_blake2b();
    blake2b.update(first_input.as_slice());
    blake2b.update(&output_index.to_le_bytes());
    let mut out = [0u8; 32];
    blake2b.finalize(&mut out);
    out
}
```

- [ ] **Step 4: Add create action builder**

In `tests/src/fixtures/limit_order.rs`, expose:

```rust
pub fn create_order_action_data(order: LimitOrderState) -> Vec<u8>
```

and `LimitOrderCobuildMessageExt::limit_order_create`.

In `nft_for_udt.rs`, use:

```rust
let order_state = LimitOrderState {
    owner_lock_hash: script_hash(&owner_lock),
    offered_nft_type_hash: nft.script_hash,
    requested_asset_id: udt.script_hash,
    min_requested_amount: 30,
};
let message = fixture
    .cobuild()
    .output_type_action(order_type_hash)
    .limit_order_create(order_state)
    .build();
```

- [ ] **Step 5: Build create transaction**

Implement:

```rust
pub fn limit_order_create_nft_order_case() -> (CobuildTestFixture, TransactionView) {
    let mut fixture = CobuildTestFixture::new();
    let limit_order_code = fixture.deploy_limit_order();
    let always_success = fixture.deploy_always_success();
    let owner_lock = always_success.script.clone();
    let funding_input = live_input(
        fixture.context_mut(),
        normal_output(owner_lock.clone(), 200_000_000_000),
        Vec::new(),
    );
    let nft = deploy_test_nft(&mut fixture, NFT_TYPE_ARGS);
    let udt = deploy_test_udt_with_owner(&mut fixture, script_hash(&always_success.script));

    let order_type_id = type_id_args(&funding_input, 0);
    let order_type = fixture
        .context()
        .build_script_with_hash_type(
            &limit_order_code.out_point,
            ScriptHashType::Data2,
            Bytes::copy_from_slice(&order_type_id),
        )
        .expect("build order type-id script");
    let order_type_hash = script_hash(&order_type);
    let proxy_lock = deploy_input_type_proxy_lock(&mut fixture, order_type_hash);
    let order_state = LimitOrderState {
        owner_lock_hash: script_hash(&owner_lock),
        offered_nft_type_hash: nft.script_hash,
        requested_asset_id: udt.script_hash,
        min_requested_amount: 30,
    };
    let order_output = TestCellOutput::new(
        typed_output(owner_lock, order_type, 100_000_000_000),
        order_data(order_state),
    );
    let nft_output = TestCellOutput::new(
        typed_output(proxy_lock.script.clone(), nft.script.clone(), 90_000_000_000),
        nft_data(b"order-nft", [1, 2, 3, 4], 1_717_171_717),
    );
    let message = fixture
        .cobuild()
        .output_type_action(order_type_hash)
        .limit_order_create(order_state)
        .build();
    let tx = fixture
        .tx()
        .allow_no_otx()
        .cell_dep(cell_dep_for_script(&limit_order_code))
        .cell_dep(cell_dep_for_script(&always_success))
        .cell_dep(cell_dep_for_script(&proxy_lock))
        .cell_dep(cell_dep_for_script(&nft))
        .cell_dep(cell_dep_for_script(&udt))
        .base_input(funding_input)
        .base_output(order_output)
        .base_output(nft_output)
        .tx_level_message(message)
        .build();

    (fixture, tx)
}
```

If `fixture.context().build_script_with_hash_type` needs mutable access, use `fixture.context_mut()` and keep borrows scoped.

- [ ] **Step 6: Build contracts and run green**

Run:

```bash
CARGO_TARGET_DIR=/home/xcshuan/contracts/ckb/cobuild-otx-contracts/target make -e -C tests/vendor/ckb-proxy-locks/contracts/input-type-proxy-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CUSTOM_RUSTFLAGS='-C debug-assertions' CARGO_ARGS=--offline
make -e -C tests/contracts/limit-order-type build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-nft build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-udt build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
cargo test -p tests --test limit_order --offline limit_order_type_accepts_create_order_with_nft_proxy_output -- --nocapture
```

Expected: pass.

- [ ] **Step 7: Commit**

Run:

```bash
git add tests/src/fixtures/limit_order.rs tests/src/fixtures/limit_order/nft_for_udt.rs tests/tests/limit_order.rs tests/contracts/limit-order-type/src/generated_proxy_lock.rs
git commit -m "test: add limit order create fixture"
```

**Red/Green Record:**

```text
Red:
Green:
```

## Task 7: Add CreateOrder Failure Cases

**Files:**
- Modify: `tests/src/fixtures/limit_order/nft_for_udt.rs`
- Modify: `tests/tests/limit_order.rs`

- [ ] **Step 1: Add create scenario enum**

In `nft_for_udt.rs`, add:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CreateOrderCase {
    Valid,
    MissingNftProxyOutput,
    WrongNftType,
    WrongProxyOrder,
    StateActionMismatch,
    InvalidTypeId,
    InputAndOutputGroupShape,
}
```

Refactor:

```rust
pub fn limit_order_create_nft_order_case() -> (CobuildTestFixture, TransactionView) {
    limit_order_create_nft_order_case_with(CreateOrderCase::Valid)
}

pub fn limit_order_create_nft_order_case_with(
    case: CreateOrderCase,
) -> (CobuildTestFixture, TransactionView)
```

- [ ] **Step 2: Write failing thin tests**

Add tests in `tests/tests/limit_order.rs`:

```rust
#[test]
fn limit_order_type_rejects_create_order_without_nft_proxy_output() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) =
        limit_order_create_nft_order_case_with(CreateOrderCase::MissingNftProxyOutput);

    fixture.assert_output_type_script_exit(&tx, 0, 12);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_create_order_wrong_nft_type() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_create_nft_order_case_with(CreateOrderCase::WrongNftType);

    fixture.assert_output_type_script_exit(&tx, 0, 12);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_create_order_wrong_proxy_order() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_create_nft_order_case_with(CreateOrderCase::WrongProxyOrder);

    fixture.assert_output_type_script_exit(&tx, 0, 12);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_create_order_state_action_mismatch() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) =
        limit_order_create_nft_order_case_with(CreateOrderCase::StateActionMismatch);

    fixture.assert_output_type_script_exit(&tx, 0, 10);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}
```

- [ ] **Step 3: Run red**

Run:

```bash
cargo test -p tests --test limit_order --offline limit_order_type_rejects_create_order_without_nft_proxy_output -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_create_order_wrong_nft_type -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_create_order_wrong_proxy_order -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_create_order_state_action_mismatch -- --nocapture
```

Expected: compile failure because `CreateOrderCase` and `limit_order_create_nft_order_case_with` do not exist, or tests fail because variants are not implemented.

- [ ] **Step 4: Implement variants**

In create fixture:

- `MissingNftProxyOutput`: omit the NFT output entirely.
- `WrongNftType`: deploy a second test NFT with `[6; 32]`, use it in the NFT output, but keep order state/action `offered_nft_type_hash` as the original NFT.
- `WrongProxyOrder`: build proxy lock args from `[8; 32]` instead of current order type hash.
- `StateActionMismatch`: use order output `min_requested_amount = 30`, but CreateOrder action `min_requested_amount = 31`.

Keep the transaction otherwise valid so failure originates from `Outputs[0].Type` for the order type group.

- [ ] **Step 5: Run green for core create failures**

Run the four tests from Step 3.

Expected: all pass and no tracked failed tx files are added.

- [ ] **Step 6: Add type-id and group-shape tests**

Add:

```rust
#[test]
fn limit_order_type_rejects_create_order_invalid_type_id() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_create_nft_order_case_with(CreateOrderCase::InvalidTypeId);

    fixture.assert_output_type_script_exit(&tx, 0, 14);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}

#[test]
fn limit_order_type_rejects_order_input_and_output_group_shape() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) =
        limit_order_create_nft_order_case_with(CreateOrderCase::InputAndOutputGroupShape);

    fixture.assert_type_script_exit(&tx, 0, 5);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}
```

Type-id failure must map to `Error::TypeId = 14`; do not reuse `UnsupportedAction = 8`.

- [ ] **Step 7: Implement remaining variants**

- `InvalidTypeId`: use `[9; 32]` as order type args instead of computed type id.
- `InputAndOutputGroupShape`: create a live input with the same order type script and also include an order output with the same type script. This should fail group shape before create/fill validation.

- [ ] **Step 8: Run green**

Run:

```bash
cargo test -p tests --test limit_order --offline limit_order_type_rejects_create_order_invalid_type_id -- --nocapture
cargo test -p tests --test limit_order --offline limit_order_type_rejects_order_input_and_output_group_shape -- --nocapture
cargo test -p tests --test limit_order --offline -- --nocapture
```

Expected: pass.

- [ ] **Step 9: Commit**

Run:

```bash
git add tests/src/fixtures/limit_order/nft_for_udt.rs tests/tests/limit_order.rs
git commit -m "test: cover limit order create failures"
```

**Red/Green Record:**

```text
Red:
Green:
```

## Task 8: Update Fill Fixtures to New ABI and Preserve Regressions

**Files:**
- Modify: `tests/src/fixtures/limit_order.rs`
- Modify: `tests/src/fixtures/limit_order/nft_for_udt.rs`
- Modify: `tests/tests/limit_order.rs`
- Modify: `tests/src/framework/mod.rs`

- [ ] **Step 1: Write failing regression expectations**

Run before implementation:

```bash
cargo test -p tests --test limit_order --offline limit_order_type_accepts_nft_for_udt_otx_fill -- --nocapture
```

Expected: fail because fixtures still encode legacy `FillOrder` with `order_id/offered_amount` or order state with legacy fields.

- [ ] **Step 2: Update legacy `limit_order_case`**

In `tests/src/fixtures/limit_order.rs`, update `limit_order_case`:

```rust
.offered_nft_type_hash(OFFERED_ASSET_ID)
.requested_asset_id(REQUESTED_ASSET_ID)
.min_requested_amount(30)
```

Use:

```rust
.limit_order_fill(REQUESTED_ASSET_ID, 30)
```

For `limit_order_case(settlement_amount)`, keep `min_requested_amount` fixed at `30`; the variable controls settlement amount only.

- [ ] **Step 3: Update NFT fill fixture**

In `nft_for_udt.rs`:

- Replace `.offered_asset_id(nft.script_hash)` with `.offered_nft_type_hash(nft.script_hash)`.
- Replace `.offered_remaining(10).min_requested_per_offered(3)` with `.min_requested_amount(30)`.
- Replace `.limit_order_fill(ORDER_ID, udt.script_hash, 10, 30)` with `.limit_order_fill(udt.script_hash, 30)`.
- Delete `FillActionCase::OfferedAmountMismatch` because offered amount is no longer in `FillOrder`. Keep `MinRequestedBelowRequired` and `RequestedAssetMismatch` as the action mismatch coverage.

- [ ] **Step 4: Update thin tests**

In `tests/tests/limit_order.rs`, remove:

```rust
limit_order_type_rejects_offered_amount_mismatch
```

Add:

```rust
#[test]
fn limit_order_type_rejects_fill_amount_below_order_minimum() {
    let failed_txs_before = failed_txs_count();
    let (fixture, tx) = limit_order_action_failure_case(FillActionCase::MinRequestedBelowRequired);

    fixture.assert_type_script_exit(&tx, 0, 11);

    if std::env::var("COBUILD_TEST_DUMP_EXPECTED_FAILURES").as_deref() != Ok("1") {
        assert_eq!(failed_txs_count(), failed_txs_before);
    }
}
```

- [ ] **Step 5: Run green**

Run:

```bash
cargo test -p tests --test limit_order --offline -- --nocapture
cargo test -p tests --lib --offline
```

Expected: pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add tests/src/fixtures/limit_order.rs tests/src/fixtures/limit_order/nft_for_udt.rs tests/tests/limit_order.rs tests/src/framework/mod.rs
git commit -m "test: migrate limit order fill fixtures"
```

**Red/Green Record:**

```text
Red:
Green:
```

## Task 9: Update Docs and Boundary Tests

**Files:**
- Modify: `docs/superpowers/specs/2026-06-07-limit-order-test-type-script-spec.md`
- Modify: `docs/superpowers/plans/2026-06-08-limit-order-create-order-plan.md`

- [ ] **Step 1: Update old spec**

In `docs/superpowers/specs/2026-06-07-limit-order-test-type-script-spec.md`, add a note at the top:

```markdown
> Superseded for the CreateOrder stage by
> `docs/superpowers/specs/2026-06-08-limit-order-create-order-design.md`.
> The original fill-only MVP semantics remain useful historical context.
```

Do not rewrite the whole historical spec.

- [ ] **Step 2: Run boundary tests**

Run:

```bash
cargo test -p tests --lib --offline fixtures_live_in_dedicated_module_files -- --nocapture
cargo test -p tests --lib --offline limit_order_test_file_contains_no_fixture_scenario_builder -- --nocapture
```

Expected: pass. On line-count failure, split `nft_for_udt.rs` into `tests/src/fixtures/limit_order/create.rs` and `tests/src/fixtures/limit_order/fill.rs`, then re-export both modules from `limit_order.rs` before continuing.

- [ ] **Step 3: Commit**

Run:

```bash
git add docs/superpowers/specs/2026-06-07-limit-order-test-type-script-spec.md tests/src/tests.rs docs/superpowers/plans/2026-06-08-limit-order-create-order-plan.md
git commit -m "docs: mark limit order fill spec superseded"
```

**Red/Green Record:**

```text
Red:
Green:
```

## Task 10: Final Verification

**Files:**
- All files changed by prior tasks.

- [ ] **Step 1: Format**

Run:

```bash
cargo fmt
```

Expected: pass.

- [ ] **Step 2: Build contracts**

Run:

```bash
CARGO_TARGET_DIR=/home/xcshuan/contracts/ckb/cobuild-otx-contracts/target make -e -C tests/vendor/ckb-proxy-locks/contracts/input-type-proxy-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CUSTOM_RUSTFLAGS='-C debug-assertions' CARGO_ARGS=--offline
make -e -C tests/contracts/limit-order-type build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-udt build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-nft build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
```

Expected: all pass. The vendored proxy lock may emit its existing dead-code warning.

- [ ] **Step 3: Run required tests**

Run:

```bash
cargo test -p limit-order-type --offline
cargo test -p tests --test limit_order --offline
cargo test -p tests --lib --offline
cargo test --workspace --offline
```

Expected: all pass.

- [ ] **Step 4: Hygiene checks**

Run:

```bash
cargo fmt --check
git diff --check
git status --short
find tests/failed_txs -maxdepth 1 -type f 2>/dev/null | wc -l
git status --short --ignored tests/failed_txs
```

Expected:

- `cargo fmt --check` passes.
- `git diff --check` prints no output.
- `git status --short` shows only intended plan verification record before final commit.
- `failed_txs` count is unchanged except ignored pre-existing local files; no tracked failed tx files are added.

- [ ] **Step 5: Record verification**

Update this task's **Verification Record** with command results.

- [ ] **Step 6: Final verification-record commit**

After updating this plan's verification record:

```bash
git add docs/superpowers/plans/2026-06-08-limit-order-create-order-plan.md
git commit -m "docs: record create order verification"
```

**Verification Record:**

```text
cargo fmt:
make vendor input-type-proxy-lock:
make limit-order-type:
make test-udt:
make test-nft:
cargo test -p limit-order-type --offline:
cargo test -p tests --test limit_order --offline:
cargo test -p tests --lib --offline:
cargo test --workspace --offline:
cargo fmt --check:
git diff --check:
git status --short:
failed_txs:
```

## Plan Self-Review

Spec coverage:

- Dual mode by group shape: Tasks 4 and 5.
- Type-id order identity and no `order_id`: Tasks 3, 5, 6, 8.
- No `offered_remaining` or `nonce`: Task 3.
- `CreateOrder` tag `1` and `FillOrder` tag `2`: Task 3.
- Create validates state/action and NFT proxy output: Tasks 5, 6, 7.
- Proxy lock code hash not in ABI: Task 2.
- Fill remains OTX scoped payment-only: Tasks 4 and 8.
- Create fail cases: Task 7.
- Existing fill regressions: Task 8 and Task 10.
- No production crate/schema changes: File Structure and task scopes forbid them.

Placeholder scan:

- No task should contain `TBD`, `TODO`, or "similar to"; every task has concrete files, commands, and expected results. Empty `Red:` and `Green:` lines are intentional execution log slots.

Type consistency:

- `LimitOrderAction`, `CreateOrderAction`, `FillOrderAction`, `CreateOrderCase`, and fixture methods are defined before use.
- `min_requested_amount` is the single amount field across state, create, and fill.
- `offered_nft_type_hash` is the single offered asset field for this NFT-only stage.
