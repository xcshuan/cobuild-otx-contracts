# Cobuild Test Framework Fixtures Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** Refactor the `tests` support system so protocol-level Cobuild/CKB helpers live in `tests/src/framework`, concrete test-contract business fixtures live in `tests/src/fixtures`, and all current cobuild-otx-lock and limit-order tests migrate to typed handles, signing facts, scenario outcomes, and staged mutation APIs.

**Architecture:** Build a new protocol-first framework skeleton around `TxShape`, `BuiltTxShape`, typed entity handles, resolved input facts, protocol builders, signing hash oracle, signing facts, and protocol/shape mutation. Then build `fixtures/common`, `fixtures/cobuild_otx_lock`, and `fixtures/limit_order` on top of that framework, with no reverse dependency from framework to fixtures and no compatibility requirement for old helper APIs.

**Tech Stack:** Rust integration tests in the `tests` crate, `ckb-testtool`, `ckb-types`, `cobuild-types` entity builders for host tests, `cobuild-core` lazy-reader/hash-compatible views for test oracle logic, `secp256k1`, `cargo test -p tests --offline`, and `cargo test --workspace --offline`.

**Split Decision:** Keep this as one ordered plan because the phases are sequential and each phase leaves a runnable test subset. Do not split into separate plans unless execution discovers that the framework skeleton alone cannot be reviewed in one branch; if that happens, split after Task 4, then execute `fixtures/common`, `cobuild_otx_lock`, `limit_order`, and cleanup in that order.

**Implementation Status (2026-06-11): COMPLETE.** Implemented on branch `test-framework-fixtures-refactor` in worktree `.worktrees/test-framework-fixtures-refactor`. The migration was executed with subagent-driven review checkpoints; Task 10's scenario-model review was re-run after fixes and passed.

**Completion Commits:**
- `49bc2e4 refactor: remove stale limit order input builder`
- `b8bbbe6 refactor: remove legacy otx transaction builder`
- `d1581b0 fix: model limit order lock mutations as scenarios`
- `aa99816 fix: align limit order lock scenarios`
- `f591b86 refactor: migrate limit order lock tests`
- `1a829e2 fix: preserve limit order type case coverage`
- `767b698 refactor: migrate limit order type tests`
- `a6bab33 feat: add typed limit order fixture model`
- `9741d66 refactor: migrate cobuild otx lock fixtures`

**Final Verification:**
- PASS: `cargo test -p tests --offline --test cobuild_otx_lock`
- PASS: `cargo test -p tests --offline --test limit_order_type`
- PASS: `cargo test -p tests --offline --test limit_order_lock`
- PASS: `cargo test --workspace --offline`

**Implemented Outcome:** `framework` now owns protocol builders, `TxShape`/typed handles, resolved input facts, signing oracle/facts, expected outcomes, and protocol/shape mutations. `fixtures` now owns named contracts/assets/personas and limit-order/cobuild-otx-lock business cases. `limit_order_lock` uses `LockScenario` with happy path, `BusinessMutation`, expected error, coverage, and scenario fields instead of the old `LockFillCase` branch model. Legacy `OtxTransactionBuilder` and stale limit-order bare `CellInput` builder were removed.

---

## File Structure

- Create `tests/src/framework/cobuild/mod.rs`
  - Re-export protocol-level Cobuild builders.
- Create `tests/src/framework/cobuild/message.rs`
  - Define `ActionSpec`, `ActionRole`, and `MessageBuilder`; no business action payload helpers.
- Create `tests/src/framework/cobuild/otx.rs`
  - Define `OtxSpec`, `OtxBuilder`, raw count/mask/permission overrides, mask cover/uncover helpers, and `BuiltOtxSpec`.
- Create `tests/src/framework/cobuild/witness.rs`
  - Define `WitnessSpec`, `OtxStartSpec`, `WitnessHandle`, raw witness override helpers, and carrier witness builders.
- Create `tests/src/framework/cobuild/layout.rs`
  - Define `OtxSegment`, `OtxRangeFacts`, and layout calculation helpers.
- Replace `tests/src/framework/tx.rs` with `tests/src/framework/tx/mod.rs`, `builder.rs`, `handles.rs`, `mutate.rs`, and `malformed.rs`
  - Define `TxShape`, `BuiltTxShape`, `InputHandle`, `OutputHandle`, `CellDepHandle`, `HeaderDepHandle`, typed index maps, normalized tx builder, raw malformed layout builder, and post-build mutation.
- Expand `tests/src/framework/cells.rs`
  - Replace `live_input` usage in new APIs with `live_resolved_input`-style helpers returning `ResolvedInputFacts`.
- Replace `tests/src/framework/signing.rs` with `tests/src/framework/signing/mod.rs`, `keys.rs`, `oracle.rs`, `tx.rs`, and `otx.rs`
  - Define `SignerId`, `SigningHashOracle`, `TestSigningHashOracle`, `SigningFacts`, `SignatureScope`, scope signing helpers, and signing-hash assertions.
- Create `tests/src/framework/scenario/mod.rs`, `outcome.rs`, and `runner.rs`
  - Define protocol-level `ExpectedOutcome`, script locations using typed handles, and common runner assertions.
- Rename `tests/src/framework/contracts.rs` to `tests/src/framework/deploy.rs`
  - Keep only deployment primitives that accept binary bytes or explicit script names without named fixture semantics; named contract catalog moves to fixtures.
- Modify `tests/src/framework/mod.rs`
  - Re-export new modules and rewrite framework self-tests so they use only framework dummy cells/scripts/messages.
- Create `tests/src/fixtures/common/mod.rs`, `contracts.rs`, `personas.rs`, `assets.rs`, and `errors.rs`
  - Own named test contract catalog, personas, test UDT/NFT factories, proxy locks, always-success convenience, and shared fixture error helpers.
