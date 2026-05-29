# Cobuild OTX Lock Design

## Status

This document defines the design baseline for the new `cobuild-otx-lock`
contract crate in this workspace.

Unless explicitly revised, this document is the authoritative design target for
the first implementation phase of `cobuild-otx-lock`.

## Related Baselines

This design builds on the following approved baselines:

- [2026-05-28-cobuild-core-community-redraft-design.md](/home/xcshuan/contracts/ckb/cobuild-otx-contracts/docs/superpowers/specs/2026-05-28-cobuild-core-community-redraft-design.md:1)
- [2026-05-28-cobuild-shared-modules-and-types-design.md](/home/xcshuan/contracts/ckb/cobuild-otx-contracts/docs/superpowers/specs/2026-05-28-cobuild-shared-modules-and-types-design.md:1)
- [2026-05-28-cobuild-shared-modules-and-types-implementation-plan.md](/home/xcshuan/contracts/ckb/cobuild-otx-contracts/docs/superpowers/plans/2026-05-28-cobuild-shared-modules-and-types-implementation-plan.md:1)

If this document conflicts with the Core protocol spec, the Core protocol spec
wins. If this document conflicts with the shared Rust module boundary, the Core
protocol spec still wins and this document must be revised.

## 1. Goals And Scope

### Goal

`cobuild-otx-lock` is a contract-first, `no_std`, lock-script crate that
consumes the finalized Cobuild Core protocol and performs lock-side signature
verification for Cobuild tx-level and OTX-level flows.

Its purpose is to provide a production-oriented lock-side integration point
with:

- clear contract responsibilities;
- minimal lock-local logic;
- stable interfaces for future verifier replacement;
- low conceptual overhead for auditing.

### In Scope

Phase 1 of `cobuild-otx-lock` includes:

- consuming `cobuild-core` task/query outputs from contract code;
- parsing current lock arguments into a lock-local authentication context;
- collecting relevant tx-level and OTX-level signing tasks for the current
  lock script hash;
- calling a narrow verifier boundary for each relevant task;
- failing closed on relevant Cobuild protocol errors, verifier errors, and
  lock-local semantic errors;
- defining stable seams for future `ckb-auth` integration without taking that
  dependency yet.

### Out Of Scope

`cobuild-otx-lock` does not:

- redefine Cobuild witness, layout, hash, or task rules;
- interpret application-specific `Action.data`;
- provide off-chain builder, packet, wallet, or agent abstractions;
- introduce a provider trait or full owned transaction view abstraction;
- require chain-off-chain API unification;
- integrate `ckb-auth` in Phase 1.

### Cobuild-Only Position

Phase 1 is explicitly Cobuild-only.

This crate is designed to validate Cobuild tasks. It does not implement legacy
`WitnessArgs` fallback in Phase 1. If no relevant Cobuild task exists for the
current lock execution, the lock fails closed.

This choice narrows the design surface, keeps the execution path auditable, and
avoids coupling the first version to legacy coexistence logic.

## 2. Crate Boundary

### Physical Placement And Template Convention

`cobuild-otx-lock` must be implemented as a contract crate under:

- `contracts/cobuild-otx-lock`

This is a hard layout constraint, not a cosmetic preference.

Phase 1 should follow `ckb-script-template` workspace conventions for contract
projects:

- Rust contracts live under `contracts/`;
- a workspace-level `Makefile` owns contract build entrypoints;
- `scripts/find_clang` is used for native toolchain discovery;
- the contract crate keeps its own template-style `Makefile` for
  `riscv64imac-unknown-none-elf` builds;
- host-side script verification tests live in the workspace `tests` crate.

The repository already contains non-contract dependency crates under
`crates/`. That remains valid. The constraint is only that the new lock
contract itself must be bootstrapped and maintained as a contract crate under
`contracts/`, not as a dependency crate under `crates/`.

### Dependency Direction

`cobuild-otx-lock` should directly depend on:

- `cobuild-core`
- `ckb-std`

It may additionally depend on `cobuild-types` for contract-local tests or
fixture helpers, but production validation flow should not require bypassing
`cobuild-core` to reach raw Cobuild entities.

The intended dependency shape is:

```text
cobuild-otx-lock -> cobuild-core -> cobuild-types
cobuild-otx-lock -> ckb-std
```

### What Must Be Reused From `cobuild-core`

`cobuild-otx-lock` must reuse `cobuild-core` for:

- witness parsing and `WitnessLayout` recognition;
- OTX start detection and OTX witness segmentation;
- OTX layout and scope partitioning;
- tx-level and OTX-level signing hash construction;
- lock query and task derivation;
- `SealPair` scope handling;
- Core-level message target existence checks;
- Core-level malformed witness, layout, and hash-input validation.

The lock crate should consume:

- `CobuildContext`
- `LockScriptQuery`
- `TxLevelLockTask`
- `OtxLockTask`

### What Must Stay In The Lock Crate

`cobuild-otx-lock` should keep only lock-specific logic:

- current script args parsing;
- current script hash loading;
- lock-local authentication context construction;
- orchestration of task collection and verification;
- verifier invocation;
- lock-local error mapping and script return semantics.

### What Must Not Be Reimplemented

The following logic must not be duplicated inside `cobuild-otx-lock`:

- OTX witness parsing;
- OTX layout derivation;
- Core hash preimage construction;
- `SealPair` matching rules;
- message target existence validation;
- base/append scope splitting;
- tx-level `SighashAll` and `SighashAllOnly` selection logic.

If a thin capability is missing in `cobuild-core`, it should be evaluated as a
small Core addition before duplicating logic in the lock crate.

## 3. Witness, Message, Seal, And Task Usage

### High-Level Consumption Model

`cobuild-otx-lock` is query-first and contract-first.

The lock does not scan arbitrary witnesses by itself. Instead, it:

1. loads transaction-facing inputs through `LoaderSession`;
2. constructs `CobuildContext`;
3. queries the current lock by current script hash;
4. consumes returned tx-level and OTX-level tasks;
5. verifies every relevant task.

### Tx-Level Flow

For tx-level Cobuild flow, the lock should call:

- `context.lock_query(current_script_hash).tx_tasks(&tx_hash_parts)`

For each returned `TxLevelLockTask`, the lock should:

- use `seal` as verifier input;
- use `signing_message_hash` as the standardized Core signing digest;
- treat the task as authoritative for carrier witness selection and tx-level
  flow mode.

The lock crate should not re-derive whether the task came from
`SighashAll` or `SighashAllOnly` beyond what is already encoded in the task.

### OTX-Level Flow

For OTX Cobuild flow, the lock should call:

- `context.lock_query(current_script_hash).otx_tasks(&parts_by_otx)`

The required `parts_by_otx` vector must be built in the same OTX order used by
Core layout scanning:

1. load one `LayoutTx` snapshot;
2. derive `CobuildLayout` from that `LayoutTx` with `build_layout(&tx)`;
3. load one `OtxHashParts` per `layout.otxs[i]` in that exact order;
4. pass the resulting ordered vector unchanged into `otx_tasks`.

This ordered-loading contract is part of the design. The lock crate must not
invent a second OTX ordering rule.

If the current `cobuild-core` API makes this alignment awkward, Phase 1 may add
a thin Core helper such as a layout-aligned OTX hash-parts loader. That helper
is preferred over lock-side duplication of OTX scan or layout semantics.

For each returned `OtxLockTask`, the lock should:

- verify the returned `seal`;
- verify against the returned `signing_message_hash`;
- respect the returned `scope` (`Base` or `Append`);
- treat `covered_ranges` as diagnostic and audit-facing metadata, not as a
  place to rebuild Core hashing logic.

### Combined Flow

One lock execution may have both:

- one or more tx-level tasks; and
- one or more OTX-level tasks.

`cobuild-otx-lock` must support this combined case in one execution path.

The crate must not split tx-level and OTX-level validation into two unrelated
top-level entry modes.

### Message And Action Responsibility

At lock level, the minimum validation responsibility is:

- trust `cobuild-core` to reject malformed related Cobuild structures;
- trust `cobuild-core` to reject missing message targets when a related message
  is relevant;
- not interpret `Action.data`;
- not add app-specific action policy in the generic lock crate.

`cobuild-otx-lock` therefore treats message semantics as:

- structurally relevant for hash and target integrity;
- not semantically interpreted beyond Core responsibilities.

## 4. Signature Verification Boundary

### Phase 1 Verifier Shape

Phase 1 should define a narrow verifier interface inside the lock crate.

The essential contract is:

```rust
fn verify(
    auth: &AuthContext,
    seal: &[u8],
    signing_message_hash: &[u8; 32],
) -> Result<(), VerifyError>;
```

The exact Rust trait or function wrapper can vary, but the boundary must remain
narrow and message-digest-centric.

### `AuthContext`

Phase 1 should introduce a lock-local `AuthContext` built from script args and
fixed verifier configuration.

