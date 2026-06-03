# Cobuild Core Streaming Reader And View Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace owned transaction/hash inputs with streaming cursor-backed sources, then make `view.rs` a clean cursor-backed protocol view boundary.

**Architecture:** Round 1 introduces `reader.rs` and `source.rs`, moves hash construction to source/cursor reads, and replaces full transaction loading in the lock crate with syscall-backed readers. Round 2 replaces owned `*Data` view DTOs with cursor-backed `*View` types and keeps ownership only at final external boundaries such as `SignatureRequest.seal`.

**Tech Stack:** Rust `no_std` with `alloc`, `cobuild_types::lazy_reader`, `molecule::lazy_reader::support::Cursor`, `blake2b-ref`, `ckb-std`, workspace Cargo tests, CKB debug contract build.

---

## Reference Documents

- Spec: `docs/superpowers/specs/2026-06-03-cobuild-core-streaming-reader-and-hash-input-design.md`
- As-built baseline: `docs/superpowers/specs/2026-06-03-cobuild-core-signature-flow-refactor-as-built.md`
- Protocol redraft: `docs/superpowers/specs/2026-05-28-cobuild-core-community-redraft-design.md`
- Reference repo: `ref/repo/ckb-transaction-cobuild-poc/ckb-transaction-cobuild/src/lazy_reader.rs`

## File Map

Round 1 files:

- Create: `crates/cobuild-core/src/reader.rs`
  - Own `OwnedReader`, `cursor_from_slice`, `cursor_bytes`, `update_cursor`, and cursor length-prefix hash helpers.
- Create: `crates/cobuild-core/src/source.rs`
  - Own core source traits and in-memory source used by tests.
- Move: `crates/cobuild-core/src/loader.rs` to `crates/cobuild-core/src/prepare.rs`
  - Prepare `PreparedContext` from `TransactionSource`.
- Modify: `crates/cobuild-core/src/lib.rs`
  - Export `reader`, `source`, and `prepare`; remove `loader`.
- Modify: `crates/cobuild-core/src/context.rs`
  - Rename `TxScriptHashes` to `ScriptHashIndex`; remove `raw_parts`.
- Modify: `crates/cobuild-core/src/hash.rs`
  - Replace `RawTxParts`, `ResolvedInputHashPart`, and `SigningHashParts.trailing_witnesses` with source/cursor hashing.
- Modify: `crates/cobuild-core/src/{layout,sighash,otx_request,message,seal,view,witness}.rs`
  - Read witness/message/hash payloads through cursor-backed APIs.
- Move: `contracts/cobuild-otx-lock/src/loader.rs` to `contracts/cobuild-otx-lock/src/chain.rs`
  - Own syscall-backed readers and `ChainSource`.
- Modify: `contracts/cobuild-otx-lock/src/{entry,lib}.rs`
  - Use `chain` and `prepare`.
- Modify tests:
  - `crates/cobuild-core/tests/{hash,layout,signature_requests,view,witness,no_entity_dependency}.rs`
  - `tests/tests/{cobuild_otx_lock,contract_template_layout,workspace_layout}.rs`
  - `tests/src/lib.rs`

Round 2 files:

- Modify: `crates/cobuild-core/src/view.rs`
  - Replace owned DTOs with cursor-backed protocol views.
- Modify: `crates/cobuild-core/src/{layout,sighash,otx_request,message,seal,hash}.rs`
  - Consume `*View` and `MaskView` types.
- Modify tests:
  - `crates/cobuild-core/tests/{view,layout,signature_requests,no_entity_dependency}.rs`
  - `tests/tests/contract_template_layout.rs`

## Round 1: Streaming Source And Hash Input Boundary

### Task 1: Add Structural Tests For The New Boundaries

**Files:**
- Modify: `tests/tests/contract_template_layout.rs`
- Modify: `crates/cobuild-core/tests/no_entity_dependency.rs`

- [ ] **Step 1: Add source and reader boundary assertions**

In `tests/tests/contract_template_layout.rs`, add this test after `cobuild_core_uses_explicit_signature_request_names`:

```rust
#[test]
fn cobuild_core_uses_streaming_source_boundaries() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");
    let lock_src = workspace_root.join("contracts/cobuild-otx-lock/src");

    assert!(core_src.join("reader.rs").is_file(), "reader.rs must own cursor helpers");
    assert!(core_src.join("source.rs").is_file(), "source.rs must own source traits");
    assert!(core_src.join("prepare.rs").is_file(), "prepare.rs must own context preparation");
    assert!(!core_src.join("loader.rs").exists(), "core loader.rs should be renamed to prepare.rs");

    let core_lib = fs::read_to_string(core_src.join("lib.rs")).expect("core lib.rs");
    assert!(core_lib.contains("pub mod reader"), "core should export reader helpers");
    assert!(core_lib.contains("pub mod source"), "core should export source traits");
    assert!(core_lib.contains("pub mod prepare"), "core should export prepare");
    assert!(!core_lib.contains("pub mod loader"), "core should not export loader");

    let reader_rs = fs::read_to_string(core_src.join("reader.rs")).expect("reader.rs");
    for expected in ["OwnedReader", "cursor_from_slice", "cursor_bytes", "update_cursor"] {
        assert!(reader_rs.contains(expected), "reader.rs should define {expected}");
    }

    let view_rs = fs::read_to_string(core_src.join("view.rs")).expect("view.rs");
    for forbidden in ["struct OwnedReader", "fn cursor_from_slice", "fn cursor_bytes", "fn update_cursor"] {
        assert!(!view_rs.contains(forbidden), "view.rs must not define {forbidden}");
    }

    assert!(lock_src.join("chain.rs").is_file(), "lock chain.rs must own syscall-backed source");
    assert!(!lock_src.join("loader.rs").exists(), "lock loader.rs should be renamed to chain.rs");
    let lock_lib = fs::read_to_string(lock_src.join("lib.rs")).expect("lock lib.rs");
    assert!(lock_lib.contains("mod chain"), "lock lib should include chain module");
    assert!(!lock_lib.contains("mod loader"), "lock lib should not include loader module");
}
```

- [ ] **Step 2: Add no-`ckb_std` source boundary check**

In `crates/cobuild-core/tests/no_entity_dependency.rs`, add this test:

```rust
#[test]
fn core_source_does_not_import_ckb_std() {
    for path in [
        "src/context.rs",
        "src/error.rs",
        "src/hash.rs",
        "src/layout.rs",
        "src/lib.rs",
        "src/message.rs",
        "src/otx_request.rs",
        "src/prepare.rs",
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

- [ ] **Step 3: Update the existing core file list**

In `core_source_does_not_import_entity_module`, replace `"src/loader.rs"` with these entries:

```rust
"src/prepare.rs",
"src/reader.rs",
"src/source.rs",
```

- [ ] **Step 4: Run structural tests and confirm failure**

Run:

```bash
cargo test -p tests --offline --test contract_template_layout cobuild_core_uses_streaming_source_boundaries
cargo test -p cobuild-core --offline --test no_entity_dependency core_source_does_not_import_ckb_std
```

Expected:

- first command fails because `reader.rs`, `source.rs`, `prepare.rs`, and `chain.rs` do not exist yet;
- second command fails because the file list references files that do not exist yet.

**Rollback risk:** Low. This task only adds failing structural tests for the target module boundaries.

### Task 2: Move Reader Helpers Out Of `view.rs`

**Files:**
- Create: `crates/cobuild-core/src/reader.rs`
- Modify: `crates/cobuild-core/src/lib.rs`
- Modify: `crates/cobuild-core/src/view.rs`
- Modify: `crates/cobuild-core/src/hash.rs`
- Modify: `crates/cobuild-core/src/loader.rs`
- Modify: `crates/cobuild-core/tests/view.rs`
- Modify: `crates/cobuild-core/tests/hash.rs`

- [ ] **Step 1: Create `reader.rs`**

Create `crates/cobuild-core/src/reader.rs` with:

```rust
use alloc::{boxed::Box, vec, vec::Vec};
use core::cmp::min;

use blake2b_ref::Blake2b;
use cobuild_types::lazy_reader::support::{Cursor, Error as MoleculeError, Read};

use crate::{error::CoreError, hash::checked_len_prefix};

pub struct OwnedReader {
    data: Vec<u8>,
}

impl OwnedReader {
    pub fn new(data: &[u8]) -> Self {
        Self {
            data: data.to_vec(),
        }
    }
}

impl Read for OwnedReader {
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, MoleculeError> {
        if offset >= self.data.len() {
            return Err(MoleculeError::OutOfBound(offset, self.data.len()));
        }

        let read_len = min(buf.len(), self.data.len() - offset);
        buf[..read_len].copy_from_slice(&self.data[offset..offset + read_len]);
        Ok(read_len)
    }
}

pub fn cursor_from_slice(data: &[u8]) -> Cursor {
    let reader: Box<dyn Read> = Box::new(OwnedReader::new(data));
    Cursor::new(data.len(), reader)
}

pub fn cursor_bytes(cursor: &Cursor) -> Result<Vec<u8>, CoreError> {
    let mut bytes = vec![0; cursor.size];
    let read = cursor
        .read_at(&mut bytes)
        .map_err(|_| CoreError::MalformedCobuild)?;
    if read != bytes.len() {
        return Err(CoreError::MalformedCobuild);
    }
    Ok(bytes)
}

pub fn update_cursor(hasher: &mut Blake2b, cursor: &Cursor) -> Result<(), CoreError> {
    let mut offset = 0usize;
    let mut buf = [0u8; 256];

    while offset < cursor.size {
        let read_len = min(buf.len(), cursor.size - offset);
        let mut chunk = cursor.clone();
        chunk
            .add_offset(offset)
            .map_err(|_| CoreError::MalformedCobuild)?;
        chunk.size = read_len;

        let read = chunk
            .read_at(&mut buf[..read_len])
            .map_err(|_| CoreError::MalformedCobuild)?;
        if read != read_len {
            return Err(CoreError::MalformedCobuild);
        }

        hasher.update(&buf[..read_len]);
        offset = offset
            .checked_add(read_len)
            .ok_or(CoreError::MalformedCobuild)?;
    }

    Ok(())
}

pub fn update_len_prefixed_cursor(
    hasher: &mut Blake2b,
    cursor: &Cursor,
) -> Result<(), CoreError> {
    hasher.update(&checked_len_prefix(cursor.size)?);
    update_cursor(hasher, cursor)
}
```

- [ ] **Step 2: Export `reader`**

In `crates/cobuild-core/src/lib.rs`, add:

```rust
pub mod reader;
```

- [ ] **Step 3: Update imports**

In `view.rs`, remove local definitions of `OwnedReader`, `cursor_from_slice`, `cursor_bytes`, and `update_cursor`. Add:

```rust
use crate::{
    error::CoreError,
    reader::{cursor_bytes, cursor_from_slice},
};
```

In `hash.rs`, replace:

```rust
view::{cursor_from_slice, update_cursor, OtxData},
```

with:

```rust
reader::{cursor_from_slice, update_cursor},
view::OtxData,
```

In `loader.rs`, replace:

```rust
view::{cursor_bytes, cursor_from_slice},
```

with:

```rust
reader::{cursor_bytes, cursor_from_slice},
```

In `crates/cobuild-core/tests/view.rs`, replace:

```rust
use cobuild_core::view::OwnedReader;
```

with:

```rust
use cobuild_core::reader::OwnedReader;
```

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p cobuild-core --offline --test view
cargo test -p cobuild-core --offline --test hash
```

Expected: both pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/cobuild-core/src crates/cobuild-core/tests tests/tests/contract_template_layout.rs
git commit -m "refactor: move cursor reader helpers out of view"
```

**Rollback risk:** Low. Bodies are moved unchanged; failures should be import or visibility issues.

### Task 3: Introduce Core Source Traits And In-Memory Test Source

**Files:**
- Create: `crates/cobuild-core/src/source.rs`
- Modify: `crates/cobuild-core/src/lib.rs`
- Modify: `crates/cobuild-core/tests/hash.rs`
- Modify: `crates/cobuild-core/tests/signature_requests.rs`

- [ ] **Step 1: Create `source.rs` with traits**

Create `crates/cobuild-core/src/source.rs`:

```rust
use alloc::vec::Vec;

use cobuild_types::lazy_reader::{
    blockchain::Transaction,
    support::Cursor,
};

use crate::{
    error::CoreError,
    reader::cursor_from_slice,
};

pub trait TransactionSource {
    fn transaction_cursor(&self) -> Result<Cursor, CoreError>;
    fn script_cursor(&self) -> Result<Cursor, CoreError>;
    fn tx_hash(&self) -> Result<[u8; 32], CoreError>;
    fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError>;
    fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError>;
    fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError>;
    fn resolved_input_output_cursor(&self, index: usize) -> Result<Cursor, CoreError>;
    fn resolved_input_data_cursor(&self, index: usize) -> Result<Cursor, CoreError>;
}

