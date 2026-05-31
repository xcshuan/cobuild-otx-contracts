# Cobuild Validation And Safety Hardening Design

## Status

This document defines the first optimization phase for the current clean
Cobuild OTX implementation.

It follows the completed clean implementation baseline, but narrows this phase
to verification reliability and protocol safety behavior. It intentionally does
not redesign the lazy-reader lifetime boundary or replace owned buffers in the
hash hot path.

## Related Baselines

- `docs/superpowers/specs/2026-05-28-cobuild-core-community-redraft-design.md`
- `docs/superpowers/specs/2026-05-29-clean-cobuild-otx-contracts-design.md`
- `docs/superpowers/specs/2026-05-29-cobuild-otx-lock-design.md`
- `docs/CobuildAgentDevelopGuide.md`

If this document conflicts with the Core protocol spec, the Core protocol spec
wins.

## Problem Statement

The current implementation has the right broad shape: `cobuild-core` owns
Cobuild protocol logic, `cobuild-otx-lock` remains a thin orchestration and
verification crate, and the contract integration happy paths pass after a debug
contract build.

However, the implementation is not yet ready to treat as a stable baseline
because the verification loop is not reliable:

- `cargo test --workspace --offline` fails in source-boundary tests because the
  tests build source paths incorrectly.
- `cargo test -p cobuild-otx-lock --offline` fails in the runner test because
  it assumes a host binary environment variable that Cargo does not provide in
  this package shape.
- `cargo run -p xtask --offline -- codegen cobuild-types --check` panics during
  code generation, so generated-output drift cannot be checked reliably.
- contract error mapping currently collapses protocol layout failures into the
  same public category as lock-local semantic failures.
- several fail-closed Core v1 behaviors are only partially protected by tests.

These issues make future protocol changes hard to trust. The first optimization
phase must make verification deterministic and tighten protocol failure tests
before doing larger production refactors.

## Goals

- Make the documented verification matrix runnable and deterministic.
- Preserve the existing crate boundary:
  - `cobuild-core` owns witness parsing, OTX layout, message target validation,
    hash construction, and task derivation.
  - `cobuild-otx-lock` owns script args, syscall loading, task orchestration,
    verifier dispatch, and public exit-code mapping.
- Make public lock error categories match the design intent:
  - malformed Cobuild encoding/layout errors map to `MalformedCobuild`;
  - missing relevant tasks and lock-local semantic failures map to
    `LockSemanticFailure`;
  - verifier failures map to `VerifyFailure`;
  - syscall failures map to `SyscallFailure`;
  - impossible internal hash-input gaps map to `InternalFailure`.
- Add focused negative tests for high-risk Core v1 fail-closed rules.
- Keep the lock contract Cobuild-only in Phase 1.
- Keep all changes small enough to review independently.

## Non-Goals

- Removing the `unsafe` lifetime erasure in `cobuild-core::view`.
- Replacing owned `Vec` hash inputs with fully borrowed cursor/slice views.
- Adding `ckb-auth`.
- Adding legacy `WitnessArgs` fallback.
- Redesigning schemas, witness variants, or Core hash preimages.
- Changing the 21-byte Phase 1 lock args ABI.

The `unsafe` and owned-copy concerns remain important, but they should be
handled in a later productionization phase after the validation matrix is
trustworthy.

## Verification Matrix

This phase treats the following commands as the minimum success matrix:

```bash
cargo run -p xtask --offline -- codegen cobuild-types --check
cargo test --workspace --offline
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

The generated lazy-reader warnings are allowed to remain unless a task
explicitly touches generated-output warning cleanup. A passing command with
known generated warnings is acceptable.

## Test And Tooling Fixes

### `cobuild-core` Source-Boundary Tests

The entity-boundary test must read source files relative to the crate manifest
directory correctly. It should continue to assert that normal `cobuild-core`
source files do not import `cobuild_types::entity` and that generated reader
internals are not publicly exposed through the view boundary.

The test must be robust when invoked from either the workspace root or the
crate directory.

### `cobuild-otx-lock` Runner Test

The runner test must stop depending on `env!("CARGO_BIN_EXE_cobuild-otx-lock")`
unless the package configuration reliably produces that binary for integration
tests.

The preferred fix is to turn the test into a library-level runner/orchestration
test that exercises public error mapping or a small test-only runner seam. If a
host binary test remains, it must be made deterministic under
`cargo test -p cobuild-otx-lock --offline`.

The test should not require a built RISC-V contract binary. Contract binary
behavior belongs to the workspace `tests` integration crate.

### `xtask` Codegen Check

`xtask` check mode must generate both lazy-reader and entity outputs into the
check directory and compare them with committed outputs.

The check command must return a structured error with context instead of
panicking. If molecule-codegen requires schema include paths or current working
directory assumptions, `xtask` must satisfy them explicitly.

The check path must leave committed generated files unchanged.

## Error Mapping

The lock contract should expose the small stable exit-code categories already
defined by `contracts/cobuild-otx-lock/src/error.rs`.

The mapping from `CoreError` should be revised as follows:

- `MalformedCobuild` -> `Error::MalformedCobuild`
- `InvalidLayout` -> `Error::MalformedCobuild`
- `InvalidMessageTarget` -> `Error::MalformedCobuild`
- `MissingSealPair` -> `Error::MalformedCobuild`
- `DuplicateSealPair` -> `Error::MalformedCobuild`
- `MissingHashParts` -> `Error::InternalFailure`

Rationale:

- `InvalidLayout`, invalid message targets, missing required seals, and
  duplicate required seals are malformed or invalid Cobuild protocol data from
  the lock's perspective.
- `LockSemanticFailure` should be reserved for lock-local policy outcomes such
  as no relevant Cobuild task in Cobuild-only mode.
- `MissingHashParts` indicates an internal loader/context mismatch after the
  lock has already committed to building a Core context, so it remains an
  internal failure.

If later work adds finer-grained `CoreError` variants, the public mapping must
remain category-based and stable.

## Core Negative Coverage

This phase should add or strengthen tests for the following Core v1 rules:

- Reject duplicate `OtxStart` witnesses.
- Reject valid `Otx` witnesses before `OtxStart`.
- Reject non-contiguous `Otx` sequences after `OtxStart`.
- Reject `OtxStart` with no following `Otx`.
- Reject `Otx` with `base_input_cells == 0`.
- Reject reserved `append_permissions` bits.
- Reject append counts when the corresponding append permission bit is zero.
- Reject invalid mask lengths.
- Reject non-zero reserved padding bits in masks.
- Reject missing required `SealPair`.
- Reject duplicate `SealPair` for the same `(script_hash, scope)`.
- Reject invalid `SealPair.scope`.
- Reject invalid or missing `Message.Action` targets when a relevant message is
  consumed.
- Keep unrelated malformed non-Cobuild witnesses from forcing unrelated scripts
  into Cobuild flow.

Tests should be placed at the lowest useful layer:

- layout-only rules in `crates/cobuild-core/tests/layout.rs`;
- task and seal-selection rules in `crates/cobuild-core/tests/tasks.rs`;
- lock public exit-code mapping in `contracts/cobuild-otx-lock/tests/error.rs`
  or focused runner tests;
- full contract behavior only in `tests/tests/cobuild_otx_lock.rs` when syscall
  or binary execution behavior is required.

## Contract Integration Coverage

The existing contract integration tests already cover:

- invalid args;
- missing relevant task;
- tx-level signing;
- OTX base and append signing;
- mixed tx-level and OTX tasks;
- bad seal;
- malformed related Cobuild witness.

This phase should add integration coverage only where unit tests cannot prove
the behavior. At minimum, it should verify the public exit code for a malformed
OTX layout case that previously would have mapped to `LockSemanticFailure`.

Avoid duplicating every Core negative case at integration level. Core unit tests
should carry the broad protocol matrix; contract tests should prove syscall
loading, public exit mapping, and verifier dispatch.

## Simplicity And Boundary Requirements

- Do not move Cobuild parsing, OTX scanning, hash construction, or seal matching
  into `cobuild-otx-lock`.
- Do not introduce a provider trait or framework-style abstraction to fix these
  issues.
- Keep new test builders local and small. If duplicate witness-builder helpers
  become noisy, extract only the minimum helper needed inside the relevant test
  module.
- Do not change committed generated files unless the codegen task demonstrates
  that the current generated files are stale.
- Do not broaden dependencies or add a local atomic/critical-section shim.

## Risks

Changing `CoreError` mapping may require updating integration tests that assert
numeric exit codes. That is expected when the previous numeric behavior was not
aligned with the design.

Fixing `xtask` may expose generated-output drift. If drift exists, the
implementation plan should make that explicit and regenerate committed outputs
in a separate, reviewable task.

Some negative tests may require additional small witness-builder helpers. Those
helpers should remain test-only and must not leak into production code.

## Success Criteria

This phase is complete when:

- the full verification matrix in this document passes;
- `cargo test --workspace --offline` passes without excluding packages;
- `xtask` check mode no longer panics and detects generated-output drift;
- public lock exit-code tests reflect the revised error categories;
- Core negative tests cover the listed high-risk fail-closed rules;
- static boundary checks still show no `cobuild_types::entity` dependency in
  `crates/cobuild-core` or `contracts/cobuild-otx-lock` production code.

## Deferred Phase 2

A later productionization phase should address:

- removing or narrowing `unsafe` lifetime erasure in `cobuild-core::view`;
- reducing owned-buffer copies in witness views and hash input structures;
- evaluating cycle and memory impact of current loader/hash paths;
- preparing a verifier adapter for future `ckb-auth` integration.
