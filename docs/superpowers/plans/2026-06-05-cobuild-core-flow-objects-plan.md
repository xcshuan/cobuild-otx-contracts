# Cobuild Core Flow Objects Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Reorganize the syscall-concrete Cobuild core around concrete flow objects instead of scattered helper functions, without reintroducing trait abstractions or compatibility shims.

**Architecture:** `CobuildContext` becomes the prepared validation state created from syscalls. `SyscallTxReader`, `TxScriptHashes`, `WitnessScan`, `LockPlanBuilder`, and `TypePlanBuilder` own the major flows as concrete structs with methods. Small leaf helpers such as seal lookup and Molecule view conversion remain plain functions.

**Tech Stack:** Rust `no_std`, `alloc`, `ckb-std` syscall/high-level helpers, Molecule lazy readers from `cobuild-types`, `cargo test`, `cargo clippy`, root `make build`.

**Cleanup Rule:** Do not keep compatibility aliases, wrapper facades, unused free functions, dead modules, stale tests, or documentation references for removed names. Each task that moves a flow into a concrete struct must delete the old implementation path in the same commit.

---

## File Structure

- Modify `crates/cobuild-core/src/lib.rs`
  - Keep `engine` public for the validation API.
  - Remove `mod flow` after moving its helpers onto `TxScriptHashes`.
  - Remove `mod message` and `pub mod prepare` after their remaining logic is moved or deleted.
  - Do not keep public aliases for removed names.
- Modify `crates/cobuild-core/src/syscalls.rs`
  - Introduce concrete `SyscallTxReader`.
  - Move counts cache and transaction access methods behind `SyscallTxReader` methods.
  - Make transaction access free functions private implementation helpers; callers outside `syscalls.rs` must use `SyscallTxReader`.
  - Keep low-level `SyscallBackedReader` and cursor construction private.
- Modify `crates/cobuild-core/src/context.rs`
  - Rename `ScriptHashIndex` to `TxScriptHashes`.
  - Add `TxScriptHashes::from_reader(&SyscallTxReader)`.
  - Move `flow.rs` query helpers into `TxScriptHashes` methods.
- Delete `crates/cobuild-core/src/flow.rs`
  - No free-standing flow helper module remains.
- Modify `crates/cobuild-core/src/witness.rs`
  - Keep public `parse_witness` test/API helper.
  - Add crate-private `WitnessScan`; keep `WitnessSummary` private to `witness.rs`.
  - Move witness summary and unique sighash-all message logic out of `engine.rs`.
- Modify `crates/cobuild-core/src/engine.rs`
  - Rename `PreparedCobuild` to `CobuildContext`.
  - Remove the empty `CobuildEngine` facade.
  - Add `CobuildContext::from_syscalls()`.
  - Introduce crate-private `LockPlanBuilder` and `TypePlanBuilder`.
  - Delete moved helper functions after their bodies are owned by `WitnessScan`, `TxScriptHashes`, or the plan builders.
- Modify `crates/cobuild-core/src/hash/mod.rs`
  - Accept `&SyscallTxReader` instead of `&TxCountsCache`.
  - Use reader methods for transaction/hash preimage reads.
- Delete `crates/cobuild-core/src/message.rs`
  - Move message target validation onto `TxScriptHashes::validate_message_targets`, then remove the old free-function wrapper module.
- Delete `crates/cobuild-core/src/prepare.rs`
  - Remove unused legacy preparation helper and `pub mod prepare`.
- Modify `contracts/cobuild-otx-lock/src/entry.rs`
  - Call `CobuildContext::from_syscalls()?.plan_lock_validation(current_script_hash)?`.
- Modify `tests/tests/contract_template_layout.rs`
  - Update architecture guards to require concrete flow objects and reject removed names.
- Modify `docs/CobuildAgentDevelopGuide.md`
  - Replace `CobuildEngine::prepare_from_syscalls()` references with `CobuildContext::from_syscalls()`.
  - Document the concrete flow objects and deleted helper module.

---

### Task 1: Rewrite Red Architecture Guards For Flow Objects

**Files:**
- Modify: `tests/tests/contract_template_layout.rs`
- Test: `tests/tests/contract_template_layout.rs`

- [ ] **Step 1: Update lock entry API guard**

In `cobuild_otx_lock_entry_owns_contract_flow`, replace the expected core API string:

```rust
"CobuildEngine::prepare_from_syscalls",
```

with:

```rust
"CobuildContext::from_syscalls",
```

Also add `"CobuildEngine"` to the forbidden list for `entry.rs`.

- [ ] **Step 2: Update syscall streaming guard**

In `cobuild_core_owns_syscall_streaming_without_full_transaction_load`, keep the full-transaction-load rejections. Replace the expected helper list with reader-oriented expectations:

