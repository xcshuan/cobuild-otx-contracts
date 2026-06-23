# Cobuild OTX Append Segment Extension Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement a prototype `SegmentedOtx` Cobuild extension witness that supports append segments with `allow_more_segments_after` and `coverage_previous_segments` flags while preserving existing Core v1 `Otx` behavior.

**Architecture:** Add a new witness variant instead of changing the existing `Otx` schema in place. Keep Core v1 paths working, then add parallel segmented parsing, layout, hash, and lock-signature planning. Test-side builders and signing oracles mirror the contract-side rules so fixture tests can generate valid and invalid segmented OTX cases.

**Tech Stack:** Rust, Molecule schemas/codegen, `cobuild-types`, `cobuild-core`, test framework under `tests/src/framework`, `cargo test --offline`, `xtask` codegen.

---

## Scope And Compatibility

This plan implements the design in:

`docs/superpowers/specs/2026-06-23-cobuild-otx-append-segment-extension-design.zh-CN.md`

Implementation choice:

- Add `SegmentedOtx` as a new `WitnessLayout` union item.
- Preserve current `Otx` schema, layout, hash, and validation behavior.
- Treat `SegmentedOtx` as an extension/prototype path. Existing tests for `Otx` must continue to pass unchanged.

Out of scope:

- Do not implement following segment commitment.
- Do not bind final segment count.
- Do not add business-specific action semantics.
- Do not require type scripts to consume segmented OTX actions in this first pass beyond existing action-target validation and relation plumbing.

## File Structure

Create or modify:

- Modify `crates/cobuild-types/schemas/core.mol`
  - Add `SegmentSealPair`, `SegmentSealPairVec`, `OtxAppendSegment`, `OtxAppendSegmentVec`, and `SegmentedOtx`.
- Modify `crates/cobuild-types/schemas/witness.mol`
  - Add `SegmentedOtx` union id.
- Regenerate:
  - `crates/cobuild-types/src/lazy_reader/core.rs`
  - `crates/cobuild-types/src/lazy_reader/witness.rs`
  - `crates/cobuild-types/src/entity/core.rs`
  - `crates/cobuild-types/src/entity/witness.rs`
- Modify `crates/cobuild-core/src/protocol.rs`
  - Add `SegmentFlags`.
- Modify `crates/cobuild-core/src/view.rs`
  - Add segmented witness views and parsing helpers.
- Modify `crates/cobuild-core/src/layout.rs`
  - Add segmented layout entries and per-segment ranges.
- Modify `crates/cobuild-core/src/hash/mod.rs`
  - Add `OtxAppendSegment` signing hash and per-segment append writers.
- Modify `crates/cobuild-core/src/seal.rs`
  - Add unique segment seal lookup.
- Modify `crates/cobuild-core/src/plan.rs`
  - Add `SignatureOrigin::OtxAppendSegment { segment_index }`.
- Modify `crates/cobuild-core/src/engine.rs`
  - Add segmented lock-signature requirements and coverage checks.
- Modify `crates/cobuild-core/src/context.rs`
  - Add helper methods for segmented ranges only where needed by engine/type relation tests.
- Modify test framework:
  - `tests/src/framework/tx/builder.rs`
  - `tests/src/framework/signing/oracle.rs`
  - `tests/src/framework/signing/otx.rs`
  - `tests/src/framework/cobuild/otx.rs`
- Add or modify tests:
  - `crates/cobuild-types/tests/generated_compile.rs`
  - `crates/cobuild-types/tests/witness_layout.rs`
  - `crates/cobuild-core/src/layout/tests.rs`
  - `crates/cobuild-core/tests/plan.rs`
  - `tests/src/tests/signing_hash.rs`
  - `tests/src/fixtures/cobuild_otx_lock/cases/otx_signatures.rs`

## Constants

Use these constants consistently:

```rust
pub const SEGMENT_FLAG_ALLOW_MORE_AFTER: u8 = 0x01;
pub const SEGMENT_FLAG_COVERAGE_PREVIOUS: u8 = 0x02;
pub const SEGMENT_FLAG_ALLOWED_MASK: u8 = 0x03;
```

Use this signing personalization:

```rust
const OTX_APPEND_SEGMENT_PERSONAL: &[u8; 16] = b"ckbcb_ots_core1\0";
```

`ots` means OTX append segment. It is 16 bytes and does not collide with existing Core v1 constants.

Use this witness union id:

```text
SegmentedOtx: 4278190085
```

It follows the existing high custom-id range and preserves existing ids.

---

### Task 1: Add Schema And Codegen For SegmentedOtx

**Files:**
- Modify: `crates/cobuild-types/schemas/core.mol`
- Modify: `crates/cobuild-types/schemas/witness.mol`
- Modify: generated files under `crates/cobuild-types/src/`
- Test: `crates/cobuild-types/tests/generated_compile.rs`
- Test: `crates/cobuild-types/tests/witness_layout.rs`

- [ ] **Step 1: Write failing generated compile test**

Modify `crates/cobuild-types/tests/generated_compile.rs`:

```rust
#[test]
fn generated_segmented_otx_types_compile() {
    let _ = core::any::type_name::<lazy_reader::core::SegmentedOtx>();
    let _ = core::any::type_name::<lazy_reader::core::OtxAppendSegment>();
    let _ = core::any::type_name::<lazy_reader::core::SegmentSealPair>();
    let _ = core::any::type_name::<entity::core::SegmentedOtx>();
    let _ = core::any::type_name::<entity::core::OtxAppendSegment>();
    let _ = core::any::type_name::<entity::core::SegmentSealPair>();
}
```

- [ ] **Step 2: Run compile test and verify it fails**

Run:

```bash
cargo test --offline -p cobuild-types generated_segmented_otx_types_compile -- --nocapture
```

Expected: FAIL with missing `SegmentedOtx`, `OtxAppendSegment`, or `SegmentSealPair` types.

- [ ] **Step 3: Add Molecule schema**

Modify `crates/cobuild-types/schemas/core.mol` after `SealPairVec`:

```text
table SegmentSealPair {
  script_hash: Byte32,
  seal: Bytes,
}

vector SegmentSealPairVec <SegmentSealPair>;

table OtxAppendSegment {
  segment_flags: byte,
  input_cells: Uint32,
  output_cells: Uint32,
  cell_deps: Uint32,
  header_deps: Uint32,
  seals: SegmentSealPairVec,
}

vector OtxAppendSegmentVec <OtxAppendSegment>;
```

Append after `table Otx`:

```text
table SegmentedOtx {
  message: Message,
  append_permissions: byte,
  base_input_cells: Uint32,
  base_input_masks: Bytes,
  base_output_cells: Uint32,
  base_output_masks: Bytes,
  base_cell_deps: Uint32,
  base_cell_dep_masks: Bytes,
  base_header_deps: Uint32,
  base_header_dep_masks: Bytes,
  append_segments: OtxAppendSegmentVec,
  base_seals: SealPairVec,
}
```

Modify `crates/cobuild-types/schemas/witness.mol`:

```text
union WitnessLayout {
  SighashAll: 4278190081,
  SighashAllOnly: 4278190082,
  Otx: 4278190083,
  OtxStart: 4278190084,
  SegmentedOtx: 4278190085,
}
```

