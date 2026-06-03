# Cobuild Core Streaming Reader And View Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace owned transaction/hash inputs with streaming cursor-backed sources, then make `view.rs` a clean cursor-backed protocol view boundary.

**Architecture:** Round 1 introduces reader/source boundaries and migrates all query hash construction to `SigningDataSource` in one commit so tx-level and OTX hashing do not diverge. Round 2 replaces owned `*Data` view DTOs with cursor-backed `*View` types while preserving protocol semantics, hash bytes, ABI, and exit-code categories.

**Tech Stack:** Rust `no_std` with `alloc`, `cobuild_types::lazy_reader`, Molecule `Cursor`, `blake2b-ref`, `ckb-std`, workspace Cargo tests, CKB debug contract build.

---

## Guardrails From Review

- Do not add structural tests that knowingly fail across committed intermediate states. Introduce each structural assertion in the task that satisfies it.
- Do not change `required_signatures` to source-only until both tx-level and OTX hash paths can use `SigningDataSource`.
- Keep old `parse_transaction_info(data)`, `PreparedContextInput`, and `prepare_context` until the lock crate no longer needs the owned loading path.
- Use absolute transaction witness indexes. Tx-level trailing witness hashing iterates `input_count..witness_count`.
- Source-backed cursors must carry read-error context. Lazy-reader `Cursor::read_at` errors alone are not enough to preserve public error categories.
- Cursor-backed view structs must not derive traits that `Cursor` does not implement.

## File Map

- Create: `crates/cobuild-core/src/reader.rs`
- Create: `crates/cobuild-core/src/source.rs`
- Move: `crates/cobuild-core/src/loader.rs` to `crates/cobuild-core/src/prepare.rs`
- Move: `contracts/cobuild-otx-lock/src/loader.rs` to `contracts/cobuild-otx-lock/src/chain.rs`
- Modify: `crates/cobuild-core/src/{lib.rs,context.rs,hash.rs,layout.rs,message.rs,otx_request.rs,query.rs,seal.rs,sighash.rs,view.rs,witness.rs}`
- Modify: `contracts/cobuild-otx-lock/src/{entry.rs,lib.rs,error.rs,errors.rs}`
- Modify: `crates/cobuild-core/tests/{hash.rs,layout.rs,no_entity_dependency.rs,signature_requests.rs,view.rs,witness.rs}`
- Modify: `tests/tests/{cobuild_otx_lock.rs,contract_template_layout.rs,workspace_layout.rs}`
- Modify: `tests/src/lib.rs`

## Round 1: Streaming Source And Hash Input Boundary

### Task 1: Move Reader Helpers Out Of `view.rs`

**Files:**
- Create: `crates/cobuild-core/src/reader.rs`
- Modify: `crates/cobuild-core/src/lib.rs`
- Modify: `crates/cobuild-core/src/{view.rs,hash.rs,loader.rs}`
- Modify: `crates/cobuild-core/tests/{view.rs,hash.rs}`
- Modify: `tests/tests/contract_template_layout.rs`

- [ ] **Step 1: Add structural assertions for reader extraction only**

In `tests/tests/contract_template_layout.rs`, add:

```rust
#[test]
fn cobuild_core_reader_helpers_are_not_owned_by_view() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");

    assert!(core_src.join("reader.rs").is_file(), "reader.rs must own cursor helpers");
    let lib_rs = fs::read_to_string(core_src.join("lib.rs")).expect("core lib.rs");
    assert!(lib_rs.contains("pub mod reader"), "core should export reader helpers");

    let reader_rs = fs::read_to_string(core_src.join("reader.rs")).expect("reader.rs");
    for expected in ["OwnedReader", "cursor_from_slice", "cursor_bytes", "update_cursor"] {
        assert!(reader_rs.contains(expected), "reader.rs should define {expected}");
    }

    let view_rs = fs::read_to_string(core_src.join("view.rs")).expect("view.rs");
    for forbidden in [
        "struct OwnedReader",
        "fn cursor_from_slice",
        "fn cursor_bytes",
        "fn update_cursor",
    ] {
        assert!(!view_rs.contains(forbidden), "view.rs must not define {forbidden}");
    }
}
```

- [ ] **Step 2: Run failing structural test**

Run:

```bash
cargo test -p tests --offline --test contract_template_layout cobuild_core_reader_helpers_are_not_owned_by_view
```

Expected: fails because `reader.rs` does not exist yet.

- [ ] **Step 3: Create `reader.rs`**

