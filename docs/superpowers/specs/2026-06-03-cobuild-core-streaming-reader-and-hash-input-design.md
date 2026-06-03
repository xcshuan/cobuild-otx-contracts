# Cobuild Core Streaming Reader And Hash Input Design

## Status

This document defines the next refactor phase for `cobuild-core` and
`contracts/cobuild-otx-lock`.

The goal is to turn the existing lazy-reader usage from a parsing convenience
into a real streaming data boundary for on-chain verification, and then cleanly
separate the view layer from reader infrastructure and owned payload storage.
The current implementation still loads the full transaction, all witnesses,
resolved input cells, and raw transaction parts into owned `Vec` structures
before hashing. It also lets `view.rs` own reader helpers and eager DTO
copying. That weakens the memory benefit expected from lazy readers and leaves
the protocol-view boundary less crisp than it should be.

This phase may break Rust helper APIs, module names, and test target names. It
must not change Cobuild Core protocol semantics.

If this document conflicts with the active Cobuild Core protocol redraft, the
protocol redraft wins:

- `docs/superpowers/specs/2026-05-28-cobuild-core-community-redraft-design.md`

This document follows the as-built refactor record:

- `docs/superpowers/specs/2026-06-03-cobuild-core-signature-flow-refactor-as-built.md`

## Reference Behavior

`ref/repo/ckb-transaction-cobuild-poc` demonstrates the desired data-access
shape:

- transaction parsing uses a syscall-backed lazy-reader `Read` implementation
  instead of first loading the whole transaction into a `Vec<u8>`;
- tx-level signing hash iterates `tx.witnesses()?.iter().skip(inputs_len)` and
  streams each witness cursor into the hasher;
- resolved input cell and cell-data hashing use syscall-backed cursors;
- witness layout discovery may still collect a compact parsed layout list, but
  the signing hash path does not clone trailing witnesses.

This repo does not need to copy the reference implementation literally. It
should adopt the same principle: hash and parse from cursors/sources whenever
the data does not need to be owned.

## Current Problems

### Full Transaction Load

`contracts/cobuild-otx-lock/src/loader.rs` calls:

```rust
parse_transaction_info(&load_transaction()?)
```

`load_transaction` materializes the whole transaction bytes. `cobuild-core`
then builds a lazy-reader cursor on top of those owned bytes. This preserves
safe parsing, but not streaming memory behavior.

### Duplicated Trailing Witnesses

`load_prepared_context` currently creates:

```rust
let witnesses = info.witnesses;
let trailing_witnesses = witnesses.iter().skip(input_count).cloned().collect();
```

`SigningHashParts` stores those cloned trailing witnesses and `hash.rs` hashes
them later. Since `CobuildContext` already owns all witnesses, this is a
straight duplicate allocation.

### Eager Raw Hash Inputs

`parse_transaction_info` builds `RawTxParts` as owned vectors for all inputs,
outputs, output data, cell deps, and header deps. `load_resolved_inputs` also
loads every resolved input output and data into owned vectors.

OTX hashing only needs ranges selected by relevant OTX layouts. Loading all
raw parts up front makes relevance-aware query flow less useful from a memory
perspective.

### View DTOs Own Hash Payloads And Reader Infrastructure

`view.rs` converts message bytes, masks, and seals from cursors into `Vec<u8>`.
This is acceptable for small values and returned seals, but it should not be
the default for hash inputs that can be streamed from a cursor.

`view.rs` also owns `OwnedReader`, `cursor_from_slice`, `cursor_bytes`, and
`update_cursor`. Those are reader/hash support utilities, not protocol-view
conversion responsibilities.

## Goals

- Preserve protocol semantics, signing hash bytes, witness layout rules, lock
  ABI, and exit code numbers.
- Remove duplicated trailing witness storage.
- Avoid loading the whole transaction into memory on the lock path.
- Avoid loading all raw transaction parts and all resolved input data before
  relevance is known.
- Keep `cobuild-core` independent from `ckb-std` syscalls.
- Keep generated `entity` modules out of core public abstractions.
- Keep `unsafe` out of core and lock code.
- Keep `entry.rs` as the high-level contract flow.
- Keep tests expressive around streaming behavior and source boundaries.
- Make `view.rs` a clean Molecule-to-core protocol view boundary.
- Remove default owned payload conversion from view DTOs unless ownership is
  needed by an external consumer such as signature verification.

## Non-Goals

