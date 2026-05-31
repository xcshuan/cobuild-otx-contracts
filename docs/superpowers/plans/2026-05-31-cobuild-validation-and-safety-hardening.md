# Cobuild Validation And Safety Hardening Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the Cobuild OTX implementation verification matrix deterministic and tighten Core v1 fail-closed behavior coverage.

**Architecture:** Keep protocol logic in `cobuild-core` and lock-local orchestration in `cobuild-otx-lock`. Fix tooling and tests first, then adjust public error mapping and add focused negative coverage at the lowest useful layer.

**Tech Stack:** Rust 2021/2024, Molecule 0.9.2 codegen, `ckb-std`, `ckb-testtool`, Cargo offline tests, Makefile contract build.

---

## Spec

Implement:

`docs/superpowers/specs/2026-05-31-cobuild-validation-and-safety-hardening-design.md`

## File Structure

- Modify `crates/cobuild-core/tests/no_entity_dependency.rs`
  - Fix source path handling so the test works from any Cargo test cwd.
- Modify `xtask/src/main.rs`
  - Run Molecule codegen from the schema directory so `import core` and
    `import blockchain` resolve.
  - Keep `--check` output under `target/xtask-codegen-check`.
- Modify `contracts/cobuild-otx-lock/src/runner.rs`
  - Make `map_core_error` testable inside the library.
  - Remap Core protocol errors to `MalformedCobuild`.
- Modify `contracts/cobuild-otx-lock/tests/error.rs`
  - Add unit coverage for `CoreError` to public `Error` mapping.
- Modify `contracts/cobuild-otx-lock/tests/runner.rs`
  - Replace the host-binary test with a deterministic library-level test.
- Modify `crates/cobuild-core/tests/layout.rs`
  - Add OTX layout fail-closed negative coverage.
- Modify `crates/cobuild-core/tests/tasks.rs`
  - Add seal and message target fail-closed negative coverage.
- Modify `tests/src/lib.rs`
  - Add one integration fixture for malformed OTX layout exit mapping.
- Modify `tests/tests/cobuild_otx_lock.rs`
  - Assert malformed OTX layout exits with `MalformedCobuild`.

Do not modify generated `crates/cobuild-types/src/{lazy_reader,entity}` files
unless the fixed `xtask --check` reports real drift.

## Task 1: Stabilize `cobuild-core` Source Boundary Tests

**Files:**
- Modify: `crates/cobuild-core/tests/no_entity_dependency.rs`

- [ ] **Step 1: Run the boundary test baseline**

Run:

```bash
cargo test -p cobuild-core --offline --test no_entity_dependency
```

Expected before this hardening pass: the test may already pass in some Cargo
invocations. Continue with this task because the replacement makes path handling
explicit and gives better failure diagnostics without changing the assertion.

- [ ] **Step 2: Fix manifest-relative source paths**

Replace `crates/cobuild-core/tests/no_entity_dependency.rs` with:

```rust
use std::path::{Path, PathBuf};

fn manifest_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

#[test]
fn core_source_does_not_import_entity_module() {
    for path in [
        "src/context.rs",
        "src/hash.rs",
        "src/layout.rs",
        "src/lib.rs",
        "src/loader.rs",
        "src/tasks.rs",
        "src/view.rs",
        "src/witness.rs",
    ] {
        let full_path = manifest_path(path);
        let text = std::fs::read_to_string(&full_path)
            .unwrap_or_else(|err| panic!("read {}: {err}", full_path.display()));
        let forbidden = ["cobuild_types", "entity"].join("::");
        assert!(
            !text.contains(&forbidden),
            "{path} must not import {forbidden}"
        );
    }
}

#[test]
fn view_does_not_publicly_expose_generated_inner_reader() {
    let path = manifest_path("src/view.rs");
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    assert!(
        !text.contains("pub fn inner("),
        "view must not expose generated lazy-reader internals outside cobuild-core"
    );
}
```

- [ ] **Step 3: Verify the boundary test passes**

Run:

```bash
cargo test -p cobuild-core --offline --test no_entity_dependency
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/cobuild-core/tests/no_entity_dependency.rs
git commit -m "test: stabilize cobuild core boundary paths"
```

