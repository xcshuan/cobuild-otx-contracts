# Cobuild Core Syscall Concrete Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the source-trait-driven Cobuild core with a concrete syscall-backed core API while keeping `LockVerifier` for now.

**Architecture:** `cobuild-core` owns CKB syscall transaction reading and exposes `CobuildEngine::prepare_from_syscalls()`. `PreparedCobuild` plans lock/type validation without receiving a reader/source parameter. The lock contract no longer has a `chain` module and only parses args, calls core, and verifies signatures.

**Tech Stack:** Rust `no_std`, `ckb-std` 1.1 with `allocator`, `ckb-types`, and `dummy-atomic`, Molecule lazy readers from `cobuild-types`, `cargo test`, `cargo clippy`, root `make build`.

---

## File Structure

- Modify `crates/cobuild-core/Cargo.toml`
  - Add `ckb-std` dependency with the same contract-safe features currently used by `cobuild-otx-lock`.
- Modify `crates/cobuild-core/src/lib.rs`
  - Add internal `syscalls` module.
  - Remove public `source` module export.
- Create `crates/cobuild-core/src/syscalls.rs`
  - Own syscall-backed cursor construction, tx counts cache, raw tx lazy access, witness access, resolved input access, and script hash helpers.
- Modify `crates/cobuild-core/src/hash/writer.rs`
  - Replace `ClassifiedCursor` writer helpers with `Cursor + CoreError` helpers.
- Modify `crates/cobuild-core/src/hash/mod.rs`
  - Remove `HashInputSource` generics and call `syscalls` helpers directly.
- Modify `crates/cobuild-core/src/engine.rs`
  - Replace `prepare(source)` with `prepare_from_syscalls()`.
  - Remove reader/source parameters from plan methods and internal helpers.
- Modify `crates/cobuild-core/src/layout.rs`
  - Remove `WitnessCursorSource`.
  - Add direct syscall layout scanning while keeping test fixture layout construction.
- Delete `crates/cobuild-core/src/source.rs`
  - Remove `TransactionSource`, `HashInputSource`, `ClassifiedCursor`, `CursorReadContext`, and `InMemorySource`.
- Delete `contracts/cobuild-otx-lock/src/chain.rs`
- Delete `contracts/cobuild-otx-lock/src/chain/reader.rs`
- Modify `contracts/cobuild-otx-lock/src/lib.rs`
  - Remove `mod chain`.
- Modify `contracts/cobuild-otx-lock/src/entry.rs`
  - Call `CobuildEngine::prepare_from_syscalls()` and `plan_lock_validation(current_script_hash)`.
- Modify tests under `crates/cobuild-core/tests/`
  - Delete source-driven tests that depend on `InMemorySource`, `TransactionSource`, or `HashInputSource`.
- Modify `tests/tests/contract_template_layout.rs`
  - Guard against old abstractions and require concrete syscall core.
- Modify `docs/CobuildAgentDevelopGuide.md`
  - Document syscall concrete core and removed source abstractions.

---

### Task 1: Add Red Architecture Guards For Concrete Syscall Core

**Files:**
- Modify: `tests/tests/contract_template_layout.rs`
- Test: `tests/tests/contract_template_layout.rs`

- [ ] **Step 1: Replace source-boundary guard with syscall-concrete guard**

In `tests/tests/contract_template_layout.rs`, replace the whole `cobuild_core_exposes_source_boundary_without_ckb_std` test with:

```rust
#[test]
fn cobuild_core_uses_concrete_syscall_reader_without_source_traits() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");
    let lock_src = workspace_root.join("contracts/cobuild-otx-lock/src");

    assert!(
        core_src.join("syscalls.rs").is_file(),
        "cobuild-core must own syscall-backed transaction reading"
    );
    assert!(
        !core_src.join("source.rs").exists(),
        "source.rs must be removed with TransactionSource/HashInputSource"
    );
    assert!(
        !lock_src.join("chain.rs").exists(),
        "lock crate must not keep syscall tx reader logic"
    );
    assert!(
        !lock_src.join("chain").exists(),
        "lock crate must not keep chain/reader.rs"
    );

    let lib_rs = fs::read_to_string(core_src.join("lib.rs")).expect("core lib.rs");
    assert!(
        lib_rs.contains("mod syscalls"),
        "core should keep syscall helpers internal"
    );
    assert!(
        !lib_rs.contains("pub mod source"),
        "core should not export source traits"
    );

    let core_text = fs::read_to_string(core_src.join("engine.rs")).expect("engine.rs")
        + &fs::read_to_string(core_src.join("hash/mod.rs")).expect("hash/mod.rs")
        + &fs::read_to_string(core_src.join("hash/writer.rs")).expect("hash/writer.rs");
    for forbidden in [
        "TransactionSource",
        "HashInputSource",
        "InMemorySource",
        "ClassifiedCursor",
        "CursorReadContext",
        "<S:",
        "source: &S",
    ] {
        assert!(
            !core_text.contains(forbidden),
            "core production path must not keep deleted source abstraction {forbidden}"
        );
    }

    let syscalls_rs = fs::read_to_string(core_src.join("syscalls.rs")).expect("syscalls.rs");
    for expected in [
        "ckb_std",
        "SyscallBackedReader",
        "SyscallReadTarget",
        "pub(crate) fn counts(",
        "pub(crate) fn witness_cursor(",
        "pub(crate) fn raw_input_cursor(",
        "pub(crate) fn resolved_input_output_cursor(",
        "pub(crate) fn input_lock_hash(",
    ] {
        assert!(
            syscalls_rs.contains(expected),
            "syscalls.rs should expose concrete helper {expected}"
        );
    }
}
```