- Create `tests/src/fixtures/cobuild_otx_lock/mod.rs`, `cases.rs`, and `errors.rs`
  - Own cobuild-otx-lock contract scenarios and exit-code catalog while delegating signing/hash/layout to framework.
- Split `tests/src/fixtures/limit_order.rs` and `tests/src/fixtures/limit_order/*.rs` into `actions.rs`, `state.rs`, `scenarios.rs`, `mutations.rs`, `errors.rs`, `type_nft_for_udt.rs`, and `lock_nft_for_udt.rs`
  - Own limit-order action encoding, state builders, happy paths, business mutations, business expected outcomes, and coverage tags.
- Modify `tests/tests/cobuild_otx_lock.rs`
  - Consume `fixtures::cobuild_otx_lock::cases()` and `ExpectedOutcome`.
- Modify `tests/tests/limit_order_type.rs`
  - Consume `fixtures::limit_order` type-script built cases and expected outcomes.
- Modify `tests/tests/limit_order_lock.rs`
  - Consume `fixtures::limit_order` lock-script built cases and expected outcomes.
- Delete `tests/src/fixtures/otx_hash.rs` after its reusable oracle logic moves into `framework/signing`.
- Delete old helper APIs only after all three integration test files have migrated.

## Required Boundaries

- `framework` must not import `crate::fixtures`, `limit_order`, `cobuild_otx_lock`, `test-udt`, `test-nft`, `input-type-proxy-lock`, `always-success` named helpers, or business error enums.
- `fixtures` may import `framework`.
- `SigningHashOracle` must live under `tests/src/framework/signing` and be the single test-side oracle for tx-level, OTX base, and OTX append hashes.
- Business action builders, business mutations, named contracts, personas, and expected business error catalogs must live under `tests/src/fixtures`.
- New business action, mutation, and signing code must use typed handles or facts, not naked `usize` indexes.

---

### Task 1: Add Framework Boundary Guards Before Moving Code

**Files:**
- Modify: `tests/src/tests.rs`
- Test: `tests/src/tests.rs`

- [x] **Step 1: Add red boundary tests for framework-to-fixtures imports**

Add a test that scans `tests/src/framework` and rejects fixture imports and named test-contract terms:

```rust
#[test]
fn framework_does_not_depend_on_fixtures_or_named_test_contracts() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/framework");
    for path in rust_files_under(&root) {
        let src = std::fs::read_to_string(&path).expect("read framework file");
        for forbidden in [
            "crate::fixtures",
            "fixtures::",
            "limit_order",
            "cobuild_otx_lock",
            "test-udt",
            "test-nft",
            "input-type-proxy-lock",
            "wrong-owner",
        ] {
            assert!(
                !src.contains(forbidden),
                "{} must not contain fixture/business dependency {forbidden}",
                path.display()
            );
        }
    }
}

fn rust_files_under(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            for entry in std::fs::read_dir(&path).expect("read directory") {
                stack.push(entry.expect("directory entry").path());
            }
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    files
}
```

- [x] **Step 2: Add red tests for signing oracle location**

Add a guard that rejects the old fixture-local oracle and requires the new framework module names:

```rust
#[test]
fn signing_hash_oracle_is_framework_owned() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let framework_signing = std::fs::read_to_string(root.join("framework/signing/oracle.rs"))
        .unwrap_or_default();
    assert!(
        framework_signing.contains("pub trait SigningHashOracle"),
        "framework/signing/oracle.rs must define SigningHashOracle"
    );
    assert!(
        !root.join("fixtures/otx_hash.rs").exists(),
        "fixture-local otx_hash oracle should be removed after migration"
    );
}
```

- [x] **Step 3: Run boundary tests and verify they fail**

Run: `cargo test -p tests --offline --lib framework_does_not_depend_on_fixtures_or_named_test_contracts signing_hash_oracle_is_framework_owned`

Expected: FAIL. The current `tests/src/framework/mod.rs` imports `crate::fixtures::limit_order`, and `tests/src/fixtures/otx_hash.rs` still exists.

- [x] **Step 4: Commit boundary tests**

```bash
git add tests/src/tests.rs
git commit -m "test: guard fixture framework boundaries"
```

### Task 2: Build `framework/cells` And `framework/tx` Skeleton With Typed Handles

**Files:**
- Modify: `tests/src/framework/cells.rs`
- Create: `tests/src/framework/tx/mod.rs`
- Create: `tests/src/framework/tx/handles.rs`
- Create: `tests/src/framework/tx/builder.rs`
- Create: `tests/src/framework/tx/mutate.rs`
- Create: `tests/src/framework/tx/malformed.rs`
- Modify: `tests/src/framework/mod.rs`
- Test: `tests/src/framework/tx/mod.rs`

- [x] **Step 1: Define resolved facts and handles**

In `cells.rs`, replace new-code use of `TestResolvedInput` with:

```rust
#[derive(Clone, Debug)]
pub struct ResolvedInputFacts {
    pub input: CellInput,
    pub output: CellOutput,
    pub data: Bytes,
    pub lock_hash: [u8; 32],
    pub type_hash: Option<[u8; 32]>,
}
```

Keep old helpers temporarily, but add `live_resolved_facts(context, output, data) -> ResolvedInputFacts`.

In `tx/handles.rs`, define typed handles:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct InputHandle(pub(crate) usize);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct OutputHandle(pub(crate) usize);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct CellDepHandle(pub(crate) usize);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct HeaderDepHandle(pub(crate) usize);
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct OtxHandle(pub(crate) usize);
```

- [x] **Step 2: Define `TxShape`, `BuiltTxShape`, and range facts**

In `tx/builder.rs`, add:

```rust
pub struct TxShape {
    prefix_inputs: Vec<ResolvedInputFacts>,
    otxs: Vec<OtxSegment>,
    remainder_inputs: Vec<ResolvedInputFacts>,
    prefix_outputs: Vec<TestCellOutput>,
    remainder_outputs: Vec<TestCellOutput>,
    cell_deps: Vec<CellDep>,
    header_deps: Vec<[u8; 32]>,
    witnesses: Vec<WitnessSpec>,
}