```rust
for expected in [
    "pub(crate) struct SyscallTxReader",
    "impl SyscallTxReader",
    "struct SyscallBackedReader",
    "fn syscall_cursor(",
    "fn hash_transaction_cursor(",
    "fn map_syscall_read_error(",
    "high_level::load_tx_hash()",
    "high_level::load_cell_lock_hash(",
    "high_level::load_cell_type_hash(",
] {
    assert!(
        syscalls_rs.contains(expected),
        "syscalls.rs should keep syscall streaming helper {expected}"
    );
}
for forbidden in [
    "pub(crate) fn counts(",
    "pub(crate) fn witness_cursor(",
    "pub(crate) fn raw_input_cursor(",
    "pub(crate) fn resolved_input_output_cursor(",
] {
    assert!(
        !syscalls_rs.contains(forbidden),
        "syscall transaction access should be exposed through SyscallTxReader methods, not free helper {forbidden}"
    );
}
```

- [ ] **Step 3: Update module ownership guard**

In `cobuild_core_uses_explicit_signature_request_names`, stop requiring `message.rs`. Replace:

```rust
for module in ["message", "seal"] {
    assert!(
        lib_rs.contains(&format!("mod {module}")),
        "core should keep {module}.rs as a focused internal module"
    );
    assert!(
        core_src.join(format!("{module}.rs")).is_file(),
        "missing focused core module {module}.rs"
    );
}
```

with:

```rust
assert!(
    lib_rs.contains("mod seal"),
    "core should keep seal.rs as a focused internal module"
);
assert!(core_src.join("seal.rs").is_file(), "missing seal.rs");
assert!(
    !lib_rs.contains("mod message"),
    "message target validation should move onto TxScriptHashes"
);
assert!(
    !core_src.join("message.rs").exists(),
    "message.rs should be deleted after validation moves onto TxScriptHashes"
);
```

In the same test, replace the `context.rs` assertion that forbids `validate_message_targets` with an assertion that requires it:

```rust
assert!(
    context_rs.contains("validate_message_targets"),
    "TxScriptHashes should own message target validation"
);
```

- [ ] **Step 4: Replace prepare-module guard**

Replace the whole `cobuild_core_prepares_context_in_prepare_module` test with:

```rust
#[test]
fn cobuild_core_context_preparation_is_owned_by_engine_context() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");

    assert!(
        !core_src.join("prepare.rs").exists(),
        "unused prepare.rs should be deleted"
    );
    assert!(
        !core_src.join("loader.rs").exists(),
        "core loader.rs should not be reintroduced"
    );

    let lib_rs = fs::read_to_string(core_src.join("lib.rs")).expect("core lib.rs");
    assert!(
        !lib_rs.contains("pub mod prepare"),
        "core should not export unused prepare module"
    );
    assert!(
        !lib_rs.contains("pub mod loader"),
        "core should not export loader"
    );

    let context_rs = fs::read_to_string(core_src.join("context.rs")).expect("core context.rs");
    assert!(
        context_rs.contains("pub struct TxScriptHashes"),
        "context.rs should expose TxScriptHashes"
    );
    assert!(
        !context_rs.contains("ScriptHashIndex"),
        "ScriptHashIndex should be removed"
    );

    let engine_rs = fs::read_to_string(core_src.join("engine.rs")).expect("engine.rs");
    assert!(
        engine_rs.contains("pub struct CobuildContext"),
        "engine.rs should expose CobuildContext"
    );
    assert!(
        engine_rs.contains("pub fn from_syscalls()"),
        "CobuildContext should own syscall preparation"
    );
}
```

- [ ] **Step 5: Update concrete syscall reader guard**

In `cobuild_core_uses_concrete_syscall_reader_without_source_traits`, remove `format!("{}{}", "SyscallTx", "Reader")` from the forbidden list. Replace the expected `syscalls.rs` list with:

```rust
for expected in [
    "ckb_std",
    "pub(crate) struct SyscallTxReader",
    "impl SyscallTxReader",
    "SyscallBackedReader",
    "SyscallReadTarget",
    "fn counts(",
    "fn witness_cursor(",
    "fn raw_input_cursor(",
    "fn hash_transaction_cursor(",
    "fn resolved_input_output_cursor(",
    "fn input_lock_hash(",
] {
    assert!(
        syscalls_rs.contains(expected),
        "syscalls.rs should contain concrete reader implementation {expected}"
    );
}
```

- [ ] **Step 6: Add a guard for concrete flow objects**

Add this test to `tests/tests/contract_template_layout.rs`:

