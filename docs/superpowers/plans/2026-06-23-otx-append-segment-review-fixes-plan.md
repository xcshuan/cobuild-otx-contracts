# OTX Append Segment Review Fixes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Clean up the current append-segment branch without preserving legacy single-append compatibility, then add focused tests for multi-segment signing, duplicate seals, permission validation, and segment hash binding.

**Architecture:** Keep the production append-segment model as the only OTX append model: base OTX plus ordered append segments, each segment carrying flags, entity ranges, and its own lock seals. Cached aggregate append ranges in `OtxLayout` remain because engine planning uses them, but old test-framework helpers that silently mutate “the first append segment” are removed. Tests should exercise both core layout/engine behavior and end-to-end contract fixtures.

**Tech Stack:** Rust, `cargo test --offline`, existing `tests/src` fixture framework, `cobuild-core` unit tests.

---

## Phase 1: Remove Legacy Test-Framework Append APIs

- [x] Delete first-segment compatibility helpers from `tests/src/framework/cobuild/otx.rs`:
  - Remove `RawOtxBuilder::append_input_cells`.
  - Remove `RawOtxBuilder::append_output_cells`.
  - Remove `RawOtxBuilder::append_cell_deps`.
  - Remove `RawOtxBuilder::append_header_deps`.
  - Remove matching forwarding methods on the higher-level OTX builder wrapper in the same file.
  - Keep explicit segment construction APIs: `append_segment(...)`, `append_segment_spec(...)`, and `AppendSegmentSpec::with_*`.

- [x] Replace all call sites of the removed helpers with explicit append segment construction:
  - In `tests/src/tests.rs`, update helpers such as `signing_otx_witness_with_append_output_count` and `signing_otx_witness` to build:
    ```rust
    append_segment_spec(0)
        .with_inputs(...)
        .with_outputs(...)
    ```
    instead of `.append_input_cells(...)` / `.append_output_cells(...)`.
  - In `tests/src/framework/mod.rs`, update the framework smoke test to inspect `otx.append_segments[0]` instead of aggregate compatibility fields.
  - In any fixture builder that still relies on the old methods, construct segment specs explicitly.

- [x] Replace hardcoded permission bits in `tests/src/framework/cobuild/otx.rs` with production constants from `cobuild_core::protocol`:
  - `APPEND_PERMISSION_INPUTS`
  - `APPEND_PERMISSION_OUTPUTS`
  - `APPEND_PERMISSION_CELL_DEPS`
  - `APPEND_PERMISSION_HEADER_DEPS`
  This keeps fixture builders aligned with protocol definitions while preserving existing method names such as `allow_append_outputs`.

- [x] Rename confusing test-framework types in `tests/src/framework/tx/builder.rs`:
  - `OtxSegment` -> `OtxSpec`
  - `TrackedOtxSegment` -> `TrackedOtx`
  - Update imports and references in `tests/src/framework/tx.rs`, `tests/src/framework/signing/otx.rs`, and any fixture modules.
  Rationale: an append segment is now a real protocol object, so a full OTX spec should not be called `OtxSegment`.

- [x] Run the narrow compile check:
  ```bash
  MODE=debug cargo test --offline --no-run
  ```
  Expected result: compile succeeds; no references to removed first-segment compatibility helpers remain.

---

## Phase 2: Core Engine Multi-Segment Signature Planning Tests

- [x] Extend the engine test helper in `crates/cobuild-core/src/engine.rs` to build OTX layout entries with multiple append segments.
  - Keep the existing `test_otx(...)` helper for simple tests if useful.
  - Add a helper like:
    ```rust
    fn test_otx_with_append_segments(
        message: &Message,
        base_inputs: u32,
        append_inputs_per_segment: &[u32],
    ) -> OtxEntry
    ```
  - The helper must populate `layout.append_inputs` as the aggregate range and `layout.append_segments[*].inputs` as per-segment ranges.

- [x] Add a unit test in `crates/cobuild-core/src/engine.rs` for the same lock appearing in two append segments:
  - Base has no current-lock input.
  - Append segment 0 has one current-lock input.
  - Append segment 1 has one current-lock input.
  - Assert `required_append_segment_indices(...) == vec![0, 1]`.

- [x] Add a unit test in `crates/cobuild-core/src/engine.rs` for base plus two append segments:
  - Base includes a current-lock input.
  - Append segment 0 includes a current-lock input.
  - Append segment 1 includes a current-lock input.
  - Assert lock planning requires the base seal and both append segment seals.
  - If the public assertion point is `build_lock_plan`, assert the produced `OtxLockPlan` contains base coverage plus append segment requirements for both segment indices.

- [x] Run the focused core tests:
  ```bash
  MODE=debug cargo test --offline -p cobuild-core append_segment -- --nocapture
  ```
  Expected result: all append-segment core tests pass.

---

## Phase 3: End-to-End OTX Lock Fixture Coverage

- [x] Extend `tests/src/fixtures/cobuild_otx_lock/cases/otx_signatures.rs`:
  - Add `OtxSealShape::DuplicateAppend`.
  - Add `OtxSealShape::TwoAppendSegments`.
  - Add `OtxSealShape::MissingSecondAppend`.
  - Add `OtxTamper::CorruptSecondAppendSeal` if corruption is easier to express as tamper than seal shape.

- [x] Update `signed_otx_case(...)` so it can build either one or two append segments:
  - Segment 0 should keep the existing append inputs/outputs/deps coverage.
  - Segment 1 should include at least one current-lock input and one output, so it requires a distinct append segment seal.
  - Sign base, segment 0, and segment 1 with the existing `sign_otx_append_segment(...)` helper.
  - Fill seals in the append segment that owns each seal.