pub struct BuiltTxShape {
    pub tx: TransactionView,
    pub inputs: EntityIndexMap<InputHandle>,
    pub outputs: EntityIndexMap<OutputHandle>,
    pub cell_deps: EntityIndexMap<CellDepHandle>,
    pub header_deps: EntityIndexMap<HeaderDepHandle>,
    pub witnesses: EntityIndexMap<WitnessHandle>,
    pub resolved_inputs: Vec<ResolvedInputFacts>,
    pub otx_ranges: Vec<OtxRangeFacts>,
}
```

`EntityIndexMap<T>` should provide `tx_index(handle) -> usize` and `handle_at_tx_index(index) -> Option<T>`.

- [x] **Step 3: Support named OTX entity placement without global indexes**

Add builder methods that return handles at insertion time:

```rust
impl TxShape {
    pub fn push_prefix_input(&mut self, input: ResolvedInputFacts) -> InputHandle;
    pub fn push_otx(&mut self, segment: OtxSegment) -> OtxHandle;
    pub fn push_remainder_output(&mut self, output: TestCellOutput) -> OutputHandle;
    pub fn otx_append_output(&self, otx: OtxHandle, local_index: usize) -> OutputHandle;
    pub fn otx_base_output(&self, otx: OtxHandle, local_index: usize) -> OutputHandle;
}
```

- [x] **Step 4: Add framework self-tests for shape mapping**

Add tests that build two OTXs plus a remainder output and assert:

```rust
assert_eq!(built.outputs.tx_index(payment_a), 1);
assert_eq!(built.outputs.tx_index(payment_b), 3);
assert_eq!(built.outputs.tx_index(remainder_payment), 4);
assert_eq!(built.otx_ranges[0].append_outputs.contains(payment_a), true);
assert_eq!(built.otx_ranges[1].append_outputs.contains(payment_b), true);
assert_eq!(built.otx_ranges[0].append_outputs.contains(remainder_payment), false);
```

- [x] **Step 5: Run targeted tx skeleton tests**

Run: `cargo test -p tests --offline --lib tx_shape`

Expected: PASS. The new skeleton can build handle/index maps without migrating integration tests yet.

- [x] **Step 6: Commit tx skeleton**

```bash
git add tests/src/framework/cells.rs tests/src/framework/tx tests/src/framework/mod.rs
git commit -m "feat: add typed tx shape test framework"
```

### Task 3: Move Protocol Builders Under `framework/cobuild`

**Files:**
- Create: `tests/src/framework/cobuild/mod.rs`
- Create: `tests/src/framework/cobuild/message.rs`
- Create: `tests/src/framework/cobuild/otx.rs`
- Create: `tests/src/framework/cobuild/witness.rs`
- Create: `tests/src/framework/cobuild/layout.rs`
- Modify: `tests/src/framework/mod.rs`
- Test: `tests/src/framework/cobuild/mod.rs`

- [x] **Step 1: Replace flat `cobuild.rs` with focused protocol modules**

Move protocol-only pieces from `tests/src/framework/cobuild.rs` into new modules. Keep these public names:

```rust
pub use layout::{OtxRangeFacts, OtxSegment};
pub use message::{ActionRole, ActionSpec, MessageBuilder};
pub use otx::{BuiltOtxSpec, OtxBuilder, OtxSpec};
pub use witness::{OtxStartSpec, WitnessHandle, WitnessSpec};
```

- [x] **Step 2: Add raw override APIs needed by malformed protocol tests**

`OtxBuilder` must expose:

```rust
pub fn append_permissions_raw(self, value: u8) -> Self;
pub fn base_input_masks_raw(self, masks: Vec<u8>) -> Self;
pub fn base_output_masks_raw(self, masks: Vec<u8>) -> Self;
pub fn base_cell_dep_masks_raw(self, masks: Vec<u8>) -> Self;
pub fn base_header_dep_masks_raw(self, masks: Vec<u8>) -> Self;
pub fn raw_base_input_cells(self, value: u32) -> Self;
pub fn raw_append_output_cells(self, value: u32) -> Self;
pub fn allow_append_inputs(self) -> Self;
pub fn allow_append_outputs(self) -> Self;
pub fn allow_append_cell_deps(self) -> Self;
pub fn allow_append_header_deps(self) -> Self;
```

- [x] **Step 3: Add mask cover/uncover helpers**

`OtxBuilder` must support protocol-level mask intent:

```rust
pub fn cover_base_input_since(self, local_input: usize) -> Self;
pub fn cover_base_input_previous_output(self, local_input: usize) -> Self;
pub fn cover_base_output_capacity(self, local_output: usize) -> Self;
pub fn cover_base_output_lock(self, local_output: usize) -> Self;
pub fn cover_base_output_type(self, local_output: usize) -> Self;
pub fn cover_base_output_data(self, local_output: usize) -> Self;
pub fn uncover_base_output_data(self, local_output: usize) -> Self;
```

- [x] **Step 4: Add witness raw override builders**

`witness.rs` must provide:

```rust
pub enum WitnessSpec {
    Empty,
    Legacy(Bytes),
    SighashAll { message: CobuildMessage, seal: Vec<u8> },
    SighashAllOnly { seal: Vec<u8> },
    OtxStart(OtxStartSpec),
    Otx(BuiltOtxSpec),
    RawCobuild(Bytes),
}
```

- [x] **Step 5: Run protocol builder tests**

Run: `cargo test -p tests --offline --lib cobuild_protocol_builders`

Expected: PASS. Tests should prove raw permission high bits, invalid masks, custom `OtxStart`, and raw malformed witness bytes can be encoded without fixture code.

- [x] **Step 6: Commit protocol builders**

```bash
git add tests/src/framework/cobuild tests/src/framework/mod.rs
git commit -m "feat: split cobuild protocol test builders"
```

### Task 4: Add Framework-Owned Signing Hash Oracle And Signing Facts

**Files:**
- Create: `tests/src/framework/signing/mod.rs`
- Create: `tests/src/framework/signing/keys.rs`
- Create: `tests/src/framework/signing/oracle.rs`
- Create: `tests/src/framework/signing/tx.rs`
- Create: `tests/src/framework/signing/otx.rs`
- Modify: `tests/src/framework/mod.rs`
- Keep until migration completes: `tests/src/fixtures/otx_hash.rs`
- Test: `tests/src/framework/signing/mod.rs`

- [x] **Step 1: Define stable signing API**

In `oracle.rs`, define:

```rust
pub trait SigningHashOracle {
    fn tx_without_message(&self, built: &BuiltTxShape) -> [u8; 32];
    fn tx_with_message(&self, built: &BuiltTxShape, message: &CobuildMessage) -> [u8; 32];
    fn otx_base(&self, built: &BuiltTxShape, otx: OtxHandle) -> [u8; 32];
    fn otx_append(
        &self,
        built: &BuiltTxShape,
        otx: OtxHandle,
        base_hash: [u8; 32],
    ) -> [u8; 32];
}

