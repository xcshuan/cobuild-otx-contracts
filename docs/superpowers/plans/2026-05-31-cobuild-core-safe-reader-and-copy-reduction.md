# Cobuild Core Safe Reader And Copy Reduction Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove unsafe lifetime erasure from `cobuild-core` view parsing and reduce the narrow OTX base hash cursor-to-`Vec` copies without changing protocol behavior.

**Architecture:** Replace the borrowed slice lazy-reader adapter with an owned byte reader, so generated lazy-reader cursors can hold `Box<dyn Read>` safely. Add a crate-private cursor streaming helper and use it only where the Core hash preimage requires raw Molecule object bytes, leaving owned public task/hash data structures unchanged.

**Tech Stack:** Rust `no_std` with `alloc`, generated `cobuild_types::lazy_reader`, `blake2b-ref`, workspace Cargo tests, `xtask` codegen check, CKB contract debug build.

---

## Files

- Modify `crates/cobuild-core/src/view.rs`
  - Replace `SliceReader<'a>` and `erase_reader_lifetime` with `OwnedReader`.
  - Remove `WitnessLayoutView`'s artificial input lifetime.
  - Add `update_cursor` for bounded cursor streaming into a `Blake2b` hasher.
- Modify `crates/cobuild-core/src/witness.rs`
  - Remove the lifetime from `ParsedWitness` now that `WitnessLayoutView` owns its reader bytes.
- Modify `crates/cobuild-core/src/hash.rs`
  - Use `update_cursor` for OTX base hash `previous_output`, output `lock`, and output `type` bytes.
- Modify `crates/cobuild-core/tests/view.rs`
  - Update reader boundary tests to use `OwnedReader`.
- Modify `crates/cobuild-core/tests/no_entity_dependency.rs`
  - Add a static regression that rejects `unsafe` in `src/view.rs`.
- Modify `crates/cobuild-core/tests/hash.rs`
  - Add an independent OTX base hash regression covering `previous_output`, output `lock`, and output `type`.

## Task 1: Lock The Safety Boundary With Tests

**Files:**
- Modify: `crates/cobuild-core/tests/no_entity_dependency.rs`
- Modify: `crates/cobuild-core/tests/view.rs`

- [ ] **Step 1: Add the static unsafe regression**

Add this test to the end of `crates/cobuild-core/tests/no_entity_dependency.rs`:

```rust
#[test]
fn view_source_contains_no_unsafe() {
    let path = manifest_path("src/view.rs");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    assert!(
        !text.contains("unsafe"),
        "view.rs must not contain unsafe code"
    );
}
```

- [ ] **Step 2: Run the new static regression and verify it fails**

Run:

```bash
cargo test -p cobuild-core --offline --test no_entity_dependency view_source_contains_no_unsafe
```

Expected: FAIL, because `crates/cobuild-core/src/view.rs` still contains `unsafe` and `erase_reader_lifetime`.

- [ ] **Step 3: Prepare the view test for the new owned reader name**

Replace `crates/cobuild-core/tests/view.rs` with:

```rust
use cobuild_core::view::OwnedReader;
use cobuild_core::view::WitnessLayoutView;
use cobuild_types::lazy_reader::support::{Error as MoleculeError, Read};

#[test]
fn empty_witness_is_not_a_cobuild_layout() {
    assert!(WitnessLayoutView::from_slice(&[]).is_err());
}

#[test]
fn owned_reader_reports_out_of_bound_offsets() {
    let reader = OwnedReader::new(&[]);
    let mut buf = [0u8; 1];
    assert!(matches!(
        reader.read(&mut buf, 1),
        Err(MoleculeError::OutOfBound(1, 0))
    ));
}

#[test]
fn parsed_view_survives_source_slice_drop() {
    let view = {
        let witness = sighash_all_only_witness_bytes(&[0x11, 0x22, 0x33]);
        WitnessLayoutView::from_slice(&witness).unwrap()
    };

    assert_eq!(
        view.sighash_all_only_seal().unwrap(),
        Some(vec![0x11, 0x22, 0x33])
    );
}

fn sighash_all_only_witness_bytes(seal: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&4_278_190_082u32.to_le_bytes());
    bytes.extend_from_slice(&table_bytes(&[molecule_bytes(seal)]));
    bytes
}

fn molecule_bytes(raw: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + raw.len());
    bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    bytes.extend_from_slice(raw);
    bytes
}

fn table_bytes(fields: &[Vec<u8>]) -> Vec<u8> {
    let header_size = 4 + fields.len() * 4;
    let total_size = header_size + fields.iter().map(Vec::len).sum::<usize>();
    let mut bytes = Vec::with_capacity(total_size);
    bytes.extend_from_slice(&(total_size as u32).to_le_bytes());

    let mut offset = header_size;
    for field in fields {
        bytes.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += field.len();
    }
    for field in fields {
        bytes.extend_from_slice(field);
    }

    bytes
}
```