## Task 2: Fix `xtask` Codegen Check

**Files:**
- Modify: `xtask/src/main.rs`

- [ ] **Step 1: Run the failing codegen check**

Run:

```bash
cargo run -p xtask --offline -- codegen cobuild-types --check
```

Expected: FAIL or panic while resolving Molecule imports such as
`import core`.

- [ ] **Step 2: Run Molecule codegen from the schema directory**

In `xtask/src/main.rs`, add this helper after `generate_family`:

```rust
fn run_codegen(schema_dir: &Path, schema: &str, out_dir: &Path, language: Language) -> Result<()> {
    let previous_dir = env::current_dir().context("read current directory before codegen")?;
    env::set_current_dir(schema_dir)
        .with_context(|| format!("enter schema directory {}", schema_dir.display()))?;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        Compiler::new()
            .generate_code(language)
            .input_schema_file(schema)
            .output_dir(out_dir)
            .run()
            .map_err(anyhow::Error::msg)
    }));

    env::set_current_dir(&previous_dir)
        .with_context(|| format!("restore working directory {}", previous_dir.display()))?;

    match result {
        Ok(result) => result.with_context(|| format!("failed to generate {schema}")),
        Err(_) => bail!("molecule codegen panicked while generating {schema}"),
    }
}
```

Then replace the `Compiler::new()...run()` block inside `generate_family` with:

```rust
run_codegen(schema_dir, schema, out_dir, language)?;
```

Keep the existing `rewrite_lazy_reader_imports` and `run_rustfmt` calls.

- [ ] **Step 3: Verify check mode succeeds or reports drift**

Run:

```bash
cargo run -p xtask --offline -- codegen cobuild-types --check
```

Expected: PASS if committed generated files match schemas. If it fails with
`generated output differs`, inspect the diff under
`target/xtask-codegen-check/cobuild-types/src` and proceed to Step 4.

- [ ] **Step 4: Regenerate only if check reports drift**

Run only if Step 3 reports generated-output drift:

```bash
cargo run -p xtask --offline -- codegen cobuild-types
cargo run -p xtask --offline -- codegen cobuild-types --check
```

Expected: second command PASS. If generated files changed, review them before
staging.

- [ ] **Step 5: Commit**

If only `xtask` changed:

```bash
git add xtask/src/main.rs
git commit -m "fix: make cobuild type codegen check deterministic"
```

If generated files changed because drift was real:

```bash
git add xtask/src/main.rs crates/cobuild-types/src/lazy_reader crates/cobuild-types/src/entity
git commit -m "fix: refresh cobuild type codegen outputs"
```

## Task 3: Make Core Error Mapping Explicit And Testable

**Files:**
- Modify: `contracts/cobuild-otx-lock/src/runner.rs`
- Delete: `contracts/cobuild-otx-lock/tests/runner.rs`

- [ ] **Step 1: Add failing error-mapping unit tests**

Append to `contracts/cobuild-otx-lock/src/runner.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn core_protocol_errors_map_to_malformed_cobuild() {
        for err in [
            CoreError::MalformedCobuild,
            CoreError::InvalidLayout,
            CoreError::InvalidMessageTarget,
            CoreError::MissingSealPair,
            CoreError::DuplicateSealPair,
        ] {
            assert_eq!(map_core_error(err), Error::MalformedCobuild);
        }
    }

    #[test]
    fn missing_hash_parts_maps_to_internal_failure() {
        assert_eq!(
            map_core_error(CoreError::MissingHashParts),
            Error::InternalFailure
        );
    }
}
```

- [ ] **Step 2: Delete the non-deterministic runner integration test**

Delete `contracts/cobuild-otx-lock/tests/runner.rs`. Its current host-binary
assertion depends on `CARGO_BIN_EXE_cobuild-otx-lock`, which is not reliable for
this package. Contract binary behavior remains covered by the workspace
`tests` crate.

- [ ] **Step 3: Run tests and verify failure**

Run:

```bash
cargo test -p cobuild-otx-lock --offline --lib --test error
```