Move `OwnedReader`, `cursor_from_slice`, `cursor_bytes`, and `update_cursor` from `view.rs` into `crates/cobuild-core/src/reader.rs`. Add:

```rust
pub fn update_len_prefixed_cursor(
    hasher: &mut blake2b_ref::Blake2b,
    cursor: &Cursor,
    read_error: CoreError,
) -> Result<(), CoreError> {
    hasher.update(&crate::hash::checked_len_prefix(cursor.size)?);
    update_cursor_with_error(hasher, cursor, read_error)
}

pub fn update_cursor_with_error(
    hasher: &mut blake2b_ref::Blake2b,
    cursor: &Cursor,
    read_error: CoreError,
) -> Result<(), CoreError> {
    let mut offset = 0usize;
    let mut buf = [0u8; 256];
    while offset < cursor.size {
        let read_len = core::cmp::min(buf.len(), cursor.size - offset);
        let mut chunk = cursor.clone();
        chunk.add_offset(offset).map_err(|_| read_error.clone())?;
        chunk.size = read_len;
        let read = chunk.read_at(&mut buf[..read_len]).map_err(|_| read_error.clone())?;
        if read != read_len {
            return Err(read_error);
        }
        hasher.update(&buf[..read_len]);
        offset = offset.checked_add(read_len).ok_or(CoreError::MalformedCobuild)?;
    }
    Ok(())
}
```

Keep `update_cursor` as a protocol helper:

```rust
pub fn update_cursor(
    hasher: &mut blake2b_ref::Blake2b,
    cursor: &Cursor,
) -> Result<(), CoreError> {
    update_cursor_with_error(hasher, cursor, CoreError::MalformedCobuild)
}
```

- [ ] **Step 4: Export and update imports**

In `lib.rs`, add:

```rust
pub mod reader;
```

Update imports:

- `view.rs` imports `cursor_bytes` and `cursor_from_slice` from `crate::reader`;
- `hash.rs` imports `cursor_from_slice`, `update_cursor`, `update_cursor_with_error`, and `update_len_prefixed_cursor` from `crate::reader`;
- `loader.rs` imports `cursor_bytes` and `cursor_from_slice` from `crate::reader`;
- `crates/cobuild-core/tests/view.rs` imports `OwnedReader` from `cobuild_core::reader`.

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p cobuild-core --offline --test view
cargo test -p cobuild-core --offline --test hash
cargo test -p tests --offline --test contract_template_layout cobuild_core_reader_helpers_are_not_owned_by_view
```

Expected: all pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/cobuild-core tests/tests/contract_template_layout.rs
git commit -m "refactor: move cursor reader helpers out of view"
```

**Rollback risk:** Low. This is a direct helper extraction; behavior should not change.

### Task 2: Add Source Traits With Read Error Context

**Files:**
- Create: `crates/cobuild-core/src/source.rs`
- Modify: `crates/cobuild-core/src/lib.rs`
- Modify: `crates/cobuild-core/tests/no_entity_dependency.rs`
- Modify: `tests/tests/contract_template_layout.rs`

- [ ] **Step 1: Add structural tests for source boundary**

In `tests/tests/contract_template_layout.rs`, add:

```rust
#[test]
fn cobuild_core_exposes_source_boundary_without_ckb_std() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");
    assert!(core_src.join("source.rs").is_file(), "source.rs must own source traits");
    let lib_rs = fs::read_to_string(core_src.join("lib.rs")).expect("core lib.rs");
    assert!(lib_rs.contains("pub mod source"), "core should export source traits");
    let source_rs = fs::read_to_string(core_src.join("source.rs")).expect("source.rs");
    for expected in ["ClassifiedCursor", "CursorReadContext", "TransactionSource", "SigningDataSource"] {
        assert!(source_rs.contains(expected), "source.rs should define {expected}");
    }
    assert!(!source_rs.contains("ckb_std"), "core source boundary must not import ckb_std");
}
```

In `crates/cobuild-core/tests/no_entity_dependency.rs`, add:

```rust
#[test]
fn core_source_does_not_import_ckb_std() {
    for path in [
        "src/context.rs",
        "src/error.rs",
        "src/hash.rs",
        "src/layout.rs",
        "src/lib.rs",
        "src/loader.rs",
        "src/message.rs",
        "src/otx_request.rs",
        "src/protocol.rs",
        "src/query.rs",
        "src/reader.rs",
        "src/seal.rs",
        "src/signature.rs",
        "src/sighash.rs",
        "src/source.rs",
        "src/view.rs",
        "src/witness.rs",
    ] {
        let full_path = manifest_path(path);
        let text = std::fs::read_to_string(&full_path)
            .unwrap_or_else(|err| panic!("read {}: {err}", full_path.display()));
        assert!(!text.contains("ckb_std"), "{path} must not import ckb_std");
    }
}
```