```rust
#[test]
fn cobuild_core_uses_concrete_flow_objects_without_scattered_flow_helpers() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");

    let syscalls_rs = fs::read_to_string(core_src.join("syscalls.rs")).expect("syscalls.rs");
    assert!(
        syscalls_rs.contains("pub(crate) struct SyscallTxReader"),
        "syscall tx access should be owned by SyscallTxReader"
    );
    assert!(
        syscalls_rs.contains("impl SyscallTxReader"),
        "SyscallTxReader should expose concrete reader methods"
    );

    let context_rs = fs::read_to_string(core_src.join("context.rs")).expect("context.rs");
    for expected in [
        "pub struct TxScriptHashes",
        "impl TxScriptHashes",
        "from_reader",
        "SyscallTxReader",
        "first_input_with_lock",
        "lock_in_input_range",
        "type_relation_for_otx",
        "lock_group_fully_covered_by_otx",
        "validate_message_targets",
    ] {
        assert!(
            context_rs.contains(expected),
            "TxScriptHashes should own script-hash flow method {expected}"
        );
    }

    let witness_rs = fs::read_to_string(core_src.join("witness.rs")).expect("witness.rs");
    for expected in [
        "pub(crate) struct WitnessScan",
        "enum WitnessSummary",
        "impl WitnessScan",
        "push_witness",
        "tx_level_carrier_has_sighash_all_layout",
        "unique_sighash_all_message",
        "unique_sighash_all_message_with_index",
    ] {
        assert!(
            witness_rs.contains(expected),
            "WitnessScan should own witness scan method {expected}"
        );
    }

    let engine_rs = fs::read_to_string(core_src.join("engine.rs")).expect("engine.rs");
    for expected in [
        "pub struct CobuildContext",
        "impl CobuildContext",
        "from_syscalls()",
        "struct LockPlanBuilder",
        "LockPlanBuilder",
        "struct TypePlanBuilder",
        "TypePlanBuilder",
    ] {
        assert!(
            engine_rs.contains(expected),
            "engine.rs should expose concrete flow object {expected}"
        );
    }

    let lib_rs = fs::read_to_string(core_src.join("lib.rs")).expect("lib.rs");
    assert!(
        !core_src.join("flow.rs").exists(),
        "flow.rs should be deleted after its logic moves onto TxScriptHashes"
    );
    assert!(
        !lib_rs.contains("mod flow"),
        "lib.rs should not keep the deleted flow module"
    );

    for forbidden in [
        "pub struct CobuildEngine;",
        "PreparedCobuild",
        "ScriptHashIndex",
        "crate::flow::",
        "TxCountsCache",
    ] {
        assert!(
            !engine_rs.contains(forbidden),
            "engine.rs should not keep old scattered flow name {forbidden}"
        );
    }
}
```

- [ ] **Step 7: Run the red guards**

Run:

```bash
cargo test -p tests --offline --test contract_template_layout
```

Expected: FAIL because the guards now require deleted `prepare.rs`/`message.rs`, `SyscallTxReader`, `TxScriptHashes`, `WitnessScan`, `CobuildContext`, `LockPlanBuilder`, and `TypePlanBuilder`.

- [ ] **Step 8: Commit the red guards**

```bash
git add tests/tests/contract_template_layout.rs
git commit -m "test: require concrete cobuild flow objects"
```

---

### Task 2: Introduce `SyscallTxReader`

**Files:**
- Modify: `crates/cobuild-core/src/syscalls.rs`
- Modify: `crates/cobuild-core/src/hash/mod.rs`
- Modify: `crates/cobuild-core/src/engine.rs`
- Test: `crates/cobuild-core/src/syscalls.rs`

- [ ] **Step 1: Add the reader struct**

In `crates/cobuild-core/src/syscalls.rs`, add the concrete reader while keeping `TxCountsCache` private to the module:

```rust
#[derive(Default)]
pub(crate) struct SyscallTxReader {
    counts_cache: TxCountsCache,
}

impl SyscallTxReader {
    pub(crate) fn counts(&self) -> Result<TxCounts, CoreError> {
        counts(&self.counts_cache)
    }

    pub(crate) fn tx_hash(&self) -> Result<[u8; 32], CoreError> {
        tx_hash()
    }

    pub(crate) fn witness_cursor(&self, absolute_index: usize) -> Result<Cursor, CoreError> {
        witness_cursor(absolute_index)
    }

    pub(crate) fn raw_input_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        raw_input_cursor(index)
    }

    pub(crate) fn raw_output_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        raw_output_cursor(index)
    }

    pub(crate) fn raw_output_data_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        raw_output_data_cursor(index)
    }

    pub(crate) fn raw_cell_dep_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        raw_cell_dep_cursor(index)
    }

    pub(crate) fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        raw_header_dep_hash(index)
    }

    pub(crate) fn resolved_input_output_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        resolved_input_output_cursor(index)
    }

    pub(crate) fn resolved_input_data_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        resolved_input_data_cursor(index)
    }

    pub(crate) fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        input_lock_hash(index)
    }

    pub(crate) fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        input_type_hash(index)
    }

    pub(crate) fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        output_type_hash(index)
    }
}
```