- [ ] **Step 4: Run the view test and verify the compile failure**

Run:

```bash
cargo test -p cobuild-core --offline --test view parsed_view_survives_source_slice_drop
```

Expected: FAIL. Before implementation this can fail because `OwnedReader` does not exist and because the existing `WitnessLayoutView<'a>` cannot outlive the source `witness` bytes. This confirms the test is ahead of the implementation.

## Task 2: Replace The Unsafe Slice Reader

**Files:**
- Modify: `crates/cobuild-core/src/view.rs`
- Modify: `crates/cobuild-core/src/witness.rs`
- Test: `crates/cobuild-core/tests/view.rs`
- Test: `crates/cobuild-core/tests/no_entity_dependency.rs`

- [ ] **Step 1: Replace the borrowed reader with an owned reader**

In `crates/cobuild-core/src/view.rs`, replace the top imports and reader type with this shape:

```rust
use alloc::{boxed::Box, vec, vec::Vec};
use core::{cmp::min, convert::TryInto};

use cobuild_types::lazy_reader::{
    core::{Message, Otx, OtxStart, SealPair},
    support::{Cursor, Error as MoleculeError, Read},
    witness::WitnessLayout,
};

use crate::error::CoreError;

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
```

- [ ] **Step 2: Remove the artificial lifetime from `WitnessLayoutView`**

In `crates/cobuild-core/src/view.rs`, replace:

```rust
pub struct WitnessLayoutView<'a> {
    #[allow(dead_code)]
    pub(crate) inner: WitnessLayout,
    _data: PhantomData<&'a [u8]>,
}
```

with:

```rust
pub struct WitnessLayoutView {
    #[allow(dead_code)]
    pub(crate) inner: WitnessLayout,
}
```

Then replace the impl header and constructor:

```rust
impl WitnessLayoutView {
    pub fn from_slice(data: &[u8]) -> Result<Self, CoreError> {
        let cursor = cursor_from_slice(data);
        let inner = WitnessLayout::try_from(cursor).map_err(|_| CoreError::MalformedCobuild)?;

        inner.verify(false).map_err(|_| CoreError::InvalidLayout)?;

        Ok(Self { inner })
    }
```

Leave the existing methods inside the impl unchanged.

- [ ] **Step 3: Make `cursor_from_slice` safe by construction**

At the bottom of `crates/cobuild-core/src/view.rs`, replace `cursor_from_slice` and delete `erase_reader_lifetime` entirely:

```rust
pub(crate) fn cursor_from_slice(data: &[u8]) -> Cursor {
    let reader: Box<dyn Read> = Box::new(OwnedReader::new(data));
    Cursor::new(data.len(), reader)
}
```

- [ ] **Step 4: Remove the witness lifetime**

Replace all contents of `crates/cobuild-core/src/witness.rs` with:

```rust
use crate::{error::CoreError, view::WitnessLayoutView};

pub enum ParsedWitness {
    None,
    Cobuild(WitnessLayoutView),
}

pub fn parse_witness(data: &[u8]) -> Result<ParsedWitness, CoreError> {
    match WitnessLayoutView::from_slice(data) {
        Ok(view) => Ok(ParsedWitness::Cobuild(view)),
        Err(CoreError::MalformedCobuild | CoreError::InvalidLayout) => Ok(ParsedWitness::None),
        Err(err) => Err(err),
    }
}
```

- [ ] **Step 5: Run focused reader and static tests**

Run:

```bash
cargo test -p cobuild-core --offline --test view
cargo test -p cobuild-core --offline --test no_entity_dependency
```

Expected: both commands PASS.

- [ ] **Step 6: Confirm no unsafe remains in `view.rs`**

Run:

```bash
rg -n "unsafe" crates/cobuild-core/src/view.rs
```

Expected: no matches. `rg` may exit with status 1 when there are no matches; that is acceptable.

- [ ] **Step 7: Commit the safe reader change**

Run:

```bash
git add crates/cobuild-core/src/view.rs crates/cobuild-core/src/witness.rs crates/cobuild-core/tests/view.rs crates/cobuild-core/tests/no_entity_dependency.rs
git commit -m "fix: remove unsafe cobuild view reader"
```

## Task 3: Add The OTX Base Hash Streaming Regression

**Files:**
- Modify: `crates/cobuild-core/tests/hash.rs`

- [ ] **Step 1: Add a focused independent hash regression**

First update the imports at the top of `crates/cobuild-core/tests/hash.rs`:

```rust
use cobuild_core::{
    error::CoreError,
    hash::{
        checked_len_prefix, otx_base_hash, tx_without_message_hash, RawTxParts,
        ResolvedInputHashPart, TxHashParts,
    },
    layout::{OtxLayout, Range},
    view::{OwnedReader, OtxData},
};
use cobuild_types::lazy_reader::{
    blockchain::{CellInput, CellOutput},
    support::Cursor,
};
```

Add this test after `otx_base_hash_includes_local_indices_for_base_deps_and_headers` in `crates/cobuild-core/tests/hash.rs`:

```rust
#[test]
fn otx_base_hash_streamed_cursor_fields_match_independent_preimage() {
    let previous_output = out_point_bytes([0x21; 32], 7);
    let input = cell_input_bytes(0x0102_0304_0506_0708, &previous_output);
    let lock = script_bytes([0x31; 32], 1, &[0x41, 0x42, 0x43]);
    let type_script = script_bytes([0x51; 32], 0, &[0x61, 0x62]);
    let output = cell_output_bytes(0x1112_1314_1516_1718, &lock, &type_script);
    CellInput::from(Cursor::new(input.len(), Box::new(OwnedReader::new(&input))))
        .verify(false)
        .unwrap();
    CellOutput::from(Cursor::new(
        output.len(),
        Box::new(OwnedReader::new(&output)),
    ))
    .verify(false)
    .unwrap();

    let resolved_output = vec![0x71, 0x72, 0x73];
    let resolved_data = vec![0x81, 0x82];

    let otx = OtxData {
        message: vec![0x91, 0x92],
        append_permissions: 0x03,
        base_input_cells: 1,
        base_input_masks: vec![0b0000_0011],
        base_output_cells: 1,
        base_output_masks: vec![0b0000_0110],
        base_cell_deps: 0,
        base_cell_dep_masks: Vec::new(),
        base_header_deps: 0,
        base_header_dep_masks: Vec::new(),
        append_input_cells: 0,
        append_output_cells: 0,
        append_cell_deps: 0,
        append_header_deps: 0,
        seals: Vec::new(),
    };
    let layout = OtxLayout {
        witness_index: 0,
        base_inputs: range(0, 1),
        append_inputs: range(1, 0),
        base_outputs: range(0, 1),
        append_outputs: range(1, 0),
        base_cell_deps: range(0, 0),
        append_cell_deps: range(0, 0),
        base_header_deps: range(0, 0),
        append_header_deps: range(0, 0),
    };
    let raw = RawTxParts {
        inputs: vec![input],
        outputs: vec![output],
        outputs_data: vec![vec![0xa1, 0xa2]],
        ..RawTxParts::default()
    };
    let resolved_inputs = vec![ResolvedInputHashPart {
        output: resolved_output.clone(),
        data: resolved_data.clone(),
    }];

    let actual = otx_base_hash(&otx, &layout, &raw, &resolved_inputs).unwrap();

    let mut expected = [0u8; 32];
    let mut hasher = blake2b_ref::Blake2bBuilder::new(32)
        .personal(b"ckbcb_otb_core1\0")
        .build();
    hasher.update(&otx.message);
    hasher.update(&[otx.append_permissions]);
    hasher.update(&1u32.to_le_bytes());
    update_len_prefixed_for_test(&mut hasher, &[0b0000_0011]);
    hasher.update(&0u32.to_le_bytes());
    hasher.update(&0x0102_0304_0506_0708u64.to_le_bytes());
    hasher.update(&previous_output);
    hasher.update(&resolved_output);
    update_len_prefixed_for_test(&mut hasher, &resolved_data);
    hasher.update(&1u32.to_le_bytes());
    update_len_prefixed_for_test(&mut hasher, &[0b0000_0110]);
    hasher.update(&0u32.to_le_bytes());
    hasher.update(&lock);
    hasher.update(&type_script);
    hasher.update(&0u32.to_le_bytes());
    update_len_prefixed_for_test(&mut hasher, &[]);
    hasher.update(&0u32.to_le_bytes());
    update_len_prefixed_for_test(&mut hasher, &[]);
    hasher.finalize(&mut expected);

    assert_eq!(actual, expected);
}
```