- [ ] **Step 2: Update lock entry guard to require core API**

In `cobuild_otx_lock_entry_owns_contract_flow`, change the expected strings to:

```rust
for expected in [
    "high_level::{load_script, load_script_hash}",
    "load_script()?",
    "AuthContext::try_from",
    "load_script_hash()?",
    "CobuildEngine::prepare_from_syscalls",
    "plan_lock_validation(current_script_hash)",
    "required_signatures",
    "LocalVerifier",
] {
    assert!(
        entry_rs.contains(expected),
        "entry.rs should expose the high-level contract flow via {expected}"
    );
}
for forbidden in [
    "from_lock_args",
    "load_current_script_args",
    "prepare_cobuild_from_syscalls",
    "PreparedCobuildContext",
    "context.tx_reader",
    "chain::",
] {
    assert!(
        !entry_rs.contains(forbidden),
        "entry.rs should not use removed wrapper {forbidden}"
    );
}
```

- [ ] **Step 3: Replace lock chain streaming guard**

Replace `cobuild_otx_lock_streams_chain_data_without_full_transaction_load` with:

```rust
#[test]
fn cobuild_core_owns_syscall_streaming_without_full_transaction_load() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");
    let lock_src = workspace_root.join("contracts/cobuild-otx-lock/src");

    assert!(
        core_src.join("syscalls.rs").is_file(),
        "syscalls.rs must own syscall-backed streaming"
    );
    assert!(
        !lock_src.join("chain.rs").exists(),
        "lock crate must not own syscall-backed streaming"
    );

    let syscalls_rs = fs::read_to_string(core_src.join("syscalls.rs")).expect("syscalls.rs");
    assert!(
        !syscalls_rs.contains("fn load_transaction() -> Result<Vec<u8>"),
        "core syscall path must not load the full transaction into Vec"
    );
    assert!(
        !syscalls_rs.contains("parse_transaction_info(&load_transaction()?"),
        "core syscall path must parse transaction from source cursor"
    );
    for expected in [
        "struct SyscallBackedReader",
        "fn syscall_cursor(",
        "fn transaction_cursor(",
        "fn script_cursor(",
        "fn resolved_input_cell_cursor(",
        "fn resolved_input_data_cursor(",
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
}
```

- [ ] **Step 4: Update hash guard away from source abstractions**

In `cobuild_core_hashing_uses_source_not_owned_hash_parts`, rename the function to `cobuild_core_hashing_uses_syscalls_not_owned_hash_parts` and replace source-specific expectations with:

```rust
assert!(
    hash_mod_rs.contains("crate::syscalls"),
    "hash/mod.rs should hash through concrete syscall helpers"
);
assert!(
    !hash_mod_rs.contains("HashInputSource"),
    "hash/mod.rs must not keep HashInputSource generic hashing"
);
assert!(
    hash_mod_rs.contains("mod writer"),
    "hash/mod.rs should keep preimage writer helpers in hash/writer.rs"
);
for expected in [
    "writer::write_cursor_with_error",
    "writer::write_len_prefixed_cursor_with_error",
] {
    assert!(
        hash_mod_rs.contains(expected),
        "hash/mod.rs should write preimages through helper {expected}"
    );
}
for forbidden in [
    "ClassifiedCursor",
    "write_len_prefixed_classified_cursor",
] {
    assert!(
        !hash_writer_rs.contains(forbidden),
        "hash/writer.rs must not keep deleted classified cursor helper {forbidden}"
    );
}
```

- [ ] **Step 5: Run guard test to verify failure**

Run:

```bash
cargo test -p tests --offline --test contract_template_layout
```

Expected: FAIL. Failure messages should mention missing `syscalls.rs`, lingering `source.rs`, or old source abstraction strings.

- [ ] **Step 6: Commit red guard**

```bash
git add tests/tests/contract_template_layout.rs
git commit -m "test: require syscall concrete cobuild core"
```

---

### Task 2: Move Syscall Transaction Reader Into `cobuild-core`

**Files:**
- Modify: `crates/cobuild-core/Cargo.toml`
- Modify: `crates/cobuild-core/src/lib.rs`
- Create: `crates/cobuild-core/src/syscalls.rs`
- Test: `tests/tests/contract_template_layout.rs`

- [ ] **Step 1: Add `ckb-std` to `cobuild-core`**

In `crates/cobuild-core/Cargo.toml`, add:

```toml
ckb-std = { version = "1.1", default-features = false, features = ["ckb-types", "dummy-atomic"] }
```

Keep existing dependencies unchanged.

- [ ] **Step 2: Add internal syscalls module**

In `crates/cobuild-core/src/lib.rs`, add:

```rust
mod syscalls;
```

Do not make it public.

- [ ] **Step 3: Create syscall reader module**

Create `crates/cobuild-core/src/syscalls.rs` with:

```rust
use alloc::boxed::Box;
use core::{cell::Cell, cmp::min};

use ckb_std::{
    ckb_constants::Source,
    error::SysError,
    high_level, syscalls,
};
use cobuild_types::lazy_reader::{
    blockchain::{RawTransaction, Transaction},
    support::{Cursor, Error as MoleculeError, Read},
};

use crate::error::CoreError;

#[derive(Clone, Copy)]
enum SyscallReadTarget {
    Transaction,
    Script,
    ResolvedInputCell { index: usize },
    ResolvedInputData { index: usize },
}

impl SyscallReadTarget {
    fn load(&self, buf: &mut [u8], offset: usize) -> Result<usize, SysError> {
        match *self {
            Self::Transaction => syscalls::load_transaction(buf, offset),
            Self::Script => syscalls::load_script(buf, offset),
            Self::ResolvedInputCell { index } => {
                syscalls::load_cell(buf, offset, index, Source::Input)
            }
            Self::ResolvedInputData { index } => {
                syscalls::load_cell_data(buf, offset, index, Source::Input)
            }
        }
    }
}

struct SyscallBackedReader {
    total_size: usize,
    target: SyscallReadTarget,
}

impl SyscallBackedReader {
    fn new(target: SyscallReadTarget) -> Result<Self, SysError> {
        let mut probe = [0u8; 1];
        let total_size = match target.load(&mut probe, 0) {
            Ok(size) => size,
            Err(SysError::LengthNotEnough(size)) => size,
            Err(err) => return Err(err),
        };
        Ok(Self { total_size, target })
    }
}

impl Read for SyscallBackedReader {
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, MoleculeError> {
        if buf.is_empty() {
            return Ok(0);
        }
        if offset >= self.total_size {
            return Err(MoleculeError::OutOfBound(offset, self.total_size));
        }

        let read_len = min(buf.len(), self.total_size - offset);
        match self.target.load(&mut buf[..read_len], offset) {
            Ok(size) => Ok(min(size, read_len)),
            Err(err) => map_syscall_read_error(err, read_len),
        }
    }
}

fn map_syscall_read_error(err: SysError, read_len: usize) -> Result<usize, MoleculeError> {
    match err {
        SysError::LengthNotEnough(available) if available >= read_len => Ok(read_len),
        SysError::LengthNotEnough(available) => {
            Err(MoleculeError::Read(min(available, read_len), read_len))
        }
        _ => Err(MoleculeError::Read(0, read_len)),
    }
}

fn syscall_cursor(target: SyscallReadTarget, error: CoreError) -> Result<Cursor, CoreError> {
    let reader = SyscallBackedReader::new(target).map_err(|_| error)?;
    let total_size = reader.total_size;
    Ok(Cursor::new(total_size, Box::new(reader)))
}

pub(crate) fn transaction_cursor() -> Result<Cursor, CoreError> {
    syscall_cursor(SyscallReadTarget::Transaction, CoreError::InvalidContextInput)
}

pub(crate) fn hash_transaction_cursor() -> Result<Cursor, CoreError> {
    syscall_cursor(SyscallReadTarget::Transaction, CoreError::MissingHashInput)
}

pub(crate) fn script_cursor() -> Result<Cursor, CoreError> {
    syscall_cursor(SyscallReadTarget::Script, CoreError::InvalidContextInput)
}

pub(crate) fn resolved_input_output_cursor(index: usize) -> Result<Cursor, CoreError> {
    syscall_cursor(
        SyscallReadTarget::ResolvedInputCell { index },
        CoreError::MissingHashInput,
    )
}

pub(crate) fn resolved_input_data_cursor(index: usize) -> Result<Cursor, CoreError> {
    syscall_cursor(
        SyscallReadTarget::ResolvedInputData { index },
        CoreError::MissingHashInput,
    )
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct TxCounts {
    pub inputs: usize,
    pub outputs: usize,
    pub cell_deps: usize,
    pub header_deps: usize,
    pub witnesses: usize,
}

#[derive(Default)]
pub(crate) struct TxCountsCache {
    counts: Cell<Option<TxCounts>>,
}

impl TxCountsCache {
    pub(crate) fn counts(&self) -> Option<TxCounts> {
        self.counts.get()
    }

    pub(crate) fn set_counts(&self, counts: TxCounts) {
        self.counts.set(Some(counts));
    }
}

fn transaction_view_for_context() -> Result<Transaction, CoreError> {
    transaction_cursor().map(Transaction::from)
}

fn transaction_view_for_hash() -> Result<Transaction, CoreError> {
    hash_transaction_cursor().map(Transaction::from)
}

fn raw_transaction_for_hash() -> Result<RawTransaction, CoreError> {
    transaction_view_for_hash()?
        .raw()
        .map_err(|_| CoreError::MissingHashInput)
}

pub(crate) fn counts(cache: &TxCountsCache) -> Result<TxCounts, CoreError> {
    if let Some(counts) = cache.counts() {
        return Ok(counts);
    }

    let tx = transaction_view_for_hash()?;
    let raw = tx.raw().map_err(|_| CoreError::MissingHashInput)?;
    let counts = TxCounts {
        inputs: raw
            .inputs()
            .and_then(|inputs| inputs.len())
            .map_err(|_| CoreError::MissingHashInput)?,
        outputs: raw
            .outputs()
            .and_then(|outputs| outputs.len())
            .map_err(|_| CoreError::MissingHashInput)?,
        cell_deps: raw
            .cell_deps()
            .and_then(|cell_deps| cell_deps.len())
            .map_err(|_| CoreError::MissingHashInput)?,
        header_deps: raw
            .header_deps()
            .and_then(|header_deps| header_deps.len())
            .map_err(|_| CoreError::MissingHashInput)?,
        witnesses: tx
            .witnesses()
            .and_then(|witnesses| witnesses.len())
            .map_err(|_| CoreError::MissingHashInput)?,
    };
    cache.set_counts(counts);
    Ok(counts)
}

pub(crate) fn witness_cursor(absolute_index: usize) -> Result<Cursor, CoreError> {
    transaction_view_for_hash()?
        .witnesses()
        .and_then(|witnesses| witnesses.get(absolute_index))
        .map_err(|_| CoreError::MissingHashInput)
}

pub(crate) fn raw_input_cursor(index: usize) -> Result<Cursor, CoreError> {
    Ok(raw_transaction_for_hash()?
        .inputs()
        .and_then(|inputs| inputs.get(index))
        .map_err(|_| CoreError::MissingHashInput)?
        .cursor)
}

pub(crate) fn raw_output_cursor(index: usize) -> Result<Cursor, CoreError> {
    Ok(raw_transaction_for_hash()?
        .outputs()
        .and_then(|outputs| outputs.get(index))
        .map_err(|_| CoreError::MissingHashInput)?
        .cursor)
}

pub(crate) fn raw_output_data_cursor(index: usize) -> Result<Cursor, CoreError> {
    raw_transaction_for_hash()?
        .outputs_data()
        .and_then(|outputs_data| outputs_data.get(index))
        .map_err(|_| CoreError::MissingHashInput)
}

pub(crate) fn raw_cell_dep_cursor(index: usize) -> Result<Cursor, CoreError> {
    Ok(raw_transaction_for_hash()?
        .cell_deps()
        .and_then(|cell_deps| cell_deps.get(index))
        .map_err(|_| CoreError::MissingHashInput)?
        .cursor)
}

pub(crate) fn raw_header_dep_hash(index: usize) -> Result<[u8; 32], CoreError> {
    raw_transaction_for_hash()?
        .header_deps()
        .and_then(|header_deps| header_deps.get(index))
        .map_err(|_| CoreError::MissingHashInput)
}

pub(crate) fn tx_hash() -> Result<[u8; 32], CoreError> {
    high_level::load_tx_hash().map_err(|_| CoreError::InvalidContextInput)
}

pub(crate) fn input_lock_hash(index: usize) -> Result<[u8; 32], CoreError> {
    high_level::load_cell_lock_hash(index, Source::Input)
        .map_err(|_| CoreError::InvalidContextInput)
}

pub(crate) fn input_type_hash(index: usize) -> Result<Option<[u8; 32]>, CoreError> {
    high_level::load_cell_type_hash(index, Source::Input)
        .map_err(|_| CoreError::InvalidContextInput)
}

pub(crate) fn output_type_hash(index: usize) -> Result<Option<[u8; 32]>, CoreError> {
    high_level::load_cell_type_hash(index, Source::Output)
        .map_err(|_| CoreError::InvalidContextInput)
}

#[cfg(test)]
mod tests {
    #[test]
    fn cached_counts_are_returned_without_recomputing() {
        let counts = super::TxCounts {
            inputs: 1,
            outputs: 2,
            cell_deps: 3,
            header_deps: 4,
            witnesses: 5,
        };
        let cache = super::TxCountsCache::default();

        cache.set_counts(counts);

        assert_eq!(cache.counts(), Some(counts));
    }
}
```