- [ ] **Step 2: Run failing tests**

Run:

```bash
cargo test -p tests --offline --test contract_template_layout cobuild_core_exposes_source_boundary_without_ckb_std
cargo test -p cobuild-core --offline --test no_entity_dependency core_source_does_not_import_ckb_std
```

Expected: fail because `source.rs` does not exist yet.

- [ ] **Step 3: Create `source.rs`**

Create `crates/cobuild-core/src/source.rs`:

```rust
use alloc::vec::Vec;

use cobuild_types::lazy_reader::support::Cursor;

use crate::{error::CoreError, reader::cursor_from_slice};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CursorReadContext {
    Protocol,
    SourceInput,
    HashInput,
}

#[derive(Clone)]
pub struct ClassifiedCursor {
    pub cursor: Cursor,
    pub read_context: CursorReadContext,
}

impl ClassifiedCursor {
    pub fn protocol(cursor: Cursor) -> Self {
        Self {
            cursor,
            read_context: CursorReadContext::Protocol,
        }
    }

    pub fn source_input(cursor: Cursor) -> Self {
        Self {
            cursor,
            read_context: CursorReadContext::SourceInput,
        }
    }

    pub fn hash_input(cursor: Cursor) -> Self {
        Self {
            cursor,
            read_context: CursorReadContext::HashInput,
        }
    }

    pub fn read_error(&self) -> CoreError {
        match self.read_context {
            CursorReadContext::Protocol => CoreError::MalformedCobuild,
            CursorReadContext::SourceInput => CoreError::InvalidContextInput,
            CursorReadContext::HashInput => CoreError::MissingHashInput,
        }
    }
}

pub trait TransactionSource {
    fn transaction_cursor(&self) -> Result<ClassifiedCursor, CoreError>;
    fn script_cursor(&self) -> Result<ClassifiedCursor, CoreError>;
    fn tx_hash(&self) -> Result<[u8; 32], CoreError>;
    fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError>;
    fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError>;
    fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError>;
    fn resolved_input_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn resolved_input_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
}

pub trait SigningDataSource: TransactionSource {
    fn input_count(&self) -> Result<usize, CoreError>;
    fn witness_count(&self) -> Result<usize, CoreError>;
    fn witness_cursor(&self, absolute_index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_input_cursor(&self, tx_index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_output_cursor(&self, tx_index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_output_data_cursor(&self, tx_index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_cell_dep_cursor(&self, tx_index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_header_dep_hash(&self, tx_index: usize) -> Result<[u8; 32], CoreError>;
}

#[derive(Clone, Debug, Default)]
pub struct InMemorySource {
    pub transaction: Vec<u8>,
    pub script: Vec<u8>,
    pub tx_hash: [u8; 32],
    pub input_locks: Vec<[u8; 32]>,
    pub input_types: Vec<Option<[u8; 32]>>,
    pub output_types: Vec<Option<[u8; 32]>>,
    pub resolved_outputs: Vec<Vec<u8>>,
    pub resolved_data: Vec<Vec<u8>>,
    pub raw_inputs: Vec<Vec<u8>>,
    pub raw_outputs: Vec<Vec<u8>>,
    pub raw_outputs_data: Vec<Vec<u8>>,
    pub raw_cell_deps: Vec<Vec<u8>>,
    pub raw_header_deps: Vec<[u8; 32]>,
    pub witnesses: Vec<Vec<u8>>,
}
```

Add `TransactionSource` and `SigningDataSource` impls for `InMemorySource`, returning `ClassifiedCursor::source_input` for transaction/script and `ClassifiedCursor::hash_input` for raw/resolved/witness hash payload cursors.

- [ ] **Step 4: Export and test**

In `lib.rs`, add:

```rust
pub mod source;
```

Run:

```bash
cargo test -p tests --offline --test contract_template_layout cobuild_core_exposes_source_boundary_without_ckb_std
cargo test -p cobuild-core --offline --test no_entity_dependency core_source_does_not_import_ckb_std
```

