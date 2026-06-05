# Cobuild Core Signature Flow Refactor As-Built

## Status

This document records the refactor merged in commit
`a6d7864 refactor: clarify cobuild core signature flow`.

It is an as-built design note, not an implementation plan. It describes the
current module boundaries, naming decisions, and safety constraints after the
refactor. It does not revise Cobuild Core protocol semantics.

If this document conflicts with the active Cobuild Core protocol redraft, the
protocol redraft wins:

- `docs/superpowers/specs/2026-05-28-cobuild-core-community-redraft-design.md`

## Scope

The refactor changed Rust module names, internal code organization, core helper
types, error names, and tests. It intentionally preserved:

- Molecule schemas and witness union ids.
- BLAKE2b personalization constants.
- Signing preimage order, length framing, local index rules, mask rules, and
  resolved-input coverage.
- `cobuild-otx-lock` args ABI.
- Public lock exit code numbers.
- Existing Cobuild witness acceptance and rejection semantics, except where the
  previous behavior contradicted relevance-driven lock querying.
- The constraint that `cobuild-core` must not depend on generated entity module
  types as its public abstraction.
- The no-`unsafe` policy.

## Final Module Boundaries

`cobuild-core` now separates the lock-signature flow into focused modules:

```text
crates/cobuild-core/src/
  context.rs       transaction state, script hashes, raw hash input storage
  query.rs         LockScriptQuery::required_signatures orchestration
  signature.rs     SignatureRequest and SignatureOrigin
  sighash.rs       transaction-level SighashAll request collection
  otx_request.rs   OTX base and append request collection
  layout.rs        OTX witness sequence scanning and range construction
  hash.rs          signing hash construction
  loader.rs        transaction parsing and PreparedContext assembly
  message.rs       Message.Action target validation
  seal.rs          SealPair selection and uniqueness checks
  protocol.rs      typed protocol-byte wrappers
  view.rs          Molecule witness/message to owned core DTO conversion
  witness.rs       Cobuild witness layout detection helpers
  error.rs         CoreError
```

The lock crate keeps the existing CKB contract-template split:

```text
contracts/cobuild-otx-lock/src/
  entry.rs       high-level contract flow
  loader.rs      syscall-backed loading and context preparation
  args.rs        lock args parsing
  error.rs       stable Error and ExitCode definitions
  errors.rs      mapping from syscall/core/verifier errors to Error
  verify/        verifier boundary and local verifier
```

The earlier idea of renaming core `loader.rs` to `prepare.rs`, lock
`loader.rs` to `chain.rs`, and merging lock `errors.rs` into `error.rs` was
deferred. The current names are still understandable after the higher-value
core query split, and changing them would add churn without changing the
auditable contract flow.

## Signature Requests

The old task vocabulary was removed from the core API. The canonical request
type is now:

```rust
pub struct SignatureRequest {
    pub script_hash: [u8; 32],
    pub carrier_witness_index: usize,
    pub origin: SignatureOrigin,
    pub seal: Vec<u8>,
    pub signing_message_hash: [u8; 32],
}
```

`SignatureOrigin` identifies why the signature is required:

- `SighashAll`
- `OtxBase`
- `OtxAppend`

`crates/cobuild-core/src/tasks.rs` and the old
`LockSignatureRequest` name were removed. Tests were renamed from task language
to signature-request language.

## Query Flow

The lock query path is now:

1. `LockScriptQuery::required_signatures` collects transaction-level
   SighashAll requests.
2. It collects relevant OTX base and append requests.
3. If OTX requests exist without a transaction-level signature request, it
   verifies that every input using the current lock script is covered by OTX
   ranges.
4. It returns a unified `Vec<SignatureRequest>`.

This preserves mixed SighashAll and OTX verification while making the coverage
rule explicit.

## Relevance-Aware OTX Layout Handling

`layout.rs` now exposes `scan_layout`, which records one of:

- no OTX layout;
- a complete OTX layout;
- an invalid OTX layout error.

Malformed OTX layout fails closed with `CoreError::InvalidOtxLayout`.

The refactor also fixes the relevance-before-hash-input behavior: an unrelated
lock query no longer needs OTX raw hash inputs merely because the transaction
contains an OTX layout.

## Protocol Byte Wrappers

`protocol.rs` owns typed wrappers for byte-valued protocol fields:

- `ScriptRole`
- `SealScope`
- `AppendPermissions`

These wrappers centralize validation for message targets, seal scope, and
append permission bits. They do not alter the raw bytes used by Molecule data
or signing preimages.

## Error Model

`CoreError` now separates protocol failures from internal input failures:

```rust
pub enum CoreError {
    MalformedCobuild,
    InvalidOtxLayout,
    InvalidContextInput,
    InvalidMessageTarget,
    MissingHashInput,
    HashInputTooLarge,
    DuplicateSighashAll,
    MissingLockGroupCoverage,
    MissingSealPair,
    DuplicateSealPair,
    InvalidSealScope,
}
```

`cobuild-otx-lock` maps protocol-level Core errors to
`Error::MalformedCobuild` and internal input/hash construction failures to
`Error::InternalFailure`. Public exit code categories remain stable.

## View Boundary

`view.rs` remains the boundary that converts generated lazy-reader output into
owned core DTOs. It does not expose generated entity module types as the core
public abstraction, and boundary tests enforce that `cobuild-core` source does
not import `cobuild_types::entity`.

The possible extraction of reader/cursor helpers into a separate `reader.rs`
module was deferred. Keeping the safe reader support in `view.rs` is acceptable
for the current size and avoids an extra public module that would not yet carry
clear standalone semantics.

## Test Coverage Added Or Updated

The refactor added or updated tests for:

- signature-request naming and structural boundaries;
- duplicate transaction-level SighashAll classification;
- OTX relevance before raw hash input lookup;
- invalid OTX layout relevance;
- missing lock-group coverage when OTX requests exist without SighashAll;
- typed seal scope validation;
- message action target validation;
- source-boundary checks for generated entity usage and `unsafe`;
- clippy-clean generated lazy-reader output.

## Deferred Work

These items are intentionally not part of the merged refactor:

- Rename core `loader.rs` to `prepare.rs`.
- Rename lock `loader.rs` to `chain.rs`.
- Merge lock `errors.rs` into `error.rs`.
- Rename legacy DTOs such as `OtxStartData`, `OtxData`, `BuiltLayout`, and
  `OtxLayoutData`.
- Extract safe-reader helpers into `reader.rs`.
- Redesign `RawTxParts` and `SigningHashParts` beyond the current
  `SigningHashParts` naming.

Each deferred item may still be reasonable later, but it should be justified by
a concrete readability, safety, or verification improvement before being done.

## Verification

The merged implementation was verified with:

```bash
cargo fmt --check
cargo run -p xtask --offline -- codegen cobuild-types --check
cargo clippy --workspace --all-targets --offline
cargo test --workspace --offline
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```