After adding these methods, remove `pub(crate)` visibility from the free helper functions used by the reader. `counts`, `witness_cursor`, `raw_input_cursor`, `raw_output_cursor`, `raw_output_data_cursor`, `raw_cell_dep_cursor`, `raw_header_dep_hash`, `resolved_input_output_cursor`, `resolved_input_data_cursor`, `tx_hash`, `input_lock_hash`, `input_type_hash`, and `output_type_hash` should become private `fn` helpers.

- [ ] **Step 2: Store `SyscallTxReader` in the prepared state**

In `crates/cobuild-core/src/engine.rs`, replace:

```rust
pub(crate) counts_cache: TxCountsCache,
```

with:

```rust
pub(crate) tx: syscalls::SyscallTxReader,
```

In `CobuildEngine::prepare_from_syscalls()`, replace:

```rust
let counts_cache = TxCountsCache::default();
let counts = syscalls::counts(&counts_cache)?;
let script_hashes = script_hashes_from_syscalls(counts)?;
```

with:

```rust
let tx = syscalls::SyscallTxReader::default();
let counts = tx.counts()?;
let script_hashes = script_hashes_from_syscalls(&tx, counts)?;
```

Replace `syscalls::witness_cursor(index)?` with `tx.witness_cursor(index)?` in preparation. Store `tx` in `PreparedCobuild`.

Change `script_hashes_from_syscalls` to:

```rust
fn script_hashes_from_syscalls(
    tx: &syscalls::SyscallTxReader,
    counts: TxCounts,
) -> Result<ScriptHashIndex, CoreError>
```

and replace calls to `syscalls::input_lock_hash`, `syscalls::input_type_hash`, and `syscalls::output_type_hash` with reader methods.

- [ ] **Step 3: Update hash functions to accept the reader**

In `crates/cobuild-core/src/hash/mod.rs`, change function signatures from `&syscalls::TxCountsCache` to `&syscalls::SyscallTxReader`:

```rust
pub(crate) fn tx_without_message_hash(reader: &syscalls::SyscallTxReader) -> Result<[u8; 32], CoreError>
pub(crate) fn tx_with_message_hash(message: &Cursor, reader: &syscalls::SyscallTxReader) -> Result<[u8; 32], CoreError>
fn tx_signing_hash(personalization: &[u8; 16], message: Option<&Cursor>, reader: &syscalls::SyscallTxReader) -> Result<[u8; 32], CoreError>
pub(crate) fn otx_base_hash(otx: &OtxView, layout: &OtxLayout, reader: &syscalls::SyscallTxReader) -> Result<[u8; 32], CoreError>
pub(crate) fn otx_append_hash(otx: &OtxView, layout: &OtxLayout, reader: &syscalls::SyscallTxReader, base_hash: [u8; 32]) -> Result<[u8; 32], CoreError>
```

Inside those functions, replace calls such as `syscalls::tx_hash()?` with `reader.tx_hash()?`, and `syscalls::raw_output_cursor(tx_index)?` with `reader.raw_output_cursor(tx_index)?`.

- [ ] **Step 4: Update engine hash call sites**

In `engine.rs`, replace every `&self.counts_cache` hash argument with `&self.tx`:

```rust
otx_base_hash(&otx.witness, &otx.layout, &self.tx)?
otx_append_hash(&otx.witness, &otx.layout, &self.tx, base_hash)?
tx_with_message_hash(message, &self.tx)?
tx_without_message_hash(&self.tx)?
```

Also replace `syscalls::witness_cursor(carrier_witness_index)?` with `self.tx.witness_cursor(carrier_witness_index)?` in tx-level lock planning.

- [ ] **Step 5: Run targeted tests**

Run:

```bash
cargo test -p cobuild-core --offline syscalls::tests::cached_counts_are_returned_without_recomputing
cargo check -p cobuild-core --offline
cargo test -p tests --offline --test contract_template_layout cobuild_core_uses_concrete_flow_objects_without_scattered_flow_helpers
```

Expected: the syscalls unit test and core compile pass; the architecture guard still fails on later missing flow objects.

- [ ] **Step 6: Commit**

```bash
git add crates/cobuild-core/src/syscalls.rs crates/cobuild-core/src/hash/mod.rs crates/cobuild-core/src/engine.rs
git commit -m "refactor: introduce concrete syscall tx reader"
```

---

### Task 3: Rename Script Hash State To `TxScriptHashes`

**Files:**
- Modify: `crates/cobuild-core/src/context.rs`
- Modify: `crates/cobuild-core/src/engine.rs`
- Modify: `crates/cobuild-core/src/lib.rs`
- Delete: `crates/cobuild-core/src/flow.rs`
- Delete: `crates/cobuild-core/src/message.rs`
- Test: `tests/tests/contract_template_layout.rs`
- Test: `crates/cobuild-core/src/context.rs`