Expected: FAIL because `map_core_error` still maps several protocol errors to
`LockSemanticFailure`.

- [ ] **Step 4: Update the mapping**

In `contracts/cobuild-otx-lock/src/runner.rs`, keep `map_core_error` private
and replace its match body with:

```rust
match err {
    CoreError::MalformedCobuild
    | CoreError::InvalidLayout
    | CoreError::InvalidMessageTarget
    | CoreError::MissingSealPair
    | CoreError::DuplicateSealPair => Error::MalformedCobuild,
    CoreError::MissingHashParts => Error::InternalFailure,
}
```

- [ ] **Step 5: Verify package tests**

Run:

```bash
cargo test -p cobuild-otx-lock --offline
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add contracts/cobuild-otx-lock/src/runner.rs contracts/cobuild-otx-lock/tests/runner.rs
git commit -m "fix: classify cobuild protocol errors as malformed"
```

## Task 4: Expand Core Layout Negative Coverage

**Files:**
- Modify: `crates/cobuild-core/tests/layout.rs`

- [ ] **Step 1: Add missing layout regression tests**

Keep the existing tests for `Otx` before `OtxStart` and non-contiguous `Otx`
sequences. Append these additional tests to
`crates/cobuild-core/tests/layout.rs`:


```rust
#[test]
fn duplicate_otx_start_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_start_witness(), otx_witness()],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}

#[test]
fn otx_start_without_following_otx_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness()],
        input_count: 0,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}

#[test]
fn zero_base_inputs_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_counts(0, 0, 0, 0, 0, 0)],
        input_count: 0,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}

#[test]
fn reserved_append_permission_bits_are_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_permissions(0x10)],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}

#[test]
fn append_count_without_permission_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_append_counts(0, 1, 0, 0, 0)],
        input_count: 2,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}

#[test]
fn append_output_without_permission_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_append_counts(0, 0, 1, 0, 0)],
        input_count: 1,
        output_count: 1,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}

#[test]
fn append_cell_dep_without_permission_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_append_counts(0, 0, 0, 1, 0)],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 1,
        header_dep_count: 0,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}

#[test]
fn append_header_dep_without_permission_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_append_counts(0, 0, 0, 0, 1)],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 1,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}

#[test]
fn invalid_base_input_mask_length_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_base_input_mask(&[])],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}

#[test]
fn invalid_base_output_mask_length_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_base_output_mask(1, &[])],
        input_count: 1,
        output_count: 1,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}

#[test]
fn invalid_base_cell_dep_mask_length_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_base_cell_dep_mask(1, &[])],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 1,
        header_dep_count: 0,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}

#[test]
fn invalid_base_header_dep_mask_length_is_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_base_header_dep_mask(1, &[])],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 1,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}

#[test]
fn non_zero_base_input_mask_padding_bits_are_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_base_input_mask(&[0b0000_0100])],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}

#[test]
fn non_zero_base_output_mask_padding_bits_are_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_base_output_mask(1, &[0b0001_0000])],
        input_count: 1,
        output_count: 1,
        cell_dep_count: 0,
        header_dep_count: 0,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}

#[test]
fn non_zero_base_cell_dep_mask_padding_bits_are_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_base_cell_dep_mask(1, &[0b0000_0010])],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 1,
        header_dep_count: 0,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}

#[test]
fn non_zero_base_header_dep_mask_padding_bits_are_invalid() {
    let result = build_layout(&LayoutTx {
        witnesses: vec![otx_start_witness(), otx_witness_with_base_header_dep_mask(1, &[0b0000_0010])],
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 1,
    });

    assert_eq!(result, Err(CoreError::InvalidLayout));
}
```

- [ ] **Step 2: Add test helpers**

Add these helpers below the existing `otx_witness()` helper:

```rust
fn otx_witness_with_permissions(append_permissions: u8) -> Vec<u8> {
    otx_witness_custom(append_permissions, 1, &[0], 0, &[], 0, &[], 0, &[], 0, 0, 0, 0)
}

fn otx_witness_with_append_counts(
    append_permissions: u8,
    append_inputs: u32,
    append_outputs: u32,
    append_cell_deps: u32,
    append_header_deps: u32,
) -> Vec<u8> {
    otx_witness_custom(
        append_permissions,
        1,
        &[0],
        0,
        &[],
        0,
        &[],
        0,
        &[],
        append_inputs,
        append_outputs,
        append_cell_deps,
        append_header_deps,
    )
}

fn otx_witness_with_base_input_mask(mask: &[u8]) -> Vec<u8> {
    otx_witness_custom(0, 1, mask, 0, &[], 0, &[], 0, &[], 0, 0, 0, 0)
}

fn otx_witness_with_base_output_mask(base_outputs: u32, mask: &[u8]) -> Vec<u8> {
    otx_witness_custom(0, 1, &[0], base_outputs, mask, 0, &[], 0, &[], 0, 0, 0, 0)
}

fn otx_witness_with_base_cell_dep_mask(base_cell_deps: u32, mask: &[u8]) -> Vec<u8> {
    otx_witness_custom(0, 1, &[0], 0, &[], base_cell_deps, mask, 0, &[], 0, 0, 0, 0)
}

fn otx_witness_with_base_header_dep_mask(base_header_deps: u32, mask: &[u8]) -> Vec<u8> {
    otx_witness_custom(0, 1, &[0], 0, &[], 0, &[], base_header_deps, mask, 0, 0, 0, 0)
}

fn otx_witness_with_counts(
    base_inputs: u32,
    append_inputs: u32,
    base_outputs: u32,
    append_outputs: u32,
    base_cell_deps: u32,
    base_header_deps: u32,
) -> Vec<u8> {
    let input_mask = vec![0; ((base_inputs as usize) * 2).div_ceil(8)];
    let output_mask = vec![0; ((base_outputs as usize) * 4).div_ceil(8)];
    let cell_dep_mask = vec![0; (base_cell_deps as usize).div_ceil(8)];
    let header_dep_mask = vec![0; (base_header_deps as usize).div_ceil(8)];
    otx_witness_custom(
        0,
        base_inputs,
        &input_mask,
        base_outputs,
        &output_mask,
        base_cell_deps,
        &cell_dep_mask,
        base_header_deps,
        &header_dep_mask,
        append_inputs,
        append_outputs,
        0,
        0,
    )
}

fn otx_witness_custom(
    append_permissions: u8,
    base_inputs: u32,
    base_input_mask: &[u8],
    base_outputs: u32,
    base_output_mask: &[u8],
    base_cell_deps: u32,
    base_cell_dep_mask: &[u8],
    base_header_deps: u32,
    base_header_dep_mask: &[u8],
    append_inputs: u32,
    append_outputs: u32,
    append_cell_deps: u32,
    append_header_deps: u32,
) -> Vec<u8> {
    witness_union(
        0xff00_0003,
        &table(&[
            empty_message(),
            vec![append_permissions],
            base_inputs.to_le_bytes().to_vec(),
            molecule_bytes(base_input_mask),
            base_outputs.to_le_bytes().to_vec(),
            molecule_bytes(base_output_mask),
            base_cell_deps.to_le_bytes().to_vec(),
            molecule_bytes(base_cell_dep_mask),
            base_header_deps.to_le_bytes().to_vec(),
            molecule_bytes(base_header_dep_mask),
            append_inputs.to_le_bytes().to_vec(),
            append_outputs.to_le_bytes().to_vec(),
            append_cell_deps.to_le_bytes().to_vec(),
            append_header_deps.to_le_bytes().to_vec(),
            empty_dynvec(),
        ]),
    )
}
```

- [ ] **Step 3: Run layout tests**

Run:

```bash
cargo test -p cobuild-core --offline --test layout
```

Expected: PASS. If any new test unexpectedly passes before implementation,
keep it as regression coverage.

- [ ] **Step 4: Commit**

```bash
git add crates/cobuild-core/tests/layout.rs
git commit -m "test: cover cobuild otx layout failures"
```

## Task 5: Expand Core Task And Seal Negative Coverage

**Files:**
- Modify: `crates/cobuild-core/tests/tasks.rs`

- [ ] **Step 1: Add task fail-closed tests**

