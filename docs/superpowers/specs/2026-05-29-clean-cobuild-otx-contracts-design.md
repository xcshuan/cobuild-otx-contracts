# Clean Cobuild OTX Contracts Design

## Status

This document defines the clean-room design baseline for the
`cobuild-otx-contracts` sub-repository at:

`/home/xcshuan/contracts/ckb/cobuild-otx-contracts/cobuild-otx-contracts`

The intent is to restart the implementation in this sub-repository from the
approved Cobuild protocol specifications, not from the previous experimental
implementation in the parent workspace.

## 1. Baselines And Non-Goals

### Authoritative Inputs

The authoritative protocol and boundary inputs are:

- `docs/superpowers/specs/2026-05-28-cobuild-core-community-redraft-design.md`
- `docs/superpowers/specs/2026-05-28-cobuild-shared-modules-and-types-design.md`
- `docs/superpowers/specs/2026-05-29-cobuild-otx-lock-design.md`

Existing code from the parent workspace and historical POC repositories may be
used as reference material, but it is not inherited by default.

### Clean-Room Position

This sub-repository follows a clean-room implementation approach:

- existing `cobuild-core` code is reference only;
- existing `cobuild-otx-lock` code is reference only;
- existing generated type layout is reference only;
- new code should be derived from the approved specs and the crate boundaries in
  this document.

### Problems This Design Avoids

The clean design specifically avoids the previous implementation issues:

- no contract hot path based on owned Molecule `Bytes` carriers;
- no local `critical-section` shim crate as a baseline design requirement;
- no mixing of chain-facing reader code with future host-side owned helpers;
- no broad provider trait or full owned transaction view abstraction;
- no verifier details leaking into Cobuild Core protocol code.

If a future dependency reintroduces atomic or `Bytes` issues, the preferred fix
is to adjust the chain-facing codegen and dependency boundary first. Adding a
local runtime shim must be treated as an explicit design change, not a default
implementation technique.

## 2. Workspace Shape

The clean implementation should use the `ckb-script-template` workspace shape.

The first implementation phase should contain these workspace members:

- `crates/cobuild-types`
- `crates/cobuild-core`
- `contracts/cobuild-otx-lock`
- `tests`
- `xtask`

The current sub-repository is already isolated from the parent workspace. New
implementation work may replace the copied `crates/cobuild-types` contents
inside this sub-repository, but the crate name remains `cobuild-types`.

## 3. `cobuild-types`

### Role

`cobuild-types` is the schema and generated-type crate for chain-facing Cobuild
contracts.

It must not contain:

- Cobuild protocol decisions;
- signing message hash logic;
- layout scanning;
- task/query logic;
- contract loader orchestration;
- verifier logic.

### Codegen Outputs

`cobuild-types` should generate and commit two Molecule output families:

- `cobuild_types::lazy_reader::*`
- `cobuild_types::entity::*`

The names are public API names. They should not use historical names such as
`schemas`, `schemas2`, or `generated/rust`.

The expected source layout is:

```text
crates/cobuild-types/
  schemas/
    blockchain.mol
    core.mol
    witness.mol
  src/
    lib.rs
    lazy_reader/
      mod.rs
      blockchain.rs
      core.rs
      witness.rs
    entity/
      mod.rs
      blockchain.rs
      core.rs
      witness.rs
```

### Chain-Facing Rule

The contract hot path must use `lazy_reader`.

`entity` exists because owned Molecule entities are useful for tests, future
host-side utilities, and possibly off-chain builders. It must not enter
`cobuild-core`'s chain-facing protocol execution path in Phase 1.

### Generation Management

Generated files are committed to the repository.

The sub-repository owns its own `xtask`. That `xtask` is responsible for:

- generating `lazy_reader` outputs;
- generating `entity` outputs;
- regenerating all `cobuild-types` outputs in one command;
- checking that committed generated outputs match the schemas.

The exact CLI shape may be finalized in the implementation plan, but the
responsibility must remain local to this sub-repository.

## 4. `cobuild-core`

### Role

`cobuild-core` is a contract-first, `no_std`, chain-facing Cobuild protocol
crate.

It provides:

- witness parsing;
- thin protocol views over `cobuild_types::lazy_reader`;
- OTX layout scanning;
- Cobuild signing hash construction;
- context and query logic;
- tx-level and OTX-level task generation;
- internal error types useful for tests and diagnostics.

It does not provide:

- host-side builder APIs;
- wallet APIs;
- full owned transaction models;
- provider traits;
- verifier implementations;
- lock-specific authentication rules.

### Reader Boundary

`cobuild-core` should not scatter generated Molecule lazy-reader types across
the whole crate.

Instead, it should contain a thin boundary layer that projects lazy-reader
values into stable protocol views. Examples of expected view objects are:

- `WitnessLayoutView`
- `OtxView`
- `OtxStartView`
- `MessageView`
- `SealPairView`

These views should:

- own no large buffers;
- expose only protocol fields needed by core logic;
- keep cursor or reader details near the boundary;
- avoid generalized provider abstractions.

The protocol execution layer should be implemented on top of these views rather
than directly on top of generated code.

### Module Shape

The expected initial module shape is:

- `view`
- `witness`
- `layout`
- `hash`
- `context`
- `query` or `tasks`
- `error`

The implementation plan may split or combine small files, but the boundary
principle must hold: generated reader handling stays near `view` and
protocol decisions stay above it.

### Dependency Rule

`cobuild-core` may depend on `cobuild-types::lazy_reader`.

`cobuild-core` must not depend on `cobuild-types::entity` in its chain-facing
path. Tests may use `entity` only if it does not become a normal dependency of
the contract path.