Expected: both pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/cobuild-core tests/tests/contract_template_layout.rs
git commit -m "refactor: add classified cobuild source cursors"
```

**Rollback risk:** Low. Traits are additive and not yet wired into query flow.

### Task 3: Move All Query Hashing To `SigningDataSource`

**Files:**
- Modify: `crates/cobuild-core/src/{hash.rs,query.rs,sighash.rs,otx_request.rs,context.rs,loader.rs,source.rs}`
- Modify: `contracts/cobuild-otx-lock/src/loader.rs`
- Modify: `crates/cobuild-core/tests/{hash.rs,signature_requests.rs}`
- Modify: `tests/src/lib.rs`
- Modify: `tests/tests/{cobuild_otx_lock.rs,contract_template_layout.rs}`

- [ ] **Step 1: Add structural assertions for removed owned hash inputs**

In `tests/tests/contract_template_layout.rs`, add:

```rust
#[test]
fn cobuild_core_hashing_uses_source_not_owned_hash_parts() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");
    let hash_rs = fs::read_to_string(core_src.join("hash.rs")).expect("hash.rs");
    for forbidden in [
        "struct RawTxParts",
        "struct ResolvedInputHashPart",
        "struct SigningHashParts",
        "trailing_witnesses",
    ] {
        assert!(!hash_rs.contains(forbidden), "hash.rs must not define {forbidden}");
    }
    assert!(hash_rs.contains("SigningDataSource"), "hash.rs should hash through SigningDataSource");
}
```

- [ ] **Step 2: Run failing structural test**

Run:

```bash
cargo test -p tests --offline --test contract_template_layout cobuild_core_hashing_uses_source_not_owned_hash_parts
```

Expected: fails because owned hash input structs still exist.

- [ ] **Step 3: Change tx-level hash functions**

In `hash.rs`, replace `SigningHashParts`-based tx hash functions with:

```rust
pub fn tx_without_message_hash<S: SigningDataSource>(source: &S) -> Result<[u8; 32], CoreError> {
    tx_signing_hash(b"ckbcb_tnm_core1\0", None, source)
}

pub fn tx_with_message_hash<S: SigningDataSource>(
    message: &Cursor,
    source: &S,
) -> Result<[u8; 32], CoreError> {
    tx_signing_hash(b"ckbcb_twm_core1\0", Some(message), source)
}
```

The tx signing loop must be:

```rust
for index in 0..source.input_count()? {
    let output = source.resolved_input_output_cursor(index)?;
    update_cursor_with_error(&mut hasher, &output.cursor, output.read_error())?;
    let data = source.resolved_input_data_cursor(index)?;
    update_len_prefixed_cursor(&mut hasher, &data.cursor, data.read_error())?;
}
for index in source.input_count()?..source.witness_count()? {
    let witness = source.witness_cursor(index)?;
    update_len_prefixed_cursor(&mut hasher, &witness.cursor, witness.read_error())?;
}
```

- [ ] **Step 4: Change OTX hash functions in the same task**

Change `otx_base_hash` and `otx_append_hash` to accept `source: &impl SigningDataSource`. Replace all `RawTxParts` and `ResolvedInputHashPart` reads with source cursor reads. Use `update_cursor_with_error` and `update_len_prefixed_cursor` for every cursor returned from source.

- [ ] **Step 5: Change query APIs atomically**

Change:

```rust
required_signatures(&self, parts: &SigningHashParts)
```

to:

```rust
required_signatures<S: SigningDataSource>(&self, source: &S)
```

Update `collect_sighash_all_signatures` and `collect_otx_signatures` in the same commit. Do not leave an intermediate public API that requires both source and old hash parts.

- [ ] **Step 6: Remove owned hash input structs**

Delete from `hash.rs`:

```rust
RawTxParts
ResolvedInputHashPart
SigningHashParts
```

Delete from `context.rs`:

```rust
raw_parts: Option<RawTxParts>
CobuildContext::with_raw_parts
PreparedContext.signing_hash_parts
```

Keep `PreparedContextInput`, `parse_transaction_info(data)`, and `prepare_context` only if they are rewritten to build an `InMemorySource` and call the source-backed path. They must not expose removed hash input types.

- [ ] **Step 7: Update lock loader transitionally**

Before `chain.rs` exists, keep `contracts/cobuild-otx-lock/src/loader.rs` compiling by building an `InMemorySource` from the currently loaded owned values and passing it to `required_signatures`. This is a temporary bridge; Task 5 removes full transaction loading from the lock path.

- [ ] **Step 8: Run focused verification**

Run:

```bash
cargo test -p cobuild-core --offline --test hash
cargo test -p cobuild-core --offline --test signature_requests
cargo test -p tests --offline --test contract_template_layout cobuild_core_hashing_uses_source_not_owned_hash_parts
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: all pass. Hash expected-value tests must remain byte-for-byte unchanged.

- [ ] **Step 9: Commit**