- [ ] **Step 1: Move flow helpers into `TxScriptHashes`**

Replace `crates/cobuild-core/src/context.rs` with a struct named `TxScriptHashes`. Implement `pub(crate) fn from_reader(reader: &SyscallTxReader)`, `first_input_with_lock`, `lock_in_input_range`, `type_in_input_range`, `type_in_output_range`, `type_relation_for_otx`, `type_hash_present`, `type_hash_outside_otx_ranges`, `lock_group_fully_covered_by_otx`, and `validate_message_targets`.

Move the bodies from `flow.rs` and `message.rs` into these methods. `type_relation_for_otx` constructs `OtxTypeRelation` using the same base/append input/output checks currently in `PreparedCobuild::plan_type_validation`. `validate_message_targets` keeps the existing `ScriptRole` matching logic and checks `self.input_locks`, `self.input_types`, and `self.output_types`.

- [ ] **Step 2: Update callers**

In `engine.rs`, replace:

```rust
crate::flow::script_in_input_range(&self.script_hashes.input_locks, range, lock_script_hash)
```

with:

```rust
self.script_hashes.lock_in_input_range(range, lock_script_hash)
```

Do the same for type range checks, coverage checks, and message target validation.

- [ ] **Step 3: Add focused context tests**

Add `#[cfg(test)]` tests in `crates/cobuild-core/src/context.rs` for the pure range/coverage helpers. The tests should build `TxScriptHashes` directly with small vectors and assert:

```rust
assert_eq!(hashes.first_input_with_lock(lock_a), Some(0));
assert!(hashes.lock_in_input_range(Range { start: 0, count: 1 }, lock_a));
assert!(!hashes.lock_in_input_range(Range { start: 1, count: 1 }, lock_a));
assert!(hashes.type_hash_present(type_a));
assert!(hashes.lock_group_fully_covered_by_otx(lock_a, &[layout_covering_lock_a]));
assert!(!hashes.lock_group_fully_covered_by_otx(lock_a, &[]));
```

Keep the tests local to `context.rs` so private fields remain testable without making new public constructors.

- [ ] **Step 4: Delete old modules**

Delete `crates/cobuild-core/src/flow.rs` and remove `mod flow;` from `crates/cobuild-core/src/lib.rs`.

Delete `crates/cobuild-core/src/message.rs` and remove `mod message;` from `crates/cobuild-core/src/lib.rs`.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p cobuild-core --offline context
cargo test -p tests --offline --test contract_template_layout cobuild_core_uses_concrete_flow_objects_without_scattered_flow_helpers
```

Expected: still FAIL because `WitnessScan`, `CobuildContext`, and plan builders are not complete.

- [ ] **Step 6: Commit**

```bash
git add crates/cobuild-core/src/context.rs crates/cobuild-core/src/message.rs crates/cobuild-core/src/engine.rs crates/cobuild-core/src/lib.rs crates/cobuild-core/src/flow.rs
git commit -m "refactor: move script hash flow onto context object"
```

---

### Task 4: Extract `WitnessScan`

**Files:**
- Modify: `crates/cobuild-core/src/witness.rs`
- Modify: `crates/cobuild-core/src/engine.rs`
- Test: `crates/cobuild-core/tests/witness.rs`
- Test: `crates/cobuild-core/src/witness.rs`

- [ ] **Step 1: Add `WitnessScan`**

In `crates/cobuild-core/src/witness.rs`, add:

```rust
pub(crate) struct WitnessScan {
    summaries: Vec<WitnessSummary>,
}

#[derive(Clone)]
enum WitnessSummary {
    Empty,
    Other,
    Malformed(CoreError),
    SighashAll { message: Cursor },
    SighashAllOnly,
}
```

Move `witness_summary`, `has_tx_level_witness_id`, `unique_sighash_all_message_from_summaries`, and `unique_sighash_all_message_with_index_from_summaries` from `engine.rs` into methods on `WitnessScan`.

- [ ] **Step 2: Implement scanner mutation and queries**

Add:

```rust
impl WitnessScan {
    pub(crate) fn with_capacity(capacity: usize) -> Self {
        Self {
            summaries: Vec::with_capacity(capacity),
        }
    }

    pub(crate) fn push_witness(&mut self, witness: &[u8]) -> Result<(), CoreError> {
        self.summaries.push(Self::summarize_witness(witness)?);
        Ok(())
    }

    pub(crate) fn tx_level_carrier_has_sighash_all_layout(
        &self,
        index: usize,
    ) -> Result<bool, CoreError> {
        match self.summaries.get(index) {
            Some(WitnessSummary::SighashAll { .. }) | Some(WitnessSummary::SighashAllOnly) => {
                Ok(true)
            }
            Some(WitnessSummary::Malformed(error)) => Err(error.clone()),
            Some(WitnessSummary::Empty | WitnessSummary::Other) | None => Ok(false),
        }
    }