## 5. `cobuild-otx-lock`

### Role

`cobuild-otx-lock` is a thin lock contract under:

`contracts/cobuild-otx-lock`

It is responsible only for lock-side execution:

- loading current script args;
- loading current script hash;
- invoking `cobuild-core` loader/context/query functionality;
- collecting tx-level and OTX-level tasks for the current lock;
- invoking a verifier for every relevant task;
- mapping failures to stable exit codes.

It must not:

- parse Cobuild witness protocol details directly;
- scan OTX layouts directly;
- construct Cobuild hash preimages directly;
- check message targets directly;
- depend on `cobuild_types::entity`;
- contain legacy fallback logic in Phase 1.

### Main Flow

The first phase lock flow is:

1. Load current script args and script hash.
2. Build or load the `cobuild-core` execution context.
3. Query tx-level tasks for the current lock hash.
4. Query OTX-level tasks for the current lock hash.
5. Verify every task.
6. Return success only if at least one relevant task was verified.

The contract is Cobuild-only in Phase 1. If no relevant Cobuild task exists, it
fails closed.

### Verifier Boundary

The verifier interface should be narrow:

```rust
trait LockVerifier {
    fn verify(
        &self,
        auth: &AuthContext,
        seal: &[u8],
        signing_message_hash: &[u8; 32],
    ) -> Result<(), VerifyError>;
}
```

Responsibility split:

- `cobuild-core` provides `seal` and `signing_message_hash`;
- `cobuild-otx-lock` parses `AuthContext` and calls the verifier;
- the verifier validates the signature or reports verification failure;
- future `ckb-auth` integration is implemented as a verifier adapter.

`ckb-auth` details must not leak into `cobuild-core`.

## 6. Error Model

### Contract Exit Codes

Although internal code may use structured errors for testing and diagnostics,
the on-chain contract should expose only a small stable set of exit codes.

Initial exit code categories:

- `InvalidArgs`
- `MalformedCobuild`
- `LockSemanticFailure`
- `VerifyFailure`
- `SyscallFailure`
- `InternalFailure`

The exact numeric values are assigned in the implementation plan.

### Mapping Rules

The contract maps errors as follows:

- script args length or format errors map to `InvalidArgs`;
- Molecule reader, witness union, schema encoding, and malformed Cobuild layout
  errors map to `MalformedCobuild`;
- missing relevant tasks and lock-specific semantic failures map to
  `LockSemanticFailure`;
- invalid seal encoding, signature mismatch, and verifier backend failures map
  to `VerifyFailure`;
- syscall loading failures map to `SyscallFailure`;
- unexpected invariant violations map to `InternalFailure`.

`cobuild-core` may retain fine-grained errors internally so unit tests can
assert precise behavior. Those fine-grained errors are not part of the lock
contract's public behavior.

### Fail-Closed Rule

The lock fails closed for:

- malformed Cobuild data;
- verifier failures;
- syscall failures;
- unsupported args;
- no relevant task.

Phase 1 does not provide legacy fallback.

## 7. Testing Strategy

### `cobuild-types`

Tests should cover:

- both `lazy_reader` and `entity` outputs compile;
- witness union readers can distinguish supported variants;
- basic core schema structures are readable;
- `xtask` check mode detects generated output drift.

### `cobuild-core`

Tests should cover:

- view layer field access and malformed reader handling;
- witness parsing;
- OTX layout scanning;
- tx-level signing hash construction;
- OTX base and append signing hash construction;
- script visibility and query behavior;
- tx-level and OTX-level task generation;
- regression that core chain-facing logic does not depend on `entity`.

### `cobuild-otx-lock`

Tests should cover:

- args ABI parsing;
- verifier interface behavior;
- error-to-exit-code mapping;
- single tx-level task verification;
- OTX base and append verification;
- mixed tx-level plus OTX verification;
- multiple tasks in one execution;
- no relevant task fails closed;
- malformed witness fails;
- invalid args fail;
- bad seal fails;
- verifier backend failure fails.

### Contract Integration

Host-side contract tests should cover:

- tx-level only;
- OTX base plus append;
- mixed tx-level and OTX tasks;
- malformed Cobuild witness;
- invalid args;
- bad seal;
- verifier backend failure.

Expected verification commands:

- `cargo test -p cobuild-types`
- `cargo test -p cobuild-core`
- `cargo test -p cobuild-otx-lock`
- `make build CONTRACT=cobuild-otx-lock MODE=debug`
- `MODE=debug cargo test -p tests --test cobuild_otx_lock`

## 8. Phasing

### Phase 1

Phase 1 delivers:

- clean `cobuild-types` with committed `lazy_reader` and `entity` outputs;
- local `xtask` codegen management;
- chain-facing `cobuild-core` built on thin reader views;
- Cobuild-only `cobuild-otx-lock`;
- local verifier boundary without `ckb-auth`;
- contract integration tests for tx-level, OTX, and mixed flows.

### Phase 2

Phase 2 may add:

- `ckb-auth` verifier adapter;
- host-side utilities using `entity`;
- additional off-chain tests and builders;
- stricter generated-output CI checks.

Phase 2 must not require changing `cobuild-core` task/hash/query APIs merely to
support `ckb-auth`.

## 9. Review Checklist

Before implementation planning starts, reviewers should check:

- `entity` is present in `cobuild-types` but not in the chain-facing core path;
- `lazy_reader` is the only generated type family used by `cobuild-core`;
- the view boundary is thin and protocol-oriented;
- `cobuild-otx-lock` remains a lock orchestration crate, not a protocol crate;
- exit codes are small and stable;
- internal fine-grained errors do not become contract ABI;
- no local atomic runtime shim is assumed by the design.