- [ ] **Step 4: Run guard test to confirm partial progress**

Run:

```bash
cargo test -p tests --offline --test contract_template_layout
```

Expected: Still FAIL because `source.rs`, old engine/hash generics, and lock `chain.rs` still exist.

- [ ] **Step 5: Commit syscall module scaffold**

```bash
git add crates/cobuild-core/Cargo.toml crates/cobuild-core/src/lib.rs crates/cobuild-core/src/syscalls.rs
git commit -m "refactor: add core syscall tx reader"
```

---

### Task 3: Replace Hash Source Generics With Syscall Helpers

**Files:**
- Modify: `crates/cobuild-core/src/hash/writer.rs`
- Modify: `crates/cobuild-core/src/hash/mod.rs`
- Test: `cargo check -p cobuild-core --offline`

- [ ] **Step 1: Replace classified writer helpers**

In `crates/cobuild-core/src/hash/writer.rs`, remove any import of `ClassifiedCursor` and ensure the file contains these helpers:

```rust
use blake2b_ref::Blake2b;
use cobuild_types::lazy_reader::support::Cursor;

use crate::{
    error::CoreError,
    hash::checked_len_prefix,
    reader::{cursor_bytes_with_error, update_cursor_with_error},
};

pub fn write_count(hasher: &mut Blake2b, count: usize) -> Result<(), CoreError> {
    hasher.update(&checked_len_prefix(count)?);
    Ok(())
}

pub fn write_cursor_with_error(
    hasher: &mut Blake2b,
    cursor: &Cursor,
    error: CoreError,
) -> Result<(), CoreError> {
    update_cursor_with_error(hasher, cursor, error)
}

pub fn write_len_prefixed_cursor_with_error(
    hasher: &mut Blake2b,
    cursor: &Cursor,
    error: CoreError,
) -> Result<(), CoreError> {
    let bytes = cursor_bytes_with_error(cursor, error)?;
    hasher.update(&checked_len_prefix(bytes.len())?);
    hasher.update(&bytes);
    Ok(())
}
```

