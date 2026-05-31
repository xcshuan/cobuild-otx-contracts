# Cobuild Core Safe Reader And Copy Reduction Design

## Status

This document defines Phase 2A production hardening for the clean Cobuild OTX
implementation.

Phase 2A follows the completed validation hardening phase. It focuses on
removing unsafe reader lifetime erasure from `cobuild-core` and reducing
obvious hot-path byte copies without changing protocol semantics.

## Related Baselines

- `docs/superpowers/specs/2026-05-28-cobuild-core-community-redraft-design.md`
- `docs/superpowers/specs/2026-05-29-clean-cobuild-otx-contracts-design.md`
- `docs/superpowers/specs/2026-05-31-cobuild-validation-and-safety-hardening-design.md`
- `docs/CobuildAgentDevelopGuide.md`

If this document conflicts with the Core protocol spec, the Core protocol spec
wins.

## Problem Statement

`cobuild-core::view` currently adapts borrowed byte slices into generated
Molecule lazy readers by erasing a reader lifetime with `unsafe transmute`.
The surrounding API attempts to prevent reader escape, but the safety argument
depends on discipline instead of Rust's type system.

The current hot path also materializes several byte ranges into owned `Vec<u8>`
before hashing. This is acceptable for the Phase 1 functional baseline, but it
adds avoidable allocation and makes the code harder to audit.

Phase 2A should make the reader boundary memory-safe by construction and remove
low-risk temporary copies where the implementation can still preserve current
public APIs and hash behavior.

## Goals

- Remove `unsafe` from `crates/cobuild-core/src/view.rs`.
- Remove the lifetime-erasing reader adapter currently used by
  `cursor_from_slice`.
- Keep Core v1 witness, layout, task, and hash semantics unchanged.
- Keep `cobuild-otx-lock` thin and avoid moving protocol logic into the lock
  crate.
- Preserve existing public task shapes unless a narrow internal helper can
  reduce copies without API churn.
- Reduce obvious temporary `Vec<u8>` allocations in hash construction where the
  source is already a cursor and the bytes can be streamed directly into the
  hasher.
- Add regression tests that prove digest outputs and task behavior remain
  unchanged.
- Keep generated Molecule output committed and unchanged unless codegen drift is
  explicitly detected.

## Non-Goals

- Rewriting all Core data structures into fully borrowed views.
- Replacing `PreparedContextInput`, `RawTxParts`, or `TxHashParts` with
  borrowed lifetime-heavy API in this phase.
- Changing `OtxData`, `SealPairData`, or `TxLevelWitness` public field types
  unless the implementation plan proves the change is local and low risk.
- Changing Core hash preimage order, framing, personalization, or masks.
- Adding `ckb-auth`.
- Adding legacy `WitnessArgs` fallback.
- Optimizing generated `cobuild-types` warning output.

## Design Direction

### Safe Reader Boundary

The preferred implementation is to make `cursor_from_slice` use an owned byte
reader instead of a borrowed slice reader with lifetime erasure.

The reader should own an `alloc::vec::Vec<u8>` and implement the generated
lazy-reader `Read` trait safely. A cursor created from this reader can then
store `Box<dyn Read>` without pretending that borrowed data lives longer than
it does.

This trades one explicit copy at the view boundary for removal of unsafe code.
That is an acceptable Phase 2A tradeoff because:

- witness and transaction bytes are already owned by the contract loader before
  Core views parse them;
- safety and auditability are higher priority than micro-optimizing this
  boundary;
- lower-level cursor streaming can still reduce later temporary copies during
  hashing.

The old `SliceReader<'a>` and `erase_reader_lifetime` functions should be
removed unless a safe lifetime-preserving generated-reader approach is available
without changing generated code.

### Cursor Streaming Helpers

`cursor_bytes(&Cursor) -> Vec<u8>` is useful where owned bytes are part of a
public task or view object. It should remain available if needed.

For hash construction, add a helper that streams a cursor into a hasher without
returning an owned `Vec<u8>`. Suggested shape:

```rust
pub(crate) fn update_cursor(
    hasher: &mut blake2b_ref::Blake2b,
    cursor: &Cursor,
) -> Result<(), CoreError>
```

This helper should:

- read the cursor in bounded chunks;
- update the hasher incrementally;
- fail with `CoreError::MalformedCobuild` if the cursor cannot be fully read.

Use it in hash paths that currently do:

```rust
hasher.update(&cursor_bytes(&some_cursor)?);
```

Do not use streaming where the Core hash spec requires a length prefix unless
the implementation still writes the length first. The helper only replaces raw
Molecule object byte updates, not length-framed `Bytes` fields.

### Copy Reduction Scope

Phase 2A copy reduction is intentionally narrow:

- Replace temporary cursor-to-`Vec` conversions inside `otx_base_hash` when
  hashing `previous_output`, output `lock`, and output `type`.
- Consider the same pattern in loader parsing only if it does not change data
  ownership contracts. Loader-owned `RawTxParts` may remain owned for now.
- Do not redesign `TxHashParts`, `RawTxParts`, or `ResolvedInputHashPart` in
  this phase.

This gives a measurable cleanup while avoiding a broad lifetime-heavy rewrite.

## Testing Requirements

### Safety Boundary Tests

Tests should cover:

- `WitnessLayoutView::from_slice` continues to reject empty or malformed data.
- `cursor_from_slice` no longer requires unsafe lifetime erasure.
- The source-boundary test or a new static test fails if `unsafe` reappears in
  `crates/cobuild-core/src/view.rs`.

The static unsafe check should be precise enough not to ban unrelated future
unsafe in other modules without an explicit design decision.

### Hash Regression Tests

Tests should prove that streaming cursor updates do not change digests:

- existing tx-level hash tests must continue to pass;
- existing OTX base and append hash tests must continue to pass;
- add a focused OTX base hash regression where `previous_output`, output
  `lock`, and output `type` are all covered and therefore use cursor streaming.

The regression should compare against an independently built expected preimage,
not against the implementation under test.

### Integration And Verification

The Phase 2A implementation is complete only when this matrix passes:

```bash
cargo run -p xtask --offline -- codegen cobuild-types --check
cargo test --workspace --offline
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
rg -n "unsafe" crates/cobuild-core/src/view.rs
```

The final `rg` command must print no matches.

Existing generated-code warnings remain non-blocking.

## Boundary Requirements

- `cobuild-core` must still use `cobuild_types::lazy_reader`, not
  `cobuild_types::entity`, in chain-facing code.
- `cobuild-otx-lock` must not parse Cobuild witness details or build Core hash
  preimages directly.
- No local `critical-section` shim or unsafe `portable-atomic` assumptions may
  be added.
- Public lock args ABI remains unchanged.
- Public contract exit codes remain unchanged.

## Risks

Using an owned reader at `cursor_from_slice` may copy input bytes more often
than the old unsafe borrowed reader. This is acceptable for Phase 2A if it
removes unsafe and the later hash path avoids additional temporary copies.

Changing hash update helpers can accidentally alter preimage bytes if a length
prefix is omitted or added in the wrong place. This risk must be managed with
focused independent expected-hash tests.

Trying to remove every allocation in one pass would likely force broad API
changes. That is explicitly out of scope.

## Success Criteria

Phase 2A is complete when:

- `crates/cobuild-core/src/view.rs` contains no `unsafe`;
- no lifetime-erasing transmute remains in `cobuild-core`;
- hash and task behavior is unchanged under the verification matrix;
- at least the obvious cursor-to-`Vec` hash temporaries are replaced by
  streaming updates;
- production dependency boundary checks still show no `cobuild_types::entity`
  usage in `cobuild-core` or `cobuild-otx-lock`;
- all changes are committed in focused commits.

## Deferred Work

Later phases may:

- redesign `RawTxParts`, `TxHashParts`, and `OtxData` into borrowed views;
- benchmark cycle and memory impact under realistic transaction sizes;
- introduce a `ckb-auth` verifier adapter;
- evaluate whether generated lazy-reader support should expose a safe borrowed
  cursor API directly.
