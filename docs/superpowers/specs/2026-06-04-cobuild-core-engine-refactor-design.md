# Cobuild Core Engine Refactor Design

## Status

This document defines the approved design for a breaking refactor of
`cobuild-core` and `contracts/cobuild-otx-lock`.

The refactor is intentionally not constrained by compatibility with the current
Rust helper APIs. It must preserve Cobuild Core protocol semantics, on-chain
hash bytes, lock behavior, Molecule schemas, witness union ids, and the
Cobuild-only position of `cobuild-otx-lock`.

## References

This design builds on:

- `docs/CobuildAgentDevelopGuide.md`
- `docs/superpowers/specs/2026-05-28-cobuild-core-community-redraft-design.md`
- `docs/superpowers/specs/2026-05-29-cobuild-otx-lock-design.md`
- `docs/superpowers/specs/2026-06-03-cobuild-core-streaming-reader-and-hash-input-design.md`
- `ref/repo/ckb-cobuild-poc`
- `ref/repo/standard-udt-contracts`

The reference PoC demonstrates the desired lazy syscall reader shape and uses
`signing_message_hash` terminology at the verifier boundary. The standard UDT
contracts demonstrate clear script responsibility boundaries: entry modules
express transaction flow, helper modules provide mechanics, and contracts keep
ownership of their own invariants.

## Goals

- Reshape `cobuild-core` into a clear protocol engine instead of scattered
  context/query extension methods.
- Make lock-side validation output explicit through a `LockValidationPlan`.
- Add a type-script validation flow so Core can expose related Cobuild messages
  without interpreting application-specific `Action.data`.
- Remove owned compatibility APIs and intermediate transaction structures from
  the main chain-facing path.
- Eliminate `LayoutTx { witnesses: Vec<Vec<u8>> }` from the primary layout
  scanner.
- Reduce repeated lazy transaction parsing on the lock path with source-level
  transaction/count caching.
- Keep hash construction byte-for-byte compatible with the approved Core v1
  spec.
- Keep `cobuild-otx-lock` thin: load args and script hash, provide source data,
  request a lock validation plan, invoke the verifier, and map errors.

## Non-Goals

- No Molecule schema changes.
- No witness union id changes.
- No BLAKE2b personalization changes.
- No signing preimage order or framing changes.
- No legacy `WitnessArgs` fallback.
- No `ckb-auth` integration.
- No compatibility layer for old Rust helper APIs such as
  `PreparedContextInput`, `TransactionInfo`, `LockScriptQuery`, or
  `SignatureRequest`.
- No application-specific interpretation of `Action.data`.
- No `ckb-std` dependency in `cobuild-core`.
- No use of `cobuild_types::entity` in normal core or lock paths.
- No `unsafe` in `cobuild-core` or `cobuild-otx-lock`.

## Hard Boundaries

`cobuild-core` owns Cobuild protocol interpretation:

- transaction preparation from abstract sources;
- witness recognition and local flow selection;
- OTX sequence and scope partitioning;
- message target validation;
- seal lookup and duplicate detection;
- signing hash construction;
- lock validation plan generation;
- type validation plan generation.

`cobuild-otx-lock` owns lock-local mechanics:

- current script args loading and parsing;
- current script hash loading;
- syscall-backed source implementation;
- verifier selection and invocation;
- lock-local "no relevant Cobuild requirement" failure;
- stable error mapping and script return semantics.

The lock crate must not parse Cobuild protocol details, scan OTX layout, build
hash preimages, or directly inspect Molecule entity builders.

## Core Module Shape

The refactor should organize `cobuild-core` around these modules.

### `source.rs`

`source.rs` defines data access traits and read-error classification. Source
traits provide cursors, hashes, and counts. They do not encode Cobuild flow
rules.

The current `TransactionSource` and `SigningDataSource` should be refined into
a clearer source family:

```rust
pub trait TransactionSource {
    fn transaction_cursor(&self) -> Result<ClassifiedCursor, CoreError>;
    fn script_cursor(&self) -> Result<ClassifiedCursor, CoreError>;
    fn tx_hash(&self) -> Result<[u8; 32], CoreError>;

    fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError>;
    fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError>;
    fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError>;
}

pub trait HashInputSource: TransactionSource {
    fn counts(&self) -> Result<TxCounts, CoreError>;

    fn witness_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_input_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_output_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_cell_dep_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError>;

    fn resolved_input_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn resolved_input_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
}
```

The exact trait names may change during implementation, but the behavior must
not: counts are acquired as a unit, hash inputs are cursor-backed, and resolved
input output/data remain source-provided because raw transaction bytes do not
contain them.

`ClassifiedCursor` remains necessary. Read failures must retain public error
categories:

- protocol/view reads map to malformed Cobuild data or invalid OTX layout;
- source/context reads map to invalid context input;
- hash payload reads map to missing hash input.

