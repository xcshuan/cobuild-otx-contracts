# Cobuild Security Coverage Follow-up Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Continue Cobuild security coverage after the framework/fixtures refactor by adding high-value protocol and business tests without changing the completed architecture.

**Architecture:** Keep protocol helpers in `tests/src/framework` and concrete contract scenarios, error catalogs, coverage tags, and expected outcomes in `tests/src/fixtures`. Add small framework APIs only when a security case cannot be expressed with the existing `TxShape`, typed handles, signing oracle/facts, protocol mutations, shape mutations, and scenario outcomes.

**Tech Stack:** Rust integration tests in the `tests` crate, `ckb-testtool`, `cobuild-core` host/unit tests where appropriate, `cobuild-types` entity builders, and offline cargo test targets.

---

## Audit And Priority List

### P1-A: Cobuild lock end-to-end security error codes

Current coverage is strong for happy paths and broad malformed examples: tx-level signing, OTX base+append signing, mixed tx-level+OTX, bad seal, malformed Cobuild witness, malformed OTX layout, and two independent OTX transfers are covered in `tests/src/fixtures/cobuild_otx_lock/cases.rs` and `tests/tests/cobuild_otx_lock.rs`.

Coverage gaps:

- OTX plus same lock group outside OTX without tx-level signature is not covered end-to-end; expected stable error is `MissingLockGroupCoverage`.
- Same lock in both base and append scopes currently has a success case, but missing base seal, missing append seal, duplicate seal pair, invalid seal scope, and wrong script hash are not each represented end-to-end.
- Action target validation has unit coverage in `crates/cobuild-core/src/context.rs`, but no lock E2E representative for illegal role or missing target mapping to `InvalidMessageTarget`.
- Other-lock inputs should not affect current lock group coverage; two-OTX distinct lock facts cover part of this, but a direct mixed-current/other-lock E2E case remains useful.

First batch:

- Add fixture error variants for the stable lock/core errors already exposed by `contracts/cobuild-otx-lock/src/error.rs`.
- Add E2E cases for same-lock OTX plus outside-lock input without tx-level carrier, missing base seal, missing append seal, duplicate seal pair, invalid seal scope, wrong script hash as missing seal, and invalid action target.
- Keep expected outcomes inside `fixtures/cobuild_otx_lock`, with integration test runner unchanged.

Implementation note from the first batch: the same-lock OTX plus outside-lock-input E2E path currently fails earlier with `InvalidLockGroupWitness` because tx-level carrier validation runs before `ensure_otx_lock_group_coverage`. A direct `MissingLockGroupCoverage` E2E remains unproven and should be revisited with a core-level reachability test before changing contract expectations.

### P1-B: OTX layout malformed coverage

Current coverage includes one E2E malformed OTX layout representative using invalid permission high bits. Framework has protocol mutations for duplicate `SighashAll`, non-contiguous OTX witness, OTX before OTX start, raw `OtxStart`, raw permission, and base input mask replacement.

Coverage gaps:

- Duplicate `OtxStart` is not directly covered.
- `OtxStart` after which no `Otx` appears is not directly covered.
- Base/append entity counts exceeding transaction entity counts are not directly covered.
- Append count non-zero with permission bit disabled is not directly covered.
- Mask padding bit non-zero and mask length mismatch are not directly covered.

Recommended sequence:

- First add framework/core host or unit tests for exact `CoreError::InvalidOtxLayout` branches.
- Add one or two `cobuild_otx_lock` E2E representatives after unit coverage proves the intended branch.

### P1-C: Signing hash preimage mutation coverage

Current coverage includes successful OTX full-preimage signing with inputs, outputs, cell deps, and header deps, but not enough mutation-after-signing cases.

Coverage gaps:

- Changing `previous_output` after signing should invalidate the base seal when previous output is masked in.
- Changing append input/output/cell_dep/header_dep after signing should invalidate append seal.
- Reordering entities or changing local index should prevent signature reuse.
- Changing a base output field not covered by mask may keep the base hash valid; business scripts must reject business-relevant changes separately.

Recommended sequence:

- Add framework signing tests that compute before/after hash changes using `SigningHashOracle`.
- Add one lock E2E representative where a signed append field is mutated and the old seal fails with `BadSeal`/`VerifyFailure`.

### P2: Type plan and business matrix gaps

Current limit-order fixtures are table-driven and use `BuiltLimitOrderCase`, business mutations, expected outcomes, and typed handles.

Coverage gaps:

- `output_type_in_base_covered = false` business E2E case is not explicit.
- Full type input/output base/append and `TargetOnly` vs `InOtxScope` matrix is incomplete.
- Tx-level and OTX action coexistence matrix should be broadened so business scripts consume only the correct action origin.
- A coverage checklist/manifest would make critical tags auditable.

Recommended sequence:

- Add narrow type-plan host tests first.
- Then add focused `limit_order_type` and `limit_order_lock` E2E cases that reuse existing scenario models.

## Task 1: P1-A Cobuild Lock E2E Error Codes

**Files:**
- Modify: `tests/src/fixtures/cobuild_otx_lock/errors.rs`
- Modify: `tests/src/fixtures/cobuild_otx_lock/cases.rs`
- Test: `tests/tests/cobuild_otx_lock.rs`

- [ ] **Step 1: Write failing E2E cases**

Add cases to `cases()` for:

- `contract_rejects_otx_and_outside_same_lock_without_tx_level_signature`
- `contract_rejects_otx_missing_base_seal`
- `contract_rejects_otx_missing_append_seal`
- `contract_rejects_otx_duplicate_base_seal`
- `contract_rejects_otx_invalid_seal_scope`
- `contract_rejects_otx_wrong_script_hash_seal`
- `contract_rejects_otx_action_target_missing`

Run:

```bash
cargo test -p tests --offline --test cobuild_otx_lock cobuild_otx_lock_cases_match_expected_outcomes
```

Expected: fail until the fixture cases and error mappings are fully implemented.

- [ ] **Step 2: Add fixture error mappings**

Extend `CobuildOtxLockError` with:

```rust
InvalidMessageTarget = 32
MissingLockGroupCoverage = 34
MissingSealPair = 35
DuplicateSealPair = 36
InvalidSealScope = 37
InvalidLockGroupWitness = 39
```

- [ ] **Step 3: Implement minimal case construction**

Reuse existing `signed_otx_case` logic by introducing a local config/mutation enum for seal pair shape and message target mutations. Keep tx construction typed through `TxShape`, `OtxHandle`, `InputHandle`, `SigningFacts`, and `ExpectedOutcome`.

- [ ] **Step 4: Verify targeted test**

Run:

```bash
cargo test -p tests --offline --test cobuild_otx_lock cobuild_otx_lock_cases_match_expected_outcomes
```

Expected: pass.

- [ ] **Step 5: Commit first batch**

```bash
git add docs/superpowers/plans/2026-06-11-cobuild-security-coverage-followup-plan.md tests/src/fixtures/cobuild_otx_lock
git commit -m "test: cover cobuild lock e2e errors"
```

## Task 2: P1-B OTX Layout Malformed Coverage

**Files:**
- Modify: `crates/cobuild-core/src/layout/tests.rs`
- Optionally modify: `tests/src/framework/tx/mutate.rs`
- Optionally modify: `tests/src/fixtures/cobuild_otx_lock/cases.rs`
- Test: `crates/cobuild-core/src/layout/tests.rs`
- Test: `tests/tests/cobuild_otx_lock.rs`

- [ ] Add exact layout tests for duplicate `OtxStart`, no OTX after start, count overrun, append permission mismatch, mask padding, and mask length.
- [ ] Add one E2E representative if the current fixture mutation API can express it without broad refactor.
- [ ] Run `cargo test -p cobuild-core --offline layout` and the cobuild lock integration target.

## Task 3: P1-C Signing Hash Preimage Mutations

**Files:**
- Modify: `tests/src/framework/signing/*`
- Optionally modify: `tests/src/framework/tx/mutate.rs`
- Optionally modify: `tests/src/fixtures/cobuild_otx_lock/cases.rs`

- [ ] Add hash-change tests for base previous output and append input/output/cell_dep/header_dep mutation.
- [ ] Add entity reorder/local-index signature reuse regression if current mutation API can express it narrowly.
- [ ] Add one lock E2E representative for old append seal rejection after signed append mutation.

## Task 4: P2 Type Plan And Business Matrix

**Files:**
- Modify: `crates/cobuild-core/src/engine.rs` tests or a focused host test file.
- Modify: `tests/src/fixtures/limit_order/*`
- Modify: `tests/tests/limit_order_type.rs`
- Modify: `tests/tests/limit_order_lock.rs`

- [ ] Cover `output_type_in_base_covered = false`.
- [ ] Cover type base/append/target-only relation matrix.
- [ ] Extend tx-level plus OTX action coexistence cases.
- [ ] Add coverage tag/checklist manifest if the fixture tag model is stable enough.
