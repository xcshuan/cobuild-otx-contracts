# Current Script Context Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make `CurrentScriptContext::from_reader` the single production constructor that directly scans transaction script hashes while tracking current-script indices.

**Architecture:** Keep `CurrentScriptContext` as the owner of current-script indices and full-transaction action target hashes. Remove the production-visible `from_script_hashes` helper and keep test-only construction inside `context.rs` tests, where private fields are accessible. Keep `SyscallTxReader` free of script-hash fixtures.

**Tech Stack:** Rust, `alloc`, `BTreeSet`, `ckb-std` high-level syscall readers, cargo test.

---

### Task 1: Rewrite `CurrentScriptContext::from_reader`

**Files:**
- Modify: `crates/cobuild-core/src/context.rs`

- [ ] **Step 1: Remove the delegating constructor path**

Delete `CurrentScriptContext::from_script_hashes`. Keep `from_reader` as `pub(crate)` and make it construct the context directly.

- [ ] **Step 2: Implement direct input/output scanning**

Replace the current `from_reader` body with this shape:

```rust
pub(crate) fn from_reader(
    reader: &SyscallTxReader,
    current_script: CurrentScript,
) -> Result<Self, CoreError> {
    let counts = reader.counts();
    let mut context = Self {
        current_script,
        indices: CurrentScriptIndices::from_script(current_script),
        script_hashes: ScriptHashes::default(),
    };

    for index in 0..counts.inputs {
        let lock_hash = reader.input_lock_hash(index)?;
        context.push_input_lock_hash(index, lock_hash)?;

        if let Some(type_hash) = reader.input_type_hash(index)? {
            context.push_input_type_hash(index, type_hash)?;
        }
    }

    for index in 0..counts.outputs {
        if let Some(type_hash) = reader.output_type_hash(index)? {
            context.push_output_type_hash(index, type_hash)?;
        }
    }

    Ok(context)
}
```

- [ ] **Step 3: Split the internal push helpers by role**

Replace `push_input_script_hash` with two small helpers so the scan reads in the same order as the transaction data:

```rust
fn push_input_lock_hash(&mut self, index: usize, lock_hash: [u8; 32]) -> Result<(), CoreError> {
    self.script_hashes.input_locks.insert(lock_hash);
    if self.current_script == CurrentScript::InputLock(lock_hash) {
        self.indices.push_input(index)?;
    }
    Ok(())
}

fn push_input_type_hash(&mut self, index: usize, type_hash: [u8; 32]) -> Result<(), CoreError> {
    self.script_hashes.input_types.insert(type_hash);
    if self.current_script == CurrentScript::Type(type_hash) {
        self.indices.push_input(index)?;
    }
    Ok(())
}

fn push_output_type_hash(&mut self, index: usize, type_hash: [u8; 32]) -> Result<(), CoreError> {
    self.script_hashes.output_types.insert(type_hash);
    if self.current_script == CurrentScript::Type(type_hash) {
        self.indices.push_output(index)?;
    }
    Ok(())
}
```

- [ ] **Step 4: Run focused compile feedback**

Run:

```sh
cargo test -p cobuild-core context::tests --no-run
```

Expected: compile errors only from tests or engine helpers that still call `from_script_hashes`.

### Task 2: Move test construction inside `context.rs`

**Files:**
- Modify: `crates/cobuild-core/src/context.rs`
- Modify: `crates/cobuild-core/src/engine.rs`

- [ ] **Step 1: Replace `context.rs` test helper**

Change `context_from_script_hashes` to directly construct a `CurrentScriptContext` and feed the private push helpers:

```rust
fn context_from_script_hashes(
    current_script: CurrentScript,
    input_locks: Vec<[u8; 32]>,
    input_types: Vec<Option<[u8; 32]>>,
    output_types: Vec<Option<[u8; 32]>>,
) -> CurrentScriptContext {
    assert_eq!(input_locks.len(), input_types.len());
    let mut context = CurrentScriptContext {
        current_script,
        indices: CurrentScriptIndices::from_script(current_script),
        script_hashes: ScriptHashes::default(),
    };

    for (index, (lock_hash, type_hash)) in input_locks.into_iter().zip(input_types).enumerate() {
        context.push_input_lock_hash(index, lock_hash).unwrap();
        if let Some(type_hash) = type_hash {
            context.push_input_type_hash(index, type_hash).unwrap();
        }
    }

    for (index, type_hash) in output_types.into_iter().enumerate() {
        if let Some(type_hash) = type_hash {
            context.push_output_type_hash(index, type_hash).unwrap();
        }
    }

    context
}
```