### `prepare.rs`

`prepare.rs` performs transaction-level preparation once:

- parse the transaction cursor;
- verify transaction shape enough to read raw counts and witnesses;
- produce `TxCounts`;
- build `ScriptHashIndex` from source-provided script hashes;
- prepare witness metadata needed by the engine;
- avoid storing owned transaction bytes, raw transaction parts, or all witness
  bytes.

The main API should not expose `PreparedContextInput`, `TransactionInfo`, or
`parse_transaction_info`. Any in-memory helpers needed by tests should live as
test-only utilities or as a clearly test-oriented source implementation.

### `engine.rs`

`engine.rs` is the central protocol engine.

The intended API shape is:

```rust
let prepared = CobuildEngine::prepare(source)?;
let lock_plan = prepared.plan_lock_validation(lock_script_hash, source)?;
let type_plan = prepared.plan_type_validation(type_script_hash, source)?;
```

`CobuildEngine` or the prepared engine state owns Core flow decisions:

- tx-level local flow selection;
- OTX relevance selection;
- related malformed witness handling;
- message target validation;
- seal lookup;
- signing hash request orchestration;
- lock group OTX coverage checks.

### `plan.rs`

`plan.rs` defines the engine outputs.

For lock scripts:

```rust
pub struct LockValidationPlan {
    pub lock_script_hash: [u8; 32],
    pub required_signatures: Vec<SigningRequirement>,
}

pub struct SigningRequirement {
    pub origin: SignatureOrigin,
    pub carrier_witness_index: usize,
    pub seal: Vec<u8>,
    pub signing_message_hash: [u8; 32],
}

pub enum SignatureOrigin {
    TxLevel,
    OtxBase,
    OtxAppend,
}
```

`SigningRequirement` is preferred over `SignatureTask` because this structure
expresses a lock-side requirement produced by Core, not an off-chain signing
job. The field name `signing_message_hash` is retained to match the PoC and
current verifier terminology.

For type scripts:

```rust
pub struct TypeValidationPlan {
    pub type_script_hash: [u8; 32],
    pub related_messages: Vec<RelatedMessage>,
}

pub struct RelatedMessage {
    pub origin: MessageOrigin,
    pub message: MessageView,
}

pub enum MessageOrigin {
    TxLevel {
        carrier_witness_index: usize,
    },
    Otx {
        witness_index: usize,
        otx_index: usize,
        layout: OtxMessageLayout,
        relation: OtxTypeRelation,
    },
}

pub struct OtxMessageLayout {
    pub base_inputs: Range,
    pub append_inputs: Range,
    pub base_outputs: Range,
    pub append_outputs: Range,
    pub base_cell_deps: Range,
    pub append_cell_deps: Range,
    pub base_header_deps: Range,
    pub append_header_deps: Range,
}

pub struct OtxTypeRelation {
    pub input_type_in_base: bool,
    pub input_type_in_append: bool,
    pub output_type_in_base: bool,
    pub output_type_in_base_covered: bool,
    pub output_type_in_append: bool,
}
```

`Message` contains `ActionVec`, so `TypeValidationPlan` must not duplicate
actions as top-level plan data. `MessageView` should expose cursor-backed
access to actions, including convenience helpers for filtering actions that
target a given `(script_role, script_hash)`. Type scripts decide whether a
message or action is required, whether multiple matching actions are allowed,
and how to interpret `Action.data`.

`MessageOrigin::Otx` must include OTX information. Type scripts need the
witness index, OTX index, relevant layout ranges, and relation flags to make
scope-aware policy decisions. `output_type_in_base` means the current type hash
appears in a base output range. `output_type_in_base_covered` separately tells
whether at least one matching base output type field is covered by the base
output type mask and therefore by the OTX base signing hash. Append outputs are
fully covered in Core v1, so `output_type_in_append` does not need a separate
coverage flag.

### `layout.rs`

`layout.rs` becomes the OTX partition engine. It should not require
`LayoutTx { witnesses: Vec<Vec<u8>> }` in the primary path.

The scanner reads witnesses from prepared witness metadata or a witness source.
It outputs compact OTX segment data:

- OTX witness index;
- OTX ordinal index;
- base and append ranges for inputs, outputs, cell deps, and header deps;
- cursor-backed `OtxView`;
- invalid scan state carrying the layout error.

It remains responsible for:

- rejecting `Otx` before `OtxStart` when relevant;
- rejecting duplicated `OtxStart`;
- rejecting non-contiguous `Otx` witnesses after `OtxStart`;
- requiring at least one `Otx` after `OtxStart`;
- validating append permissions;
- validating mask lengths and padding bits;
- validating non-overflowing, in-bounds OTX partitions.

### `flow.rs`

