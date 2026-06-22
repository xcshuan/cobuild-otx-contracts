# OTX Mask Default-Value Signing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]` / `- [x]`) syntax for tracking.

**Goal:** Change OTX base signing hashes so uncovered mask-controlled fields write canonical default values instead of disappearing from the preimage.

**Architecture:** Keep the OTX witness schema and mask layout unchanged. Update the on-chain hash builder and the test signing oracle in lockstep, then add focused hash-oracle regression tests before running the contract fixture suite.

**Tech Stack:** Rust 2021/2024, `cobuild-core`, `cobuild-types`, CKB packed Molecule types, `ckb-testtool`, `cargo test`, existing `make build` flows.

**Status:** Completed on 2026-06-22.

**Implementation commit:** `626c1cb feat: default uncovered otx mask fields`

**Final golden hash:** `5ed573c86ca864c867e6523c68578c89762659c08f33abdf50a89c2fd7760120`

**Execution notes:**
- All planned behavior was implemented in the contract hash and the test signing oracle.
- The contract-side default writers intentionally avoided importing `ckb_std::ckb_types` outside syscall-owned code, preserving the existing guard test. Fixed Molecule default bytes are used in `cobuild-core`; the test oracle uses packed builders to confirm the same encodings.
- The implementation also updated the limit-order reuse-payment fixture to accept either lock input as the first detected duplicate-payment failure.
- Untracked forum/spec draft documents were left outside the implementation commit.

**Verification completed:**
- `cargo fmt --check`
- `git diff --check`
- `MODE=debug cargo test --offline signing_hash -- --nocapture`
- `MODE=debug cargo test --offline`
- `MODE=release cargo test --offline`
- Final subagent code review: no findings.

---

## File Structure

- Modify `crates/cobuild-core/src/hash/mod.rs`.
  This file owns the contract-side OTX base signing hash. It will add small
  helper writers for canonical defaults and use them in base input/output/cell
  dep/header dep hash construction.
- Modify `tests/src/framework/signing/otx.rs`.
  This file mirrors the contract hash for test signing. It must use the same
  default-value rules so generated seals match contract verification.
- Modify `tests/src/tests/signing_hash.rs`.
  This file will gain focused regression tests for uncovered mask-controlled
  fields and existing coverage guarantees.

No schema files, contract entry files, or application contracts should change.

## Task 1: Add Failing Signing-Hash Tests

**Files:**
- Modify: `tests/src/tests/signing_hash.rs`
- Modify: `tests/src/tests.rs`

- [x] **Step 1: Add a default-slot golden regression test**

First update the `framework::cobuild` import in `tests/src/tests.rs` so the
child `signing_hash` module can use all mask DSL helpers:

```rust
cobuild::{
    BaseInputMaskField, BaseOutputMaskField, CobuildMessageBuilder, OtxStartSpec,
    RawOtxBuilder, base_cell_dep_item_mask, base_header_dep_item_mask, base_input_mask,
    base_output_mask, seal_pair,
},
```

Add this test near the other OTX base signing hash tests. The exact golden
value is filled after implementing the default-slot writer once and recording
the produced hash; before implementation, the hash must differ because the old
oracle skips uncovered slots instead of writing canonical defaults.

```rust
#[test]
fn signing_hash_oracle_otx_base_all_uncovered_fields_matches_default_slot_golden() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        base_cell_deps: vec![signing_cell_dep(3)],
        base_header_deps: vec![[4; 32]],
        base_input_masks: Some(base_input_mask(1).bytes()),
        base_output_masks: Some(base_output_mask(1).bytes()),
        base_cell_dep_masks: Some(base_cell_dep_item_mask(1).bytes()),
        base_header_dep_masks: Some(base_header_dep_item_mask(1).bytes()),
        ..Default::default()
    });
    let built = shape.build();

    let actual = TestSigningHashOracle.otx_base(&built, otx);
    let expected = [
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    assert_eq!(actual, expected);
}
```

The all-zero `expected` value is intentionally wrong. The first test run should
fail and print the actual pre-implementation hash. After Task 3 is complete,
replace the all-zero array with the new default-slot hash value printed by the
failing assertion. This pins the new preimage shape and prevents accidental
reversion to skip semantics.

- [x] **Step 2: Add uncovered base input flexibility tests**

Append these tests after
`signing_hash_oracle_otx_base_changes_when_covered_previous_output_changes`:

```rust
#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_since_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_input_masks: Some(
            base_input_mask(1)
                .cover_field(0, BaseInputMaskField::PreviousOutput)
                .bytes(),
        ),
        ..Default::default()
    });
    let input = shape.otx_base_input(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceInput {
        input,
        replacement: signing_resolved_input_with_since(1, 99, vec![0xaa]),
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}

#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_previous_output_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_input_masks: Some(
            base_input_mask(1)
                .cover_field(0, BaseInputMaskField::Since)
                .bytes(),
        ),
        ..Default::default()
    });
    let input = shape.otx_base_input(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceInput {
        input,
        replacement: signing_resolved_input(9, vec![0xaa]),
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}
```

Add this helper below `signing_resolved_input` in `tests/src/tests.rs`:

```rust
fn signing_resolved_input_with_since(
    tag: u8,
    since: u64,
    data: impl Into<Bytes>,
) -> ResolvedInputFacts {
    let mut facts = signing_resolved_input(tag, data);
    facts.input = facts.input.as_builder().since(since.pack()).build();
    facts
}
```

Implementation note: the final helper used `.since(since)` because the builder
API accepts the raw `u64` value in this codebase.

- [x] **Step 3: Add uncovered base output flexibility tests**

Append these tests in `tests/src/tests/signing_hash.rs`:

```rust
#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_output_capacity_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_outputs: vec![signing_output(2, vec![0xbb])],
        base_output_masks: Some(
            base_output_mask(1)
                .cover_field(0, BaseOutputMaskField::Lock)
                .cover_field(0, BaseOutputMaskField::Type)
                .cover_field(0, BaseOutputMaskField::Data)
                .bytes(),
        ),
        ..Default::default()
    });
    let output = shape.otx_base_output(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
        output,
        replacement: signing_output(9, vec![0xbb]),
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}

#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_output_lock_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_outputs: vec![signing_output(2, vec![0xbb])],
        base_output_masks: Some(
            base_output_mask(1)
                .cover_field(0, BaseOutputMaskField::Capacity)
                .cover_field(0, BaseOutputMaskField::Type)
                .cover_field(0, BaseOutputMaskField::Data)
                .bytes(),
        ),
        ..Default::default()
    });
    let output = shape.otx_base_output(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
        output,
        replacement: signing_output_with_lock_tag(2, 9, vec![0xbb]),
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}

#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_output_type_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_outputs: vec![signing_output(2, vec![0xbb])],
        base_output_masks: Some(
            base_output_mask(1)
                .cover_field(0, BaseOutputMaskField::Capacity)
                .cover_field(0, BaseOutputMaskField::Lock)
                .cover_field(0, BaseOutputMaskField::Data)
                .bytes(),
        ),
        ..Default::default()
    });
    let output = shape.otx_base_output(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
        output,
        replacement: signing_typed_output(2, 9, vec![0xbb]),
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}

#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_output_data_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_outputs: vec![signing_output(2, vec![0xbb])],
        base_output_masks: Some(
            base_output_mask(1)
                .cover_field(0, BaseOutputMaskField::Capacity)
                .cover_field(0, BaseOutputMaskField::Lock)
                .cover_field(0, BaseOutputMaskField::Type)
                .bytes(),
        ),
        ..Default::default()
    });
    let output = shape.otx_base_output(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
        output,
        replacement: signing_output(2, vec![0xcc]),
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}
```

Add these helpers below `signing_output` in `tests/src/tests.rs`:

```rust
fn signing_output_with_lock_tag(
    capacity_tag: u8,
    lock_tag: u8,
    data: impl Into<Bytes>,
) -> TestCellOutput {
    TestCellOutput::new(
        CellOutput::new_builder()
            .capacity(2_000 + u64::from(capacity_tag))
            .lock(signing_test_script(lock_tag))
            .build(),
        data,
    )
}

fn signing_typed_output(
    capacity_tag: u8,
    type_tag: u8,
    data: impl Into<Bytes>,
) -> TestCellOutput {
    TestCellOutput::new(
        CellOutput::new_builder()
            .capacity(2_000 + u64::from(capacity_tag))
            .lock(signing_test_script(capacity_tag))
            .type_(Some(signing_test_script(type_tag)).pack())
            .build(),
        data,
    )
}
```

- [x] **Step 4: Add uncovered base cell/header dep tests**

Append these tests:

```rust
#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_cell_dep_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_cell_deps: vec![signing_cell_dep(3)],
        base_cell_dep_masks: Some(base_cell_dep_item_mask(1).bytes()),
        ..Default::default()
    });
    let cell_dep = shape.otx_base_cell_dep(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceCellDep {
        cell_dep,
        replacement: signing_cell_dep(9),
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}

#[test]
fn signing_hash_oracle_otx_base_ignores_uncovered_header_dep_change() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_header_deps: vec![[4; 32]],
        base_header_dep_masks: Some(base_header_dep_item_mask(1).bytes()),
        ..Default::default()
    });
    let header_dep = shape.otx_base_header_dep(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceHeaderDep {
        header_dep,
        replacement: [9; 32],
    });

    assert_eq!(before, TestSigningHashOracle.otx_base(&built, otx));
}
```

- [x] **Step 5: Run the focused tests and verify the golden test fails**

Run:

```bash
MODE=debug cargo test --offline signing_hash_oracle_otx_base_all_uncovered_fields_matches_default_slot_golden -- --nocapture
```

Expected before implementation: FAIL with an assertion showing the actual hash.
Do not replace the expected array yet; first implement Tasks 2 and 3, then run
this same test again and pin the new default-slot hash.

## Task 2: Implement Default Writers in `cobuild-core`

**Files:**
- Modify: `crates/cobuild-core/src/hash/mod.rs`

- [x] **Step 1: Add packed imports for canonical default encodings**

Change the imports near the top of `crates/cobuild-core/src/hash/mod.rs`:

```rust
use ckb_std::ckb_types::{packed, prelude::Entity};
```

Implementation note: the final patch did not add this import. `cobuild-core`
kept `ckb-std` usage isolated to syscall code and wrote the canonical default
bytes directly where needed.

Keep the existing lazy-reader imports:

```rust
use cobuild_types::lazy_reader::{
    blockchain::{CellInput, CellOutput},
    support::Cursor,
};
```

- [x] **Step 2: Add canonical default writer helpers**

Add these helpers after `finalize_hash`:

```rust
fn write_default_out_point(hasher: &mut Blake2b) {
    let value = packed::OutPoint::new_builder().build();
    hasher.update(value.as_slice());
}

fn write_default_script(hasher: &mut Blake2b) {
    let value = packed::Script::new_builder().build();
    hasher.update(value.as_slice());
}

fn write_default_script_opt(hasher: &mut Blake2b) {
    let value = packed::ScriptOpt::new_builder().build();
    hasher.update(value.as_slice());
}

fn write_default_cell_dep(hasher: &mut Blake2b) {
    let value = packed::CellDep::new_builder().build();
    hasher.update(value.as_slice());
}
```

- [x] **Step 3: Update base input hashing**

In `write_otx_base_input_cells`, replace the two mask-controlled blocks with:

```rust
        if otx.includes_base_input_since(local_index)? {
            hasher.update(
                &input_view
                    .since()
                    .map_err(|_| CoreError::MissingHashInput)?
                    .to_le_bytes(),
            );
        } else {
            hasher.update(&0u64.to_le_bytes());
        }
        if otx.includes_base_input_previous_output(local_index)? {
            let previous_output = input_view
                .previous_output()
                .map_err(|_| CoreError::MissingHashInput)?;
            writer::write_cursor_with_error(
                hasher,
                &previous_output.cursor,
                CoreError::MissingHashInput,
            )?;
        } else {
            write_default_out_point(hasher);
        }
```

Leave the resolved output and resolved data writes unchanged.

- [x] **Step 4: Update base output hashing**

In `write_otx_base_output_cells`, replace each mask-controlled block with this
default-aware form:

```rust
        if otx.includes_base_output_capacity(local_index)? {
            hasher.update(
                &output_view
                    .capacity()
                    .map_err(|_| CoreError::MissingHashInput)?
                    .to_le_bytes(),
            );
        } else {
            hasher.update(&0u64.to_le_bytes());
        }
        if otx.includes_base_output_lock(local_index)? {
            let lock = output_view
                .lock()
                .map_err(|_| CoreError::MissingHashInput)?;
            writer::write_cursor_with_error(hasher, &lock.cursor, CoreError::MissingHashInput)?;
        } else {
            write_default_script(hasher);
        }
        if otx.includes_base_output_type(local_index)? {
            let type_cursor = output_view
                .cursor
                .table_slice_by_index(2)
                .map_err(|_| CoreError::MissingHashInput)?;
            writer::write_cursor_with_error(hasher, &type_cursor, CoreError::MissingHashInput)?;
        } else {
            write_default_script_opt(hasher);
        }
        if otx.includes_base_output_data(local_index)? {
            let output_data = reader.raw_output_data_cursor(tx_index)?;
            writer::write_len_prefixed_cursor_with_error(
                hasher,
                &output_data,
                CoreError::MissingHashInput,
            )?;
        } else {
            writer::write_len_prefixed_bytes(hasher, &[])?;
        }
```