- [ ] **Step 4: Regenerate cobuild-types**

Run:

```bash
cargo run --offline -p xtask -- codegen cobuild-types
```

Expected: command exits 0 and generated files under `crates/cobuild-types/src/` change.

- [ ] **Step 5: Add witness layout discriminant test**

Append to `crates/cobuild-types/tests/witness_layout.rs`:

```rust
#[test]
fn witness_layout_preserves_segmented_otx_discriminant() {
    use cobuild_types::entity::core::{
        Message, OtxAppendSegmentVec, SealPairVec, SegmentedOtx,
    };

    let witness = WitnessLayout::new_builder()
        .set(WitnessLayoutUnion::SegmentedOtx(
            SegmentedOtx::new_builder()
                .message(Message::default())
                .append_permissions(0u8.into())
                .base_input_cells(1u32.into())
                .base_input_masks(vec![0u8].into())
                .base_output_cells(0u32.into())
                .base_output_masks(Vec::<u8>::new().into())
                .base_cell_deps(0u32.into())
                .base_cell_dep_masks(Vec::<u8>::new().into())
                .base_header_deps(0u32.into())
                .base_header_dep_masks(Vec::<u8>::new().into())
                .append_segments(OtxAppendSegmentVec::default())
                .base_seals(SealPairVec::default())
                .build(),
        ))
        .build();
    let parsed = WitnessLayout::from_slice(witness.as_slice()).unwrap();

    assert!(matches!(
        parsed.to_enum(),
        WitnessLayoutUnion::SegmentedOtx(_)
    ));
}
```

- [ ] **Step 6: Run schema tests**

Run:

```bash
cargo test --offline -p cobuild-types generated_segmented_otx_types_compile witness_layout_preserves_segmented_otx_discriminant -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/cobuild-types/schemas crates/cobuild-types/src crates/cobuild-types/tests
git commit -m "feat: add segmented otx molecule schema"
```

---

### Task 2: Parse Segment Flags And Segmented Witness Views

**Files:**
- Modify: `crates/cobuild-core/src/protocol.rs`
- Modify: `crates/cobuild-core/src/view.rs`
- Test: `crates/cobuild-core/tests/view.rs`

- [ ] **Step 1: Write failing flag tests**

Append to an existing test module in `crates/cobuild-core/src/protocol.rs` or create `#[cfg(test)] mod tests` there:

```rust
#[cfg(test)]
mod tests {
    use super::{SegmentFlags, SEGMENT_FLAG_ALLOW_MORE_AFTER, SEGMENT_FLAG_COVERAGE_PREVIOUS};
    use crate::error::CoreError;

    #[test]
    fn segment_flags_parse_valid_bits() {
        let flags = SegmentFlags::try_from(0x03).unwrap();
        assert!(flags.allow_more_segments_after());
        assert!(flags.coverage_previous_segments());
        assert_eq!(flags.raw(), SEGMENT_FLAG_ALLOW_MORE_AFTER | SEGMENT_FLAG_COVERAGE_PREVIOUS);
    }

    #[test]
    fn segment_flags_reject_reserved_bits() {
        assert_eq!(
            SegmentFlags::try_from(0x04),
            Err(CoreError::InvalidOtxLayout)
        );
        assert_eq!(
            SegmentFlags::try_from(0x80),
            Err(CoreError::InvalidOtxLayout)
        );
    }
}
```

- [ ] **Step 2: Run flag tests and verify failure**

Run:

```bash
cargo test --offline -p cobuild-core segment_flags -- --nocapture
```

Expected: FAIL because `SegmentFlags` is not defined.

- [ ] **Step 3: Implement flags**

Append to `crates/cobuild-core/src/protocol.rs`:

```rust
pub const SEGMENT_FLAG_ALLOW_MORE_AFTER: u8 = 0x01;
pub const SEGMENT_FLAG_COVERAGE_PREVIOUS: u8 = 0x02;
pub const SEGMENT_FLAG_ALLOWED_MASK: u8 =
    SEGMENT_FLAG_ALLOW_MORE_AFTER | SEGMENT_FLAG_COVERAGE_PREVIOUS;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SegmentFlags {
    raw: u8,
}

impl SegmentFlags {
    pub fn raw(self) -> u8 {
        self.raw
    }

    pub fn allow_more_segments_after(self) -> bool {
        self.raw & SEGMENT_FLAG_ALLOW_MORE_AFTER != 0
    }

    pub fn coverage_previous_segments(self) -> bool {
        self.raw & SEGMENT_FLAG_COVERAGE_PREVIOUS != 0
    }
}

impl TryFrom<u8> for SegmentFlags {
    type Error = CoreError;

    fn try_from(raw: u8) -> Result<Self, Self::Error> {
        if raw & !SEGMENT_FLAG_ALLOWED_MASK != 0 {
            return Err(CoreError::InvalidOtxLayout);
        }
        Ok(Self { raw })
    }
}
```

- [ ] **Step 4: Add segmented view structs**

Modify imports in `crates/cobuild-core/src/view.rs` to include generated types:

```rust
use cobuild_types::lazy_reader::{
    core::{
        Action, Message, Otx, OtxAppendSegment, OtxStart, SealPair, SegmentSealPair, SegmentedOtx,
    },
    support::Cursor,
    witness::WitnessLayout as CobuildWitnessLayout,
};
```

Add structs near `OtxView`:

```rust
#[derive(Clone)]
pub struct SegmentSealPairView {
    pub script_hash: [u8; 32],
    pub seal: Cursor,
}

#[derive(Clone)]
pub struct OtxAppendSegmentView {
    pub segment_flags: u8,
    pub input_cells: usize,
    pub output_cells: usize,
    pub cell_deps: usize,
    pub header_deps: usize,
    pub seals: Vec<SegmentSealPairView>,
}

#[derive(Clone)]
pub struct SegmentedOtxView {
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
    pub append_segments: Vec<OtxAppendSegmentView>,
    pub base_seals: Vec<SealPairView>,
}
```

- [ ] **Step 5: Add segmented parsing helper functions**

In `crates/cobuild-core/src/view.rs`, add:

```rust
fn segmented_otx_data(otx: &SegmentedOtx) -> Result<SegmentedOtxView, CoreError> {
    let segment_reader = otx
        .append_segments()
        .map_err(|_| CoreError::MalformedCobuild)?;
    let segment_count = segment_reader
        .len()
        .map_err(|_| CoreError::MalformedCobuild)?;
    let mut append_segments = Vec::with_capacity(segment_count);
    for index in 0..segment_count {
        append_segments.push(append_segment_data(
            &segment_reader
                .get(index)
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?);
    }

    let seals_reader = otx.base_seals().map_err(|_| CoreError::MalformedCobuild)?;
    let seal_count = seals_reader
        .len()
        .map_err(|_| CoreError::MalformedCobuild)?;
    let mut base_seals = Vec::with_capacity(seal_count);
    for index in 0..seal_count {
        base_seals.push(seal_pair_data(
            &seals_reader
                .get(index)
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?);
    }

    Ok(SegmentedOtxView {
        message: otx.message().map_err(|_| CoreError::MalformedCobuild)?.cursor,
        append_permissions: otx
            .append_permissions()
            .map_err(|_| CoreError::MalformedCobuild)?,
        base_input_cells: u32_to_usize(otx.base_input_cells()?),
        base_input_masks: MaskView::new(cursor_bytes(
            &otx.base_input_masks().map_err(|_| CoreError::MalformedCobuild)?,
        )?),
        base_output_cells: u32_to_usize(otx.base_output_cells()?),
        base_output_masks: MaskView::new(cursor_bytes(
            &otx.base_output_masks().map_err(|_| CoreError::MalformedCobuild)?,
        )?),
        base_cell_deps: u32_to_usize(otx.base_cell_deps()?),
        base_cell_dep_masks: MaskView::new(cursor_bytes(
            &otx.base_cell_dep_masks().map_err(|_| CoreError::MalformedCobuild)?,
        )?),
        base_header_deps: u32_to_usize(otx.base_header_deps()?),
        base_header_dep_masks: MaskView::new(cursor_bytes(
            &otx.base_header_dep_masks().map_err(|_| CoreError::MalformedCobuild)?,
        )?),
        append_segments,
        base_seals,
    })
}

fn append_segment_data(
    segment: &OtxAppendSegment,
) -> Result<OtxAppendSegmentView, CoreError> {
    let seals_reader = segment.seals().map_err(|_| CoreError::MalformedCobuild)?;
    let seal_count = seals_reader
        .len()
        .map_err(|_| CoreError::MalformedCobuild)?;
    let mut seals = Vec::with_capacity(seal_count);
    for index in 0..seal_count {
        seals.push(segment_seal_pair_data(
            &seals_reader
                .get(index)
                .map_err(|_| CoreError::MalformedCobuild)?,
        )?);
    }

    Ok(OtxAppendSegmentView {
        segment_flags: segment
            .segment_flags()
            .map_err(|_| CoreError::MalformedCobuild)?,
        input_cells: u32_to_usize(segment.input_cells()?),
        output_cells: u32_to_usize(segment.output_cells()?),
        cell_deps: u32_to_usize(segment.cell_deps()?),
        header_deps: u32_to_usize(segment.header_deps()?),
        seals,
    })
}

fn segment_seal_pair_data(pair: &SegmentSealPair) -> Result<SegmentSealPairView, CoreError> {
    Ok(SegmentSealPairView {
        script_hash: byte32_to_array(pair.script_hash().map_err(|_| CoreError::MalformedCobuild)?)?,
        seal: pair.seal().map_err(|_| CoreError::MalformedCobuild)?,
    })
}
```

Call the existing local helpers in `view.rs`: `u32_to_usize`, `byte32_to_array`, `cursor_bytes`, and `seal_pair_data`.

- [ ] **Step 6: Expose `segmented_otx()` on `CobuildWitnessLayoutView`**

Add this method near existing `otx()`:

```rust
pub fn segmented_otx(&self) -> Result<Option<SegmentedOtxView>, CoreError> {
    match self.inner.to_enum() {
        cobuild_types::lazy_reader::witness::WitnessLayoutUnionReader::SegmentedOtx(value) => {
            Ok(Some(segmented_otx_data(&value)?))
        }
        _ => Ok(None),
    }
}
```

After Task 1 codegen, verify the generated enum reader name with:

```bash
rg -n "enum WitnessLayoutUnionReader|SegmentedOtx" crates/cobuild-types/src/lazy_reader/witness.rs
```

Expected: the generated reader exposes `WitnessLayoutUnionReader::SegmentedOtx`; the method above must compile before running Step 7.

- [ ] **Step 7: Run view and protocol tests**

Run:

```bash
cargo test --offline -p cobuild-core segment_flags view -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/cobuild-core/src/protocol.rs crates/cobuild-core/src/view.rs crates/cobuild-core/tests/view.rs
git commit -m "feat: parse segmented otx witnesses"
```

---

### Task 3: Add Segmented Layout Ranges

**Files:**
- Modify: `crates/cobuild-core/src/layout.rs`
- Modify: `crates/cobuild-core/src/context.rs`
- Test: `crates/cobuild-core/src/layout/tests.rs`

- [ ] **Step 1: Write failing layout tests**

Append to `crates/cobuild-core/src/layout/tests.rs`:

```rust
#[test]
fn segmented_otx_layout_tracks_each_append_segment_range() {
    let start = otx_start_witness(2, 3, 5, 7);
    let first = segmented_otx_witness(SegmentedOtxParams {
        base_inputs: 1,
        base_outputs: 1,
        base_cell_deps: 1,
        base_header_deps: 1,
        append_segments: vec![
            segment_params(0x01, 1, 2, 0, 1),
            segment_params(0x00, 2, 1, 1, 0),
        ],
        ..Default::default()
    });
    let layout = build_layout(vec![start, first], 6, 7, 7, 9).unwrap();
    let entry = &layout.segmented_otx_entries[0];

    assert_eq!(entry.layout.base_inputs, Range { start: 2, count: 1 });
    assert_eq!(entry.layout.append_inputs, Range { start: 3, count: 3 });
    assert_eq!(entry.layout.append_segments[0].inputs, Range { start: 3, count: 1 });
    assert_eq!(entry.layout.append_segments[1].inputs, Range { start: 4, count: 2 });
    assert_eq!(entry.layout.append_segments[0].outputs, Range { start: 4, count: 2 });
    assert_eq!(entry.layout.append_segments[1].outputs, Range { start: 6, count: 1 });
}

#[test]
fn segmented_otx_layout_rejects_non_final_closed_segment_before_end() {
    let start = otx_start_witness(0, 0, 0, 0);
    let witness = segmented_otx_witness(SegmentedOtxParams {
        append_segments: vec![
            segment_params(0x00, 1, 0, 0, 0),
            segment_params(0x00, 1, 0, 0, 0),
        ],
        ..Default::default()
    });

    assert_eq!(
        build_layout(vec![start, witness], 3, 0, 0, 0).unwrap_err(),
        CoreError::InvalidOtxLayout
    );
}

#[test]
fn segmented_otx_layout_rejects_reserved_segment_flags() {
    let start = otx_start_witness(0, 0, 0, 0);
    let witness = segmented_otx_witness(SegmentedOtxParams {
        append_segments: vec![segment_params(0x04, 1, 0, 0, 0)],
        ..Default::default()
    });

    assert_eq!(
        build_layout(vec![start, witness], 2, 0, 0, 0).unwrap_err(),
        CoreError::InvalidOtxLayout
    );
}
```

Add local test builders in the same file using entity builders generated in Task 1:

```rust
#[derive(Default)]
struct SegmentedOtxParams {
    base_inputs: u32,
    base_outputs: u32,
    base_cell_deps: u32,
    base_header_deps: u32,
    append_segments: Vec<SegmentParams>,
}

#[derive(Clone)]
struct SegmentParams {
    flags: u8,
    inputs: u32,
    outputs: u32,
    cell_deps: u32,
    header_deps: u32,
}

fn segment_params(
    flags: u8,
    inputs: u32,
    outputs: u32,
    cell_deps: u32,
    header_deps: u32,
) -> SegmentParams {
    SegmentParams {
        flags,
        inputs,
        outputs,
        cell_deps,
        header_deps,
    }
}
```

Implement `segmented_otx_witness(params)` with `cobuild_types::entity::core::SegmentedOtx` and `WitnessLayoutUnion::SegmentedOtx`.