It should contain only data needed for verification, such as:

- lock identity material;
- algorithm selection or verifier mode;
- any verifier-entry metadata that is part of fixed verifier configuration and
  will later be mapped into a `ckb-auth` adapter.

Phase 1 freezes the minimum script-args ABI as:

- `1` byte verifier kind
- `20` bytes identity payload

This 21-byte args ABI is intentionally small and stable. Future `ckb-auth`
adapter inputs that are not identity data should come from fixed verifier
configuration, not by expanding the Phase 1 lock args ABI.

`AuthContext` must remain lock-local. It should not pull `ckb-auth` types into
the main task collection and orchestration flow.

### Responsibility Split

The boundary is:

- `cobuild-core` computes `signing_message_hash`;
- `cobuild-core` decides task relevance and task contents;
- `cobuild-otx-lock` supplies current lock auth context and dispatches verify
  calls;
- verifier logic decides whether `seal` authorizes the given
  `signing_message_hash`.

This keeps Cobuild hashing and witness semantics outside verifier code.

### Why This Matches Future `ckb-auth`

`ckb-auth` is naturally keyed on:

- auth identity;
- algorithm/backend selection;
- raw signature bytes;
- a 32-byte message digest.

That is close to the proposed verifier boundary. As a result, future
integration can be implemented as a verifier adapter rather than a redesign of
task collection or lock orchestration.

### What The Verifier Must Not Own

The verifier must not:

- parse Cobuild witnesses;
- decide tx-level vs OTX-level flow;
- compute Core hashes;
- decide base vs append scope;
- inspect or interpret `Action`s;
- scan transaction structure for target existence.

## 5. Error Model

### Top-Level Categories

`cobuild-otx-lock` should separate:

- Core protocol errors from `cobuild-core`;
- lock argument and lock semantic errors;
- verifier errors.

A recommended top-level shape is:

```rust
pub enum Error {
    Core(CoreError),
    InvalidArgs,
    UnsupportedAuth,
    MissingRelevantTask,
    Verify(VerifyError),
}
```

The exact variant names may differ, but the category split should remain.

### Errors That Must Fail Immediately

The following must fail closed:

- any relevant `CoreError`;
- malformed or unsupported lock args;
- missing required auth material;
- no relevant Cobuild task in Cobuild-only mode;
- verifier-reported signature failure;
- verifier-reported seal encoding failure;
- verifier backend failure.

### Malformed Witness Handling

Malformed witness or layout conditions are not reclassified in the lock crate.
If `cobuild-core` determines a related Cobuild structure is malformed, the lock
must fail through the `Core(CoreError)` path.

This preserves the protocol distinction between:

- malformed witness / layout / hash input;
- verifier failure;
- lock-local semantic failure.

### Verifier Failure Bucket

Verifier-facing failures should be distinct from Core errors. Recommended
subcategories are:

- invalid seal encoding;
- signature verification failed;
- backend unavailable or misconfigured.

This distinction matters for future `ckb-auth` integration and for test
coverage, even if final numeric script error codes are coarse.

### Lock Semantic Failure Bucket

Lock semantic failures should cover cases such as:

- args length mismatch;
- unsupported verifier mode;
- missing relevant task in Cobuild-only mode;
- lock-local auth context construction failure.

These are not Cobuild protocol errors. They belong to the lock crate.

## 6. Suggested Code Structure

### Modules

The new contract crate at `contracts/cobuild-otx-lock` should remain small and
fixed-purpose. Recommended modules:

- `main.rs`
  - contract binary entry
- `entry.rs`
  - high-level contract entry function and exit mapping
- `error.rs`
  - lock-local error definitions
- `args.rs`
  - script args parsing into `AuthContext`
- `runner.rs`
  - fixed orchestration flow
- `verify/mod.rs`
  - verifier interface and shared verifier error types
- `verify/local.rs`
  - Phase 1 real local verifier implementation used by the contract

Future Phase 2 may add:

- `verify/ckb_auth.rs`

At workspace level, Phase 1 should also keep the standard template-facing
support files:

- `Makefile`
  - top-level build/test entry for contract crates
- `scripts/find_clang`
  - native toolchain discovery helper
- `tests`
  - host-side contract verification crate used by `make test`

### State Objects

Recommended small state objects:

- `AuthContext`
  - parsed lock authentication inputs
- `OtxLockRunner`
  - one execution session for current script validation

`OtxLockRunner` should own the fixed flow state required during one run, such
as:

- current script hash;
- auth context;
- verifier configuration;
- loader session or loaded Core-facing data;
- verifier handle/reference.