- [ ] **Step 2: Add molecule encoding helpers for the test**

Add these helper functions near the bottom of `crates/cobuild-core/tests/hash.rs`, before or after `update_len_prefixed_for_test`:

```rust
fn out_point_bytes(tx_hash: [u8; 32], index: u32) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(36);
    bytes.extend_from_slice(&tx_hash);
    bytes.extend_from_slice(&index.to_le_bytes());
    bytes
}

fn cell_input_bytes(since: u64, previous_output: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(44);
    bytes.extend_from_slice(&since.to_le_bytes());
    bytes.extend_from_slice(previous_output);
    bytes
}

fn script_bytes(code_hash: [u8; 32], hash_type: u8, args: &[u8]) -> Vec<u8> {
    table_bytes(&[
        code_hash.to_vec(),
        vec![hash_type],
        molecule_bytes(args),
    ])
}

fn cell_output_bytes(capacity: u64, lock: &[u8], type_script: &[u8]) -> Vec<u8> {
    table_bytes(&[
        capacity.to_le_bytes().to_vec(),
        lock.to_vec(),
        type_script.to_vec(),
    ])
}

fn molecule_bytes(raw: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(4 + raw.len());
    bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    bytes.extend_from_slice(raw);
    bytes
}

fn table_bytes(fields: &[Vec<u8>]) -> Vec<u8> {
    let header_size = 4 + fields.len() * 4;
    let total_size = header_size + fields.iter().map(Vec::len).sum::<usize>();
    let mut bytes = Vec::with_capacity(total_size);
    bytes.extend_from_slice(&(total_size as u32).to_le_bytes());

    let mut offset = header_size;
    for field in fields {
        bytes.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += field.len();
    }
    for field in fields {
        bytes.extend_from_slice(field);
    }

    bytes
}
```

- [ ] **Step 3: Run the new hash regression**

Run:

```bash
cargo test -p cobuild-core --offline --test hash otx_base_hash_streamed_cursor_fields_match_independent_preimage
```

Expected: PASS after Task 2 and before Task 4, then PASS again after Task 4. This regression protects the bytes that will move from `cursor_bytes` to `update_cursor`, and the generated-reader `verify(false)` calls guard the hand-written Molecule helpers.

- [ ] **Step 4: Commit the regression**

Run:

```bash
git add crates/cobuild-core/tests/hash.rs
git commit -m "test: cover streamed otx base hash fields"
```