pub trait SigningDataSource: TransactionSource {
    fn raw_input_cursor(&self, index: usize) -> Result<Cursor, CoreError>;
    fn raw_output_cursor(&self, index: usize) -> Result<Cursor, CoreError>;
    fn raw_output_data_cursor(&self, index: usize) -> Result<Cursor, CoreError>;
    fn raw_cell_dep_cursor(&self, index: usize) -> Result<Cursor, CoreError>;
    fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError>;
    fn witness_count(&self) -> Result<usize, CoreError>;
    fn witness_cursor(&self, index: usize) -> Result<Cursor, CoreError>;
}

pub fn transaction_from_source<S: TransactionSource>(
    source: &S,
) -> Result<Transaction, CoreError> {
    Ok(Transaction::from(source.transaction_cursor()?))
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
}

impl TransactionSource for InMemorySource {
    fn transaction_cursor(&self) -> Result<Cursor, CoreError> {
        Ok(cursor_from_slice(&self.transaction))
    }

    fn script_cursor(&self) -> Result<Cursor, CoreError> {
        Ok(cursor_from_slice(&self.script))
    }

    fn tx_hash(&self) -> Result<[u8; 32], CoreError> {
        Ok(self.tx_hash)
    }

    fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        self.input_locks
            .get(index)
            .copied()
            .ok_or(CoreError::InvalidContextInput)
    }

    fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        self.input_types
            .get(index)
            .copied()
            .ok_or(CoreError::InvalidContextInput)
    }

    fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        self.output_types
            .get(index)
            .copied()
            .ok_or(CoreError::InvalidContextInput)
    }

    fn resolved_input_output_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        self.resolved_outputs
            .get(index)
            .map(|bytes| cursor_from_slice(bytes))
            .ok_or(CoreError::MissingHashInput)
    }

    fn resolved_input_data_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        self.resolved_data
            .get(index)
            .map(|bytes| cursor_from_slice(bytes))
            .ok_or(CoreError::MissingHashInput)
    }
}
```

- [ ] **Step 2: Export `source`**

In `crates/cobuild-core/src/lib.rs`, add:

```rust
pub mod source;
```

- [ ] **Step 3: Run source boundary test**

Run:

```bash
cargo test -p cobuild-core --offline --test no_entity_dependency core_source_does_not_import_ckb_std
```

Expected: fails until `loader.rs` is moved to `prepare.rs` and the file list is aligned, but does not fail because `source.rs` imports `ckb_std`.

- [ ] **Step 4: Commit**

Run:

```bash
git add crates/cobuild-core/src/source.rs crates/cobuild-core/src/lib.rs crates/cobuild-core/tests/no_entity_dependency.rs
git commit -m "refactor: add cobuild transaction source traits"
```

**Rollback risk:** Medium-low. Traits are additive at this point; compile failures should be module/import related.

### Task 4: Stream Transaction-Level Signing Hash Inputs

**Files:**
- Modify: `crates/cobuild-core/src/hash.rs`
- Modify: `crates/cobuild-core/src/sighash.rs`
- Modify: `crates/cobuild-core/src/query.rs`
- Modify: `crates/cobuild-core/src/context.rs`
- Modify: `crates/cobuild-core/src/loader.rs`
- Modify: `contracts/cobuild-otx-lock/src/loader.rs`
- Modify tests importing `SigningHashParts`

- [ ] **Step 1: Add failing structural assertions**

In `tests/tests/contract_template_layout.rs`, inside `cobuild_core_uses_streaming_source_boundaries`, add:

```rust
let hash_rs = fs::read_to_string(core_src.join("hash.rs")).expect("hash.rs");
assert!(
    !hash_rs.contains("trailing_witnesses"),
    "SigningHashParts must not own trailing witnesses"
);
let lock_loader = fs::read_to_string(lock_src.join("loader.rs"))
    .or_else(|_| fs::read_to_string(lock_src.join("chain.rs")))
    .expect("lock loader/chain module");
assert!(
    !lock_loader.contains(".skip(input_count).cloned().collect()"),
    "lock path must not clone trailing witnesses"
);
```

- [ ] **Step 2: Replace tx signing hash signatures**

In `hash.rs`, replace `SigningHashParts` with:

```rust
use crate::source::SigningDataSource;

pub fn tx_without_message_hash<S: SigningDataSource>(source: &S) -> Result<[u8; 32], CoreError> {
    tx_signing_hash(b"ckbcb_tnm_core1\0", None, source)
}

pub fn tx_with_message_hash<S: SigningDataSource>(
    message: &Cursor,
    source: &S,
) -> Result<[u8; 32], CoreError> {
    tx_signing_hash(b"ckbcb_twm_core1\0", Some(message), source)
}

fn tx_signing_hash<S: SigningDataSource>(
    personalization: &[u8; 16],
    message: Option<&Cursor>,
    source: &S,
) -> Result<[u8; 32], CoreError> {
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32).personal(personalization).build();

    if let Some(message) = message {
        update_cursor(&mut hasher, message)?;
    }
    hasher.update(&source.tx_hash()?);
    for index in 0..source.input_count()? {
        update_cursor(&mut hasher, &source.resolved_input_output_cursor(index)?)?;
        update_len_prefixed_cursor(&mut hasher, &source.resolved_input_data_cursor(index)?)?;
    }
    for index in source.input_count()?..source.witness_count()? {
        update_len_prefixed_cursor(&mut hasher, &source.witness_cursor(index)?)?;
    }
    hasher.finalize(&mut out);

    Ok(out)
}
```

Also add this required method to `SigningDataSource` in `source.rs`:

```rust
fn input_count(&self) -> Result<usize, CoreError>;
```

- [ ] **Step 3: Update `InMemorySource` for signing**

Add these fields:

```rust
pub raw_inputs: Vec<Vec<u8>>,
pub raw_outputs: Vec<Vec<u8>>,
pub raw_outputs_data: Vec<Vec<u8>>,
pub raw_cell_deps: Vec<Vec<u8>>,
pub raw_header_deps: Vec<[u8; 32]>,
pub witnesses: Vec<Vec<u8>>,
```

Implement `SigningDataSource`:

```rust
impl SigningDataSource for InMemorySource {
    fn input_count(&self) -> Result<usize, CoreError> {
        Ok(self.input_locks.len())
    }