Run:

```bash
git add crates/cobuild-core contracts/cobuild-otx-lock tests
git commit -m "refactor: hash cobuild signatures from source"
```

**Rollback risk:** High. This is the main hash-plumbing change; tx-level and OTX hashing intentionally move together to avoid a half-migrated query API.

### Task 4: Rename Core `loader.rs` To `prepare.rs`

**Files:**
- Move: `crates/cobuild-core/src/loader.rs` to `crates/cobuild-core/src/prepare.rs`
- Modify: `crates/cobuild-core/src/lib.rs`
- Modify imports in `contracts`, `tests`, and `crates/cobuild-core/tests`
- Modify: `tests/tests/contract_template_layout.rs`

- [ ] **Step 1: Add structural assertion for core prepare module**

In `tests/tests/contract_template_layout.rs`, add:

```rust
#[test]
fn cobuild_core_prepares_context_in_prepare_module() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");
    assert!(core_src.join("prepare.rs").is_file(), "prepare.rs must own context preparation");
    assert!(!core_src.join("loader.rs").exists(), "core loader.rs should be renamed to prepare.rs");
    let lib_rs = fs::read_to_string(core_src.join("lib.rs")).expect("core lib.rs");
    assert!(lib_rs.contains("pub mod prepare"), "core should export prepare");
    assert!(!lib_rs.contains("pub mod loader"), "core should not export loader");
}
```

- [ ] **Step 2: Move file and module**

Run:

```bash
git mv crates/cobuild-core/src/loader.rs crates/cobuild-core/src/prepare.rs
```

In `lib.rs`, replace `pub mod loader;` with `pub mod prepare;`.

- [ ] **Step 3: Rename script hash index**

In `context.rs`, rename `TxScriptHashes` to `ScriptHashIndex` and update all imports/call sites. This task owns the rename, so later tasks must use `ScriptHashIndex`.

- [ ] **Step 4: Update imports**

Replace `cobuild_core::loader` imports with `cobuild_core::prepare` in contracts and tests.

- [ ] **Step 5: Run tests**

Run:

```bash
cargo test -p cobuild-core --offline --test no_entity_dependency
cargo test -p tests --offline --test contract_template_layout cobuild_core_prepares_context_in_prepare_module
cargo test -p tests --offline --test contract_template_layout
```

Expected: all pass.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates contracts tests
git commit -m "refactor: rename cobuild preparation module"
```

**Rollback risk:** Medium. This is mostly mechanical, but it changes public module paths.

### Task 5: Add Syscall-Backed `ChainSource` And Remove Full Transaction Load

**Files:**
- Move: `contracts/cobuild-otx-lock/src/loader.rs` to `contracts/cobuild-otx-lock/src/chain.rs`
- Modify: `contracts/cobuild-otx-lock/src/{entry.rs,lib.rs}`
- Modify: `tests/tests/contract_template_layout.rs`

- [ ] **Step 1: Add structural assertion for lock chain module**

In `tests/tests/contract_template_layout.rs`, add:

```rust
#[test]
fn cobuild_otx_lock_streams_chain_data_without_full_transaction_load() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let lock_src = workspace_root.join("contracts/cobuild-otx-lock/src");
    assert!(lock_src.join("chain.rs").is_file(), "chain.rs must own syscall-backed source");
    assert!(!lock_src.join("loader.rs").exists(), "loader.rs should be renamed to chain.rs");
    let chain_rs = fs::read_to_string(lock_src.join("chain.rs")).expect("chain.rs");
    assert!(chain_rs.contains("struct ChainSource"), "chain.rs should define ChainSource");
    assert!(!chain_rs.contains("fn load_transaction() -> Result<Vec<u8>"), "lock path must not load the full transaction into Vec");
    assert!(!chain_rs.contains("parse_transaction_info(&load_transaction()?"), "lock path must parse transaction from source cursor");
}
```

- [ ] **Step 2: Move module**

Run:

```bash
git mv contracts/cobuild-otx-lock/src/loader.rs contracts/cobuild-otx-lock/src/chain.rs
```

In `contracts/cobuild-otx-lock/src/lib.rs`, replace:

```rust
mod loader;
```

with:

```rust
mod chain;
```

In `entry.rs`, replace:

```rust
loader::{load_current_script_args, load_prepared_context, load_script_hash},
```

with:

```rust
chain::{load_current_script_args, load_prepared_context, load_script_hash},
```

- [ ] **Step 3: Add complete chain imports**

At the top of `chain.rs`, keep or add:

```rust
use alloc::{boxed::Box, vec, vec::Vec};
use core::cmp::min;