Append these tests to `crates/cobuild-core/tests/tasks.rs`:

```rust
#[test]
fn otx_task_rejects_missing_required_seal_pair() {
    let target_lock = [1u8; 32];
    let context = otx_context(target_lock, &[]);
    let parts = otx_hash_parts();

    assert_eq!(
        context.lock_query(target_lock).otx_tasks(&parts),
        Err(CoreError::MissingSealPair)
    );
}

#[test]
fn otx_task_rejects_duplicate_required_seal_pair() {
    let target_lock = [1u8; 32];
    let context = otx_context(
        target_lock,
        &[
            seal_pair(target_lock, 0, &[7u8; 65]),
            seal_pair(target_lock, 0, &[8u8; 65]),
        ],
    );
    let parts = otx_hash_parts();

    assert_eq!(
        context.lock_query(target_lock).otx_tasks(&parts),
        Err(CoreError::DuplicateSealPair)
    );
}

#[test]
fn otx_task_rejects_invalid_seal_scope() {
    let target_lock = [1u8; 32];
    let context = otx_context(target_lock, &[seal_pair(target_lock, 2, &[7u8; 65])]);
    let parts = otx_hash_parts();

    assert_eq!(
        context.lock_query(target_lock).otx_tasks(&parts),
        Err(CoreError::InvalidLayout)
    );
}

#[test]
fn otx_task_rejects_invalid_message_action_role() {
    let target_lock = [1u8; 32];
    let context = otx_context_with_message(
        target_lock,
        &message_with_action(9, target_lock),
        &[seal_pair(target_lock, 0, &[7u8; 65])],
    );
    let parts = otx_hash_parts();

    assert_eq!(
        context.lock_query(target_lock).otx_tasks(&parts),
        Err(CoreError::InvalidMessageTarget)
    );
}

#[test]
fn tx_level_unrelated_malformed_witness_does_not_force_cobuild_flow() {
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: vec![sighash_all_only_witness(&[7u8; 65]), vec![1, 2, 3, 4]],
            input_count: 2,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![[1u8; 32], [2u8; 32]],
            input_types: vec![None, None],
            output_types: Vec::new(),
        },
    )
    .unwrap();
    let parts = TxHashParts {
        tx_hash: [0u8; 32],
        resolved_inputs: Vec::new(),
        trailing_witnesses: Vec::new(),
    };

    assert_eq!(
        context.lock_query([1u8; 32]).tx_tasks(&parts).unwrap().len(),
        1
    );
}
```

- [ ] **Step 2: Add shared task helpers**

Add these helpers near the existing OTX helper functions in
`crates/cobuild-core/tests/tasks.rs`:

```rust
fn otx_context(target_lock: [u8; 32], seals: &[Vec<u8>]) -> CobuildContext {
    otx_context_with_message(target_lock, &empty_message(), seals)
}

fn otx_context_with_message(
    target_lock: [u8; 32],
    message: &[u8],
    seals: &[Vec<u8>],
) -> CobuildContext {
    CobuildContext::with_raw_parts(
        LayoutTx {
            witnesses: vec![otx_start_witness(), otx_witness(message, seals)],
            input_count: 1,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![target_lock],
            input_types: vec![None],
            output_types: Vec::new(),
        },
        RawTxParts {
            inputs: vec![Vec::new()],
            ..RawTxParts::default()
        },
    )
    .unwrap()
}

fn otx_hash_parts() -> TxHashParts {
    TxHashParts {
        tx_hash: [0u8; 32],
        resolved_inputs: vec![ResolvedInputHashPart {
            output: Vec::new(),
            data: Vec::new(),
        }],
        trailing_witnesses: Vec::new(),
    }
}
```

- [ ] **Step 3: Run task tests**

Run:

```bash
cargo test -p cobuild-core --offline --test tasks
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add crates/cobuild-core/tests/tasks.rs
git commit -m "test: cover cobuild task seal failures"
```

## Task 6: Add Contract Integration Coverage For Malformed OTX Mapping

**Files:**
- Modify: `tests/src/lib.rs`
- Modify: `tests/tests/cobuild_otx_lock.rs`