    pub(crate) fn unique_sighash_all_message(&self) -> Result<Option<Cursor>, CoreError> {
        let mut message = None;
        for summary in &self.summaries {
            match summary {
                WitnessSummary::SighashAll { message: candidate } => {
                    if message.is_some() {
                        return Err(CoreError::DuplicateSighashAll);
                    }
                    message = Some(candidate.clone());
                }
                WitnessSummary::Malformed(error) => return Err(error.clone()),
                _ => {}
            }
        }
        Ok(message)
    }

    pub(crate) fn unique_sighash_all_message_with_index(
        &self,
    ) -> Result<Option<(usize, Cursor)>, CoreError> {
        let mut message = None;
        for (index, summary) in self.summaries.iter().enumerate() {
            match summary {
                WitnessSummary::SighashAll { message: candidate } => {
                    if message.is_some() {
                        return Err(CoreError::DuplicateSighashAll);
                    }
                    message = Some((index, candidate.clone()));
                }
                WitnessSummary::Malformed(error) => return Err(error.clone()),
                _ => {}
            }
        }
        Ok(message)
    }
}
```

- [ ] **Step 3: Update engine callers**

Replace `self.witness_summaries` with `self.witnesses` and call:

```rust
self.witnesses.unique_sighash_all_message()?
self.witnesses.unique_sighash_all_message_with_index()?
self.witnesses.tx_level_carrier_has_sighash_all_layout(carrier_witness_index)?
```

Do not make `WitnessSummary` `pub(crate)` just so `engine.rs` can match variants.

- [ ] **Step 4: Add focused witness scan tests**

Add `#[cfg(test)]` tests in `crates/cobuild-core/src/witness.rs` for at least:

```rust
#[test]
fn tx_level_carrier_returns_false_for_empty_or_other_witness() {
    let mut scan = WitnessScan::with_capacity(1);
    scan.push_witness(&[]).unwrap();
    assert_eq!(scan.tx_level_carrier_has_sighash_all_layout(0), Ok(false));
    assert_eq!(scan.tx_level_carrier_has_sighash_all_layout(1), Ok(false));
}
```

Keep existing external `parse_witness` tests unchanged.

- [ ] **Step 5: Run tests**

```bash
cargo test -p cobuild-core --offline witness
cargo test -p cobuild-core --offline --test witness
cargo test -p tests --offline --test contract_template_layout cobuild_core_uses_concrete_flow_objects_without_scattered_flow_helpers
```

Expected: witness tests pass; architecture guard still fails until `CobuildContext` and builders are implemented.

- [ ] **Step 6: Commit**

```bash
git add crates/cobuild-core/src/witness.rs crates/cobuild-core/src/engine.rs
git commit -m "refactor: extract cobuild witness scan"
```

---

### Task 5: Rename Prepared State To `CobuildContext`

**Files:**
- Modify: `crates/cobuild-core/src/engine.rs`
- Modify: `crates/cobuild-core/src/lib.rs`
- Delete: `crates/cobuild-core/src/prepare.rs`
- Modify: `contracts/cobuild-otx-lock/src/entry.rs`
- Modify: `tests/tests/contract_template_layout.rs`
- Modify: `docs/CobuildAgentDevelopGuide.md`

- [ ] **Step 1: Replace the engine facade**

In `engine.rs`, replace:

```rust
pub struct CobuildEngine;
pub struct PreparedCobuild {
    pub(crate) tx: syscalls::SyscallTxReader,
    pub(crate) script_hashes: TxScriptHashes,
    witnesses: WitnessScan,
    pub(crate) layout_scan: OtxLayoutScan,
}
```

with:

```rust
pub struct CobuildContext {
    pub(crate) tx: SyscallTxReader,
    pub(crate) script_hashes: TxScriptHashes,
    witnesses: WitnessScan,
    pub(crate) layout_scan: OtxLayoutScan,
}
```

Move `prepare_from_syscalls()` to:

```rust
impl CobuildContext {
    pub fn from_syscalls() -> Result<Self, CoreError> {
        let tx = SyscallTxReader::default();
        let counts = tx.counts()?;
        let script_hashes = TxScriptHashes::from_reader(&tx)?;
        let mut witnesses = WitnessScan::with_capacity(counts.witnesses);
        let mut layout_collector = OtxLayoutCollector::new();
        for index in 0..counts.witnesses {
            let witness = tx.witness_cursor(index)?;
            let witness = cursor_bytes_with_error(&witness, CoreError::MissingHashInput)?;
            witnesses.push_witness(&witness)?;
            layout_collector.push_witness(&witness);
        }
        let layout_scan = layout_collector.finish(
            counts.inputs,
            counts.outputs,
            counts.cell_deps,
            counts.header_deps,
        );
        Ok(Self {
            tx,
            script_hashes,
            witnesses,
            layout_scan,
        })
    }
}
```