- [ ] **Step 2: Change hash API signatures**

In `crates/cobuild-core/src/hash/mod.rs`, remove `source::HashInputSource` import and add:

```rust
use crate::syscalls;
```

Change signatures:

```rust
pub fn tx_without_message_hash(counts_cache: &syscalls::TxCountsCache) -> Result<[u8; 32], CoreError>

pub fn tx_with_message_hash(
    message: &Cursor,
    counts_cache: &syscalls::TxCountsCache,
) -> Result<[u8; 32], CoreError>

fn tx_signing_hash(
    personalization: &[u8; 16],
    message: Option<&Cursor>,
    counts_cache: &syscalls::TxCountsCache,
) -> Result<[u8; 32], CoreError>

pub fn otx_base_hash(
    otx: &OtxView,
    layout: &OtxLayout,
    counts_cache: &syscalls::TxCountsCache,
) -> Result<[u8; 32], CoreError>

pub fn otx_append_hash(
    otx: &OtxView,
    layout: &OtxLayout,
    counts_cache: &syscalls::TxCountsCache,
    base_hash: [u8; 32],
) -> Result<[u8; 32], CoreError>
```

- [ ] **Step 3: Replace source calls in tx signing hash**

In `tx_signing_hash`, replace source calls with:

```rust
hasher.update(&syscalls::tx_hash()?);
let counts = syscalls::counts(counts_cache)?;
for index in 0..counts.inputs {
    let output = syscalls::resolved_input_output_cursor(index)?;
    writer::write_cursor_with_error(&mut hasher, &output, CoreError::MissingHashInput)?;
    let data = syscalls::resolved_input_data_cursor(index)?;
    writer::write_len_prefixed_cursor_with_error(
        &mut hasher,
        &data,
        CoreError::MissingHashInput,
    )?;
}
for index in counts.inputs..counts.witnesses {
    let witness = syscalls::witness_cursor(index)?;
    writer::write_len_prefixed_cursor_with_error(
        &mut hasher,
        &witness,
        CoreError::MissingHashInput,
    )?;
}
```

- [ ] **Step 4: Replace source calls in OTX hash loops**

For every raw/resolved cursor call in `otx_base_hash` and `otx_append_hash`, use this pattern:

```rust
let input = syscalls::raw_input_cursor(tx_index)?;
let input_view = CellInput::from(input.clone());

writer::write_count(&mut hasher, local_index)?;
if mask_bit(&otx.base_input_masks, local_index * 2)? {
    hasher.update(
        &input_view
            .since()
            .map_err(|_| CoreError::MissingHashInput)?
            .to_le_bytes(),
    );
}
if mask_bit(&otx.base_input_masks, local_index * 2 + 1)? {
    let previous_output = input_view
        .previous_output()
        .map_err(|_| CoreError::MissingHashInput)?;
    update_cursor_with_error(
        &mut hasher,
        &previous_output.cursor,
        CoreError::MissingHashInput,
    )?;
}
let resolved_output = syscalls::resolved_input_output_cursor(tx_index)?;
writer::write_cursor_with_error(&mut hasher, &resolved_output, CoreError::MissingHashInput)?;
let resolved_data = syscalls::resolved_input_data_cursor(tx_index)?;
writer::write_len_prefixed_cursor_with_error(
    &mut hasher,
    &resolved_data,
    CoreError::MissingHashInput,
)?;
```

Apply the same concrete replacement for:

```rust
syscalls::raw_output_cursor(tx_index)?
syscalls::raw_output_data_cursor(tx_index)?
syscalls::raw_cell_dep_cursor(tx_index)?
syscalls::raw_header_dep_hash(tx_index)?
```

When a cursor is from Cobuild protocol data, keep `CoreError::MalformedCobuild`. When a cursor is from transaction hash payload data, use `CoreError::MissingHashInput`.

- [ ] **Step 5: Run core check to expose engine call-site failures**

Run:

```bash
cargo check -p cobuild-core --offline
```

Expected: FAIL in `engine.rs` because old calls still pass `source` parameters and old source module still exists.

- [ ] **Step 6: Commit hash conversion**

```bash
git add crates/cobuild-core/src/hash/mod.rs crates/cobuild-core/src/hash/writer.rs
git commit -m "refactor: hash through core syscall helpers"
```

---

### Task 4: Convert Engine To Concrete Syscall Preparation And Planning

**Files:**
- Modify: `crates/cobuild-core/src/engine.rs`
- Modify: `crates/cobuild-core/src/context.rs` if imports require it
- Test: `cargo check -p cobuild-core --offline`

- [ ] **Step 1: Update imports**

In `crates/cobuild-core/src/engine.rs`, replace:

```rust
source::{HashInputSource, TxCounts},
```

with:

```rust
syscalls::{self, TxCounts, TxCountsCache},
```

- [ ] **Step 2: Add counts cache to `PreparedCobuild`**

Change `PreparedCobuild` to:

```rust
pub struct PreparedCobuild {
    pub(crate) counts: TxCounts,
    pub(crate) counts_cache: TxCountsCache,
    pub(crate) script_hashes: ScriptHashIndex,
    witness_summaries: Vec<WitnessSummary>,
    pub(crate) layout_scan: OtxLayoutScan,
}
```

- [ ] **Step 3: Replace prepare with syscall concrete prepare**

Replace `CobuildEngine::prepare` with:

```rust
impl CobuildEngine {
    pub fn prepare_from_syscalls() -> Result<PreparedCobuild, CoreError> {
        let counts_cache = TxCountsCache::default();
        let counts = syscalls::counts(&counts_cache)?;
        let script_hashes = script_hashes_from_syscalls(counts)?;
        let mut witness_summaries = Vec::with_capacity(counts.witnesses);
        let mut layout_collector = OtxLayoutCollector::new();
        for index in 0..counts.witnesses {
            let witness = syscalls::witness_cursor(index)?;
            let witness = cursor_bytes_with_error(&witness, CoreError::MissingHashInput)?;
            witness_summaries.push(witness_summary(&witness)?);
            layout_collector.push_witness(&witness);
        }
        let layout_scan = layout_collector.finish(
            counts.inputs,
            counts.outputs,
            counts.cell_deps,
            counts.header_deps,
        );

        Ok(PreparedCobuild {
            counts,
            counts_cache,
            script_hashes,
            witness_summaries,
            layout_scan,
        })
    }
}
```

- [ ] **Step 4: Remove source parameters from planning APIs**

Change signatures:

```rust
pub fn plan_lock_validation(
    &self,
    lock_script_hash: [u8; 32],
) -> Result<LockValidationPlan, CoreError>

pub fn plan_type_validation(
    &self,
    type_script_hash: [u8; 32],
) -> Result<TypeValidationPlan, CoreError>

fn tx_level_lock_requirements(
    &self,
    lock_script_hash: [u8; 32],
) -> Result<Vec<SigningRequirement>, CoreError>
```

- [ ] **Step 5: Update engine hash calls**

Use the stored cache:

```rust
let mut required_signatures = self.tx_level_lock_requirements(lock_script_hash)?;
```

For OTX base hash:

```rust
let base_hash = otx_base_hash(&otx.witness, &otx.layout, &self.counts_cache)?;
```

For OTX append hash:

```rust
signing_message_hash: otx_append_hash(
    &otx.witness,
    &otx.layout,
    &self.counts_cache,
    base_hash,
)?,
```

For tx-level hash:

```rust
let signing_message_hash = match message {
    Some(message) => tx_with_message_hash(&message, &self.counts_cache)?,
    None => tx_without_message_hash(&self.counts_cache)?,
};
```

- [ ] **Step 6: Replace script hash index collection**

Replace `script_hashes_from_source` with:

```rust
fn script_hashes_from_syscalls(counts: TxCounts) -> Result<ScriptHashIndex, CoreError> {
    let mut input_locks = Vec::with_capacity(counts.inputs);
    let mut input_types = Vec::with_capacity(counts.inputs);
    for index in 0..counts.inputs {
        input_locks.push(syscalls::input_lock_hash(index)?);
        input_types.push(syscalls::input_type_hash(index)?);
    }

    let mut output_types = Vec::with_capacity(counts.outputs);
    for index in 0..counts.outputs {
        output_types.push(syscalls::output_type_hash(index)?);
    }

    Ok(ScriptHashIndex {
        input_locks,
        input_types,
        output_types,
    })
}
```

- [ ] **Step 7: Run core check and production source search**

Run:

```bash
cargo check -p cobuild-core --offline
rg -n "HashInputSource|source: &S|<S: HashInputSource" crates/cobuild-core/src/engine.rs crates/cobuild-core/src/hash/mod.rs
```

Expected: `cargo check` exits 0. `rg` exits 1 with no matches, proving production engine/hash no longer use `HashInputSource`.

- [ ] **Step 8: Commit engine conversion**

```bash
git add crates/cobuild-core/src/engine.rs
git commit -m "refactor: prepare cobuild from syscalls"
```

---

### Task 5: Remove Lock Chain Module And Update Entry

**Files:**
- Modify: `contracts/cobuild-otx-lock/src/entry.rs`
- Modify: `contracts/cobuild-otx-lock/src/lib.rs`
- Delete: `contracts/cobuild-otx-lock/src/chain.rs`
- Delete: `contracts/cobuild-otx-lock/src/chain/reader.rs`
- Test: `cargo check -p cobuild-otx-lock --offline`

- [ ] **Step 1: Update lock entry imports**

In `contracts/cobuild-otx-lock/src/entry.rs`, replace:

```rust
use crate::{
    args::AuthContext,
    chain::prepare_cobuild_from_syscalls,
    error::Error,
    verify::{LockVerifier, local::LocalVerifier},
};
```

with:

```rust
use cobuild_core::engine::CobuildEngine;

use crate::{
    args::AuthContext,
    error::Error,
    verify::{LockVerifier, local::LocalVerifier},
};
```

- [ ] **Step 2: Update lock planning call**

Replace:

```rust
let context = prepare_cobuild_from_syscalls()?;
let plan = context
    .prepared
    .plan_lock_validation(current_script_hash, &context.tx_reader)?;
```

with:

```rust
let plan = CobuildEngine::prepare_from_syscalls()?.plan_lock_validation(current_script_hash)?;
```