- [x] Add fixture cases:
  - Valid two-segment OTX succeeds.
  - Missing second append segment seal fails with `MissingLockSeal`.
  - Corrupted second append segment seal fails with the existing signature validation error path.
  - Duplicate append segment seal in the same segment fails with `DuplicateLockSeal`.

- [x] Add a duplicate append seal case that places two identical current-lock append seals in one append segment. This is separate from the existing `DuplicateBase` case and must assert `DuplicateLockSeal`.

- [x] Run the focused lock fixture tests:
  ```bash
  MODE=debug cargo test --offline --test tests cobuild_otx_lock -- --nocapture
  ```
  Expected result: all OTX lock fixture cases pass, including new two-segment and duplicate-append cases.

---

## Phase 4: Layout Permission and Segment-Flag Negative Tests

- [x] Add a layout test in `crates/cobuild-core/src/layout/tests.rs` where:
  - Segment 0 is valid under current `append_permissions`.
  - Segment 1 contains a disallowed entity type.
  - Assert validation returns `InvalidOtxLayout`.
  This catches implementations that only validate permissions on the first append segment.

- [x] Add a layout test for non-final closed segment ordering:
  - Segment 0 has `segment_flags = 0` and is followed by segment 1.
  - Assert layout validation rejects it.
  - Keep the existing rule that a final segment may still set `ALLOW_MORE_AFTER`; this test is only for a non-final closed segment.

- [x] If `SegmentFlags::try_from(...)` parsing remains duplicated between validation and layout construction, add a small private helper in `crates/cobuild-core/src/layout.rs` to parse segment flags once per segment in the collector path. Keep this refactor behavior-neutral and covered by existing invalid-flag tests.

- [x] Run focused layout tests:
  ```bash
  MODE=debug cargo test --offline -p cobuild-core layout -- --nocapture
  ```
  Expected result: layout tests pass, including second-segment permission rejection and non-final closed segment rejection.

---

## Phase 5: Segment Hash Binding Tests

- [x] Extend `tests/src/tests/signing_hash.rs` previous-coverage tests so `coverage_previous_segments` binds all previous entity classes:
  - Previous append input mutation changes the later segment hash.
  - Previous append cell dep mutation changes the later segment hash.
  - Previous append header dep mutation changes the later segment hash.
  Existing tests already cover previous output, flags, count, and position.

- [x] Add base binding tests for append segment signatures:
  - Changing base message changes append segment hash.
  - Changing `append_permissions` changes append segment hash.
  - Changing a base covered input/output/cell dep/header dep changes append segment hash.
  Use the existing signing hash oracle helpers, but make each mutation spec-driven and explicit so the tests are not just production code mirrored into the fixture layer.

- [x] Add one golden-vector style unit test in `tests/src/tests/signing_hash.rs`:
  - Build a minimal OTX with deterministic scripts/cells/actions.
  - Assert the base hash and one append segment hash equal fixed 32-byte values.
  - This test must fail if the hasher ordering or serialized preimage changes unintentionally.
  - If the expected values need to be generated, first run the test with temporary debug output, copy the produced values into the assertion, and remove the debug output before committing.

- [x] Run signing hash tests:
  ```bash
  MODE=debug cargo test --offline --test tests signing_hash -- --nocapture
  ```
  Expected result: all signing hash tests pass and golden values are stable.

---

## Phase 6: Scanner/Layout Parity Inside `cobuild-core`

- [x] Add a core-crate test in `crates/cobuild-core/src/witness.rs` or `crates/cobuild-core/src/layout/tests.rs` that uses internal access to `CobuildWitnessScanner`.
  - Construct witnesses with an OTX start, base witness, and two append segment witnesses.
  - Scan the actual witness order.
  - Assert the scanner-derived `OtxLayout` matches expected base ranges, aggregate append ranges, and per-segment ranges.

- [x] Add a parity mutation test:
  - Repartition an append entity between segment 0 and segment 1, or move the OTX start/witness ordering in a way the scanner supports.
  - Assert scanner-derived layout changes accordingly.
  - This test exists because external signing fixtures currently reconstruct layout from `BuiltTransaction::otx_ranges`; core scanner parity catches stale builder/oracle assumptions.

- [x] Run the focused scanner tests:
  ```bash
  MODE=debug cargo test --offline -p cobuild-core witness -- --nocapture
  ```
  Expected result: scanner/layout parity tests pass.

---

## Phase 7: Full Verification and Cleanup

- [x] Run repository-wide tests:
  ```bash
  MODE=debug cargo test --offline
  ```
  Expected result: all tests pass.

- [x] Inspect final diff:
  ```bash
  git diff --stat
  git diff --check
  ```
  Expected result: no whitespace errors; diff only contains append-segment cleanup/tests and this plan.

- [x] Run a final search for legacy compatibility names:
  ```bash
  rg "append_input_cells\\(|append_output_cells\\(|append_cell_deps\\(|append_header_deps\\(|OtxSegment|TrackedOtxSegment" tests/src crates/cobuild-core/src
  ```
  Expected result:
  - No removed first-segment compatibility helper calls.
  - No `OtxSegment` / `TrackedOtxSegment` full-OTX type names.
  - Remaining `append_*` aggregate fields are only cached layout/range fields or explicit append segment entity fields.

- [x] Summarize changes and test results for the user, including any test that could not be run and why.
