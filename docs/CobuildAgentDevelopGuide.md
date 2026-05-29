# Cobuild Agent Develop Guide

This guide captures the working context and guardrails for agents continuing work in this sub-repository.

## Repository Scope

Work only inside this repository:

```text
/home/xcshuan/contracts/ckb/cobuild-otx-contracts/cobuild-otx-contracts
```

The parent repository and `../ref` are reference-only unless a human explicitly says otherwise. Do not make edits outside this sub-repository.

## Primary Documents

Read these before making behavior changes:

- `docs/superpowers/specs/2026-05-29-clean-cobuild-otx-contracts-design.md`
- `docs/superpowers/plans/2026-05-29-clean-cobuild-otx-contracts-implementation-plan.md`
- `docs/superpowers/specs/2026-05-28-cobuild-core-community-redraft-design.md`
- `docs/superpowers/specs/2026-05-29-cobuild-otx-lock-design.md`

The implementation plan is already marked complete. New work should be handled as a new focused task, not by silently rewriting the completed plan history.

## Architecture Boundaries

- `cobuild-types` keeps the crate name and exposes both:
  - `cobuild_types::lazy_reader`
  - `cobuild_types::entity`
- Chain-facing `cobuild-core` code must use `cobuild_types::lazy_reader`, not `cobuild_types::entity`.
- Host tests may use `entity` builders, but that dependency must not enter normal contract paths.
- `cobuild-core` owns Cobuild protocol logic:
  - lazy-reader boundary views;
  - witness parsing;
  - OTX layout scanning;
  - signing hash construction;
  - tx-level and OTX-level task generation;
  - message action target validation.
- `cobuild-otx-lock` stays thin:
  - load current script args and script hash;
  - load transaction context;
  - query `cobuild-core` tasks;
  - invoke verifier;
  - map errors to stable exit codes.
- The lock contract must not parse Cobuild protocol details, scan OTX layouts, construct Cobuild hash preimages, or depend on `cobuild_types::entity`.
- Do not add a local `critical-section` shim.
- Do not enable `portable-atomic` unsafe single-core assumptions.
- Contract fixtures use `ScriptHashType::Data2` for `cobuild-otx-lock`.

## Raw Transaction And Syscall Boundary

The Cobuild OTX hash path needs both raw transaction fields and resolved input data:

- raw transaction fields come from `load_transaction` and are parsed through lazy readers;
- resolved input cell output and data come from syscalls such as `load_cell` and `load_cell_data`;
- raw transaction data does not contain resolved input data.

This mirrors the reference POC split: raw tx lazy reads are appropriate for transaction fields such as inputs, outputs, output data, cell deps, and header deps; syscall-resolved data is still required for previous input cells.

## OTX Rules To Preserve

- Reject `Otx` witnesses before `OtxStart`.
- Reject duplicated `OtxStart`.
- Reject non-contiguous `Otx` witnesses after `OtxStart`.
- Require at least one `Otx` after `OtxStart`.
- Validate append permissions and masks before generating tasks.
- Validate every non-empty `Message.Action` target:
  - `script_role = 0`: hash must match an input lock hash;
  - `script_role = 1`: hash must match an input type hash;
  - `script_role = 2`: hash must match an output type hash;
  - other roles fail closed.
- OTX base hash includes local indices for covered base inputs, outputs, cell deps, and header deps.
- OTX append hash includes local indices for appended inputs, outputs, cell deps, and header deps.

## Common Commands

Codegen check:

```bash
cargo run -p xtask --offline -- codegen cobuild-types --check
```

Regenerate committed Cobuild type outputs:

```bash
cargo run -p xtask --offline -- codegen cobuild-types
```

Workspace tests:

```bash
cargo test --workspace --offline
```

Contract build and integration tests:

```bash
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Boundary checks:

```bash
rg -n "cobuild_types::entity|::entity::" crates/cobuild-core contracts/cobuild-otx-lock
rg -n "critical-section|portable-atomic.*unsafe-assume-single-core|\[patch.crates-io\]" Cargo.toml crates contracts
```

Both boundary commands should print no matches.

## Build Notes

- The root `Makefile` supports single-contract builds:

```bash
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
```

- The contract was originally scaffolded from the CKB script template flow. For new contracts, use:

```bash
make generate CRATE=<contract-name>
```

- `scripts/find_clang` is used by Makefile builds. In this environment it resolves to versioned LLVM tools such as `/usr/bin/clang-17`.
- Integration tests default to `build/debug` when `MODE` is unset. This avoids accidentally testing stale `build/release` binaries during `cargo test --workspace --offline`.
- Generated lazy-reader code currently emits warnings. Treat those warnings as non-blocking unless a task explicitly asks to clean generated output.

## Development Workflow

- Prefer test-driven changes:
  1. add or update a failing test;
  2. run the targeted test and confirm the expected failure;
  3. implement the smallest fix;
  4. rerun the targeted test;
  5. rerun the relevant matrix.
- Keep commits focused. Commit after a coherent task or verification fix.
- For substantial work, request a focused code review before finalizing.
- Never revert unrelated user changes. Inspect `git status --short` before staging.
- Use `apply_patch` or normal editor-grade edits. Avoid broad formatting sweeps unless formatting is the task.

## Useful Test Areas

- `crates/cobuild-types/tests`: generated module exposure and entity/lazy-reader sanity.
- `crates/cobuild-core/tests/hash.rs`: signing hash and OTX hash regression coverage.
- `crates/cobuild-core/tests/layout.rs`: OTX sequence and layout behavior.
- `crates/cobuild-core/tests/tasks.rs`: tx-level and OTX task generation behavior.
- `crates/cobuild-core/tests/no_entity_dependency.rs`: chain dependency boundary.
- `contracts/cobuild-otx-lock/tests`: args, error code, runner, and verifier unit tests.
- `tests/tests/cobuild_otx_lock.rs`: end-to-end contract behavior.

## Current Completion State

The clean Cobuild OTX implementation plan has been completed through final verification. The latest known verification set included:

```bash
cargo run -p xtask --offline -- codegen cobuild-types --check
cargo test --workspace --offline
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

If a future change touches protocol behavior, rerun the full set above before claiming completion.