- [ ] **Step 3: Remove lock chain module declaration**

In `contracts/cobuild-otx-lock/src/lib.rs`, delete:

```rust
mod chain;
```

- [ ] **Step 4: Delete lock chain files**

Run:

```bash
rm contracts/cobuild-otx-lock/src/chain.rs
rm contracts/cobuild-otx-lock/src/chain/reader.rs
rmdir contracts/cobuild-otx-lock/src/chain
```

Expected: files and empty directory are removed. Do not leave a compatibility wrapper.

- [ ] **Step 5: Run lock check**

Run:

```bash
cargo check -p cobuild-otx-lock --offline
```

Expected: PASS. If it fails, fix remaining lock references to `chain` or old `PreparedCobuildContext` before continuing.

- [ ] **Step 6: Commit lock crate cleanup**

```bash
git add -A contracts/cobuild-otx-lock/src
git commit -m "refactor: remove lock syscall chain module"
```

---

### Task 6: Delete Source Module And Remaining Source Abstractions

**Files:**
- Delete: `crates/cobuild-core/src/source.rs`
- Modify: `crates/cobuild-core/src/lib.rs`
- Modify: `crates/cobuild-core/src/layout.rs`
- Modify: `crates/cobuild-core/tests/layout.rs`
- Modify: `crates/cobuild-core/tests/no_entity_dependency.rs`
- Delete: `crates/cobuild-core/tests/source.rs`
- Delete: `crates/cobuild-core/tests/engine.rs`
- Delete: `crates/cobuild-core/tests/hash.rs`
- Delete: `crates/cobuild-core/tests/type_plan.rs`
- Test: `cargo test -p cobuild-core --offline`

- [ ] **Step 1: Remove `WitnessCursorSource` trait before deleting `source.rs`**

In `crates/cobuild-core/src/layout.rs`, delete:

```rust
pub trait WitnessCursorSource {
    fn witness_count(&self) -> usize;
    fn witness_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
}

impl WitnessCursorSource for LayoutTx {
    fn witness_count(&self) -> usize {
        self.witnesses.len()
    }

    fn witness_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.witnesses
            .get(index)
            .map(|witness| ClassifiedCursor::hash_input(cursor_from_slice(witness)))
            .ok_or(CoreError::MissingHashInput)
    }
}
```

- [ ] **Step 2: Add explicit layout scan from witness slices**

Add this helper:

```rust
fn witness_bytes_from_layout_tx(tx: &LayoutTx, index: usize) -> Result<Vec<u8>, CoreError> {
    tx.witnesses
        .get(index)
        .cloned()
        .ok_or(CoreError::MissingHashInput)
}
```

Change `build_layout_from_witnesses` to accept `&LayoutTx` directly:

```rust
pub fn build_layout_from_witnesses(
    tx: &LayoutTx,
    input_count: usize,
    output_count: usize,
    cell_dep_count: usize,
    header_dep_count: usize,
) -> Result<BuiltLayout, CoreError> {
    match scan_layout_from_witnesses(
        tx,
        input_count,
        output_count,
        cell_dep_count,
        header_dep_count,
    ) {
        OtxLayoutScan::None => Ok(empty_layout()),
        OtxLayoutScan::Complete(layout) => Ok(layout),
        OtxLayoutScan::Invalid { error, .. } => Err(error),
    }
}
```

Change `scan_layout_from_witnesses` to:

```rust
pub(crate) fn scan_layout_from_witnesses(
    tx: &LayoutTx,
    input_count: usize,
    output_count: usize,
    cell_dep_count: usize,
    header_dep_count: usize,
) -> OtxLayoutScan {
    let mut collector = OtxLayoutCollector::new();
    for index in 0..tx.witnesses.len() {
        match witness_bytes_from_layout_tx(tx, index) {
            Ok(witness) => collector.push_witness(&witness),
            Err(error) => return OtxLayoutScan::Invalid { anchor: None, error },
        }
    }
    collector.finish(input_count, output_count, cell_dep_count, header_dep_count)
}
```

- [ ] **Step 3: Update layout tests**

In `crates/cobuild-core/tests/layout.rs`, remove `WitnessCursorSource` and `ClassifiedCursor` imports. Delete `TestWitnessSource` and its impl. Any test using `TestWitnessSource` should construct:

```rust
let tx = LayoutTx {
    witnesses,
    ..LayoutTx::default()
};
```

and call:

```rust
build_layout_from_witnesses(&tx, input_count, output_count, cell_dep_count, header_dep_count)
```

- [ ] **Step 4: Remove source module export**

In `crates/cobuild-core/src/lib.rs`, delete:

```rust
pub mod source;
```

- [ ] **Step 5: Delete source module and source-driven tests**

Run:

```bash
git rm crates/cobuild-core/src/source.rs
git rm crates/cobuild-core/tests/source.rs
git rm crates/cobuild-core/tests/engine.rs
git rm crates/cobuild-core/tests/hash.rs
git rm crates/cobuild-core/tests/type_plan.rs
```

These tests are built around `InMemorySource`, `TransactionSource`, `HashInputSource`, or generic source-driven engine/hash APIs. Do not recreate an in-memory production source just to keep them. End-to-end lock behavior remains covered by `tests/tests/cobuild_otx_lock.rs`.

