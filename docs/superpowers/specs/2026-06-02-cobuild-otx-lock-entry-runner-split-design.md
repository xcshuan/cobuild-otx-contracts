# Cobuild OTX Lock Entry Runner Split Design

## Status

This is a focused refactor for `contracts/cobuild-otx-lock`.

## Problem

`entry.rs` currently delegates all contract work to `runner::run`, while
`runner.rs` mixes entry orchestration, syscall loading, context preparation,
verification error mapping, and unit tests. This makes the contract entry file
effectively meaningless and leaves unrelated responsibilities in one module.

## Goals

- Move the core contract entry flow back into `entry.rs`.
- Keep syscall loading and context preparation outside `entry.rs`.
- Keep error mapping tests close to the error mapping functions.
- Preserve public exit codes, task semantics, verification behavior, and tests.
- Avoid moving Cobuild protocol logic into the lock crate.

## Design

`entry.rs` should own the readable high-level flow:

1. load and parse current script auth args;
2. load current script hash;
3. prepare Cobuild context from chain data;
4. query tx-level and OTX lock tasks;
5. reject empty task sets with `LockSemanticFailure`;
6. verify every task with `LocalVerifier`.

`chain.rs` should own syscall-backed loading:

- current script args;
- current script hash;
- transaction bytes;
- input lock/type/output type hashes;
- resolved input output/data bytes;
- prepared Cobuild context.

`errors.rs` should own conversions from `SysError`, `CoreError`, and
`VerifyError` to the contract `Error` enum. Existing error mapping unit tests
move with these functions.

`runner.rs` should be removed unless a compatibility shim is required by tests.
The crate module exports should reflect the new boundary.

## Tests

Add or update structural tests so the refactor cannot regress into the old
shape:

- `entry.rs` must not delegate to `runner::run`;
- `runner.rs` should no longer exist or be exported;
- `entry.rs` should reference the concrete high-level flow helpers;
- existing error mapping unit tests must keep passing from their new module.

Existing workspace and contract integration tests must continue passing.