    fn raw_input_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        self.raw_inputs
            .get(index)
            .map(|bytes| cursor_from_slice(bytes))
            .ok_or(CoreError::MissingHashInput)
    }

    fn raw_output_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        self.raw_outputs
            .get(index)
            .map(|bytes| cursor_from_slice(bytes))
            .ok_or(CoreError::MissingHashInput)
    }

    fn raw_output_data_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        self.raw_outputs_data
            .get(index)
            .map(|bytes| cursor_from_slice(bytes))
            .ok_or(CoreError::MissingHashInput)
    }

    fn raw_cell_dep_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        self.raw_cell_deps
            .get(index)
            .map(|bytes| cursor_from_slice(bytes))
            .ok_or(CoreError::MissingHashInput)
    }

    fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        self.raw_header_deps
            .get(index)
            .copied()
            .ok_or(CoreError::MissingHashInput)
    }

    fn witness_count(&self) -> Result<usize, CoreError> {
        Ok(self.witnesses.len())
    }

    fn witness_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        self.witnesses
            .get(index)
            .map(|bytes| cursor_from_slice(bytes))
            .ok_or(CoreError::MissingHashInput)
    }
}
```

- [ ] **Step 4: Update call sites**

Change `LockScriptQuery::required_signatures` to accept the source:

```rust
pub fn required_signatures<S: SigningDataSource>(
    &self,
    source: &S,
) -> Result<Vec<SignatureRequest>, CoreError>
```

Change `collect_sighash_all_signatures` and `collect_otx_signatures` to accept `source: &S`.

In `sighash.rs`, replace:

```rust
tx_with_message_hash(message, parts)?
tx_without_message_hash(parts)?
```

with cursor-backed calls. Until Round 2 converts message views, use:

```rust
let message_cursor = cursor_from_slice(message);
tx_with_message_hash(&message_cursor, source)?
tx_without_message_hash(source)?
```

- [ ] **Step 5: Remove trailing witness fields**

Delete these fields:

```rust
SigningHashParts.trailing_witnesses
PreparedContextInput.trailing_witnesses
```

Delete this code from the lock loader:

```rust
let trailing_witnesses = witnesses.iter().skip(input_count).cloned().collect();
```

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test -p cobuild-core --offline --test hash
cargo test -p cobuild-core --offline --test signature_requests
cargo test -p tests --offline --test contract_template_layout cobuild_core_uses_streaming_source_boundaries
```

Expected: all pass after imports and test fixtures are updated.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/cobuild-core contracts/cobuild-otx-lock tests
git commit -m "refactor: stream transaction signing hash inputs"
```

**Rollback risk:** Medium. This changes a core public helper API and many test fixtures, but hash bytes should remain unchanged.

### Task 5: Rename Core Prepare Module And Prepare From Source

**Files:**
- Move: `crates/cobuild-core/src/loader.rs` to `crates/cobuild-core/src/prepare.rs`
- Modify: `crates/cobuild-core/src/lib.rs`
- Modify: `crates/cobuild-core/src/prepare.rs`
- Modify: `contracts/cobuild-otx-lock/src/loader.rs`
- Modify tests importing `cobuild_core::loader`

- [ ] **Step 1: Move module**

Run:

```bash
git mv crates/cobuild-core/src/loader.rs crates/cobuild-core/src/prepare.rs
```

In `lib.rs`, replace:

```rust
pub mod loader;
```

with:

```rust
pub mod prepare;
```

- [ ] **Step 2: Replace transaction parsing entrypoint**

In `prepare.rs`, replace `parse_transaction_info(data: &[u8])` with:

```rust
pub fn parse_transaction_info<S: TransactionSource>(
    source: &S,
) -> Result<TransactionInfo, CoreError> {
    let tx = Transaction::from(source.transaction_cursor()?);
    parse_transaction_from_reader(&tx)
}

fn parse_transaction_from_reader(tx: &Transaction) -> Result<TransactionInfo, CoreError> {
    tx.verify(false).map_err(|_| CoreError::InvalidOtxLayout)?;
    let raw = tx.raw().map_err(|_| CoreError::MalformedCobuild)?;
    let witnesses_reader = tx.witnesses().map_err(|_| CoreError::MalformedCobuild)?;
    let witness_count = witnesses_reader
        .len()
        .map_err(|_| CoreError::MalformedCobuild)?;
    let inputs = raw.inputs().map_err(|_| CoreError::MalformedCobuild)?;
    let outputs = raw.outputs().map_err(|_| CoreError::MalformedCobuild)?;
    let cell_deps = raw.cell_deps().map_err(|_| CoreError::MalformedCobuild)?;
    let header_deps = raw.header_deps().map_err(|_| CoreError::MalformedCobuild)?;

    Ok(TransactionInfo {
        witness_count,
        input_count: inputs.len().map_err(|_| CoreError::MalformedCobuild)?,
        output_count: outputs.len().map_err(|_| CoreError::MalformedCobuild)?,
        cell_dep_count: cell_deps.len().map_err(|_| CoreError::MalformedCobuild)?,
        header_dep_count: header_deps.len().map_err(|_| CoreError::MalformedCobuild)?,
    })
}
```

Change `TransactionInfo` to:

```rust
pub struct TransactionInfo {
    pub witness_count: usize,
    pub input_count: usize,
    pub output_count: usize,
    pub cell_dep_count: usize,
    pub header_dep_count: usize,
}
```

- [ ] **Step 3: Add `prepare_context_from_source`**

Add:

```rust
pub fn prepare_context_from_source<S: SigningDataSource>(
    source: &S,
) -> Result<PreparedContext, CoreError> {
    let info = parse_transaction_info(source)?;
    let input_locks = collect_input_locks(source, info.input_count)?;
    let input_types = collect_input_types(source, info.input_count)?;
    let output_types = collect_output_types(source, info.output_count)?;
    let witnesses = collect_witnesses_for_layout(source, info.witness_count)?;

    let context = CobuildContext::new(
        LayoutTx {
            witnesses,
            input_count: info.input_count,
            output_count: info.output_count,
            cell_dep_count: info.cell_dep_count,
            header_dep_count: info.header_dep_count,
        },
        ScriptHashIndex {
            input_locks,
            input_types,
            output_types,
        },
    )?;

    Ok(PreparedContext::new(context))
}
```

Add helpers:

```rust
fn collect_input_locks<S: TransactionSource>(
    source: &S,
    count: usize,
) -> Result<Vec<[u8; 32]>, CoreError> {
    let mut hashes = Vec::with_capacity(count);
    for index in 0..count {
        hashes.push(source.input_lock_hash(index)?);
    }
    Ok(hashes)
}

fn collect_input_types<S: TransactionSource>(
    source: &S,
    count: usize,
) -> Result<Vec<Option<[u8; 32]>>, CoreError> {
    let mut hashes = Vec::with_capacity(count);
    for index in 0..count {
        hashes.push(source.input_type_hash(index)?);
    }
    Ok(hashes)
}