- [x] **Step 5: Update base cell dep hashing**

Replace `write_otx_base_cell_deps` with:

```rust
fn write_otx_base_cell_deps(
    hasher: &mut Blake2b,
    otx: &OtxView,
    layout: &OtxLayout,
    reader: &syscalls::SyscallTxReader,
) -> Result<(), CoreError> {
    writer::write_count(hasher, otx.base_cell_deps)?;
    writer::write_len_prefixed_bytes(hasher, otx.base_cell_dep_masks.bytes())?;
    for local_index in 0..otx.base_cell_deps {
        writer::write_count(hasher, local_index)?;
        if otx.base_cell_dep_masks.get(local_index)? {
            let tx_index = checked_index(layout.base_cell_deps, local_index)?;
            let cell_dep = reader.raw_cell_dep_cursor(tx_index)?;
            writer::write_cursor_with_error(hasher, &cell_dep, CoreError::MissingHashInput)?;
        } else {
            write_default_cell_dep(hasher);
        }
    }
    Ok(())
}
```

- [x] **Step 6: Update base header dep hashing**

Replace `write_otx_base_header_deps` with:

```rust
fn write_otx_base_header_deps(
    hasher: &mut Blake2b,
    otx: &OtxView,
    layout: &OtxLayout,
    reader: &syscalls::SyscallTxReader,
) -> Result<(), CoreError> {
    writer::write_count(hasher, otx.base_header_deps)?;
    writer::write_len_prefixed_bytes(hasher, otx.base_header_dep_masks.bytes())?;
    for local_index in 0..otx.base_header_deps {
        writer::write_count(hasher, local_index)?;
        if otx.base_header_dep_masks.get(local_index)? {
            let tx_index = checked_index(layout.base_header_deps, local_index)?;
            hasher.update(&reader.raw_header_dep_hash(tx_index)?);
        } else {
            hasher.update(&[0u8; 32]);
        }
    }
    Ok(())
}
```

## Task 3: Update the Test Signing Oracle

**Files:**
- Modify: `tests/src/framework/signing/otx.rs`

- [x] **Step 1: Add packed import**

Change the top import to include packed types:

```rust
use ckb_testtool::ckb_types::{core::TransactionView, packed, prelude::Entity};
```

- [x] **Step 2: Add matching default writer helpers**

Add these helpers after `otx_append_hash`:

```rust
fn write_default_out_point(hasher: &mut Blake2b) {
    let value = packed::OutPoint::new_builder().build();
    hasher.update(value.as_slice());
}

fn write_default_script(hasher: &mut Blake2b) {
    let value = packed::Script::new_builder().build();
    hasher.update(value.as_slice());
}

fn write_default_script_opt(hasher: &mut Blake2b) {
    let value = packed::ScriptOpt::new_builder().build();
    hasher.update(value.as_slice());
}

fn write_default_cell_dep(hasher: &mut Blake2b) {
    let value = packed::CellDep::new_builder().build();
    hasher.update(value.as_slice());
}
```

- [x] **Step 3: Mirror the base input changes**

In `write_otx_base_input_cells`, change uncovered `since` and
`previous_output` handling to:

```rust
        if otx
            .includes_base_input_since(local_index)
            .expect("input mask")
        {
            hasher.update(&input.since().expect("since").to_le_bytes());
        } else {
            hasher.update(&0u64.to_le_bytes());
        }
        if otx
            .includes_base_input_previous_output(local_index)
            .expect("input mask")
        {
            update_cursor_with_error(
                hasher,
                &input.previous_output().expect("previous output").cursor,
                CoreError::MissingHashInput,
            )
            .expect("previous output cursor");
        } else {
            write_default_out_point(hasher);
        }
```

- [x] **Step 4: Mirror the base output changes**

In `write_otx_base_output_cells`, add the same default branches as the
contract-side implementation:

```rust
        if otx
            .includes_base_output_capacity(local_index)
            .expect("output mask")
        {
            hasher.update(&output_view.capacity().expect("capacity").to_le_bytes());
        } else {
            hasher.update(&0u64.to_le_bytes());
        }
        if otx
            .includes_base_output_lock(local_index)
            .expect("output mask")
        {
            update_cursor_with_error(
                hasher,
                &output_view.lock().expect("lock").cursor,
                CoreError::MissingHashInput,
            )
            .expect("lock cursor");
        } else {
            write_default_script(hasher);
        }
        if otx
            .includes_base_output_type(local_index)
            .expect("output mask")
        {
            update_cursor_with_error(
                hasher,
                &output
                    .table_slice_by_index(2)
                    .expect("output type option cursor"),
                CoreError::MissingHashInput,
            )
            .expect("type cursor");
        } else {
            write_default_script_opt(hasher);
        }
        if otx
            .includes_base_output_data(local_index)
            .expect("output mask")
        {
            write_len_prefixed_bytes(hasher, &raw_output_data_bytes(&built.tx, tx_index));
        } else {
            write_len_prefixed_bytes(hasher, &[]);
        }
```