- [ ] **Step 2: Delete unused prepare module**

Delete `crates/cobuild-core/src/prepare.rs` and remove `pub mod prepare;` from `crates/cobuild-core/src/lib.rs`. `script_args_from_slice` is not used by current production or tests; do not keep it as a compatibility helper.

- [ ] **Step 3: Update lock entry**

In `contracts/cobuild-otx-lock/src/entry.rs`, replace:

```rust
use cobuild_core::engine::CobuildEngine;
let plan = CobuildEngine::prepare_from_syscalls()?.plan_lock_validation(current_script_hash)?;
```

with:

```rust
use cobuild_core::engine::CobuildContext;
let plan = CobuildContext::from_syscalls()?.plan_lock_validation(current_script_hash)?;
```

- [ ] **Step 4: Update docs and guards**

Replace `CobuildEngine::prepare_from_syscalls()` with `CobuildContext::from_syscalls()` in `docs/CobuildAgentDevelopGuide.md` and `tests/tests/contract_template_layout.rs`.

In `docs/CobuildAgentDevelopGuide.md`, also remove the stale claim that `crates/cobuild-core/src/prepare.rs` owns context preparation. Replace it with `crates/cobuild-core/src/engine.rs` and `CobuildContext::from_syscalls()`.

- [ ] **Step 5: Run targeted compile**

```bash
cargo check --workspace --offline
cargo test -p tests --offline --test contract_template_layout
```

Expected: `cargo check` passes. `contract_template_layout` may still fail only on the Task 6 `LockPlanBuilder` / `TypePlanBuilder` requirements; all other architecture guards should pass after updating call sites.

- [ ] **Step 6: Commit**

```bash
git add crates/cobuild-core/src/engine.rs crates/cobuild-core/src/lib.rs crates/cobuild-core/src/prepare.rs contracts/cobuild-otx-lock/src/entry.rs tests/tests/contract_template_layout.rs docs/CobuildAgentDevelopGuide.md
git commit -m "refactor: rename prepared cobuild state to context"
```

---

### Task 6: Extract Lock And Type Plan Builders

**Files:**
- Modify: `crates/cobuild-core/src/engine.rs`
- Test: `tests/tests/contract_template_layout.rs`
- Test: `tests/tests/cobuild_otx_lock.rs`

- [ ] **Step 1: Add `LockPlanBuilder`**

In `engine.rs`, keep the public method small:

```rust
impl CobuildContext {
    pub fn plan_lock_validation(
        &self,
        lock_script_hash: [u8; 32],
    ) -> Result<LockValidationPlan, CoreError> {
        LockPlanBuilder::new(self, lock_script_hash).build()
    }
}
```

Move lock planning internals into:

```rust
struct LockPlanBuilder<'a> {
    context: &'a CobuildContext,
    lock_script_hash: [u8; 32],
    required_signatures: Vec<SigningRequirement>,
}

impl<'a> LockPlanBuilder<'a> {
    fn new(context: &'a CobuildContext, lock_script_hash: [u8; 32]) -> Self {
        Self {
            context,
            lock_script_hash,
            required_signatures: Vec::new(),
        }
    }

    fn build(mut self) -> Result<LockValidationPlan, CoreError> {
        self.add_tx_level_requirement()?;
        self.add_otx_requirements()?;
        self.ensure_otx_lock_group_coverage()?;
        Ok(LockValidationPlan {
            lock_script_hash: self.lock_script_hash,
            required_signatures: self.required_signatures,
        })
    }
}
```

Add private methods `add_tx_level_requirement`, `add_otx_requirements`, and `ensure_otx_lock_group_coverage` immediately below this impl block. Their bodies are direct moves from the existing `tx_level_lock_requirements`, OTX loop, and lock-group coverage section in `PreparedCobuild::plan_lock_validation`.

- [ ] **Step 2: Add `TypePlanBuilder`**

Keep the public method small:

```rust
impl CobuildContext {
    pub fn plan_type_validation(
        &self,
        type_script_hash: [u8; 32],
    ) -> Result<TypeValidationPlan, CoreError> {
        TypePlanBuilder::new(self, type_script_hash).build()
    }
}
```

Move type planning internals into:

```rust
struct TypePlanBuilder<'a> {
    context: &'a CobuildContext,
    type_script_hash: [u8; 32],
    related_messages: Vec<RelatedMessage>,
}

impl<'a> TypePlanBuilder<'a> {
    fn new(context: &'a CobuildContext, type_script_hash: [u8; 32]) -> Self {
        Self {
            context,
            type_script_hash,
            related_messages: Vec::new(),
        }
    }

    fn build(mut self) -> Result<TypeValidationPlan, CoreError> {
        let tx_level_type_relevant = self.add_otx_related_messages()?;
        self.add_tx_level_message_if_relevant(tx_level_type_relevant)?;
        Ok(TypeValidationPlan {
            type_script_hash: self.type_script_hash,
            related_messages: self.related_messages,
        })
    }
}
```