### Methods That Belong On The Runner

The following flow should be grouped as runner methods rather than loose free
functions:

- load current script context;
- load script args and verifier configuration;
- construct `CobuildContext`;
- build layout-aligned OTX hash parts;
- collect tx-level tasks;
- collect OTX-level tasks;
- verify tx-level tasks;
- verify OTX-level tasks;
- run the full validation path.

This keeps the call graph short and auditable.

### What Should Stay Out Of State Objects

State objects should not absorb:

- generalized provider traits;
- off-chain convenience views;
- dynamic plugin systems;
- script-agnostic Cobuild parsing logic already owned by `cobuild-core`.

The goal is fixed-flow clarity, not framework construction.

## 7. Test Strategy

### Unit Tests

Unit tests should cover:

- script args parsing success and failure;
- `AuthContext` construction;
- task orchestration when only tx-level tasks exist;
- task orchestration when only OTX base tasks exist;
- task orchestration when only OTX append tasks exist;
- combined tx-level and OTX task execution;
- empty relevant task set in Cobuild-only mode;
- verifier error mapping.

### Integration Tests

Integration tests should use real contract execution through the template-style
workspace `tests` crate and cover:

- tx-level Cobuild signing path;
- OTX base signing path;
- OTX append signing path;
- one lock execution verifying both tx-level and OTX-level tasks;
- one lock execution verifying both base and append tasks for the same lock;
- malformed related Cobuild witness failure;
- invalid args failure;
- invalid seal failure;
- verifier backend failure path.

### Regression Points That Must Be Nailed Down

The following regressions are high-priority:

- same lock validates multiple tasks in one run;
- base and append scopes use distinct seals and distinct digests;
- `cobuild-core` task/hash outputs are consumed directly, without lock-local
  recomputation;
- Cobuild-only mode fails when no relevant task exists;
- replacing verifier implementation does not require changing runner logic or
  args ABI.

### Future `ckb-auth` Boundary Protection

Tests should explicitly protect the future verifier seam by ensuring:

- runner tests use a mock/stub verifier interface;
- task collection tests do not depend on verifier internals;
- a future `ckb-auth` adapter can be swapped in without changing task shapes,
  runner control flow, or the Phase 1 21-byte args ABI.

## 8. Evolution Strategy

### Phase 1

Phase 1 delivers:

- the new `cobuild-otx-lock` crate;
- stable lock-side orchestration over `cobuild-core`;
- stable `AuthContext` and verifier boundary;
- a real local verifier implementation for the Phase 1 selected verifier kind;
- no direct `ckb-auth` integration yet.

This phase optimizes for interface stability, auditability, and Core reuse.

### Phase 2

Phase 2 should integrate `ckb-auth` by adding a dedicated verifier adapter.

That adapter should:

- map `AuthContext` plus fixed verifier configuration into the required
  `ckb-auth` identity and entry inputs;
- call `ckb-auth` with the already computed `signing_message_hash`;
- translate `ckb-auth` failures into `VerifyError`.

Phase 2 should not require redesign of:

- `CobuildContext` usage;
- task collection;
- Core hash generation;
- runner control flow.

### Interfaces That Must Be Correct Now

To avoid future redesign, Phase 1 must already stabilize:

- `AuthContext` as the lock-local authentication input object;
- the Phase 1 21-byte args ABI;
- the verifier interface as `seal + signing_message_hash + auth_context`;
- the rule that the lock consumes both tx-level and OTX-level tasks in one run;
- the rule that Core hashing and task derivation stay outside verifier code;
- the rule that Cobuild-only mode fails on missing relevant tasks.

If these interfaces are allowed to drift in Phase 1, future `ckb-auth`
integration will force wider changes than necessary.

## 9. Summary Of Design Choices

- `cobuild-otx-lock` is a thin contract-first lock crate, not a second Cobuild
  runtime.
- The crate directly consumes `CobuildContext`, `LockScriptQuery`,
  `TxLevelLockTask`, and `OtxLockTask`.
- All Cobuild witness, layout, hash, and target-integrity logic is reused from
  `cobuild-core`.
- The crate supports combined tx-level and OTX-level validation in one lock
  execution.
- Phase 1 is Cobuild-only and fail-closed on missing relevant tasks.
- Signature verification is isolated behind a narrow verifier boundary that is
  naturally compatible with future `ckb-auth` integration.
- The lock crate keeps only args parsing, orchestration, and verifier dispatch
  as lock-local logic.