- No Molecule schema changes.
- No witness union id changes.
- No BLAKE2b personalization changes.
- No signing preimage order or framing changes.
- No legacy `WitnessArgs` fallback.
- No `ckb-auth`.
- No broad public compatibility layer for old Rust helper APIs.
- No full lifetime-heavy borrowed API rewrite unless it is required for
  streaming. Cursor-backed owned reader handles are acceptable.
- No change to verifier behavior or lock args parsing.

## Recommended Architecture

Use a source-oriented design with three layers:

1. `cobuild-core` owns protocol parsing, layout scanning, query orchestration,
   and hash construction over abstract data sources.
2. `contracts/cobuild-otx-lock` owns syscall-backed source implementations.
3. Tests can use owned in-memory source implementations.

This avoids adding `ckb-std` to core while allowing the lock contract to parse
and hash without materializing whole transaction data.

Source-backed cursors must carry error-classification metadata. Molecule
`Cursor::read_at` can only return lazy-reader errors, so core cannot infer
whether a failed read came from malformed protocol bytes or from a source
failure. Core should therefore use a small wrapper around `Cursor` for data
obtained from a `TransactionSource`:

```rust
pub enum CursorReadContext {
    Protocol,
    SourceInput,
    HashInput,
}

pub struct ClassifiedCursor {
    pub cursor: Cursor,
    pub read_context: CursorReadContext,
}
```

The exact names can change, but the behavior must not: reader/hash helpers must
map read failures according to the cursor context. Protocol/view cursors map
failed reads to malformed Cobuild data. Transaction/script source cursors map
failed source reads to internal/source input failure. Raw transaction,
resolved-input, and witness hash cursors map failed source reads to
`MissingHashInput` or an equivalent internal hash-input failure. This preserves
public lock exit-code categories after owned buffers are removed.

Implement this as two rounds:

1. **Round 1: Streaming source and hash input boundary.** Remove full
   transaction loading, duplicate trailing witnesses, eager raw hash inputs,
   and owned resolved input data from the lock verification path.
2. **Round 2: Clean cursor-backed view layer.** After the streaming boundary is
   stable, make `view.rs` purely responsible for protocol view conversion and
   replace owned DTO payloads with cursor-backed views where ownership is not
   semantically required.

Both rounds must pass the full verification matrix. Round 2 is not optional;
it is separated only to keep failures diagnosable and avoid coupling hash
source migration bugs with protocol-view migration bugs.

## Core Module Changes

### `reader.rs`

Introduce a focused reader module for lazy-reader support:

```text
crates/cobuild-core/src/reader.rs
```

Responsibilities:

- `OwnedReader` for tests and host-side byte slices;
- `cursor_from_slice` for tests and script args parsing;
- `cursor_bytes` only for values that must become owned;
- `update_cursor` for streaming cursor contents into hashers;
- small helpers for length-prefixed cursor hashing.

`view.rs` should stop owning reader infrastructure. It should consume cursors
and produce protocol view data.

### `source.rs`

Introduce source traits that core can use without depending on syscalls:

```rust
pub trait TransactionSource {
    fn transaction_cursor(&self) -> Result<ClassifiedCursor, CoreError>;
    fn script_cursor(&self) -> Result<ClassifiedCursor, CoreError>;
    fn tx_hash(&self) -> Result<[u8; 32], CoreError>;
    fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError>;
    fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError>;
    fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError>;
    fn resolved_input_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn resolved_input_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
}
```

The trait name can be adjusted during implementation, but the boundary must
stay clear: core asks for cursors and hashes; lock code decides how to load
them.

Header dep hashes are already fixed `[u8; 32]` values inside the transaction
reader. They can continue to be read from the transaction cursor rather than
through a separate syscall source.

### `loader.rs` To `prepare.rs`

Rename core `loader.rs` to `prepare.rs`.

Responsibilities:

- parse transaction counts from a transaction cursor;
- scan witness layout from transaction witness cursors;
- build `PreparedContext`;
- build `ScriptHashIndex` from source-provided script hashes;
- keep tx hash in the signing source state.

This module should no longer return `TransactionInfo` with owned witnesses and
owned raw parts.

### Context Types

Replace:

```rust
TxScriptHashes
LayoutTx with owned witness bytes
raw_parts: Option<RawTxParts>
SigningHashParts with owned trailing witness bytes
```

with names that reflect the new model:

```rust
ScriptHashIndex
WitnessSource
SigningSource
PreparedContext
```

`CobuildContext` should hold:

- transaction counts;
- script hash index;
- witness layout scan result;
- access to witness cursors needed by query and hashing.

It should not hold a separate owned `RawTxParts` blob.

### Hash Inputs

Replace `RawTxParts` and owned `ResolvedInputHashPart` with streaming access:

```rust
pub trait SigningDataSource {
    fn tx_hash(&self) -> Result<[u8; 32], CoreError>;
    fn input_count(&self) -> Result<usize, CoreError>;
    fn witness_count(&self) -> Result<usize, CoreError>;
    fn witness_cursor(&self, absolute_index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_input_cursor(&self, tx_index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_output_cursor(&self, tx_index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_output_data_cursor(&self, tx_index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_cell_dep_cursor(&self, tx_index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError>;
    fn resolved_input_output_cursor(&self, tx_index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn resolved_input_data_cursor(&self, tx_index: usize) -> Result<ClassifiedCursor, CoreError>;
}
```

The implementation may merge `TransactionSource` and `SigningDataSource` if
one trait reads better. The spec requires the behavior, not the exact trait
names.

`tx_with_message_hash` and `tx_without_message_hash` should stream:

- optional message cursor or bytes;
- tx hash;
- resolved input output cursor;
- length-prefixed resolved input data cursor;
- length-prefixed trailing witness cursors from `input_count..witness_count`.

OTX base/append hash should stream raw inputs, outputs, output data, cell deps,
header deps, and resolved input data by index.

Witness indexes are absolute transaction witness indexes. The tx-level trailing
witness loop must iterate `input_count..witness_count` in ascending order and
hash each witness exactly once. Do not use a trailing-relative index API unless
the implementation also makes the conversion from absolute transaction order
explicit and tested.

### View DTOs

Round 2 should replace the current owned DTO shape with a clean cursor-backed
view model.

Target names:

```text
WitnessLayoutView
SighashAllWitnessView
OtxStartView
OtxView
SealPairView
MessageActionView
```

The exact names can be adjusted during implementation, but the vocabulary must
make these values views over Molecule data, not owned data records.

Required changes:

- message payload should be available as a cursor for hashing and validation;
- masks should be cursor-backed bytes or a small `MaskView` that reads bits
  without cloning the whole mask;
- seal payloads should remain cursor-backed inside `SealPairView` and be copied
  only when producing `SignatureRequest`;
- message action validation should parse directly from a message cursor instead
  of requiring `message: Vec<u8>`.
- `OwnedReader`, `cursor_from_slice`, `cursor_bytes`, and `update_cursor`
  should live in `reader.rs`, not `view.rs`;
- `view.rs` should not expose generated lazy-reader internals, but it may expose
  core-owned cursor-backed view types;
- helper names ending in `Data` should be removed or renamed when the value is
  no longer owned data.

Do not expose generated lazy-reader entity internals as public core types.
Cursor-backed core DTOs are allowed.

Owned bytes are still allowed at the final boundary where an owned value is
needed:

- `SignatureRequest.seal`;
- public helper APIs that intentionally return script args or test fixtures;
- small copied values such as `[u8; 32]` hashes and numeric counts.

## Lock Crate Changes

Rename lock `loader.rs` to `chain.rs` or `syscalls.rs`. The recommended name is
`chain.rs` because it describes the CKB chain/syscall boundary without tying
the module to one syscall implementation detail.

Responsibilities:

- implement syscall-backed readers for transaction, script, input cell, and
  input cell data;
- map `SysError` to stable contract errors;
- provide a `ChainSource` implementing the core source traits;
- call `prepare_context_from_source`;
- keep `entry.rs` focused on args parsing, source preparation, query, and
  verification.

The lock crate must not reimplement Cobuild protocol parsing, OTX layout
rules, hash preimage construction, message validation, or seal matching.

## Data Flow

The intended contract flow is:

```text
entry.rs
  -> parse auth args from script cursor
  -> build ChainSource
  -> core::prepare::prepare_context_from_source(&source)
       -> parse transaction cursor
       -> collect counts
       -> collect script hash index
       -> scan witness layout using witness cursors
  -> prepared.context.lock_query(current_script_hash)
       .required_signatures(&source)
       -> collect SighashAll request
       -> stream tx signing hash from source
       -> collect relevant OTX requests
       -> stream OTX hashes from source only when relevant
  -> verify each SignatureRequest
```

There should be no owned `trailing_witnesses` vector and no full
`RawTxParts` vector on the lock path.

Witness layout discovery is still allowed to read witness layout cursors before
a queried lock is known to be OTX-relevant. The no-unnecessary-read requirement
applies to hash payloads: raw transaction parts and resolved input output/data
for irrelevant OTX ranges must not be read.

After Round 2, the flow from witness parsing to hash construction should remain
cursor-backed:

```text
witness cursor
  -> WitnessLayoutView
  -> SighashAllWitnessView / OtxView
  -> message cursor, mask cursor, seal cursor
  -> validation/hash streaming
  -> copy seal only when building SignatureRequest
```

## Error Handling

Existing public lock error categories remain stable.

Core should keep a precise split:

- malformed Cobuild or invalid protocol data:
  - `MalformedCobuild`
  - `InvalidOtxLayout`
  - `InvalidMessageTarget`
  - `DuplicateSighashAll`
  - `MissingLockGroupCoverage`
  - `MissingSealPair`
  - `DuplicateSealPair`
  - `InvalidSealScope`
- source/syscall/internal input failures:
  - `InvalidContextInput`
  - `MissingHashInput`
  - `HashInputTooLarge`

Source cursor read failures must be classified by cursor context as described
above. Do not collapse source/hash-input failures into malformed protocol data
or malformed protocol data into syscall/source failures. If a source accessor is
called with an out-of-range index while building a relevant hash preimage, map
that to `MissingHashInput` or an equivalent internal hash-input error.

## Testing Requirements

Keep the existing verification matrix:

```bash
cargo test --workspace --offline
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Add focused tests for the new boundary:

- `SigningHashParts` no longer contains `trailing_witnesses`.
- `PreparedContextInput` no longer contains `trailing_witnesses`.
- lock loader no longer calls `load_transaction()` into a full `Vec<u8>`.
- tx signing hash streams trailing witnesses through a source/cursor API.
- OTX hash reads only ranges required by relevant OTX requests.
- an unrelated OTX lock query may read witness layout cursors, but does not read
  resolved input data or raw OTX hash inputs.
- source-boundary tests ensure `cobuild-core` does not import `ckb_std`.
- source-boundary tests ensure `cobuild-core` still does not import generated
  `entity` modules.
- source-boundary tests ensure no `unsafe` is introduced.
- `view.rs` does not define `OwnedReader`, `cursor_from_slice`,
  `cursor_bytes`, or `update_cursor`.
- `view.rs` does not expose DTO names ending in `Data` for cursor-backed
  values.
- message validation accepts a cursor-backed message view and does not require
  `Vec<u8>`.
- OTX hash paths use cursor-backed message and mask views.
- seal payload is copied only at `SignatureRequest` construction.

Tests should include a counting fake source that records which cursors were
requested. That gives direct evidence that irrelevant data is not loaded.

## Implementation Order

The implementation plan should be conservative even though Rust API
compatibility is not required. Split it into two rounds.

Round 1:

1. Move reader helpers out of `view.rs` into `reader.rs` as a prerequisite for
   source-backed cursors. DTO and view naming cleanup remains Round 2.
2. Remove `trailing_witnesses` from `SigningHashParts` and derive it from the
   witness source.
3. Introduce in-memory source traits and adapt tests.
4. Introduce syscall-backed source in the lock crate.
5. Replace full transaction loading with transaction cursor parsing.
6. Replace `RawTxParts` and owned resolved inputs with streaming hash source
   reads.

Round 2:

7. Replace `SighashAllWitnessLayout`, `OtxStartData`, `OtxData`,
   `SealPairData`, and `ActionData` with view-oriented names.
8. Convert message validation from owned message bytes to message cursor/view
   parsing.
9. Convert OTX message and masks from owned `Vec<u8>` fields to cursor-backed
   view fields.
10. Keep seal payloads cursor-backed until `SignatureRequest` construction.
11. Rename modules after the data flow and view boundary are stable.

## Deferred Work

These items are intentionally outside this phase unless they become necessary
for the streaming boundary:

- changing Cobuild Molecule schemas;
- changing verifier implementation details;
- changing auth args format;
- optimizing small copied seals and masks that are not material to memory
  pressure, unless they are part of the Round 2 view cleanup boundary;
- introducing a generic async or allocator-free IO framework.

## Acceptance Criteria

- No full transaction bytes are loaded into an owned `Vec<u8>` on the
  `cobuild-otx-lock` verification path.
- No separate owned `trailing_witnesses` vector exists.
- Relevant tx-level and OTX signing hashes are byte-for-byte unchanged.
- Irrelevant OTX hash payloads are not read from source during unrelated lock
  queries; witness layout cursors may still be read for protocol layout
  discovery.
- Core has no dependency on `ckb_std`.
- Core public abstractions do not expose generated entity module types.
- `view.rs` only owns Molecule-to-core protocol view conversion.
- reader/hash cursor helpers live outside `view.rs`.
- Cursor-backed view names replace owned `*Data` DTO names where payloads are
  not actually owned by design.
- The full verification matrix passes offline.