- [ ] **Step 6: Update no-entity/no-unsafe guard for ckb-std**

In `crates/cobuild-core/tests/no_entity_dependency.rs`, replace `core_source_does_not_import_ckb_std` with:

```rust
#[test]
fn only_syscalls_module_imports_ckb_std() {
    for path in core_source_paths() {
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        let is_syscalls = path.file_name().is_some_and(|name| name == "syscalls.rs");
        if is_syscalls {
            assert!(
                text.contains("ckb_std"),
                "syscalls.rs must own ckb_std access"
            );
        } else {
            assert!(
                !text.contains("ckb_std"),
                "{} must not import ckb_std directly",
                path.display()
            );
        }
    }
}
```

Keep `core_source_does_not_import_entity_module`, `view_does_not_publicly_expose_generated_inner_reader`, `core_source_contains_no_unsafe`, and `engine_prepare_does_not_cache_all_witness_byte_vectors`.

- [ ] **Step 7: Run layout tests**

Run:

```bash
cargo test -p cobuild-core --offline --test layout
```

Expected: PASS.

- [ ] **Step 8: Run core tests**

Run:

```bash
cargo test -p cobuild-core --offline
```

Expected: PASS for remaining pure core tests: layout, no old query API, no entity dependency, plan, view, and witness tests.

- [ ] **Step 9: Commit source abstraction deletion**

```bash
git add -A crates/cobuild-core/src crates/cobuild-core/tests
git commit -m "refactor: delete cobuild source abstraction"
```

---

### Task 7: Update Contract Integration And Architecture Tests

**Files:**
- Modify: `tests/tests/contract_template_layout.rs`
- Modify: `docs/CobuildAgentDevelopGuide.md`
- Test: `cargo test -p tests --offline --test contract_template_layout`
- Test: `MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture`

- [ ] **Step 1: Finalize architecture guard strings**

Ensure `tests/tests/contract_template_layout.rs` rejects all deleted names:

```rust
for forbidden in [
    "TransactionSource",
    "HashInputSource",
    "InMemorySource",
    "ClassifiedCursor",
    "CursorReadContext",
    "PreparedCobuildContext",
    "SyscallTxReader",
    "mod chain",
    "source.rs",
] {
    assert!(
        !all_relevant_text.contains(forbidden),
        "deleted abstraction must not remain: {forbidden}"
    );
}
```

Build `all_relevant_text` from `core/src/lib.rs`, `core/src/engine.rs`, `core/src/hash/mod.rs`, `core/src/hash/writer.rs`, `lock/src/lib.rs`, and `lock/src/entry.rs`. Keep targeted file-existence checks for removed files.

- [ ] **Step 2: Update guide**

In `docs/CobuildAgentDevelopGuide.md`, replace references to source traits / `SyscallTxReader` in lock crate with:

```markdown
- `cobuild-core` owns syscall-backed transaction reading through its internal `syscalls` module;
- `cobuild-otx-lock` does not own transaction readers or source traits;
- production Cobuild preparation uses `CobuildEngine::prepare_from_syscalls()`;
- source-trait compatibility layers are intentionally removed.
```

- [ ] **Step 3: Run architecture guard**

Run:

```bash
cargo test -p tests --offline --test contract_template_layout
```

Expected: PASS.

- [ ] **Step 4: Run contract integration test**

Run:

```bash
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: PASS, 8 tests.

- [ ] **Step 5: Commit tests and docs**

```bash
git add tests/tests/contract_template_layout.rs docs/CobuildAgentDevelopGuide.md
git commit -m "test: guard syscall concrete cobuild core"
```

---

### Task 8: Final Cleanup And Verification

**Files:**
- Inspect all production and test files
- Test: full workspace

- [ ] **Step 1: Search for deleted abstractions**

Run:

```bash
rg -n "TransactionSource|HashInputSource|InMemorySource|ClassifiedCursor|CursorReadContext|PreparedCobuildContext|SyscallTxReader|WitnessCursorSource|source.rs|mod source|mod chain|prepare_cobuild_from_syscalls|context\\.tx_reader" crates contracts tests docs
```

Expected: No matches except historical docs under `docs/superpowers/specs` or `docs/superpowers/plans`. If matches remain in production, tests, or `docs/CobuildAgentDevelopGuide.md`, delete or rewrite them.

- [ ] **Step 2: Run format**

Run:

```bash
cargo fmt
```

Expected: exit 0.

- [ ] **Step 3: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets --offline
```

Expected: exit 0.

- [ ] **Step 4: Run workspace tests**

Run:

```bash
cargo test --workspace --offline
```

Expected: exit 0.

- [ ] **Step 5: Build debug contract**

Run:

```bash
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
```

Expected: exit 0 and copied debug binary.

- [ ] **Step 6: Run lock integration tests against debug build**

Run:

```bash
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: exit 0, 8 tests pass.

- [ ] **Step 7: Check whitespace and worktree**

Run:

```bash
git diff --check
git status --short --branch
```

Expected: `git diff --check` exits 0. Status shows only intentional modified/deleted files before the final commit.

- [ ] **Step 8: Commit final cleanup**

```bash
git add crates contracts tests docs
git commit -m "refactor: remove cobuild source abstraction"
```

Expected: commit created. If all previous tasks already committed every change and status is clean, skip this final commit and record that no final cleanup commit was needed.