Add private methods `add_otx_related_messages`, `tx_level_type_relevant_from_invalid_or_none_layout`, and `add_tx_level_message_if_relevant` immediately below this impl block. Their bodies are direct moves from the existing complete/invalid/none layout branches and tx-level message section in `PreparedCobuild::plan_type_validation`.

- [ ] **Step 3: Update hash calls**

Use the reader stored on context:

```rust
otx_base_hash(&otx.witness, &otx.layout, &self.context.tx)?
otx_append_hash(&otx.witness, &otx.layout, &self.context.tx, base_hash)?
tx_with_message_hash(message, &self.context.tx)?
tx_without_message_hash(&self.context.tx)?
```

- [ ] **Step 4: Run guard and integration target**

```bash
cargo test -p tests --offline --test contract_template_layout cobuild_core_uses_concrete_flow_objects_without_scattered_flow_helpers
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: architecture guard passes; integration test passes if `build/debug/cobuild-otx-lock` already exists. If the binary is missing, run `make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline` and rerun the integration test.

- [ ] **Step 5: Commit**

```bash
git add crates/cobuild-core/src/engine.rs crates/cobuild-core/src/hash/mod.rs
git commit -m "refactor: extract cobuild validation plan builders"
```

---

### Task 7: Final Cleanup And Verification

**Files:**
- Modify: `docs/CobuildAgentDevelopGuide.md`
- Test: workspace

- [ ] **Step 1: Search for removed names**

Run:

```bash
rg -n "CobuildEngine|PreparedCobuild|ScriptHashIndex|crate::flow::|mod flow|mod message|pub mod prepare|prepare\\.rs" crates contracts tests docs/CobuildAgentDevelopGuide.md
```

Expected: no matches for removed production names or deleted modules. If `TxCountsCache` remains in `syscalls.rs`, keep it private and verify it does not appear outside that file:

```bash
rg -n "TxCountsCache" crates/cobuild-core/src/engine.rs crates/cobuild-core/src/hash crates/cobuild-core/src/context.rs crates/cobuild-core/src/witness.rs
```

Expected: no matches.

- [ ] **Step 2: Search for redundant files, wrappers, and free helpers**

Run:

```bash
test ! -f crates/cobuild-core/src/flow.rs
test ! -f crates/cobuild-core/src/message.rs
test ! -f crates/cobuild-core/src/prepare.rs
rg -n "pub struct CobuildEngine;|type .*CobuildEngine|type .*PreparedCobuild|pub use .*CobuildContext|pub use .*TxScriptHashes" crates/cobuild-core/src contracts/cobuild-otx-lock/src tests docs/CobuildAgentDevelopGuide.md
rg -n "pub\\(crate\\) fn (counts|witness_cursor|raw_input_cursor|raw_output_cursor|raw_output_data_cursor|raw_cell_dep_cursor|raw_header_dep_hash|resolved_input_output_cursor|resolved_input_data_cursor|tx_hash|input_lock_hash|input_type_hash|output_type_hash)\\(" crates/cobuild-core/src/syscalls.rs
rg -n "pub\\(crate\\)? fn validate_message_targets\\(" crates/cobuild-core/src
```

Expected: each `test` exits successfully. The alias/facade search and syscall free-helper search print no matches. The `validate_message_targets` search may print only the `TxScriptHashes` method in `context.rs`; it must not print a free function in a separate module.

- [ ] **Step 3: Run full verification**

```bash
cargo clippy --workspace --all-targets --offline
cargo test --workspace --offline
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
git diff --check
```

Expected: all commands pass; debug integration reports 8 passing tests.

- [ ] **Step 4: Commit final docs/cleanup if needed**

```bash
git add docs/CobuildAgentDevelopGuide.md tests/tests/contract_template_layout.rs crates/cobuild-core/src contracts/cobuild-otx-lock/src
git commit -m "docs: document cobuild flow objects"
```

Only create this commit if Task 7 changed files after previous commits.

---

## Self-Review

- Spec coverage: The plan covers concrete `SyscallTxReader`, `TxScriptHashes`, `WitnessScan`, `CobuildContext`, `LockPlanBuilder`, `TypePlanBuilder`, deletion of `flow.rs`, and updated lock entry API.
- Placeholder scan: No `TBD`, `TODO`, or deferred compatibility work remains.
- Type consistency: The same names are used throughout: `CobuildContext::from_syscalls`, `SyscallTxReader`, `TxScriptHashes`, `WitnessScan`, `LockPlanBuilder`, and `TypePlanBuilder`.