use ckb_std::{
    ckb_constants::{CellField, Source},
    error::SysError,
    syscalls,
};
use cobuild_core::{
    error::CoreError,
    prepare::{prepare_context_from_source, script_args_from_slice},
    source::{ClassifiedCursor, SigningDataSource, TransactionSource},
};
use cobuild_types::lazy_reader::{
    blockchain::Transaction,
    support::{Cursor, Error as MoleculeError, Read},
};

use crate::{
    error::Error,
    errors::{map_core_error, map_sys_error},
};
```

- [ ] **Step 4: Implement syscall reader and `ChainSource`**

Implement a `SyscallReader` that stores `total_size` and an owned loader closure, then implement `Read` by calling the syscall at the requested offset. Non-`LengthNotEnough` syscall failures should surface as a lazy-reader read failure; the classified cursor's `CursorReadContext` controls how core maps that failure.

Implement:

```rust
pub(crate) struct ChainSource;
```

`TransactionSource` returns:

- `ClassifiedCursor::source_input(transaction_cursor()?)` for transaction;
- `ClassifiedCursor::source_input(script_cursor()?)` for script;
- lock/type hashes through `load_cell_by_field`;
- `ClassifiedCursor::hash_input(input_cell_cursor(index, Source::Input)?)` for resolved input output;
- `ClassifiedCursor::hash_input(input_cell_data_cursor(index, Source::Input)?)` for resolved input data.

`SigningDataSource` returns absolute-index witness cursors and raw transaction cursors from a lazy `Transaction::from(self.transaction_cursor()?.cursor)`.

- [ ] **Step 5: Replace loaded context flow**

In `chain.rs`, replace `load_prepared_context` with:

```rust
pub(crate) struct LoadedContext {
    pub source: ChainSource,
    pub prepared: cobuild_core::context::PreparedContext,
}

pub(crate) fn load_prepared_context() -> Result<LoadedContext, Error> {
    let source = ChainSource;
    let prepared = prepare_context_from_source(&source).map_err(map_core_error)?;
    Ok(LoadedContext { source, prepared })
}
```

In `entry.rs`, call:

```rust
let loaded = load_prepared_context()?;
let signature_requests = loaded
    .prepared
    .context
    .lock_query(current_script_hash)
    .required_signatures(&loaded.source)
    .map_err(map_core_error)?;
```

- [ ] **Step 6: Delete full owned loading path**

Delete `load_transaction`. Delete any transitional `InMemorySource` bridge from the lock crate. Keep `load_tx_hash`, `load_script_hash`, `load_script`, and `load_cell_field_hash` only where still needed.

- [ ] **Step 7: Run contract verification**

Run:

```bash
cargo test -p tests --offline --test contract_template_layout cobuild_otx_lock_streams_chain_data_without_full_transaction_load
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: all pass.

- [ ] **Step 8: Commit**

Run:

```bash
git add contracts tests
git commit -m "refactor: stream cobuild lock chain data"
```

**Rollback risk:** High. This changes the on-chain loading path; keep it isolated.

### Task 6: Add Counting Source Tests For Relevant And Irrelevant Reads

**Files:**
- Modify: `crates/cobuild-core/tests/signature_requests.rs`

- [ ] **Step 1: Add counting source helper**

Add a `CountingSource` wrapper around `InMemorySource` that implements both source traits and increments counters for every hash payload accessor:

```rust
#[derive(Default)]
struct ReadCounters {
    resolved_outputs: core::cell::Cell<usize>,
    resolved_data: core::cell::Cell<usize>,
    raw_inputs: core::cell::Cell<usize>,
    raw_outputs: core::cell::Cell<usize>,
    raw_outputs_data: core::cell::Cell<usize>,
    raw_cell_deps: core::cell::Cell<usize>,
    raw_header_deps: core::cell::Cell<usize>,
}

struct CountingSource {
    inner: InMemorySource,
    counters: ReadCounters,
}
```

Each `SigningDataSource` accessor delegates to `inner` and increments the matching counter. Witness cursor reads do not count as OTX hash payload reads.

- [ ] **Step 2: Add irrelevant-query test**

Use the existing unrelated OTX fixture from `unrelated_otx_lock_query_does_not_require_raw_hash_parts` and assert:

```rust
let requests = context.lock_query([9u8; 32]).required_signatures(&source).unwrap();
assert!(requests.is_empty());
assert_eq!(source.counters.resolved_outputs.get(), 0);
assert_eq!(source.counters.resolved_data.get(), 0);
assert_eq!(source.counters.raw_inputs.get(), 0);
assert_eq!(source.counters.raw_outputs.get(), 0);
assert_eq!(source.counters.raw_outputs_data.get(), 0);
assert_eq!(source.counters.raw_cell_deps.get(), 0);
assert_eq!(source.counters.raw_header_deps.get(), 0);
```

- [ ] **Step 3: Add relevant-query test**

Use the existing OTX base/append fixture from `required_signatures_marks_otx_base_origin` and assert the relevant query reads at least the expected relevant raw/resolved payload classes while unrelated ranges remain unread where the fixture permits exact index checking.

- [ ] **Step 4: Run tests**

Run:

```bash
cargo test -p cobuild-core --offline --test signature_requests unrelated_otx_query_does_not_read_hash_payloads
cargo test -p cobuild-core --offline --test signature_requests relevant_otx_query_reads_hash_payloads
cargo test -p cobuild-core --offline --test signature_requests
```

Expected: all pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/cobuild-core/tests/signature_requests.rs
git commit -m "test: prove cobuild hash source relevance"
```

**Rollback risk:** Low. Tests only.

## Round 2: Cursor-Backed View Layer

### Task 7: Add Cursor-Backed View Structural Tests

**Files:**
- Modify: `tests/tests/contract_template_layout.rs`
- Modify: `crates/cobuild-core/tests/view.rs`

- [ ] **Step 1: Add structural view assertions**

In `tests/tests/contract_template_layout.rs`, add:

```rust
#[test]
fn cobuild_core_view_is_cursor_backed_protocol_boundary() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");
    let view_rs = fs::read_to_string(core_src.join("view.rs")).expect("view.rs");

    for forbidden in [
        "OtxStartData",
        "OtxData",
        "SealPairData",
        "ActionData",
        "message: Vec<u8>",
        "base_input_masks: Vec<u8>",
        "seal: Vec<u8>",
    ] {
        assert!(!view_rs.contains(forbidden), "view.rs should not expose owned DTO pattern {forbidden}");
    }

    for expected in [
        "SighashAllWitnessView",
        "OtxStartView",
        "OtxView",
        "SealPairView",
        "MessageActionView",
        "MaskView",
    ] {
        assert!(view_rs.contains(expected), "view.rs should expose cursor-backed view {expected}");
    }
}
```

- [ ] **Step 2: Run failing test**

Run:

```bash
cargo test -p tests --offline --test contract_template_layout cobuild_core_view_is_cursor_backed_protocol_boundary
```

Expected: fails because owned `*Data` DTOs still exist.

**Rollback risk:** Low. Tests only.

### Task 8: Replace Owned View DTOs With Cursor-Backed Views

**Files:**
- Modify: `crates/cobuild-core/src/{view.rs,layout.rs,message.rs,seal.rs,sighash.rs,otx_request.rs,hash.rs}`
- Modify: `crates/cobuild-core/tests/{view.rs,layout.rs,signature_requests.rs}`

- [ ] **Step 1: Add cursor-backed view types without invalid derives**

In `view.rs`, replace owned DTOs with cursor-backed types. Do not derive `Debug`, `Eq`, or `PartialEq` for structs that contain `Cursor`.

Required names:

```rust
pub enum SighashAllWitnessView {
    WithMessage { seal: Cursor, message: Cursor },
    SealOnly { seal: Cursor },
}

#[derive(Clone)]
pub struct OtxStartView {
    pub start_input_cell: usize,
    pub start_output_cell: usize,
    pub start_cell_deps: usize,
    pub start_header_deps: usize,
}

#[derive(Clone)]
pub struct MaskView {
    cursor: Cursor,
}

#[derive(Clone)]
pub struct SealPairView {
    pub script_hash: [u8; 32],
    pub scope: u8,
    pub seal: Cursor,
}