pub struct TestSigningHashOracle;
```

- [x] **Step 2: Define signer and signing facts**

Add:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct SignerId(pub &'static str);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignatureScope {
    TxWithoutMessage,
    TxWithMessage,
    OtxBase { otx: OtxHandle },
    OtxAppend { otx: OtxHandle },
}

#[derive(Clone, Debug)]
pub struct SigningFacts {
    pub signer: SignerId,
    pub scope: SignatureScope,
    pub carrier: WitnessHandle,
    pub script_hash: [u8; 32],
    pub signing_hash: [u8; 32],
    pub seal: Vec<u8>,
}
```

- [x] **Step 3: Implement oracle from existing fixture mirror**

Move reusable logic from `tests/src/fixtures/otx_hash.rs` into `signing/otx.rs`, but change inputs from `OtxFixtureParts` to `BuiltTxShape`, `OtxHandle`, `OtxRangeFacts`, and `ResolvedInputFacts`.

Move tx-level logic from current `framework/signing.rs` into `signing/tx.rs`, but change inputs from repeated `resolved_output` slices to `BuiltTxShape.resolved_inputs`.

- [x] **Step 4: Add scope signing helpers**

Expose:

```rust
pub fn sign_scope(
    built: &mut BuiltTxShape,
    oracle: &impl SigningHashOracle,
    signer: SignerId,
    secret_key: &SecretKey,
    script_hash: [u8; 32],
    scope: SignatureScope,
) -> SigningFacts;
```

This helper computes the digest, signs it, inserts the seal into the correct witness/seal pair, and records `SigningFacts`.

- [x] **Step 5: Add signing mutation assertions**

Expose:

```rust
pub fn assert_hash_changed(before: [u8; 32], after: [u8; 32], scope: SignatureScope);
pub fn assert_hash_unchanged(before: [u8; 32], after: [u8; 32], scope: SignatureScope);
```

- [x] **Step 6: Run signing oracle tests**

Run: `cargo test -p tests --offline --lib signing_hash_oracle`

Expected: PASS. Tests must cover `TxWithoutMessage`, `TxWithMessage`, `OtxBase`, `OtxAppend`, resolved input data coverage, local index coverage, and append hash binding to base hash.

- [x] **Step 7: Commit signing framework**

```bash
git add tests/src/framework/signing tests/src/framework/mod.rs
git commit -m "feat: add framework signing hash oracle"
```

### Task 5: Add Protocol And TxShape Mutations Plus Expected Outcomes

**Files:**
- Create: `tests/src/framework/tx/mutate.rs`
- Create: `tests/src/framework/tx/malformed.rs`
- Create: `tests/src/framework/scenario/mod.rs`
- Create: `tests/src/framework/scenario/outcome.rs`
- Create: `tests/src/framework/scenario/runner.rs`
- Modify: `tests/src/framework/assertions.rs`
- Modify: `tests/src/framework/mod.rs`
- Test: `tests/src/framework/scenario/mod.rs`

- [x] **Step 1: Define protocol and shape mutation APIs**

Add:

```rust
pub enum ProtocolMutation {
    DuplicateSighashAll,
    NonContiguousOtxWitness,
    OtxBeforeOtxStart,
    OtxStartRaw(OtxStartSpec),
    OtxRawPermission { otx: OtxHandle, permissions: u8 },
    OtxRawBaseInputMasks { otx: OtxHandle, masks: Vec<u8> },
    SealScopeRaw { otx: OtxHandle, script_hash: [u8; 32], scope: u8 },
}

pub enum TxShapeMutation {
    ReplaceInput { input: InputHandle, replacement: ResolvedInputFacts },
    ReplaceOutput { output: OutputHandle, replacement: TestCellOutput },
    ReplaceWitness { witness: WitnessHandle, replacement: Bytes },
    AppendRemainderOutput { output: TestCellOutput },
    MoveOutputToRemainder { output: OutputHandle },
}
```

- [x] **Step 2: Implement mutation application on `BuiltTxShape`**

Add:

```rust
impl BuiltTxShape {
    pub fn apply_protocol_mutation(&mut self, mutation: ProtocolMutation);
    pub fn apply_shape_mutation(&mut self, mutation: TxShapeMutation) -> Option<OutputHandle>;
}
```

When a mutation changes indexes, update `EntityIndexMap` and `OtxRangeFacts` or panic with a message naming the unsupported mutation.