`flow.rs` may be a separate module or an internal part of `engine.rs`. It
should make the Core local flow selection rules explicit:

- group-leading tx-level witness selection;
- unique `SighashAll` detection;
- OTX relevance for lock input scopes;
- OTX relevance for type input/output scopes;
- strict malformed OTX layout failure;
- lock group OTX coverage checks.

### `hash.rs` And `hash/writer.rs`

`hash.rs` keeps the four Core hash entries:

- `tx_with_message_hash`;
- `tx_without_message_hash`;
- `otx_base_hash`;
- `otx_append_hash`.

Internal preimage writing should move to focused helpers, either in
`preimage.rs` or `hash/writer.rs`:

- write fixed-width counts;
- write length-prefixed cursors;
- write resolved input output/data;
- write base input fields by mask;
- write base output fields by mask;
- write OTX-local indices;
- write full append-scope transaction entities.

All hash helpers must stream from source-provided cursors. They must not accept
owned raw transaction parts.

The engine computes an OTX base hash once per relevant OTX and reuses it when
building append requirements.

### `view.rs`

`view.rs` remains the Molecule-to-core protocol view boundary:

- `WitnessLayoutView`;
- `SighashAllWitnessView`;
- `OtxStartView`;
- `OtxView`;
- `SealPairView`;
- `MessageView`;
- `ActionView`;
- `MaskView`.

Views should be cursor-backed unless ownership is needed at a boundary, such as
copying a seal into `SigningRequirement`.

## Lock Validation Flow

`plan_lock_validation` generates a `LockValidationPlan` for one lock script
hash.

### Tx-Level Flow

The engine finds the first input using the current lock hash. The witness at
that absolute input index is the group-leading witness for tx-level Cobuild
flow.

If that witness is a valid `SighashAll` or `SighashAllOnly`, the engine enters
tx-level flow and emits one `SigningRequirement`:

- `origin = SignatureOrigin::TxLevel`;
- `carrier_witness_index` is the group-leading witness index;
- `seal` is copied from the carrier witness;
- `signing_message_hash` is `TxWithMessage` when there is exactly one valid
  `SighashAll`, otherwise `TxWithoutMessage` for `SighashAllOnly`.

If duplicate `SighashAll` witnesses exist and the current lock entered
tx-level flow, the engine fails with `DuplicateSighashAll`.

If the group-leading witness is non-empty and appears to be a malformed
Cobuild `WitnessLayout`, the engine fails the current lock. The malformed data
is related because it occupies the current lock group's tx-level carrier
position. If the non-empty witness is distinguishably not Cobuild, the engine
does not enter tx-level Cobuild flow.

When a tx-level message is relevant, the engine validates every action target:

- `input_lock` targets must match an input lock hash;
- `input_type` targets must match an input type hash;
- `output_type` targets must match an output type hash;
- unknown roles fail closed.

### OTX Flow

When OTX scan succeeds, the engine iterates OTX segments in layout order.

For each OTX:

- if the current lock hash appears in the base input range, emit an
  `OtxBase` signing requirement;
- if it appears in the append input range, emit an `OtxAppend` signing
  requirement;
- if both are true, emit both requirements;
- validate the OTX message targets when the OTX is relevant;
- find exactly one matching `SealPair` for each required
  `(script_hash, scope)`;
- fail missing, duplicate, or invalid-scope seals only when the OTX scope is
  relevant to the current lock.

`OtxAppend` binds to the current OTX's base hash. The engine computes the base
hash once and passes the digest to append hash construction.

When OTX scan is invalid, the engine fails closed with the layout error. Invalid
OTX layout is treated as a transaction-level Cobuild protocol error rather than
being classified per current script relevance.

### Lock Group Coverage

If the plan contains OTX requirements but no tx-level requirement, the engine
must ensure every input in the current lock group is covered by a relevant OTX
base or append input scope. If any input in the group is not covered, the
engine fails with `MissingLockGroupCoverage`.

This preserves the current safety rule while making the tx-level remainder
semantics explicit.

### Cobuild-Only Lock Policy

`cobuild-core` may return an empty `LockValidationPlan` when no related
Cobuild flow applies.

`cobuild-otx-lock` remains Cobuild-only. If the returned plan has no
`required_signatures`, the lock returns its lock-local semantic failure.

## Type Validation Flow

`plan_type_validation` generates a `TypeValidationPlan` for one type script
hash. It does not generate signing requirements.

The plan exposes related Cobuild messages:

- tx-level message when a unique valid `SighashAll` exists and the type script
  hash appears in an input type or output type position not covered by a
  relevant OTX type relation, or when the tx-level message has an `input_type`
  or `output_type` action targeting the current type script hash;
- OTX message when the type script appears in an OTX input type or output type
  range, or when the OTX message has an `input_type` or `output_type` action
  targeting the current type script hash even if that type is outside the OTX's
  local cell ranges;