- [x] **Step 5: Mirror the base cell/header dep changes**

Replace `write_otx_base_cell_deps` and `write_otx_base_header_deps` in the test
oracle with:

```rust
fn write_otx_base_cell_deps(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    otx: &OtxView,
    layout: &OtxLayout,
) {
    write_count(hasher, otx.base_cell_deps);
    write_len_prefixed_bytes(hasher, otx.base_cell_dep_masks.bytes());
    for local_index in 0..otx.base_cell_deps {
        write_count(hasher, local_index);
        if otx
            .base_cell_dep_masks
            .get(local_index)
            .expect("cell dep mask")
        {
            let tx_index = checked_index(layout.base_cell_deps, local_index);
            hasher.update(&raw_cell_dep_bytes(&built.tx, tx_index));
        } else {
            write_default_cell_dep(hasher);
        }
    }
}

fn write_otx_base_header_deps(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    otx: &OtxView,
    layout: &OtxLayout,
) {
    write_count(hasher, otx.base_header_deps);
    write_len_prefixed_bytes(hasher, otx.base_header_dep_masks.bytes());
    for local_index in 0..otx.base_header_deps {
        write_count(hasher, local_index);
        if otx
            .base_header_dep_masks
            .get(local_index)
            .expect("header dep mask")
        {
            let tx_index = checked_index(layout.base_header_deps, local_index);
            hasher.update(&raw_header_dep_hash(&built.tx, tx_index));
        } else {
            hasher.update(&[0u8; 32]);
        }
    }
}
```

## Task 4: Verify Tests and Fixtures

**Files:**
- Modify only if compilation reports missing helper imports:
  `tests/src/framework/cells.rs` or the file that currently defines
  `signing_resolved_input` and `signing_output`.

- [x] **Step 1: Run formatting**

Run:

```bash
cargo fmt --check
```

Expected: fail if new code needs formatting.

If it fails, run:

```bash
cargo fmt
```

Then rerun:

```bash
cargo fmt --check
```

Expected: pass.

- [x] **Step 2: Run focused signing hash tests**

Run:

```bash
MODE=debug cargo test --offline signing_hash -- --nocapture
```

Expected: all signing hash tests pass, including the new uncovered default
tests.

- [x] **Step 3: Build debug contracts**

Run:

```bash
make build MODE=debug CARGO_ARGS=--offline
```

Expected: all contracts build successfully.

- [x] **Step 4: Run debug integration tests**

Run:

```bash
MODE=debug cargo test --offline
```

Expected: all tests pass.

- [x] **Step 5: Build release contracts**

Run:

```bash
make build MODE=release CARGO_ARGS=--offline
```

Expected: all release contracts build successfully.

- [x] **Step 6: Run release integration tests**

Run:

```bash
MODE=release cargo test --offline
```

Expected: all tests pass.

- [x] **Step 7: Check whitespace-only diff issues**

Run:

```bash
git diff --check
```

Expected: no trailing whitespace or whitespace error output.

- [x] **Step 8: Commit implementation**

Commit only the files changed for this signing-hash feature:

```bash
git add crates/cobuild-core/src/hash/mod.rs tests/src/framework/signing/otx.rs tests/src/tests.rs tests/src/tests/signing_hash.rs
git commit -m "feat: default uncovered otx mask fields in base hash"
```

If helper functions were added elsewhere, include those specific files in the
same `git add` command.

Implementation note: the final commit was `626c1cb feat: default uncovered otx
mask fields` and included the limit-order fixture expectation update:
`tests/src/fixtures/limit_order/lock_nft_for_udt/multi_orders.rs`.

## Self-Review

- Spec coverage: the plan updates both hash implementations, preserves witness
  schemas, keeps append scope unchanged, and adds tests for each uncovered
  default category.
- Placeholder scan: no deferred tasks or undefined behavior are left for the
  implementer.
- Type consistency: helper names use existing framework naming conventions and
  all packed default writers are mirrored between contract core and test oracle.