#[derive(Clone)]
pub struct OtxView {
    pub message: Cursor,
    pub append_permissions: u8,
    pub base_input_cells: usize,
    pub base_input_masks: MaskView,
    pub base_output_cells: usize,
    pub base_output_masks: MaskView,
    pub base_cell_deps: usize,
    pub base_cell_dep_masks: MaskView,
    pub base_header_deps: usize,
    pub base_header_dep_masks: MaskView,
    pub append_input_cells: usize,
    pub append_output_cells: usize,
    pub append_cell_deps: usize,
    pub append_header_deps: usize,
    pub seals: Vec<SealPairView>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MessageActionView {
    pub script_role: u8,
    pub script_hash: [u8; 32],
}
```

- [ ] **Step 2: Convert message action parsing**

Change:

```rust
pub(crate) fn message_actions(message_bytes: &[u8]) -> Result<Vec<ActionData>, CoreError>
```

to:

```rust
pub(crate) fn message_actions(message: &Cursor) -> Result<Vec<MessageActionView>, CoreError>
```

Construct `Message::from(message.clone())`; do not call `cursor_bytes` for the message.

- [ ] **Step 3: Convert unique SighashAll message**

In `sighash.rs`, change `unique_sighash_all_message` to return `Option<Cursor>` instead of `Option<Vec<u8>>`. Where the witness carries a message, keep its cursor. Hashing then calls `tx_with_message_hash(&message_cursor, source)`.

- [ ] **Step 4: Convert seal copying boundary**

In `seal.rs`, accept `&[SealPairView]`. Only copy seal bytes when returning the final seal:

```rust
found = Some(cursor_bytes(&seal.seal)?);
```

Do not store seal bytes inside `SealPairView`.

- [ ] **Step 5: Convert masks and hash usage**

Add `MaskView::bit`, `MaskView::len`, and `MaskView::cursor`. Update `layout.rs` mask validation and `hash.rs` mask hashing to use cursor-backed masks. Use `update_len_prefixed_cursor` for mask hash preimages.

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test -p cobuild-core --offline --test view
cargo test -p cobuild-core --offline --test layout
cargo test -p cobuild-core --offline --test hash
cargo test -p cobuild-core --offline --test signature_requests
cargo test -p tests --offline --test contract_template_layout cobuild_core_view_is_cursor_backed_protocol_boundary
```

Expected: all pass.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/cobuild-core tests/tests/contract_template_layout.rs
git commit -m "refactor: use cursor backed cobuild views"
```

**Rollback risk:** High. This rewires protocol views, message validation, seal selection, and mask hashing.

### Task 9: Final Boundary Cleanup And Full Verification

**Files:**
- Modify: `crates/cobuild-core/src/lib.rs`
- Modify: `contracts/cobuild-otx-lock/src/lib.rs`
- Modify: `tests/tests/{contract_template_layout.rs,workspace_layout.rs}`

- [ ] **Step 1: Remove stale names from production code**

Run:

```bash
rg -n "loader|TxScriptHashes|RawTxParts|ResolvedInputHashPart|SigningHashParts|OtxData|OtxStartData|SealPairData|ActionData|SighashAllWitnessLayout|trailing_witnesses" crates contracts tests
```

Expected remaining matches are only in docs or structural tests that assert the names do not exist. Remove production-code matches.

- [ ] **Step 2: Run source scans**

Run:

```bash
rg -n "cobuild_types::entity|::entity::" crates/cobuild-core/src contracts/cobuild-otx-lock/src
rg -n "unsafe" crates/cobuild-core/src contracts/cobuild-otx-lock/src
rg -n "ckb_std" crates/cobuild-core/src
```

Expected: no output for all three commands.

- [ ] **Step 3: Run full verification**

Run:

```bash
cargo fmt --check
cargo run -p xtask --offline -- codegen cobuild-types --check
cargo clippy --workspace --all-targets --offline
cargo test --workspace --offline
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: all commands exit 0.

- [ ] **Step 4: Commit**

Run:

```bash
git add crates contracts tests
git commit -m "refactor: finalize streaming cobuild boundaries"
```

**Rollback risk:** Medium. This should only remove stale names and verify the full refactor.

## Self-Review Checklist

- Spec coverage:
  - Full transaction load removed by Task 5.
  - Duplicated trailing witnesses removed by Task 3.
  - Eager raw hash inputs removed by Task 3.
  - Reader helpers moved out of `view.rs` by Task 1.
  - Source cursor error classification added by Task 2 and used by Tasks 3 and 5.
  - Absolute witness index order enforced by Task 3.
  - Cursor-backed view layer implemented by Tasks 7 and 8.
  - Lock crate syscall boundary renamed and isolated by Task 5.
  - No `ckb_std` in core enforced by Tasks 2 and 9.
  - No generated entity public abstraction enforced by existing and updated boundary tests.
- Placeholder scan:
  - No placeholder markers or unspecified "add tests" steps are intentionally present.
- Type consistency:
  - `ClassifiedCursor` and `SigningDataSource` are introduced before call sites use them.
  - `required_signatures` changes only when both tx-level and OTX hashing are source-backed.
  - `ScriptHashIndex` is introduced in the task that first uses it.
  - Cursor-backed view names are introduced before hash/message/seal modules consume them.