- [ ] **Step 2: Run layout tests and verify failure**

Run:

```bash
cargo test --offline -p cobuild-core segmented_otx_layout -- --nocapture
```

Expected: FAIL because layout collector does not record segmented witnesses.

- [ ] **Step 3: Add layout structs**

Modify `crates/cobuild-core/src/layout.rs`:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OtxAppendSegmentLayout {
    pub segment_index: usize,
    pub flags: crate::protocol::SegmentFlags,
    pub inputs: Range,
    pub outputs: Range,
    pub cell_deps: Range,
    pub header_deps: Range,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SegmentedOtxLayout {
    pub witness_index: usize,
    pub base_inputs: Range,
    pub append_inputs: Range,
    pub base_outputs: Range,
    pub append_outputs: Range,
    pub base_cell_deps: Range,
    pub append_cell_deps: Range,
    pub base_header_deps: Range,
    pub append_header_deps: Range,
    pub append_segments: Vec<OtxAppendSegmentLayout>,
}

#[derive(Clone)]
pub struct SegmentedOtxLayoutEntry {
    pub layout: SegmentedOtxLayout,
    pub witness: crate::view::SegmentedOtxView,
}
```

Add to `BuiltLayout`:

```rust
pub segmented_otx_entries: Vec<SegmentedOtxLayoutEntry>,
```

All existing test constructors for `BuiltLayout` must initialize this field with `Vec::new()`.

- [ ] **Step 4: Teach collector to record segmented witnesses**

Extend internal storage:

```rust
otx_entries: Vec<RecordedOtx>,

enum RecordedOtx {
    Classic { witness_index: usize, view: OtxView },
    Segmented { witness_index: usize, view: SegmentedOtxView },
}
```

Extend `LayoutWitnessView`:

```rust
SegmentedOtx(SegmentedOtxView),
```

In `layout_witness_view`, check `segmented_otx()` after `otx()` and before `Ignore`.

- [ ] **Step 5: Implement segmented range allocation**

Add methods on `LayoutRangeCursor`:

```rust
fn take_segmented_layout(
    &mut self,
    witness_index: usize,
    view: &SegmentedOtxView,
) -> Result<SegmentedOtxLayout, CoreError> {
    let base_inputs = Self::take_range(&mut self.next_input, view.base_input_cells)?;
    let base_outputs = Self::take_range(&mut self.next_output, view.base_output_cells)?;
    let base_cell_deps = Self::take_range(&mut self.next_cell_dep, view.base_cell_deps)?;
    let base_header_deps =
        Self::take_range(&mut self.next_header_dep, view.base_header_deps)?;

    let mut append_segments = Vec::with_capacity(view.append_segments.len());
    let append_input_start = self.next_input;
    let append_output_start = self.next_output;
    let append_cell_dep_start = self.next_cell_dep;
    let append_header_dep_start = self.next_header_dep;

    for (segment_index, segment) in view.append_segments.iter().enumerate() {
        let flags = crate::protocol::SegmentFlags::try_from(segment.segment_flags)?;
        append_segments.push(OtxAppendSegmentLayout {
            segment_index,
            flags,
            inputs: Self::take_range(&mut self.next_input, segment.input_cells)?,
            outputs: Self::take_range(&mut self.next_output, segment.output_cells)?,
            cell_deps: Self::take_range(&mut self.next_cell_dep, segment.cell_deps)?,
            header_deps: Self::take_range(&mut self.next_header_dep, segment.header_deps)?,
        });
    }

    Ok(SegmentedOtxLayout {
        witness_index,
        base_inputs,
        append_inputs: Range {
            start: append_input_start,
            count: self.next_input - append_input_start,
        },
        base_outputs,
        append_outputs: Range {
            start: append_output_start,
            count: self.next_output - append_output_start,
        },
        base_cell_deps,
        append_cell_deps: Range {
            start: append_cell_dep_start,
            count: self.next_cell_dep - append_cell_dep_start,
        },
        base_header_deps,
        append_header_deps: Range {
            start: append_header_dep_start,
            count: self.next_header_dep - append_header_dep_start,
        },
        append_segments,
    })
}
```

- [ ] **Step 6: Validate segmented witnesses**

Add:

```rust
fn validate_segmented_otx_view(data: &SegmentedOtxView) -> Result<(), CoreError> {
    if data.base_input_cells == 0 {
        return Err(CoreError::InvalidOtxLayout);
    }
    let append_permissions = AppendPermissions::try_from(data.append_permissions)?;
    data.base_input_masks.validate(data.base_input_cells * 2)?;
    data.base_output_masks.validate(data.base_output_cells * 4)?;
    data.base_cell_dep_masks.validate(data.base_cell_deps)?;
    data.base_header_dep_masks.validate(data.base_header_deps)?;
    for seal in &data.base_seals {
        SealScope::try_from(seal.scope)?;
    }
    for (index, segment) in data.append_segments.iter().enumerate() {
        let flags = crate::protocol::SegmentFlags::try_from(segment.segment_flags)?;
        if index + 1 != data.append_segments.len() && !flags.allow_more_segments_after() {
            return Err(CoreError::InvalidOtxLayout);
        }
        append_permissions.require_allowed(0, segment.input_cells)?;
        append_permissions.require_allowed(1, segment.output_cells)?;
        append_permissions.require_allowed(2, segment.cell_deps)?;
        append_permissions.require_allowed(3, segment.header_deps)?;
    }
    Ok(())
}
```

- [ ] **Step 7: Run layout tests**

Run:

```bash
cargo test --offline -p cobuild-core segmented_otx_layout layout -- --nocapture
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add crates/cobuild-core/src/layout.rs crates/cobuild-core/src/layout/tests.rs crates/cobuild-core/src/context.rs
git commit -m "feat: add segmented otx layout scanning"
```

---

### Task 4: Add Segment Seal Lookup And Signing Hash

**Files:**
- Modify: `crates/cobuild-core/src/seal.rs`
- Modify: `crates/cobuild-core/src/hash/mod.rs`
- Test: `tests/src/framework/signing/otx.rs`
- Test: `tests/src/tests/signing_hash.rs`

- [ ] **Step 1: Add hash implementation before oracle parity tests**

This task adds contract-side hash code first. The executable red/green hash tests are added in Task 6 after the test transaction builder can construct segmented OTX witnesses.

- [ ] **Step 2: Add segment seal lookup**

Modify `crates/cobuild-core/src/seal.rs`:

```rust
use crate::view::{SealPairView, SegmentSealPairView};

pub(crate) fn unique_segment_seal(
    script_hash: [u8; 32],
    seals: &[SegmentSealPairView],
) -> Result<Vec<u8>, CoreError> {
    let mut found = None;
    for seal in seals {
        if seal.script_hash == script_hash {
            if found.is_some() {
                return Err(CoreError::DuplicateSealPair);
            }
            found = Some(cursor_bytes(&seal.seal)?);
        }
    }
    found.ok_or(CoreError::MissingSealPair)
}
```

- [ ] **Step 3: Add segment hash function**

Modify `crates/cobuild-core/src/hash/mod.rs`:

```rust
const OTX_APPEND_SEGMENT_PERSONAL: &[u8; 16] = b"ckbcb_ots_core1\0";

pub(crate) fn otx_append_segment_hash(
    otx: &SegmentedOtxView,
    layout: &SegmentedOtxLayout,
    segment_index: usize,
    reader: &syscalls::SyscallTxReader,
    base_hash: [u8; 32],
) -> Result<[u8; 32], CoreError> {
    let segment_layout = layout
        .append_segments
        .get(segment_index)
        .ok_or(CoreError::InvalidOtxLayout)?;
    let mut hasher = new_signing_hasher(OTX_APPEND_SEGMENT_PERSONAL);

    writer::write_cursor_with_error(&mut hasher, &otx.message, CoreError::MalformedCobuild)?;
    hasher.update(&base_hash);
    writer::write_count(&mut hasher, segment_index)?;
    hasher.update(&[segment_layout.flags.raw()]);

    if segment_layout.flags.coverage_previous_segments() {
        writer::write_count(&mut hasher, segment_index)?;
        for previous_index in 0..segment_index {
            write_segment_append_scope(&mut hasher, layout, previous_index, reader)?;
        }
    }
    write_segment_append_scope(&mut hasher, layout, segment_index, reader)?;

    Ok(finalize_hash(hasher))
}
```

Add:

```rust
fn write_segment_append_scope(
    hasher: &mut Blake2b,
    layout: &SegmentedOtxLayout,
    segment_index: usize,
    reader: &syscalls::SyscallTxReader,
) -> Result<(), CoreError> {
    let segment = layout
        .append_segments
        .get(segment_index)
        .ok_or(CoreError::InvalidOtxLayout)?;
    writer::write_count(hasher, segment_index)?;
    hasher.update(&[segment.flags.raw()]);
    write_segment_input_cells(hasher, segment.inputs, reader)?;
    write_segment_output_cells(hasher, segment.outputs, reader)?;
    write_segment_cell_deps(hasher, segment.cell_deps, reader)?;
    write_segment_header_deps(hasher, segment.header_deps, reader)?;
    Ok(())
}
```

Add these helpers in `crates/cobuild-core/src/hash/mod.rs`:

```rust
fn write_segment_input_cells(
    hasher: &mut Blake2b,
    range: Range,
    reader: &syscalls::SyscallTxReader,
) -> Result<(), CoreError> {
    writer::write_count(hasher, range.count)?;
    for local_index in 0..range.count {
        let tx_index = checked_index(range, local_index)?;
        let input = reader.raw_input_cursor(tx_index)?;
        writer::write_count(hasher, local_index)?;
        writer::write_cursor_with_error(hasher, &input, CoreError::MissingHashInput)?;
        let resolved_output = reader.resolved_input_output_cursor(tx_index)?;
        writer::write_cursor_with_error(hasher, &resolved_output, CoreError::MissingHashInput)?;
        let resolved_data = reader.resolved_input_data_cursor(tx_index)?;
        writer::write_len_prefixed_cursor_with_error(
            hasher,
            &resolved_data,
            CoreError::MissingHashInput,
        )?;
    }
    Ok(())
}

fn write_segment_output_cells(
    hasher: &mut Blake2b,
    range: Range,
    reader: &syscalls::SyscallTxReader,
) -> Result<(), CoreError> {
    writer::write_count(hasher, range.count)?;
    for local_index in 0..range.count {
        let tx_index = checked_index(range, local_index)?;
        writer::write_count(hasher, local_index)?;
        let output = reader.raw_output_cursor(tx_index)?;
        writer::write_cursor_with_error(hasher, &output, CoreError::MissingHashInput)?;
        let output_data = reader.raw_output_data_cursor(tx_index)?;
        writer::write_len_prefixed_cursor_with_error(
            hasher,
            &output_data,
            CoreError::MissingHashInput,
        )?;
    }
    Ok(())
}

fn write_segment_cell_deps(
    hasher: &mut Blake2b,
    range: Range,
    reader: &syscalls::SyscallTxReader,
) -> Result<(), CoreError> {
    writer::write_count(hasher, range.count)?;
    for local_index in 0..range.count {
        let tx_index = checked_index(range, local_index)?;
        writer::write_count(hasher, local_index)?;
        let cell_dep = reader.raw_cell_dep_cursor(tx_index)?;
        writer::write_cursor_with_error(hasher, &cell_dep, CoreError::MissingHashInput)?;
    }
    Ok(())
}

fn write_segment_header_deps(
    hasher: &mut Blake2b,
    range: Range,
    reader: &syscalls::SyscallTxReader,
) -> Result<(), CoreError> {
    writer::write_count(hasher, range.count)?;
    for local_index in 0..range.count {
        let tx_index = checked_index(range, local_index)?;
        writer::write_count(hasher, local_index)?;
        hasher.update(&reader.raw_header_dep_hash(tx_index)?);
    }
    Ok(())
}
```

- [ ] **Step 4: Run hash unit tests**

Run:

```bash
cargo test --offline -p cobuild-core hash -- --nocapture
```

Expected: PASS for existing tests.

- [ ] **Step 5: Commit**

```bash
git add crates/cobuild-core/src/hash/mod.rs crates/cobuild-core/src/seal.rs
git commit -m "feat: hash segmented otx append segments"
```

---

### Task 5: Add Segmented Lock Signature Planning

**Files:**
- Modify: `crates/cobuild-core/src/plan.rs`
- Modify: `crates/cobuild-core/src/engine.rs`
- Modify: `crates/cobuild-core/src/context.rs`
- Test: `crates/cobuild-core/tests/plan.rs`
- Test: `crates/cobuild-core/src/engine.rs`

- [ ] **Step 1: Add signature origin variant**

Modify `crates/cobuild-core/src/plan.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignatureOrigin {
    TxLevel,
    OtxBase,
    OtxAppend,
    OtxAppendSegment { segment_index: usize },
}
```

Update any `matches!` in engine to include `OtxAppendSegment { .. }`.

- [ ] **Step 2: Write failing engine tests**

Append to `crates/cobuild-core/src/engine.rs` tests:

```rust
#[test]
fn segmented_otx_append_input_requires_segment_seal() {
    let lock_hash = [7u8; 32];
    let context = lock_context_with_segmented_otx_entries(
        lock_hash,
        vec![lock_hash],
        vec![segmented_otx_entry_with_append_segment(lock_hash, 0x00)],
    );

    let plan = LockPlanBuilder::new(context).build().unwrap();

    assert_eq!(plan.required_signatures.len(), 1);
    assert_eq!(
        plan.required_signatures[0].origin,
        SignatureOrigin::OtxAppendSegment { segment_index: 0 }
    );
}

#[test]
fn segmented_otx_base_and_segment_same_lock_require_two_signatures() {
    let lock_hash = [8u8; 32];
    let context = lock_context_with_segmented_otx_entries(
        lock_hash,
        vec![lock_hash, lock_hash],
        vec![segmented_otx_entry_with_base_and_append_segment(lock_hash, 0x00)],
    );

    let plan = LockPlanBuilder::new(context).build().unwrap();

    assert_eq!(plan.required_signatures.len(), 2);
    assert!(plan
        .required_signatures
        .iter()
        .any(|requirement| requirement.origin == SignatureOrigin::OtxBase));
    assert!(plan.required_signatures.iter().any(|requirement| {
        requirement.origin == SignatureOrigin::OtxAppendSegment { segment_index: 0 }
    }));
}
```

Add helper constructors near existing test helpers. Build `SegmentedOtxLayoutEntry` with one base input range and one append segment range. Use empty `Cursor` values for seals where existing tests do not verify cryptography.

- [ ] **Step 3: Add segmented requirement loop**

Modify `LockPlanBuilder::add_otx_requirements` in `crates/cobuild-core/src/engine.rs`:

```rust
for (otx_index, otx) in layout.otx_entries.iter().enumerate() {
    self.add_otx_requirement(otx_index, otx)?;
}
for (otx_index, otx) in layout.segmented_otx_entries.iter().enumerate() {
    self.add_segmented_otx_requirement(otx_index, otx)?;
}
```

Add:

```rust
fn add_segmented_otx_requirement(
    &mut self,
    _otx_index: usize,
    otx: &crate::layout::SegmentedOtxLayoutEntry,
) -> Result<(), CoreError> {
    let base_signature = self
        .context
        .script_context
        .input_range_contains_current_lock(otx.layout.base_inputs)?;
    let mut segment_signatures = Vec::new();
    for segment in &otx.layout.append_segments {
        if self
            .context
            .script_context
            .input_range_contains_current_lock(segment.inputs)?
        {
            segment_signatures.push(segment.segment_index);
        }
    }
    if !base_signature && segment_signatures.is_empty() {
        return Ok(());
    }
    self.add_segmented_otx_signatures(otx, base_signature, &segment_signatures)
}
```

Add:

```rust
fn add_segmented_otx_signatures(
    &mut self,
    otx: &crate::layout::SegmentedOtxLayoutEntry,
    base_signature: bool,
    segment_signatures: &[usize],
) -> Result<(), CoreError> {
    let base_hash = segmented_otx_base_hash(&otx.witness, &otx.layout, &self.context.tx)?;
    if base_signature {
        let seal = crate::seal::unique_otx_seal_by_scope(
            self.lock_script_hash,
            &otx.witness.base_seals,
            SealScope::Base,
        )?;
        self.required_signatures.push(SigningRequirement {
            origin: SignatureOrigin::OtxBase,
            carrier_witness_index: otx.layout.witness_index,
            seal,
            signing_message_hash: base_hash,
        });
    }
    for &segment_index in segment_signatures {
        let segment = otx
            .witness
            .append_segments
            .get(segment_index)
            .ok_or(CoreError::InvalidOtxLayout)?;
        let seal = crate::seal::unique_segment_seal(self.lock_script_hash, &segment.seals)?;
        self.required_signatures.push(SigningRequirement {
            origin: SignatureOrigin::OtxAppendSegment { segment_index },
            carrier_witness_index: otx.layout.witness_index,
            seal,
            signing_message_hash: otx_append_segment_hash(
                &otx.witness,
                &otx.layout,
                segment_index,
                &self.context.tx,
                base_hash,
            )?,
        });
    }
    Ok(())
}
```

Implement `segmented_otx_base_hash` in Task 4 as a focused duplicated function that mirrors `otx_base_hash` over `SegmentedOtxView` and `SegmentedOtxLayout`. Do not introduce a trait abstraction in this plan.

- [ ] **Step 4: Update lock group coverage**

Modify `ensure_otx_lock_group_coverage`:

```rust
let has_otx = self.required_signatures.iter().any(|requirement| {
    matches!(
        requirement.origin,
        SignatureOrigin::OtxBase
            | SignatureOrigin::OtxAppend
            | SignatureOrigin::OtxAppendSegment { .. }
    )
});
```

Ensure aggregate `BuiltLayout.input_range` includes both classic and segmented OTX ranges from the layout scanner.

- [ ] **Step 5: Run engine tests**

Run:

```bash
cargo test --offline -p cobuild-core segmented_otx_append_input_requires_segment_seal segmented_otx_base_and_segment_same_lock_require_two_signatures -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Run plan and engine suites**

Run:

```bash
cargo test --offline -p cobuild-core plan engine -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/cobuild-core/src/plan.rs crates/cobuild-core/src/engine.rs crates/cobuild-core/src/context.rs crates/cobuild-core/tests/plan.rs
git commit -m "feat: plan segmented otx lock signatures"
```

---

### Task 6: Add Test Framework Builder And Signing Oracle Support

**Files:**
- Modify: `tests/src/framework/tx/builder.rs`
- Modify: `tests/src/framework/signing/oracle.rs`
- Modify: `tests/src/framework/signing/otx.rs`
- Modify: `tests/src/framework/cobuild/otx.rs`
- Test: `tests/src/tests/signing_hash.rs`

- [ ] **Step 1: Add framework data structs**

Modify `tests/src/framework/tx/builder.rs`:

```rust
#[derive(Clone, Debug, Default)]
pub struct SegmentedOtxSpec {
    pub message: Option<CobuildMessage>,
    pub base_inputs: Vec<ResolvedInputFacts>,
    pub base_outputs: Vec<TestCellOutput>,
    pub base_cell_deps: Vec<CellDep>,
    pub base_header_deps: Vec<[u8; 32]>,
    pub base_input_masks: Option<Vec<u8>>,
    pub base_output_masks: Option<Vec<u8>>,
    pub base_cell_dep_masks: Option<Vec<u8>>,
    pub base_header_dep_masks: Option<Vec<u8>>,
    pub append_segments: Vec<AppendSegmentSpec>,
    pub base_seals: Vec<SealPair>,
}

#[derive(Clone, Debug, Default)]
pub struct AppendSegmentSpec {
    pub flags: u8,
    pub inputs: Vec<ResolvedInputFacts>,
    pub outputs: Vec<TestCellOutput>,
    pub cell_deps: Vec<CellDep>,
    pub header_deps: Vec<[u8; 32]>,
    pub seals: Vec<SegmentSealPair>,
}
```

Add builder convenience:

```rust
pub fn append_segment_spec(flags: u8) -> AppendSegmentSpec {
    AppendSegmentSpec {
        flags,
        ..Default::default()
    }
}

impl AppendSegmentSpec {
    pub fn with_inputs(mut self, inputs: Vec<ResolvedInputFacts>) -> Self {
        self.inputs = inputs;
        self
    }

    pub fn with_outputs(mut self, outputs: Vec<TestCellOutput>) -> Self {
        self.outputs = outputs;
        self
    }
}
```

- [ ] **Step 2: Add segmented ranges**

Add:

```rust
#[derive(Clone, Debug)]
pub struct AppendSegmentRangeFacts {
    pub segment_index: usize,
    pub flags: u8,
    pub inputs: Range<usize>,
    pub outputs: Range<usize>,
    pub cell_deps: Range<usize>,
    pub header_deps: Range<usize>,
}

#[derive(Clone, Debug)]
pub struct SegmentedOtxRangeFacts {
    pub otx: OtxHandle,
    pub base_inputs: Range<usize>,
    pub append_inputs: Range<usize>,
    pub base_outputs: Range<usize>,
    pub append_outputs: Range<usize>,
    pub base_cell_deps: Range<usize>,
    pub append_cell_deps: Range<usize>,
    pub base_header_deps: Range<usize>,
    pub append_header_deps: Range<usize>,
    pub append_segments: Vec<AppendSegmentRangeFacts>,
}
```

Add to `BuiltTxShape`:

```rust
pub segmented_otx_ranges: Vec<SegmentedOtxRangeFacts>,
```

Existing `TxShape::build` must initialize this to an empty vector when no segmented OTXs exist.

- [ ] **Step 3: Add `push_segmented_otx`**

Add to `TxShape`:

```rust
pub fn push_segmented_otx(&mut self, spec: SegmentedOtxSpec) -> OtxHandle {
    assert!(
        !spec.base_inputs.is_empty(),
        "Segmented OTX requires non-zero base inputs"
    );
    let handle = OtxHandle::from_raw(self.otxs.len() + self.segmented_otxs.len());
    let tracked = TrackedSegmentedOtx::from_spec(handle, spec, self);
    self.segmented_otxs.push(tracked);
    handle
}
```

Add `segmented_otxs: Vec<TrackedSegmentedOtx>` to `TxShape`. Implement `TrackedSegmentedOtx::from_spec` using the same handle allocation style as `TrackedOtxSegment`.

- [ ] **Step 4: Build segmented witnesses**

When building witnesses, add `WitnessLayoutUnion::SegmentedOtx(...)` using generated entity builders:

```rust
WitnessLayout::new_builder()
    .set(WitnessLayoutUnion::SegmentedOtx(
        SegmentedOtx::new_builder()
            .message(message)
            .append_permissions(append_permissions)
            .base_input_cells((spec.base_inputs.len() as u32).into())
            .base_input_masks(base_input_masks.into())
            .base_output_cells((spec.base_outputs.len() as u32).into())
            .base_output_masks(base_output_masks.into())
            .base_cell_deps((spec.base_cell_deps.len() as u32).into())
            .base_cell_dep_masks(base_cell_dep_masks.into())
            .base_header_deps((spec.base_header_deps.len() as u32).into())
            .base_header_dep_masks(base_header_dep_masks.into())
            .append_segments(segment_vec)
            .base_seals(base_seals)
            .build(),
    ))
    .build()
```

Compute `append_permissions` from the union of all segment counts.

- [ ] **Step 5: Add signing oracle methods**

Modify `tests/src/framework/signing/oracle.rs`:

```rust
fn segmented_otx_append_segment(
    &self,
    built: &BuiltTxShape,
    otx: OtxHandle,
    segment_index: usize,
    base_hash: [u8; 32],
) -> [u8; 32];
```

Implement in `TestSigningHashOracle`:

```rust
fn segmented_otx_append_segment(
    &self,
    built: &BuiltTxShape,
    otx: OtxHandle,
    segment_index: usize,
    base_hash: [u8; 32],
) -> [u8; 32] {
    otx::segmented_otx_append_segment_hash(built, otx, segment_index, base_hash)
}
```

- [ ] **Step 6: Mirror segmented hash in test oracle**

Modify `tests/src/framework/signing/otx.rs`:

```rust
const OTX_APPEND_SEGMENT_PERSONAL: &[u8; 16] = b"ckbcb_ots_core1\0";

pub(crate) fn segmented_otx_append_segment_hash(
    built: &BuiltTxShape,
    otx: OtxHandle,
    segment_index: usize,
    base_hash: [u8; 32],
) -> [u8; 32] {
    let facts = segmented_otx_range_facts(built, otx);
    let segment = &facts.append_segments[segment_index];
    let mut out = [0u8; 32];
    let mut hasher = new_hasher(OTX_APPEND_SEGMENT_PERSONAL);
    let view = segmented_otx_view(built, otx);

    update_cursor_with_error(&mut hasher, &view.message, CoreError::MalformedCobuild)
        .expect("message cursor");
    hasher.update(&base_hash);
    write_count(&mut hasher, segment_index);
    hasher.update(&[segment.flags]);
    if segment.flags & 0x02 != 0 {
        write_count(&mut hasher, segment_index);
        for previous_index in 0..segment_index {
            write_segment_scope(&mut hasher, built, facts, previous_index);
        }
    }
    write_segment_scope(&mut hasher, built, facts, segment_index);
    hasher.finalize(&mut out);
    out
}
```

Add this helper:

```rust
fn write_segment_scope(
    hasher: &mut Blake2b,
    built: &BuiltTxShape,
    facts: &SegmentedOtxRangeFacts,
    segment_index: usize,
) {
    let segment = &facts.append_segments[segment_index];
    write_count(hasher, segment_index);
    hasher.update(&[segment.flags]);

    write_count(hasher, segment.inputs.end - segment.inputs.start);
    for (local_index, tx_index) in segment.inputs.clone().enumerate() {
        write_count(hasher, local_index);
        hasher.update(built.resolved_inputs[tx_index].input.as_slice());
        hasher.update(built.resolved_inputs[tx_index].output.as_slice());
        write_len_prefixed_bytes(hasher, built.resolved_inputs[tx_index].data.as_ref());
    }

    write_count(hasher, segment.outputs.end - segment.outputs.start);
    for (local_index, tx_index) in segment.outputs.clone().enumerate() {
        write_count(hasher, local_index);
        hasher.update(&raw_output_bytes(&built.tx, tx_index));
        write_len_prefixed_bytes(hasher, &raw_output_data_bytes(&built.tx, tx_index));
    }

    write_count(hasher, segment.cell_deps.end - segment.cell_deps.start);
    for (local_index, tx_index) in segment.cell_deps.clone().enumerate() {
        write_count(hasher, local_index);
        hasher.update(&raw_cell_dep_bytes(&built.tx, tx_index));
    }

    write_count(hasher, segment.header_deps.end - segment.header_deps.start);
    for (local_index, tx_index) in segment.header_deps.clone().enumerate() {
        write_count(hasher, local_index);
        hasher.update(&raw_header_dep_hash(&built.tx, tx_index));
    }
}
```

- [ ] **Step 7: Run signing hash tests**

Run:

```bash
MODE=debug cargo test --offline signing_hash_oracle_segment -- --nocapture
```

Expected: PASS for the new segmented signing hash tests.

- [ ] **Step 8: Commit**

```bash
git add tests/src/framework/tx tests/src/framework/signing tests/src/framework/cobuild tests/src/tests/signing_hash.rs
git commit -m "test: add segmented otx signing oracle"
```

---

### Task 7: Add Contract Fixture Cases For Segment Seals

**Files:**
- Modify: `tests/src/fixtures/cobuild_otx_lock/cases/otx_signatures.rs`
- Modify: `tests/src/fixtures/cobuild_otx_lock/cases/helpers.rs`
- Modify: `tests/tests/cobuild_otx_lock.rs` or existing test registration file

- [ ] **Step 1: Add positive case**

Add a fixture function:

```rust
pub(super) fn signed_segmented_otx_own_segment_case() -> BuiltCobuildOtxLockCase {
    build_segmented_otx_case(
        "contract_accepts_segmented_otx_own_segment_signature",
        SegmentCaseConfig {
            first_segment_flags: 0x01,
            second_segment_flags: 0x00,
            sign_segment_index: 0,
            tamper: SegmentTamper::None,
        },
    )
}
```

Expected behavior: current lock appears only in segment 0 append inputs. The case includes a valid segment 0 seal and passes.

- [ ] **Step 2: Add missing seal negative case**

```rust
pub(super) fn segmented_otx_missing_segment_seal_case() -> BuiltCobuildOtxLockCase {
    build_segmented_otx_case(
        "contract_rejects_segmented_otx_missing_segment_seal",
        SegmentCaseConfig {
            first_segment_flags: 0x01,
            second_segment_flags: 0x00,
            sign_segment_index: 0,
            tamper: SegmentTamper::DropSegmentSeal,
        },
    )
}
```

Expected error: `MissingSealPair`.

- [ ] **Step 3: Add previous coverage mutation negative case**

```rust
pub(super) fn segmented_otx_previous_coverage_rejects_previous_mutation_case(
) -> BuiltCobuildOtxLockCase {
    build_segmented_otx_case(
        "contract_rejects_segmented_otx_previous_coverage_after_previous_mutation",
        SegmentCaseConfig {
            first_segment_flags: 0x01,
            second_segment_flags: 0x02,
            sign_segment_index: 1,
            tamper: SegmentTamper::MutatePreviousSegmentOutputAfterSigning,
        },
    )
}
```

Expected error: signature verification failure.

- [ ] **Step 4: Add own coverage allows later mutation oracle test**

This should be a signing hash test, not a contract acceptance test if mutation creates an economically invalid business case. Add to `tests/src/tests/signing_hash.rs`:

```rust
#[test]
fn signing_hash_oracle_segment_own_coverage_does_not_bind_later_segment() {
    let mut shape = TxShape::new();
    let otx = shape.push_segmented_otx(SegmentedOtxSpec {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_segments: vec![
            append_segment_spec(0x01)
                .with_inputs(vec![signing_resolved_input(2, vec![0xbb])])
                .with_outputs(vec![signing_output(3, vec![0xcc])]),
            append_segment_spec(0x00)
                .with_inputs(vec![signing_resolved_input(4, vec![0xdd])])
                .with_outputs(vec![signing_output(5, vec![0xee])]),
        ],
        ..Default::default()
    });
    let built = shape.build();
    let base_hash = TestSigningHashOracle.otx_base(&built, otx);
    let before = TestSigningHashOracle.segmented_otx_append_segment(&built, otx, 0, base_hash);

    let output = built.segmented_otx_append_output(otx, 1, 0);
    let changed = TxMutator::new(built)
        .replace_output(output, signing_output(9, vec![0xff]))
        .build();

    assert_eq!(
        before,
        TestSigningHashOracle.segmented_otx_append_segment(&changed, otx, 0, base_hash)
    );
}
```

Also add:

```rust
#[test]
fn signing_hash_oracle_segment_previous_coverage_binds_previous_segment() {
    let mut shape = TxShape::new();
    let otx = shape.push_segmented_otx(SegmentedOtxSpec {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_segments: vec![
            append_segment_spec(0x01)
                .with_outputs(vec![signing_output(2, vec![0xbb])]),
            append_segment_spec(0x02)
                .with_outputs(vec![signing_output(3, vec![0xcc])]),
        ],
        ..Default::default()
    });
    let built = shape.build();
    let base_hash = TestSigningHashOracle.otx_base(&built, otx);
    let before = TestSigningHashOracle.segmented_otx_append_segment(&built, otx, 1, base_hash);

    let previous_output = built.segmented_otx_append_output(otx, 0, 0);
    let changed = TxMutator::new(built)
        .replace_output(previous_output, signing_output(9, vec![0xdd]))
        .build();

    assert_ne!(
        before,
        TestSigningHashOracle.segmented_otx_append_segment(&changed, otx, 1, base_hash)
    );
}
```

- [ ] **Step 5: Register cases**

Add the new fixture functions to the same module list used by existing `cobuild_otx_lock` tests.

- [ ] **Step 6: Run fixture tests**

Run:

```bash
MODE=debug cargo test --offline cobuild_otx_lock -- --nocapture
```

Expected: PASS, including the new segmented OTX cases.

- [ ] **Step 7: Commit**

```bash
git add tests/src/fixtures/cobuild_otx_lock tests/tests/cobuild_otx_lock.rs tests/src/tests/signing_hash.rs
git commit -m "test: cover segmented otx lock signatures"
```

---

### Task 8: Full Verification And Documentation Sync

**Files:**
- Review: `docs/superpowers/specs/2026-06-23-cobuild-otx-append-segment-extension-design.zh-CN.md`
- Review: `README.md`
- No code changes in this task.

- [ ] **Step 1: Run codegen check**

Run:

```bash
cargo run --offline -p xtask -- codegen cobuild-types --check
```

Expected: PASS.

- [ ] **Step 2: Run focused Rust tests**

Run:

```bash
cargo test --offline -p cobuild-types -- --nocapture
cargo test --offline -p cobuild-core -- --nocapture
MODE=debug cargo test --offline signing_hash -- --nocapture
MODE=debug cargo test --offline cobuild_otx_lock -- --nocapture
```

Expected: all commands exit 0.

- [ ] **Step 3: Run broader suite**

Run:

```bash
MODE=debug cargo test --offline -- --nocapture
```

Expected: command exits 0.

- [ ] **Step 4: Review spec against implementation**

Check:

- `segment_flags` only accepts `0x00..=0x03`.
- `0x00` and `0x01` hashes do not cover other segments.
- `0x02` and `0x03` hashes cover previous segments and own segment.
- Existing `Otx` tests still pass unchanged.
- `SegmentedOtx` is clearly documented as extension/prototype, not Core v1 replacement.

- [ ] **Step 5: Commit final docs when Step 4 changes documentation**

When Step 4 changes documentation, run:

```bash
git add docs/superpowers/specs/2026-06-23-cobuild-otx-append-segment-extension-design.zh-CN.md README.md
git commit -m "docs: sync segmented otx implementation notes"
```

When Step 4 makes no documentation changes, run:

```bash
git diff -- docs/superpowers/specs/2026-06-23-cobuild-otx-append-segment-extension-design.zh-CN.md README.md
```

Expected: no output.

- [ ] **Step 6: Final status**

Run:

```bash
git status --short --branch
git log --oneline --max-count=8
```

Expected: only pre-existing unrelated untracked docs remain; the implementation commits are on `design/otx-append-segments`.

## Self-Review Notes

Spec coverage:

- Data model: Task 1.
- Flags: Task 2 and Task 3.
- Transaction layout: Task 3 and Task 6.
- Signing domains: Task 4 and Task 6.
- Lock signature requirements: Task 5 and Task 7.
- Validation rules: Task 3, Task 5, and Task 7.
- Size impact and recommendation: documented only; no implementation task required.

Risk points:

- `BuiltLayout` changes can touch many tests. Keep compatibility by adding `segmented_otx_entries: Vec::new()` wherever tests build `BuiltLayout` manually.
- The exact generated lazy-reader union enum names must be checked after Task 1 codegen.
- The test framework currently uses `OtxHandle` for classic OTXs. This plan reuses `OtxHandle` for segmented OTXs and requires `BuiltTxShape` lookup methods to reject unknown handles with `expect("unknown segmented OTX handle")`.
- Type-script relation support is limited to existing action-target validation in this plan. Full segmented type relation APIs are out of scope for this implementation plan.