- [x] **Step 3: Define protocol expected outcomes**

In `scenario/outcome.rs`, add:

```rust
pub enum ScriptLocation {
    InputLock(InputHandle),
    InputType(InputHandle),
    OutputType(OutputHandle),
}

pub enum ExpectedOutcome {
    Pass,
    ScriptExit { location: ScriptLocation, code: i8 },
}
```

`ExpectedOutcome::assert(&self, fixture: &CobuildTestFixture, built: &BuiltTxShape)` resolves handles to current tx indexes.

- [x] **Step 4: Centralize failed tx dump assertions**

Move the repeated `failed_txs_count` pattern into framework assertions:

```rust
pub fn assert_no_failed_tx_dump_delta(before: usize);
pub fn failed_txs_count() -> usize;
```

- [x] **Step 5: Run mutation/outcome tests**

Run: `cargo test -p tests --offline --lib mutation expected_outcome`

Expected: PASS. Tests must prove an output handle remains stable when its tx index changes and expected outcome resolves the new index at assertion time.

- [x] **Step 6: Commit mutation and outcome framework**

```bash
git add tests/src/framework/tx tests/src/framework/scenario tests/src/framework/assertions.rs tests/src/framework/mod.rs
git commit -m "feat: add tx mutation and expected outcomes"
```

### Task 6: Build `fixtures/common` For Named Contracts, Personas, And Assets

**Files:**
- Create: `tests/src/fixtures/common/mod.rs`
- Create: `tests/src/fixtures/common/contracts.rs`
- Create: `tests/src/fixtures/common/personas.rs`
- Create: `tests/src/fixtures/common/assets.rs`
- Create: `tests/src/fixtures/common/errors.rs`
- Modify: `tests/src/fixtures/mod.rs`
- Modify: `tests/src/framework/deploy.rs`
- Test: `tests/src/fixtures/common/mod.rs`

- [x] **Step 1: Move named contract deployment out of framework**

`framework/deploy.rs` keeps only:

```rust
pub fn deploy_script_bytes(
    context: &mut Context,
    bin: Bytes,
    hash_type: ScriptHashType,
    args: Vec<u8>,
) -> DeployedScript;

pub fn deploy_loader_binary(
    context: &mut Context,
    name: &str,
    hash_type: ScriptHashType,
    args: Vec<u8>,
) -> DeployedScript;
```

`fixtures/common/contracts.rs` owns:

```rust
pub struct ContractCatalog {
    pub always_success: DeployedScript,
    pub cobuild_otx_lock_code: DeployedScript,
    pub limit_order_type_code: DeployedScript,
    pub limit_order_lock_code: DeployedScript,
}

pub fn deploy_always_success(context: &mut Context, args: Vec<u8>) -> DeployedScript;
pub fn deploy_test_udt(context: &mut Context, owner_lock_hash: [u8; 32]) -> DeployedScript;
pub fn deploy_test_nft(context: &mut Context, args: [u8; 32]) -> DeployedScript;
pub fn deploy_input_type_proxy_lock(context: &mut Context, owner_type_hash: [u8; 32]) -> DeployedScript;
pub fn deploy_wrong_owner_lock(context: &mut Context) -> DeployedScript;
```

- [x] **Step 2: Add personas**

`personas.rs` defines:

```rust
#[derive(Clone, Debug)]
pub struct Persona {
    pub id: SignerId,
    pub lock: Script,
    pub lock_hash: [u8; 32],
    pub secret_key: Option<SecretKey>,
}

pub struct Personas {
    pub owner: Persona,
    pub buyer: Persona,
    pub fee_payer: Persona,
    pub wrong_owner: Persona,
    pub order_lock_owner: Persona,
}
```

- [x] **Step 3: Add test asset factories**

`assets.rs` owns:

```rust
pub struct TestUdt {
    pub script: Script,
    pub script_hash: [u8; 32],
    pub cell_dep: CellDep,
}

pub struct TestNft {
    pub script: Script,
    pub script_hash: [u8; 32],
    pub cell_dep: CellDep,
}

pub fn udt_amount_data(amount: u128) -> Bytes;
pub fn nft_data(name: &[u8], attributes: [u8; 4], created_at: u64) -> Bytes;
```

- [x] **Step 4: Run fixture common tests**

Run: `cargo test -p tests --offline --lib fixtures_common`

Expected: PASS. Tests prove named contract helpers are only in fixtures and framework boundary guard passes after removing named helpers from framework tests.

- [x] **Step 5: Commit common fixtures**

```bash
git add tests/src/fixtures/common tests/src/fixtures/mod.rs tests/src/framework/deploy.rs
git commit -m "feat: add common contract fixtures"
```

### Task 7: Migrate `cobuild_otx_lock` Tests To Framework Signing And Shape APIs

**Files:**
- Create: `tests/src/fixtures/cobuild_otx_lock/mod.rs`
- Create: `tests/src/fixtures/cobuild_otx_lock/cases.rs`
- Create: `tests/src/fixtures/cobuild_otx_lock/errors.rs`
- Modify: `tests/src/fixtures/mod.rs`
- Modify: `tests/tests/cobuild_otx_lock.rs`
- Delete after migration: old `tests/src/fixtures/cobuild_otx_lock.rs`
- Test: `tests/tests/cobuild_otx_lock.rs`

- [x] **Step 1: Define lock error catalog and built case**

`errors.rs`:

```rust
pub enum CobuildOtxLockError {
    InvalidArgs,
    MalformedCobuildWitness,
    MalformedOtxLayout,
    NoRelevantSignatureRequest,
    BadSeal,
}

impl CobuildOtxLockError {
    pub fn code(self) -> i8 {
        match self {
            Self::InvalidArgs => 20,
            Self::MalformedCobuildWitness => 30,
            Self::MalformedOtxLayout => 31,
            Self::NoRelevantSignatureRequest => 40,
            Self::BadSeal => 50,
        }
    }
}
```

