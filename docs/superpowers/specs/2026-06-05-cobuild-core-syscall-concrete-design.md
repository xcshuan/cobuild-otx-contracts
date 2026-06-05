# Cobuild Core Syscall Concrete Design

> Superseded implementation note (2026-06-05): this design was implemented and
> then refined by
> `docs/superpowers/plans/2026-06-05-cobuild-core-flow-objects-plan.md`.
> References below to `CobuildEngine`, `PreparedCobuild`, and
> `ScriptHashIndex` are historical. The current API is
> `CobuildContext::from_syscalls()` with concrete flow objects
> `SyscallTxReader`, `TxScriptHashes`, `WitnessScan`, `LockPlanBuilder`, and
> `TypePlanBuilder`.

## Context

The current implementation keeps `cobuild-core` independent from `ckb-std` by routing transaction data through `TransactionSource`, `HashInputSource`, `ClassifiedCursor`, and `InMemorySource`. That split is no longer a design goal. This repository targets CKB contracts, and `cobuild-core` may depend on `ckb-std` directly.

The refactor should remove the source-trait compatibility layer instead of preserving legacy APIs. `LockVerifier` is intentionally left in place for now.

## Goals

- Make the production Cobuild path concrete and syscall-backed.
- Move syscall transaction reading from `cobuild-otx-lock` into `cobuild-core`.
- Delete the old source abstraction layer and any compatibility wrappers.
- Keep lock contract entry code thin: parse lock args, prepare/plan through core, verify signatures.
- Preserve current Cobuild behavior and error categories.

## Non-Goals

- Do not preserve historical compatibility for `TransactionSource`, `HashInputSource`, `InMemorySource`, or old lock `chain` module APIs.
- Do not remove `LockVerifier` in this refactor.
- Do not merge `cobuild-core` into the lock contract crate.
- Do not reintroduce eager full-transaction loading into `Vec`.

## Architecture

`cobuild-core` becomes the owner of syscall-backed transaction reading. It directly depends on `ckb-std` and exposes concrete engine APIs:

```rust
let prepared = CobuildEngine::prepare_from_syscalls()?;
let plan = prepared.plan_lock_validation(current_script_hash)?;
```

The lock contract no longer constructs or stores a transaction reader. `PreparedCobuild` internally uses core syscall helpers when it needs signing hash payloads during lock/type planning.

## Components

### `cobuild-core::syscalls`

Add a focused internal module for CKB transaction access:

- syscall-backed `Cursor` construction for transaction, resolved input cell output, and resolved input data;
- transaction count loading with a small counts cache where needed;
- raw input/output/output-data/cell-dep/header-dep access through lazy readers;
- witness cursor access;
- transaction hash and input/output script hash helpers through `ckb_std::high_level`;
- explicit error mapping to `CoreError`.

This module replaces the current lock crate `chain.rs` and `chain/reader.rs` production logic.

`script_cursor` is not carried forward. The lock crate loads current script args itself, and core preparation does not need a current-script cursor.

### `cobuild-core::engine`

Change engine methods from generic source-driven APIs to concrete syscall APIs:

- `CobuildEngine::prepare_from_syscalls() -> Result<PreparedCobuild, CoreError>`
- `PreparedCobuild::plan_lock_validation(lock_script_hash) -> Result<LockValidationPlan, CoreError>`
- `PreparedCobuild::plan_type_validation(type_script_hash) -> Result<TypeValidationPlan, CoreError>`

Preparation still collects transaction counts, script hash index, witness summaries, and OTX layout scan. Planning still calculates tx-level and OTX signing requirements.

### `cobuild-core::hash`

Replace `HashInputSource` parameters with calls into core syscall helpers. Hash construction remains streaming/lazy and must not load the whole transaction into an owned transaction blob.

The `hash` module becomes an internal core module. It must not expose public functions that take internal syscall types such as a counts cache.

### `cobuild-otx-lock`

Remove the `chain` module. The entry flow becomes:

1. Load current script args and parse `AuthContext`.
2. Load current script hash.
3. Call `CobuildEngine::prepare_from_syscalls()`.
4. Call `plan_lock_validation(current_script_hash)`.
5. Verify each required signature through the existing `LockVerifier` path.

## Deleted APIs And Modules

Delete these production APIs and all architecture guards that require them:

- `crates/cobuild-core/src/source.rs`
- `pub mod source`
- `TransactionSource`
- `HashInputSource`
- `InMemorySource`
- `ClassifiedCursor`
- `CursorReadContext`
- `WitnessCursorSource`
- generic `build_layout_from_witnesses` / `scan_layout_from_witnesses` APIs backed by witness source traits
- `PreparedCobuildContext`
- `SyscallTxReader` as a lock crate type
- lock crate `chain.rs`
- lock crate `chain/reader.rs`

Any test-only helpers must live in test modules or integration test support code and must not recreate the deleted production abstraction layer.

## Error Handling

The refactor keeps the existing public error categories:

- malformed Cobuild protocol data maps to `CoreError::MalformedCobuild`;
- invalid transaction/script context maps to `CoreError::InvalidContextInput`;
- missing signing hash payloads map to `CoreError::MissingHashInput`;
- lock contract error code mapping remains unchanged.

The new syscall helpers should map syscall and lazy-reader failures directly at the read site. Do not preserve `ClassifiedCursor` just to carry read context.

Expected helper-level mapping:

- protocol/view reads from Cobuild messages, masks, seals, and witness layout bodies map to `CoreError::MalformedCobuild`;
- transaction hash and script hash index helpers map syscall failures to `CoreError::InvalidContextInput`;
- signing preimage payload helpers for witnesses, raw transaction fields, resolved input outputs/data, cell deps, and header deps map failures to `CoreError::MissingHashInput`.

## Cargo Features

`cobuild-core` should add:

```toml
ckb-std = { version = "1.1", default-features = false, features = ["ckb-types", "dummy-atomic"] }
```

`allocator` stays on `cobuild-otx-lock`, where the contract allocator is configured. If host/native-simulator builds require `ckb-std/native-simulator` after moving syscalls into core, add a `native-simulator` feature to `cobuild-core` and have `cobuild-otx-lock/native-simulator` enable it. The final workspace and native simulator verification commands must prove the chosen feature wiring.

## Testing

Update tests to match the concrete syscall design:

- Architecture guard tests must reject the deleted source abstractions and lock `chain` module.
- Core unit tests should keep protocol/layout coverage using local byte fixtures or test-only helpers.
- Contract integration support must replace direct `cobuild-core::hash` / `InMemorySource` usage with test-only signing hash helpers. These helpers must not be exported from production `cobuild-core`.
- Integration tests should continue to cover end-to-end contract behavior through native simulator/debug contract builds.
- The final verification set must include:
  - `cargo clippy --workspace --all-targets --offline`
  - `cargo test --workspace --offline`
  - `make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline`
  - `MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture`
  - `git diff --check`

## Migration Notes

This is a breaking cleanup. Do not add aliases, shims, or deprecated wrappers for removed names. Update all call sites and tests to the new concrete syscall API in the same change set.