- [ ] **Step 1: Add malformed-layout integration coverage**

Append this test to `tests/tests/cobuild_otx_lock.rs`:

```rust
#[test]
fn contract_rejects_malformed_otx_layout_as_malformed_cobuild() {
    let result = fixtures::malformed_otx_layout_case().verify();
    assert_lock_script_exit(result, 2);
}
```

- [ ] **Step 2: Add the malformed OTX fixture**

In `tests/src/lib.rs`, inside `pub mod fixtures`, add this public fixture near
`malformed_cobuild_witness_case()`:

```rust
pub fn malformed_otx_layout_case() -> Case {
    signed_otx_case_with_options(false, false, Some(0x10))
}
```

Then replace the existing `signed_otx_case` function signature:

```rust
fn signed_otx_case(include_tx_level: bool, corrupt_append_seal: bool) -> Case {
```

with:

```rust
fn signed_otx_case(include_tx_level: bool, corrupt_append_seal: bool) -> Case {
    signed_otx_case_with_options(include_tx_level, corrupt_append_seal, None)
}

fn signed_otx_case_with_options(
    include_tx_level: bool,
    corrupt_append_seal: bool,
    override_append_permissions: Option<u8>,
) -> Case {
```

Inside the new function, replace:

```rust
append_permissions: 0x01,
```

with:

```rust
append_permissions: override_append_permissions.unwrap_or(0x01),
```

This creates an OTX with reserved append permission bits while preserving the
same signed fixture construction.

- [ ] **Step 3: Build contract and verify the new test fails before mapping fix if needed**

If Task 3 has not been applied, run:

```bash
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock contract_rejects_malformed_otx_layout_as_malformed_cobuild -- --nocapture
```

Expected before Task 3: FAIL with exit code 3. Expected after Task 3: PASS.

- [ ] **Step 4: Run full contract integration tests**

Run:

```bash
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/src/lib.rs tests/tests/cobuild_otx_lock.rs
git commit -m "test: cover malformed otx contract exit mapping"
```

## Task 7: Run Final Verification Matrix

**Files:**
- No code changes expected.

- [ ] **Step 1: Run codegen check**

```bash
cargo run -p xtask --offline -- codegen cobuild-types --check
```

Expected: PASS.

- [ ] **Step 2: Run workspace tests**

```bash
cargo test --workspace --offline
```

Expected: PASS.

- [ ] **Step 3: Run contract build**

```bash
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
```

Expected: PASS.

- [ ] **Step 4: Run contract integration tests**

```bash
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Run boundary grep**

```bash
rg -n "cobuild_types::entity|::entity::" crates/cobuild-core contracts/cobuild-otx-lock
rg -n "critical-section|portable-atomic.*unsafe-assume-single-core|\\[patch.crates-io\\]" Cargo.toml crates contracts
```

Expected: no matches.

- [ ] **Step 6: Inspect git status**

```bash
git status --short
```

Expected: only intentional changes are present. Pre-existing unrelated changes
such as `.gitignore` or `rust-toolchain.toml` must not be staged unless the user
explicitly asks.

- [ ] **Step 7: Confirm no final commit is needed**

Each implementation task above creates its own focused commit. If Step 6 shows
only pre-existing unrelated files such as `.gitignore` or `rust-toolchain.toml`,
do not create another commit.

## Self-Review

- Spec coverage:
  - Verification matrix: Task 7.
  - `no_entity_dependency` path failure: Task 1.
  - `xtask --check` panic: Task 2.
  - runner test nondeterminism: Task 3.
  - Core error mapping: Task 3.
  - Core negative coverage: Tasks 4 and 5.
  - Contract malformed OTX public exit coverage: Task 6.
  - Boundary checks: Task 7.
- Completeness scan:
  - No incomplete instructions are intentionally left.
- Type consistency:
  - `map_core_error` remains private and is tested from an internal
    `#[cfg(test)]` module in `runner.rs`.
  - `CoreError` variants match `crates/cobuild-core/src/error.rs`.
  - `Error` variants match `contracts/cobuild-otx-lock/src/error.rs`.