fn collect_output_types<S: TransactionSource>(
    source: &S,
    count: usize,
) -> Result<Vec<Option<[u8; 32]>>, CoreError> {
    let mut hashes = Vec::with_capacity(count);
    for index in 0..count {
        hashes.push(source.output_type_hash(index)?);
    }
    Ok(hashes)
}

fn collect_witnesses_for_layout<S: SigningDataSource>(
    source: &S,
    count: usize,
) -> Result<Vec<Vec<u8>>, CoreError> {
    let mut witnesses = Vec::with_capacity(count);
    for index in 0..count {
        witnesses.push(cursor_bytes(&source.witness_cursor(index)?)?);
    }
    Ok(witnesses)
}
```

This still materializes witnesses for layout scanning. That is allowed for Round 1 because Round 2 replaces this with cursor-backed witness views.

- [ ] **Step 4: Update `PreparedContext`**

In `context.rs`, change:

```rust
pub struct PreparedContext {
    pub context: CobuildContext,
    pub signing_hash_parts: SigningHashParts,
}
```

to:

```rust
pub struct PreparedContext {
    pub context: CobuildContext,
}
```

And change `PreparedContext::new` to:

```rust
impl PreparedContext {
    pub fn new(context: CobuildContext) -> Self {
        Self { context }
    }
}
```

- [ ] **Step 5: Keep existing lock loader compiling through the new module path**

In `contracts/cobuild-otx-lock/src/loader.rs`, replace the old core module path:

```rust
use cobuild_core::{
    prepare::{
        PreparedContextInput, parse_transaction_info, prepare_context, script_args_from_slice,
    },
};
```

Keep the existing `parse_transaction_info(data: &[u8])`, `PreparedContextInput`, and `prepare_context` functions in `prepare.rs` during this task. Task 6 removes the full owned loading path after `ChainSource` is available.

- [ ] **Step 6: Run tests**

Run:

```bash
cargo test -p cobuild-core --offline --test no_entity_dependency
cargo test -p tests --offline --test contract_template_layout
```

Expected: structural tests for core module names pass. The lock streaming structural assertion still fails until Task 6 renames `loader.rs` and removes `load_transaction`.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/cobuild-core contracts/cobuild-otx-lock tests
git commit -m "refactor: prepare cobuild context from source"
```

**Rollback risk:** Medium. This changes public module names and context construction, but leaves witness layout scanning owned until Round 2.

### Task 6: Add Syscall-Backed Chain Source And Remove Full Transaction Load

**Files:**
- Move: `contracts/cobuild-otx-lock/src/loader.rs` to `contracts/cobuild-otx-lock/src/chain.rs`
- Modify: `contracts/cobuild-otx-lock/src/{entry,lib}.rs`
- Modify: `tests/tests/contract_template_layout.rs`

- [ ] **Step 1: Move lock module**

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

In `entry.rs`, replace imports from `loader` to `chain`.

- [ ] **Step 2: Add syscall reader type**

In `chain.rs`, add:

```rust
use alloc::boxed::Box;
use core::cmp::min;

use cobuild_types::lazy_reader::support::{Cursor, Error as MoleculeError, Read};

struct SyscallReader<F> {
    total_size: usize,
    load: F,
}

impl<F> SyscallReader<F>
where
    F: Fn(&mut [u8], usize) -> Result<usize, SysError>,
{
    fn new(load: F) -> Result<Self, Error> {
        let total_size = read_size(|buf| load(buf, 0))?;
        Ok(Self { total_size, load })
    }
}

impl<F> Read for SyscallReader<F>
where
    F: Fn(&mut [u8], usize) -> Result<usize, SysError>,
{
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, MoleculeError> {
        if offset >= self.total_size {
            return Err(MoleculeError::OutOfBound(offset, self.total_size));
        }
        let requested = min(buf.len(), self.total_size - offset);
        match (self.load)(&mut buf[..requested], offset) {
            Ok(read) => Ok(min(read, requested)),
            Err(SysError::LengthNotEnough(actual)) => Ok(min(actual.saturating_sub(offset), requested)),
            Err(_) => Err(MoleculeError::OutOfBound(offset, self.total_size)),
        }
    }
}

fn read_size<F>(load: F) -> Result<usize, Error>
where
    F: Fn(&mut [u8]) -> Result<usize, SysError>,
{
    let mut buf = [0u8; 4];
    match load(&mut buf) {
        Ok(size) | Err(SysError::LengthNotEnough(size)) => Ok(size),
        Err(err) => Err(map_sys_error(err)),
    }
}
```

- [ ] **Step 3: Add cursor constructors**

Add:

```rust
fn transaction_cursor() -> Result<Cursor, Error> {
    let reader = SyscallReader::new(|buf, offset| syscalls::load_transaction(buf, offset))?;
    Ok(Cursor::new(reader.total_size, Box::new(reader)))
}

fn script_cursor() -> Result<Cursor, Error> {
    let reader = SyscallReader::new(|buf, offset| syscalls::load_script(buf, offset))?;
    Ok(Cursor::new(reader.total_size, Box::new(reader)))
}

fn input_cell_cursor(index: usize, source: Source) -> Result<Cursor, Error> {
    let reader = SyscallReader::new(|buf, offset| syscalls::load_cell(buf, offset, index, source))?;
    Ok(Cursor::new(reader.total_size, Box::new(reader)))
}

fn input_cell_data_cursor(index: usize, source: Source) -> Result<Cursor, Error> {
    let reader =
        SyscallReader::new(|buf, offset| syscalls::load_cell_data(buf, offset, index, source))?;
    Ok(Cursor::new(reader.total_size, Box::new(reader)))
}
```

- [ ] **Step 4: Implement `ChainSource`**

Add:

```rust
pub(crate) struct ChainSource;

impl cobuild_core::source::TransactionSource for ChainSource {
    fn transaction_cursor(&self) -> Result<Cursor, CoreError> {
        transaction_cursor().map_err(|_| CoreError::InvalidContextInput)
    }

    fn script_cursor(&self) -> Result<Cursor, CoreError> {
        script_cursor().map_err(|_| CoreError::InvalidContextInput)
    }

    fn tx_hash(&self) -> Result<[u8; 32], CoreError> {
        load_tx_hash().map_err(|_| CoreError::InvalidContextInput)
    }

    fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        load_cell_field_hash(index, Source::Input, CellField::LockHash)
            .map_err(|_| CoreError::InvalidContextInput)
    }

    fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        match load_cell_field_hash(index, Source::Input, CellField::TypeHash) {
            Ok(hash) => Ok(Some(hash)),
            Err(Error::LockSemanticFailure) => Ok(None),
            Err(_) => Err(CoreError::InvalidContextInput),
        }
    }

    fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        match load_cell_field_hash(index, Source::Output, CellField::TypeHash) {
            Ok(hash) => Ok(Some(hash)),
            Err(Error::LockSemanticFailure) => Ok(None),
            Err(_) => Err(CoreError::InvalidContextInput),
        }
    }

    fn resolved_input_output_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        input_cell_cursor(index, Source::Input).map_err(|_| CoreError::MissingHashInput)
    }

    fn resolved_input_data_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        input_cell_data_cursor(index, Source::Input).map_err(|_| CoreError::MissingHashInput)
    }
}
```