`cases.rs`:

```rust
pub struct BuiltCobuildOtxLockCase {
    pub fixture: CobuildTestFixture,
    pub built: BuiltTxShape,
    pub signing_facts: Vec<SigningFacts>,
    pub expected: ExpectedOutcome,
}
```

- [x] **Step 2: Rebuild tx-level sighash cases using `SigningHashOracle`**

Migrate `signed_sighash_all_case` and `signed_sighash_all_offset_lock_case` so they:

1. create `ResolvedInputFacts` for every input;
2. build a `TxShape`;
3. call `sign_scope(..., SignatureScope::TxWithoutMessage)`;
4. assert with `ExpectedOutcome::Pass`.

- [x] **Step 3: Rebuild OTX dual-scope cases using OTX handles**

Migrate `signed_otx_dual_scope_case`, `signed_otx_full_preimage_case`, and `mixed_sighash_all_and_otx_case` so base and append seals are inserted by `sign_scope` using `OtxHandle`.

- [x] **Step 4: Rebuild malformed and bad-seal cases using mutations**

Use:

```rust
ProtocolMutation::OtxRawPermission { permissions: 0x10, .. }
ProtocolMutation::SealScopeRaw { scope: 1, .. }
TxShapeMutation::ReplaceWitness { .. }
```

Expected outcomes must use `CobuildOtxLockError::code()` and typed input handles.

- [x] **Step 5: Preserve two-UDT-transfer coverage**

Move `two_udt_transfer_otxs_case` from `fixtures/udt.rs` onto the new tx shape/signing API. The case must return two OTX lock hashes and optional fee lock hash as facts, but tests must not inspect raw indexes.

- [x] **Step 6: Rewrite integration test as table-driven runner**

`tests/tests/cobuild_otx_lock.rs` should read:

```rust
#[test]
fn cobuild_otx_lock_cases_match_expected_outcomes() {
    for case in tests::fixtures::cobuild_otx_lock::cases() {
        case.expected.assert(&case.fixture, &case.built);
    }
}
```

Keep separate tests only where they assert facts such as distinct lock hashes.

- [x] **Step 7: Run cobuild-otx-lock tests**

Run: `cargo test -p tests --offline --test cobuild_otx_lock`

Expected: PASS. Existing pass/fail behavior remains, but signing hash computation no longer calls `fixtures::otx_hash`.

- [x] **Step 8: Commit cobuild-otx-lock migration**

```bash
git add tests/src/fixtures/cobuild_otx_lock tests/src/fixtures/mod.rs tests/tests/cobuild_otx_lock.rs tests/src/fixtures/udt.rs
git commit -m "refactor: migrate cobuild otx lock fixtures"
```

### Task 8: Build `fixtures/limit_order` Actions, State, Mutations, Errors, And Coverage

**Files:**
- Create: `tests/src/fixtures/limit_order/actions.rs`
- Create: `tests/src/fixtures/limit_order/state.rs`
- Create: `tests/src/fixtures/limit_order/scenarios.rs`
- Create: `tests/src/fixtures/limit_order/mutations.rs`
- Create: `tests/src/fixtures/limit_order/errors.rs`
- Modify: `tests/src/fixtures/limit_order.rs` or replace with `tests/src/fixtures/limit_order/mod.rs`
- Test: `tests/src/fixtures/limit_order/mod.rs`

- [x] **Step 1: Define business state and action APIs**

`state.rs` owns:

```rust
pub struct LimitOrderState {
    pub owner_lock_hash: [u8; 32],
    pub offered_nft_type_hash: [u8; 32],
    pub requested_asset_id: [u8; 32],
    pub requested_amount: u64,
}

pub fn order_data(order: LimitOrderState) -> Bytes;
pub fn settlement_data(asset_id: [u8; 32], amount: u64) -> Bytes;
```

`actions.rs` owns:

```rust
pub enum LimitOrderAction {
    Create { order: LimitOrderState },
    Fill { payment: OutputHandle, buyer_lock_hash: [u8; 32] },
    UnknownTag,
    MalformedFill { payment: OutputHandle, buyer_lock_hash: [u8; 32] },
}

pub fn encode_action(action: &LimitOrderAction, built: &BuiltTxShape) -> Vec<u8>;
```

The fill encoder resolves `OutputHandle` through `BuiltTxShape.outputs.tx_index(payment)`.

- [x] **Step 2: Define happy paths and business mutations**

`scenarios.rs`:

```rust
pub enum LimitOrderHappyPath {
    TypeNftForUdt,
    LockNftForUdt,
    MixedTypeAndLock,
    CreateTypeOrder,
    TwoTypeOrders,
    TwoLockOrders,
}
```

`mutations.rs`:

```rust
pub enum BusinessMutation {
    PaymentOutputWrongUdt,
    PaymentOutputWrongOwner,
    PaymentOutputInsufficient,
    PaymentOutputInAnotherOtx,
    PaymentOutputInRemainder,
    ReusePaymentOutput,
    BuyerNftMissing,
    BuyerNftWrongLock,
    BuyerNftWrongType,
    TxLevelActionInsteadOfOtxAction,
    WrongActionTarget,
    OrderInputInAppendScope,
    UnknownActionTag,
    MalformedAction,
    CreateMissingNftProxyOutput,
    CreateWrongNftType,
    CreateWrongProxyOrder,
    CreateStateActionMismatch,
    CreateInvalidTypeId,
    CreateInputAndOutputGroupShape,
}
```

- [x] **Step 3: Define business expected outcomes**

`errors.rs`:

```rust
pub enum LimitOrderTypeError {
    GroupShape,
    StateActionMismatch,
    PaymentInvalid,
    MissingOrInvalidAction,
    NftProxyInvalid,
    InvalidTypeId,
}

pub enum LimitOrderLockError {
    MalformedAction,
    UnknownAction,
    WrongNftType,
    PaymentInvalid,
    MissingOrInvalidAction,
}

pub enum LimitOrderExpectedOutcome {
    Pass,
    TypeExit { input: InputHandle, error: LimitOrderTypeError },
    OutputTypeExit { output: OutputHandle, error: LimitOrderTypeError },
    LockExit { input: InputHandle, error: LimitOrderLockError },
}
```

Use current exit code mapping: type `5`, `10`, `11`, `12`, `14`; lock `5`, `6`, `7`, `8`, `10`, `12`.

- [x] **Step 4: Define coverage tags**

Add:

```rust
pub enum FlowKind { TxLevel, OtxOnly, TxLevelAndOtx }
pub enum ScriptRoleKind { InputLock, InputType, OutputType }
pub enum OtxScopeKind { BaseInput, AppendInput, BaseOutput, AppendOutput, Remainder }
pub enum ActionSourceKind { TxLevel, Otx, Absent, WrongTarget, Duplicate }

pub struct CoverageTag {
    pub flow: FlowKind,
    pub script_role: ScriptRoleKind,
    pub otx_scope: OtxScopeKind,
    pub action_source: ActionSourceKind,
    pub mutation: Option<BusinessMutation>,
}
```

- [x] **Step 5: Run limit-order fixture unit tests**

Run: `cargo test -p tests --offline --lib limit_order`

Expected: PASS. Tests prove fill actions encode from `OutputHandle`, duplicate payment cases can reuse one handle intentionally, and payment-in-another-OTX/remainder cases point at handles outside the current `OtxRangeFacts`.

- [x] **Step 6: Commit limit-order fixture model**

```bash
git add tests/src/fixtures/limit_order tests/src/fixtures/limit_order.rs
git commit -m "feat: add typed limit order fixture model"
```

### Task 9: Migrate `limit_order_type` Tests To Built Cases

**Files:**
- Modify: `tests/src/fixtures/limit_order/type_nft_for_udt.rs`
- Modify: `tests/tests/limit_order_type.rs`
- Test: `tests/tests/limit_order_type.rs`

- [x] **Step 1: Replace `NftForUdtPaymentCase`, `FillActionCase`, and `CreateOrderCase` builders**

Create public case constructors:

```rust
pub fn type_script_cases() -> Vec<BuiltLimitOrderCase>;
pub fn type_script_create_order_cases() -> Vec<BuiltLimitOrderCase>;
pub fn type_script_fill_cases() -> Vec<BuiltLimitOrderCase>;
```

Each `BuiltLimitOrderCase` contains `fixture`, `built`, `expected`, and `coverage`.

- [x] **Step 2: Convert payment index bindings to output handles**

Replace hard-coded indexes such as `1`, `2`, and `3` with:

```rust
let payment = built.otx_append_output(otx, 0);
let other_payment = built.otx_append_output(other_otx, 0);
let remainder_payment = built.push_remainder_output(payment_output);
let action = LimitOrderAction::Fill { payment, buyer_lock_hash };
```

- [x] **Step 3: Convert expected outcomes into fixture-owned values**

For each current test, encode the current assertion in the case:

```rust
LimitOrderExpectedOutcome::TypeExit {
    input: order_input,
    error: LimitOrderTypeError::PaymentInvalid,
}
```

or:

```rust
LimitOrderExpectedOutcome::OutputTypeExit {
    output: order_output,
    error: LimitOrderTypeError::NftProxyInvalid,
}
```

- [x] **Step 4: Rewrite `limit_order_type.rs` as a case runner**

Use:

```rust
#[test]
fn limit_order_type_cases_match_expected_outcomes() {
    for case in tests::fixtures::limit_order::type_script_cases() {
        case.expected.assert(&case.fixture, &case.built);
        case.coverage.assert_has_required_tags();
    }
}
```

- [x] **Step 5: Run type test**

Run: `cargo test -p tests --offline --test limit_order_type`

Expected: PASS. All previous type-script pass/fail cases remain covered, and no test file hard-codes limit-order exit codes.

- [x] **Step 6: Commit type migration**

```bash
git add tests/src/fixtures/limit_order tests/tests/limit_order_type.rs
git commit -m "refactor: migrate limit order type tests"
```

### Task 10: Migrate `limit_order_lock` Tests To Built Cases

**Files:**
- Modify: `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`
- Modify: `tests/tests/limit_order_lock.rs`
- Test: `tests/tests/limit_order_lock.rs`

- [x] **Step 1: Replace `LimitOrderLockFillCase` with happy path plus mutation**

Create:

```rust
pub fn lock_script_cases() -> Vec<BuiltLimitOrderCase>;
pub fn lock_script_fill_cases() -> Vec<BuiltLimitOrderCase>;
pub fn mixed_type_lock_cases() -> Vec<BuiltLimitOrderCase>;
```

Each former enum value maps to `LimitOrderHappyPath::LockNftForUdt` plus zero or more `BusinessMutation` values.

- [x] **Step 2: Remove the giant branch from `limit_order_lock_nft_for_udt_case_with`**

Replace the branch-heavy builder with:

```rust
let mut scenario = LimitOrderScenario::new(LimitOrderHappyPath::LockNftForUdt);
scenario.apply(BusinessMutation::PaymentOutputWrongUdt);
let built = scenario.build();
```

Each mutation changes only its own business invariant or delegates to `TxShapeMutation`.

- [x] **Step 3: Convert lock seal construction to framework signing facts**

For limit-order-lock OTX base/append seal placeholders, call framework signing helpers or explicit empty-seal policy helper. The fixture must record:

```rust
SigningFacts {
    signer: SignerId("order_lock_owner"),
    scope: SignatureScope::OtxBase { otx },
    carrier,
    script_hash: order_lock_hash,
    signing_hash,
    seal,
}
```