## Task 4: Stream Raw Cursor Hash Fields

**Files:**
- Modify: `crates/cobuild-core/src/view.rs`
- Modify: `crates/cobuild-core/src/hash.rs`
- Test: `crates/cobuild-core/tests/hash.rs`

- [ ] **Step 1: Add `update_cursor` to `view.rs`**

Add this helper immediately after `cursor_bytes` in `crates/cobuild-core/src/view.rs`:

```rust
pub(crate) fn update_cursor(
    hasher: &mut blake2b_ref::Blake2b,
    cursor: &Cursor,
) -> Result<(), CoreError> {
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
```

This uses cloned cursor windows instead of modifying generated lazy-reader code.

- [ ] **Step 2: Import `update_cursor` in `hash.rs`**

In `crates/cobuild-core/src/hash.rs`, change:

```rust
view::{cursor_bytes, cursor_from_slice, OtxData},
```

to:

```rust
view::{cursor_from_slice, update_cursor, OtxData},
```

- [ ] **Step 3: Replace the three OTX base hash temporary copies**

In `otx_base_hash`, replace:

```rust
hasher.update(&cursor_bytes(&previous_output.cursor)?);
```

with:

```rust
update_cursor(&mut hasher, &previous_output.cursor)?;
```

Replace:

```rust
hasher.update(&cursor_bytes(&lock.cursor)?);
```

with:

```rust
update_cursor(&mut hasher, &lock.cursor)?;
```

Replace:

```rust
hasher.update(&cursor_bytes(&type_cursor)?);
```

with:

```rust
update_cursor(&mut hasher, &type_cursor)?;
```

- [ ] **Step 4: Run focused hash tests**

Run:

```bash
cargo test -p cobuild-core --offline --test hash
```

Expected: PASS.

- [ ] **Step 5: Confirm `cursor_bytes` still exists for owned task/view outputs**

Run:

```bash
rg -n "cursor_bytes|update_cursor" crates/cobuild-core/src
```

Expected: `cursor_bytes` remains used in `view.rs` for owned message/mask/seal extraction; `update_cursor` is used in `hash.rs` for `previous_output`, `lock`, and `type_cursor`.

- [ ] **Step 6: Commit the streaming helper**

Run:

```bash
git add crates/cobuild-core/src/view.rs crates/cobuild-core/src/hash.rs
git commit -m "fix: stream otx base hash cursor fields"
```

## Task 5: Full Phase 2A Verification

**Files:**
- Verify workspace and contract build outputs only.

- [ ] **Step 1: Run generated type drift check**

Run:

```bash
cargo run -p xtask --offline -- codegen cobuild-types --check
```

Expected: PASS with no generated file diffs.

- [ ] **Step 2: Run all workspace tests**

Run:

```bash
cargo test --workspace --offline
```

Expected: PASS.

- [ ] **Step 3: Build the OTX lock contract**

Run:

```bash
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
```

Expected: PASS.

- [ ] **Step 4: Run the contract integration test**

Run:

```bash
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Re-run source boundary checks**

Run:

```bash
rg -n "unsafe" crates/cobuild-core/src/view.rs
rg -n "transmute|erase_reader_lifetime|Box<dyn Read \+" crates/cobuild-core/src
rg -n "cobuild_types\s*::\s*entity|cobuild_types\s*::\s*\{[^}]*entity" crates/cobuild-core/src contracts/cobuild-otx-lock/src
rg -n "ckb_auth|ckb-auth" crates contracts
```

Expected:

- The `unsafe` search prints no matches.
- The lifetime-erasure search prints no matches.
- The `cobuild_types::entity` search prints no production matches.
- The `ckb_auth|ckb-auth` search prints no matches.
- `rg` exit status 1 is acceptable for searches with no matches.

- [ ] **Step 6: Inspect git status**

Run:

```bash
git status --short
```

Expected: clean after all task commits. If verification commands created build artifacts, leave ignored artifacts uncommitted.