Add the imports needed by the `SigningDataSource` implementation:

```rust
use cobuild_types::lazy_reader::blockchain::Transaction;
use cobuild_core::source::SigningDataSource;
```

Implement `SigningDataSource` in the same file:

```rust
impl SigningDataSource for ChainSource {
    fn input_count(&self) -> Result<usize, CoreError> {
        let tx = Transaction::from(self.transaction_cursor()?);
        let raw = tx.raw().map_err(|_| CoreError::MalformedCobuild)?;
        raw.inputs()
            .and_then(|inputs| inputs.len())
            .map_err(|_| CoreError::MalformedCobuild)
    }

    fn raw_input_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        let tx = Transaction::from(self.transaction_cursor()?);
        let raw = tx.raw().map_err(|_| CoreError::MalformedCobuild)?;
        raw.inputs()
            .and_then(|inputs| inputs.get(index))
            .map(|input| input.cursor)
            .map_err(|_| CoreError::MissingHashInput)
    }

    fn raw_output_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        let tx = Transaction::from(self.transaction_cursor()?);
        let raw = tx.raw().map_err(|_| CoreError::MalformedCobuild)?;
        raw.outputs()
            .and_then(|outputs| outputs.get(index))
            .map(|output| output.cursor)
            .map_err(|_| CoreError::MissingHashInput)
    }

    fn raw_output_data_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        let tx = Transaction::from(self.transaction_cursor()?);
        let raw = tx.raw().map_err(|_| CoreError::MalformedCobuild)?;
        raw.outputs_data()
            .and_then(|data| data.get(index))
            .map_err(|_| CoreError::MissingHashInput)
    }

    fn raw_cell_dep_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        let tx = Transaction::from(self.transaction_cursor()?);
        let raw = tx.raw().map_err(|_| CoreError::MalformedCobuild)?;
        raw.cell_deps()
            .and_then(|cell_deps| cell_deps.get(index))
            .map(|cell_dep| cell_dep.cursor)
            .map_err(|_| CoreError::MissingHashInput)
    }

    fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        let tx = Transaction::from(self.transaction_cursor()?);
        let raw = tx.raw().map_err(|_| CoreError::MalformedCobuild)?;
        raw.header_deps()
            .and_then(|header_deps| header_deps.get(index))
            .map_err(|_| CoreError::MissingHashInput)
    }

    fn witness_count(&self) -> Result<usize, CoreError> {
        let tx = Transaction::from(self.transaction_cursor()?);
        tx.witnesses()
            .and_then(|witnesses| witnesses.len())
            .map_err(|_| CoreError::MalformedCobuild)
    }

    fn witness_cursor(&self, index: usize) -> Result<Cursor, CoreError> {
        let tx = Transaction::from(self.transaction_cursor()?);
        tx.witnesses()
            .and_then(|witnesses| witnesses.get(index))
            .map_err(|_| CoreError::MissingHashInput)
    }
}
```

- [ ] **Step 5: Remove full transaction load**

Delete:

```rust
fn load_transaction() -> Result<Vec<u8>, Error>
```

Delete any call to:

```rust
parse_transaction_info(&load_transaction()?)
```

Replace `load_prepared_context` with:

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

- [ ] **Step 6: Update `entry.rs`**

Replace:

```rust
let prepared = load_prepared_context()?;
let signature_requests = prepared
    .context
    .lock_query(current_script_hash)
    .required_signatures(&prepared.signing_hash_parts)
    .map_err(map_core_error)?;
```

with:

```rust
let loaded = load_prepared_context()?;
let signature_requests = loaded
    .prepared
    .context
    .lock_query(current_script_hash)
    .required_signatures(&loaded.source)
    .map_err(map_core_error)?;
```

- [ ] **Step 7: Run contract checks**

Run:

```bash
cargo test -p tests --offline --test contract_template_layout
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
```

Expected: both pass.

- [ ] **Step 8: Commit**

Run:

```bash
git add contracts/cobuild-otx-lock tests/tests/contract_template_layout.rs
git commit -m "refactor: stream chain data through source"
```

**Rollback risk:** Medium-high. This changes the on-chain loading path. Keep the commit isolated so syscall-reader regressions are easy to revert.

### Task 7: Stream OTX Raw Hash Inputs

**Files:**
- Modify: `crates/cobuild-core/src/hash.rs`
- Modify: `crates/cobuild-core/src/otx_request.rs`
- Modify: `crates/cobuild-core/src/context.rs`
- Modify tests in `crates/cobuild-core/tests/{hash,signature_requests}.rs`

- [ ] **Step 1: Add counting source test**

In `crates/cobuild-core/tests/signature_requests.rs`, add a helper:

```rust
#[derive(Default)]
struct CountingSource {
    inner: cobuild_core::source::InMemorySource,
    resolved_data_reads: core::cell::Cell<usize>,
    raw_input_reads: core::cell::Cell<usize>,
}
```

Implement `TransactionSource` and `SigningDataSource` by delegating to `inner`, but increment counters in `resolved_input_data_cursor` and `raw_input_cursor`.

Add test:

```rust
#[test]
fn unrelated_otx_query_does_not_read_hash_payloads() {
    let context = /* reuse existing unrelated OTX fixture */;
    let source = CountingSource {
        inner: otx_signing_source(),
        ..Default::default()
    };

    let requests = context.lock_query([9u8; 32]).required_signatures(&source).unwrap();

    assert!(requests.is_empty());
    assert_eq!(source.resolved_data_reads.get(), 0);
    assert_eq!(source.raw_input_reads.get(), 0);
}
```

Use the same witness and script-hash fixture currently used by `unrelated_otx_lock_query_does_not_require_raw_hash_parts`.

- [ ] **Step 2: Change OTX hash signatures**

In `hash.rs`, change:

```rust
pub fn otx_base_hash(
    otx: &OtxData,
    layout: &OtxLayout,
    raw: &RawTxParts,
    resolved_inputs: &[ResolvedInputHashPart],
) -> Result<[u8; 32], CoreError>
```

to:

```rust
pub fn otx_base_hash<S: SigningDataSource>(
    otx: &OtxData,
    layout: &OtxLayout,
    source: &S,
) -> Result<[u8; 32], CoreError>
```

Change `otx_append_hash` similarly.

- [ ] **Step 3: Replace owned reads with cursor reads**

In `otx_base_hash`, replace raw/resolved reads with:

```rust
let input = source.raw_input_cursor(tx_index)?;
let resolved_output = source.resolved_input_output_cursor(tx_index)?;
let resolved_data = source.resolved_input_data_cursor(tx_index)?;
let input_view = CellInput::from(input.clone());
update_cursor(&mut hasher, &resolved_output)?;
update_len_prefixed_cursor(&mut hasher, &resolved_data)?;
```

For outputs:

```rust
let output = source.raw_output_cursor(tx_index)?;
let output_data = source.raw_output_data_cursor(tx_index)?;
let output_view = CellOutput::from(output.clone());
update_len_prefixed_cursor(&mut hasher, &output_data)?;
```

For cell deps and header deps:

```rust
update_cursor(&mut hasher, &source.raw_cell_dep_cursor(tx_index)?)?;
hasher.update(&source.raw_header_dep_hash(tx_index)?);
```

- [ ] **Step 4: Remove owned hash input types**

Delete:

```rust
pub struct RawTxParts
pub struct ResolvedInputHashPart
```

Delete `CobuildContext.raw_parts`.

Delete `CobuildContext::with_raw_parts`.

- [ ] **Step 5: Update OTX request collection**

In `otx_request.rs`, remove:

```rust
let raw_parts = self.context.raw_parts.as_ref().ok_or(CoreError::MissingHashInput)?;
```

Call:

```rust
let base_hash = otx_base_hash(&otx.witness, &otx.layout, source)?;
```

and:

```rust
otx_append_hash(&otx.witness, &otx.layout, source, base_hash)?
```

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test -p cobuild-core --offline --test hash
cargo test -p cobuild-core --offline --test signature_requests unrelated_otx_query_does_not_read_hash_payloads
cargo test -p cobuild-core --offline --test signature_requests
```

Expected: all pass; counting test proves irrelevant OTX query does not read hash payloads.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/cobuild-core
git commit -m "refactor: stream otx hash inputs"
```

**Rollback risk:** High. This changes OTX hash input plumbing. Keep fixtures close and compare existing hash expected values.

## Round 2: Cursor-Backed View Layer

### Task 8: Add View Cleanup Structural Tests

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

- [ ] **Step 2: Add cursor lifetime behavior test**

In `crates/cobuild-core/tests/view.rs`, replace `parsed_view_survives_source_slice_drop` with:

```rust
#[test]
fn cursor_backed_view_reads_from_owned_reader_after_source_slice_drop() {
    let view = {
        let witness = sighash_all_only_witness_bytes(&[0x11, 0x22, 0x33]);
        WitnessLayoutView::from_slice(&witness).unwrap()
    };

    let sighash = view.sighash_all_witness_view().unwrap().unwrap();
    assert_eq!(sighash.seal().unwrap(), vec![0x11, 0x22, 0x33]);
}
```

- [ ] **Step 3: Run focused tests and confirm failure**

Run:

```bash
cargo test -p tests --offline --test contract_template_layout cobuild_core_view_is_cursor_backed_protocol_boundary
cargo test -p cobuild-core --offline --test view cursor_backed_view_reads_from_owned_reader_after_source_slice_drop
```

Expected: fail because the old `*Data` DTOs and API names still exist.

**Rollback risk:** Low. Tests only.

### Task 9: Replace Owned View DTOs With Cursor-Backed Views

**Files:**
- Modify: `crates/cobuild-core/src/view.rs`
- Modify: `crates/cobuild-core/src/layout.rs`
- Modify: `crates/cobuild-core/src/sighash.rs`
- Modify: `crates/cobuild-core/src/message.rs`
- Modify: `crates/cobuild-core/src/seal.rs`

- [ ] **Step 1: Add view structs**

In `view.rs`, replace `SighashAllWitnessLayout`, `OtxStartData`, `ActionData`, `SealPairData`, and `OtxData` with:

```rust
pub enum SighashAllWitnessView {
    WithMessage { seal: Cursor, message: Cursor },
    SealOnly { seal: Cursor },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OtxStartView {
    pub start_input_cell: usize,
    pub start_output_cell: usize,
    pub start_cell_deps: usize,
    pub start_header_deps: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MessageActionView {
    pub script_role: u8,
    pub script_hash: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MaskView {
    cursor: Cursor,
}

impl MaskView {
    pub fn new(cursor: Cursor) -> Self {
        Self { cursor }
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SealPairView {
    pub script_hash: [u8; 32],
    pub scope: u8,
    pub seal: Cursor,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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
```

- [ ] **Step 2: Rename view methods**

Replace:

```rust
pub fn sighash_all_witness_layout(&self) -> Result<Option<SighashAllWitnessLayout>, CoreError>
pub fn otx_start(&self) -> Result<Option<OtxStartData>, CoreError>
pub fn otx(&self) -> Result<Option<OtxData>, CoreError>
```

with:

```rust
pub fn sighash_all_witness_view(&self) -> Result<Option<SighashAllWitnessView>, CoreError>
pub fn otx_start_view(&self) -> Result<Option<OtxStartView>, CoreError>
pub fn otx_view(&self) -> Result<Option<OtxView>, CoreError>
```

- [ ] **Step 3: Keep owned seal only at final boundary**

Add:

```rust
impl SighashAllWitnessView {
    pub fn seal(&self) -> Result<Vec<u8>, CoreError> {
        match self {
            Self::WithMessage { seal, .. } | Self::SealOnly { seal } => cursor_bytes(seal),
        }
    }

    pub fn message(&self) -> Option<&Cursor> {
        match self {
            Self::WithMessage { message, .. } => Some(message),
            Self::SealOnly { .. } => None,
        }
    }
}
```

Add:

```rust
impl SealPairView {
    pub fn seal_bytes(&self) -> Result<Vec<u8>, CoreError> {
        cursor_bytes(&self.seal)
    }
}
```

- [ ] **Step 4: Update layout and query imports**

In `layout.rs`, replace `OtxStartData` with `OtxStartView` and `OtxData` with `OtxView`.

In `sighash.rs`, replace `SighashAllWitnessLayout` with `SighashAllWitnessView`.