- [ ] **Step 2: Remove cross-module script-hash construction from `engine.rs` tests**

Delete `test_lock_context`, `test_type_context`, and `test_context_with_scripts` from `engine.rs` tests if they only exist to cross-construct `CurrentScriptContext`.

- [ ] **Step 3: Move builder tests that depend only on current-script index behavior into `context.rs` coverage**

Keep or add context tests for the behavior currently covered by script hash fixtures:

```rust
#[test]
fn current_lock_outside_otx_ranges_uses_only_current_lock_indices() {
    let lock_a = hash(1);
    let lock_b = hash(2);
    let context = context_from_script_hashes(
        CurrentScript::InputLock(lock_a),
        alloc::vec![lock_b, lock_b, lock_a],
        alloc::vec![None, None, None],
        alloc::vec![],
    );
    let otx = otx_entry(crate::layout::OtxLayout {
        witness_index: 0,
        base_inputs: range(0, 1),
        append_inputs: range(1, 1),
        base_outputs: range(0, 0),
        append_outputs: range(0, 0),
        base_cell_deps: range(0, 0),
        append_cell_deps: range(0, 0),
        base_header_deps: range(0, 0),
        append_header_deps: range(0, 0),
    });

    assert_eq!(context.current_lock_outside_otx_ranges(&[otx]), Ok(true));
}
```

- [ ] **Step 4: Preserve engine tests that do not need script-hash fixtures**

Keep the message-origin and pure witness/OTX helper tests in `engine.rs`. If builder tests cannot be expressed without constructing `CurrentScriptContext`, remove them only when equivalent `context.rs` coverage exists for the context behavior they were protecting.

- [ ] **Step 5: Run package tests**

Run:

```sh
cargo test -p cobuild-core
```

Expected: all `cobuild-core` tests pass.

### Task 3: Add structural guard and full verification

**Files:**
- Modify: `tests/tests/contract_template_layout.rs`

- [ ] **Step 1: Extend layout test forbidden strings if needed**

In `cobuild_core_uses_concrete_flow_objects_without_scattered_flow_helpers`, add a forbidden check for `from_script_hashes` only if the source still risks reintroducing it. Do not forbid the term in docs.

```rust
for forbidden in [
    "pub input_locks:",
    "pub input_types:",
    "pub output_types:",
    "lock_input_indices",
    "let mut input_locks",
    "let mut input_types",
    "let mut output_types",
    "CurrentLockGroup",
    "current_lock_group",
    "ScriptHashIndices",
    "input_lock_indices",
    "input_type_indices",
    "output_type_indices",
    "from_script_hashes",
] {
```

- [ ] **Step 2: Run required formatting and tests**

Run:

```sh
cargo fmt --check
cargo test -p cobuild-core
cargo test --test contract_template_layout
git diff --check
cargo test
```

Expected: every command exits successfully.

- [ ] **Step 3: Run forbidden-residue scan**

Run:

```sh
rg "ScriptHashScan|target_hashes_for_tests|TargetHashesForTests|with_cell_script_hashes_for_tests|CellScriptHashesForTests|cell_script_hashes" crates/cobuild-core/src tests
```

Expected: no matches.

- [ ] **Step 4: Confirm reader remains clean**

Run:

```sh
rg "script_hash|cell_script_hash|for_tests" crates/cobuild-core/src/syscalls.rs
```

Expected: no script-hash fixture matches. Existing `from_cached_parts_for_tests` may remain because it only caches counts, transaction cursor, and tx hash.

- [ ] **Step 5: Final review**

Run:

```sh
git diff -- crates/cobuild-core/src/context.rs crates/cobuild-core/src/engine.rs tests/tests/contract_template_layout.rs
git status --short
```

Expected: diff shows direct `from_reader` scanning, no production `from_script_hashes`, no `SyscallTxReader` script-hash fixture, and only intended files are modified.