- [x] **Step 4: Convert all expected lock outcomes**

For each former test assertion:

```rust
fixture.assert_lock_script_exit(&tx, 0, 10);
```

encode:

```rust
LimitOrderExpectedOutcome::LockExit {
    input: order_input,
    error: LimitOrderLockError::PaymentInvalid,
}
```

- [x] **Step 5: Rewrite `limit_order_lock.rs` as a case runner**

Use:

```rust
#[test]
fn limit_order_lock_cases_match_expected_outcomes() {
    for case in tests::fixtures::limit_order::lock_script_cases() {
        case.expected.assert(&case.fixture, &case.built);
        case.coverage.assert_has_required_tags();
    }
}
```

- [x] **Step 6: Run lock test**

Run: `cargo test -p tests --offline --test limit_order_lock`

Expected: PASS. The migrated fixture still covers malformed args, wrong NFT type, tx-level action, wrong target, append-scope input, payment mutations, missing/wrong buyer NFT, unknown/malformed action, two-lock-order payment reuse, distinct payments, and mixed type/lock duplicate payment.

- [x] **Step 7: Commit lock migration**

```bash
git add tests/src/fixtures/limit_order tests/tests/limit_order_lock.rs
git commit -m "refactor: migrate limit order lock tests"
```

### Task 11: Delete Old Helpers, Compatibility Paths, And Duplicate Code

**Files:**
- Delete: `tests/src/fixtures/otx_hash.rs`
- Delete or replace: old flat `tests/src/framework/cobuild.rs`
- Delete or replace: old flat `tests/src/framework/tx.rs`
- Delete or replace: old flat `tests/src/framework/signing.rs`
- Modify: `tests/src/fixtures/mod.rs`
- Modify: `tests/src/framework/mod.rs`
- Modify: `tests/src/tests.rs`
- Test: full tests crate and workspace

- [x] **Step 1: Remove old helper APIs**

Delete old APIs after call sites are gone:

```text
OtxTransactionBuilder
live_input
LimitOrderCobuildMessageExt::limit_order_fill(payment_output_index: u32, ...)
fixtures::otx_hash
OtxFixtureParts
OtxHashFixture
```

Keep a small `live_input` only if every caller is outside the new framework migration and the function is marked private to a compatibility-free fixture module. Prefer deleting it.

- [x] **Step 2: Remove framework self-test reverse dependency**

`tests/src/framework/mod.rs` self-tests must not import `crate::fixtures::limit_order`. Replace the old limit-order helper tests with dummy protocol tests using `MessageBuilder`, `OtxBuilder`, `TxShape`, and dummy scripts.

- [x] **Step 3: Add final API guards**

In `tests/src/tests.rs`, add guards:

```rust
assert_no_text("tests/src", "payment_output_index: u32");
assert_no_text("tests/src/framework", "crate::fixtures");
assert_no_text("tests/src/fixtures", "mod otx_hash");
assert_no_text("tests/src", "OtxTransactionBuilder");
```

- [x] **Step 4: Run required acceptance commands**

Run:

```bash
cargo test -p tests --offline --test cobuild_otx_lock
cargo test -p tests --offline --test limit_order_type
cargo test -p tests --offline --test limit_order_lock
cargo test --workspace --offline
```

Expected: all PASS.

- [x] **Step 5: Commit cleanup**

```bash
git add tests/src tests/tests
git commit -m "refactor: remove old test helper APIs"
```

### Task 12: Final Verification And Review Checklist

**Files:**
- Inspect: `tests/src/framework`
- Inspect: `tests/src/fixtures`
- Inspect: `tests/tests/cobuild_otx_lock.rs`
- Inspect: `tests/tests/limit_order_type.rs`
- Inspect: `tests/tests/limit_order_lock.rs`

- [x] **Step 1: Run dependency boundary scans**

Run:

```bash
rg -n "crate::fixtures|fixtures::|limit_order|test-udt|test-nft|input-type-proxy-lock|wrong-owner" tests/src/framework
rg -n "payment_output_index: u32|fill_action_data\\([^\\n]*u32|OtxTransactionBuilder|OtxFixtureParts|fixtures::otx_hash" tests/src tests/tests
rg -n "\\b[0-9]+\\s*,\\s*script_hash\\(&buyer_lock\\)|fill_action_data\\([0-9]" tests/src/fixtures/limit_order tests/tests
```

Expected: no matches except assertions in boundary tests that intentionally name forbidden strings.

- [x] **Step 2: Run required acceptance commands**

Run:

```bash
cargo test -p tests --offline --test cobuild_otx_lock
cargo test -p tests --offline --test limit_order_type
cargo test -p tests --offline --test limit_order_lock
cargo test --workspace --offline
```

Expected: all PASS.

- [x] **Step 3: Confirm coverage tags cover required migration priorities**

Run the fixture coverage unit test:

```bash
cargo test -p tests --offline --lib coverage_manifest
```

Expected: PASS and confirms tags exist for:

```text
magic payment output indexes removed
PaymentOutputInAnotherOtx
PaymentOutputInRemainder
ReusePaymentOutput
TxLevelActionInsteadOfOtxAction
OrderInputInAppendScope
resolved input facts used by signing
OtxTransactionBuilder removed
framework self-tests do not import fixtures
```

- [x] **Step 4: Request review before merge**

Ask for review with this checklist:

```text
Review focus:
- framework contains no named contract or business fixture semantics
- fixtures/common owns named contracts/personas/assets
- signing hash oracle is framework-owned and fed by BuiltTxShape/ResolvedInputFacts
- limit-order actions use OutputHandle/InputHandle, not raw usize indexes
- expected outcomes live in fixtures, not integration test files
- old helper APIs are deleted rather than kept as compatibility aliases
```

Expected: reviewer can validate the architecture by reading module boundaries and the acceptance commands.