In `message.rs`, accept `&Cursor`:

```rust
pub(crate) fn validate_message_targets(&self, message: &Cursor) -> Result<(), CoreError>
```

In `seal.rs`, accept `&[SealPairView]` and call `seal.seal_bytes()` only when a matching seal is selected.

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p cobuild-core --offline --test view
cargo test -p cobuild-core --offline --test layout
cargo test -p cobuild-core --offline --test signature_requests
```

Expected: all pass after call sites use the new view names.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/cobuild-core tests/tests/contract_template_layout.rs
git commit -m "refactor: use cursor backed cobuild views"
```

**Rollback risk:** High. This rewires the protocol view DTOs; keep it separate from hash source changes.

### Task 10: Stream Message And Mask Hashing From Views

**Files:**
- Modify: `crates/cobuild-core/src/hash.rs`
- Modify: `crates/cobuild-core/src/message.rs`
- Modify: `crates/cobuild-core/src/layout.rs`
- Modify: `crates/cobuild-core/src/otx_request.rs`

- [ ] **Step 1: Add `MaskView::bit`**

In `view.rs`, implement:

```rust
impl MaskView {
    pub fn bit(&self, index: usize) -> Result<bool, CoreError> {
        let byte_index = index / 8;
        let bit_index = index % 8;
        if byte_index >= self.cursor.size {
            return Err(CoreError::InvalidOtxLayout);
        }
        let mut buf = [0u8; 1];
        let mut cursor = self.cursor.clone();
        cursor
            .add_offset(byte_index)
            .map_err(|_| CoreError::MalformedCobuild)?;
        cursor.size = 1;
        let read = cursor
            .read_at(&mut buf)
            .map_err(|_| CoreError::MalformedCobuild)?;
        if read != 1 {
            return Err(CoreError::MalformedCobuild);
        }
        Ok((buf[0] & (1 << bit_index)) != 0)
    }

    pub fn len(&self) -> usize {
        self.cursor.size
    }
}
```

- [ ] **Step 2: Replace mask helper calls**

In `hash.rs`, replace calls like:

```rust
mask_bit(&otx.base_input_masks, local_index * 2)?
```

with:

```rust
otx.base_input_masks.bit(local_index * 2)?
```

Replace length-prefix hashing of masks with:

```rust
update_len_prefixed_cursor(&mut hasher, otx.base_input_masks.cursor())?;
```

- [ ] **Step 3: Validate masks without cloning**

In `layout.rs`, change `validate_mask` to accept `&MaskView`:

```rust
fn validate_mask(mask: &MaskView, bit_count: usize) -> Result<(), CoreError> {
    let expected_len = bit_count.div_ceil(8);
    if mask.len() != expected_len {
        return Err(CoreError::InvalidOtxLayout);
    }
    for bit in bit_count..expected_len * 8 {
        if mask.bit(bit)? {
            return Err(CoreError::InvalidOtxLayout);
        }
    }
    Ok(())
}
```

If `usize::div_ceil` is unavailable for the repo MSRV, use:

```rust
let expected_len = (bit_count + 7) / 8;
```

- [ ] **Step 4: Run hash/layout tests**

Run:

```bash
cargo test -p cobuild-core --offline --test hash
cargo test -p cobuild-core --offline --test layout
```

Expected: all pass.

- [ ] **Step 5: Commit**

Run:

```bash
git add crates/cobuild-core
git commit -m "refactor: stream cobuild view payload hashing"
```

**Rollback risk:** Medium-high. Mask validation and hashing are protocol-sensitive; existing layout/hash tests must stay unchanged.

### Task 11: Final Module Renames And Boundary Cleanup

**Files:**
- Modify: `crates/cobuild-core/src/lib.rs`
- Modify: `contracts/cobuild-otx-lock/src/lib.rs`
- Modify: `tests/tests/{contract_template_layout,workspace_layout}.rs`
- Modify docs if needed: `docs/superpowers/specs/2026-06-03-cobuild-core-streaming-reader-and-hash-input-design.md`

- [ ] **Step 1: Remove compatibility names**

Run:

```bash
rg -n "loader|TxScriptHashes|RawTxParts|ResolvedInputHashPart|SigningHashParts|OtxData|OtxStartData|SealPairData|ActionData|SighashAllWitnessLayout|trailing_witnesses" crates contracts tests
```

Expected remaining matches are only in tests that assert names do not exist, docs, or intentionally retained comments. Remove production-code matches.

- [ ] **Step 2: Update structural tests**

In `tests/tests/contract_template_layout.rs`, ensure production structure checks assert:

```rust
assert!(core_src.join("prepare.rs").is_file());
assert!(core_src.join("reader.rs").is_file());
assert!(core_src.join("source.rs").is_file());
assert!(!core_src.join("loader.rs").exists());
assert!(lock_src.join("chain.rs").is_file());
assert!(!lock_src.join("loader.rs").exists());
```

- [ ] **Step 3: Run source scans**

Run:

```bash
rg -n "cobuild_types::entity|::entity::" crates/cobuild-core/src contracts/cobuild-otx-lock/src
rg -n "unsafe" crates/cobuild-core/src contracts/cobuild-otx-lock/src
rg -n "ckb_std" crates/cobuild-core/src
```

Expected: no output for all three commands.

- [ ] **Step 4: Run full verification**

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

- [ ] **Step 5: Commit**

Run:

```bash
git add crates contracts tests docs
git commit -m "refactor: finalize streaming cobuild boundaries"
```

**Rollback risk:** Medium. This task should mostly remove stale names and update tests after previous behavioral changes are already passing.

## Self-Review Checklist

- Spec coverage:
  - Full transaction load removed by Tasks 5 and 6.
  - Duplicated trailing witnesses removed by Task 4.
  - Eager raw hash inputs removed by Task 7.
  - Reader helpers moved out of `view.rs` by Task 2.
  - Cursor-backed view layer implemented by Tasks 8 through 10.
  - Lock crate syscall boundary renamed and isolated by Task 6.
  - No `ckb_std` in core enforced by Tasks 1 and 11.
  - No generated entity public abstraction enforced by existing and updated boundary tests.
- Placeholder scan:
  - No placeholder markers or unspecified "add tests" steps are intentionally present.
  - Each task includes concrete files and commands.
- Type consistency:
  - `TransactionSource` and `SigningDataSource` are introduced before call sites use them.
  - `reader.rs` is introduced before `source.rs` depends on `cursor_from_slice`.
  - `prepare.rs` is introduced before lock `chain.rs` calls `prepare_context_from_source`.
  - Round 2 view names are introduced before hash/message/seal modules consume them.