- origin information for every related message.

For OTX messages, `MessageOrigin::Otx` includes:

- OTX witness index;
- OTX ordinal index;
- compact layout ranges;
- relation flags indicating whether the current type appeared in base inputs,
  append inputs, base outputs, or append outputs. These flags may all be false
  for an action-target-only OTX message.
- for base output type relations, an explicit coverage flag indicating whether
  the matching output type field was covered by the base output mask.

Core validates message/action shape and target roles while preserving the Core
rule that missing message or missing action is not a universal failure. A type
script may impose stricter policy after receiving the plan.

Type validation must also fail closed for related malformed data:

- duplicate `SighashAll` fails when a tx-level message would otherwise be
  relevant to the type hash;
- malformed OTX scan fails before building any lock or type validation plan;
- invalid action roles or malformed actions fail when they appear in a related
  message.

The type plan must not interpret `Action.data`. It may expose cursor-backed
action views and filtering helpers.

This design adds the reusable core API for type scripts. It does not add a new
type contract in this repository. A future type-script integration is
responsible for loading the current type script hash, choosing whether an empty
`TypeValidationPlan` is acceptable for that script's policy, and consuming the
cursor-backed `MessageView`s returned by core.

## Chain Source And Cache

`contracts/cobuild-otx-lock/src/chain.rs` owns the syscall-backed source.

The chain source should provide:

- transaction cursor from low-level `load_transaction`;
- current script cursor from low-level script reading when needed by lazy
  readers;
- `tx_hash` from `ckb_std::high_level::load_tx_hash`;
- input lock and type hashes from high-level cell hash helpers;
- output type hashes from high-level cell hash helpers;
- raw transaction entity cursors from the transaction lazy view;
- resolved input cell output/data cursors from syscall-backed cell readers.

The source should cache transaction-derived lazy views or counts so repeated
hash/query calls do not reconstruct `Transaction::from(transaction_cursor())`
and `raw()` for every raw entity lookup.

This cache must not materialize the full transaction into a `Vec`. It should
cache cheap parsed view handles, counts, or other compact metadata compatible
with syscall-backed cursors.

## Test Strategy

Existing hash regression coverage must remain the first guardrail: all Core v1
hash bytes must remain unchanged.

New engine tests should cover:

- lock plan with tx-level flow only;
- lock plan with OTX base only;
- lock plan with OTX append only;
- lock plan with both OTX base and append for the same lock;
- lock plan with combined tx-level and OTX requirements;
- OTX-only plan failing when the lock group is not fully OTX-covered;
- duplicate `SighashAll` failing only when tx-level flow is relevant;
- invalid OTX layout failing immediately;
- message action target validation for tx-level and OTX messages;
- type plan exposing a tx-level related message;
- type plan exposing an OTX related message with OTX origin layout and relation
  data;
- type plan preserving empty plan behavior when no related message exists;
- seal missing, duplicate, and invalid scope failures only for relevant OTX
  scopes.

Source behavior tests should prove:

- main engine APIs do not require owned transaction bytes;
- witness layout scanning does not clone every witness into `Vec<Vec<u8>>`;
- counts are acquired as a unit;
- chain-style hash access does not repeatedly request transaction counts or
  transaction raw views when a cache can satisfy them.

Boundary tests remain required:

```bash
rg -n "cobuild_types::entity|::entity::" crates/cobuild-core/src contracts/cobuild-otx-lock/src
rg -n "unsafe" crates/cobuild-core/src contracts/cobuild-otx-lock/src
rg -n "ckb_std" crates/cobuild-core/src
```

These commands should print no matches.

## Migration Plan

1. Add `plan.rs`, `engine.rs`, and `flow.rs`.
2. Introduce `LockValidationPlan`, `SigningRequirement`,
   `TypeValidationPlan`, `RelatedMessage`, and `MessageOrigin`.
3. Add engine-level tests for the new plan APIs.
4. Refactor source traits around `TxCounts` and `HashInputSource`.
5. Replace owned layout witness storage with cursor/source-driven scanning.
6. Move lock entry to `plan_lock_validation`.
7. Add `plan_type_validation` and related type-plan tests.
8. Split hash preimage helpers while preserving all hash regression outputs.
9. Remove old public APIs that only supported the pre-engine architecture:
   `SignatureRequest`, `LockScriptQuery`, `PreparedContextInput`,
   `TransactionInfo`, and `parse_transaction_info`.
10. Update tests and fixtures to use engine plans and source implementations.

## Verification

After implementation, the required verification matrix is:

```bash
cargo run -p xtask --offline -- codegen cobuild-types --check
cargo clippy --workspace --all-targets --offline
cargo test --workspace --offline
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

If a change touches generated Cobuild type outputs, regenerate them through the
workspace `xtask` flow and keep generated code ownership clear.
