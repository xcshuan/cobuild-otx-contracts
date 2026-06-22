# OTX Mask Default-Value Signing Design

## Status

Completed on 2026-06-22.

Implementation commit: `626c1cb feat: default uncovered otx mask fields`.

Final all-uncovered-slot golden hash:
`5ed573c86ca864c867e6523c68578c89762659c08f33abdf50a89c2fd7760120`.

Verification completed:
- `cargo fmt --check`
- `git diff --check`
- `MODE=debug cargo test --offline signing_hash -- --nocapture`
- `MODE=debug cargo test --offline`
- `MODE=release cargo test --offline`
- Final subagent code review: no findings.

Implementation notes:
- `cobuild-core` writes fixed canonical Molecule default bytes directly so the
  existing guard that keeps `ckb-std` imports isolated to syscall code remains
  satisfied.
- The test signing oracle writes the same defaults using `ckb-testtool` packed
  builders.
- Append hash semantics were not changed directly; append signatures only see
  the new base hash through their existing base-hash dependency.
- The witness schema, mask byte layout, and seal layout were unchanged.

## Goal

Change OTX base mask signing semantics from "skip uncovered fields" to
"write canonical default values for uncovered fields".

The intent of a mask remains unchanged: a mask bit says whether the signer
commits to the final transaction value of a field. The change is only in the
hash preimage shape. Uncovered fields will still not bind to the final
transaction value, but they will occupy a stable position in the preimage.

## Current Behavior

The current OTX base hash includes:

- the OTX message;
- append permissions;
- base counts;
- raw mask bytes;
- each base entity local index when that entity is visited;
- real field bytes only when the corresponding mask bit is covered.

For base outputs, uncovered `capacity`, `lock`, `type`, and `data` fields are
omitted from the preimage.

For base cell deps and header deps, uncovered items are skipped entirely,
including their local index.

For base inputs, `since` and `previous_output` are mask-controlled, while the
resolved input output and resolved input data are always covered.

The mask bytes themselves are already part of the signing hash, so changing the
coverage policy changes the hash.

## New Behavior

The OTX base hash will keep the same top-level domain, counts, mask bytes, and
field order. The difference is that every mask-controlled field writes either:

- the real final transaction value, when the mask bit is covered;
- a protocol-defined canonical default value, when the mask bit is uncovered.

This gives every base entity a fixed preimage shape.

## Default Value Table

| Scope | Field | Covered value | Uncovered default |
| --- | --- | --- | --- |
| base input | `since` | raw input `since` as little-endian `u64` | `0u64` as little-endian bytes |
| base input | `previous_output` | Molecule bytes of raw input previous outpoint | Molecule bytes of `OutPoint::new_builder().build()` |
| base input | resolved output | Molecule bytes of resolved input output | always real value |
| base input | resolved data | length-prefixed resolved input data | always real value |
| base output | `capacity` | raw output capacity as little-endian `u64` | `0u64` as little-endian bytes |
| base output | `lock` | Molecule bytes of raw output lock script | Molecule bytes of `Script::new_builder().build()` |
| base output | `type` | Molecule bytes of raw output `ScriptOpt` table field | Molecule bytes of `ScriptOpt::new_builder().build()` |
| base output | `data` | length-prefixed raw output data | length-prefixed empty bytes |
| base cell dep | item | Molecule bytes of raw cell dep | Molecule bytes of `CellDep::new_builder().build()` |
| base header dep | item | 32-byte raw header dep hash | `[0u8; 32]` |

All defaults are fixed protocol constants. They are not read from the final
transaction and cannot depend on transaction content.

## Security Model

This change does not make uncovered fields signed. An uncovered field can still
be changed by later builders without invalidating the base signature.

The safety property comes from two facts:

1. mask bytes remain covered by the signing hash;
2. uncovered fields use canonical constants rather than transaction values.

Therefore an attacker cannot reinterpret a signature made under one mask as a
signature under another mask, and cannot smuggle a real field value into the
preimage when the signer intentionally left that field flexible.

For base inputs, resolved output and data remain always covered. This preserves
the existing constraint that a signer may omit the exact previous outpoint but
still commits to the consumed cell state.

## Compatibility

This is a breaking signing-hash change for every OTX base signature that uses
any base entity. Existing fixtures and off-chain signers must be updated
together.

The serialized OTX witness format does not change:

- no Molecule schema changes;
- no mask byte layout changes;
- no seal layout changes.

Only the signing preimage changes.

## Implementation Scope

The change must be applied in both hash implementations:

- `crates/cobuild-core/src/hash/mod.rs`, used by contracts;
- `tests/src/framework/signing/otx.rs`, used by the test oracle and fixtures.

The implementation should keep append hash semantics unchanged except for its
indirect dependency on the new base hash value.

## Test Requirements

Focused signing-hash tests must prove:

- uncovered base input `since` mutation does not change the base hash;
- uncovered base input `previous_output` mutation does not change the base
  hash, while resolved output/data changes still do;
- uncovered base output `capacity`, `lock`, `type`, and `data` mutations do not
  change the base hash;
- uncovered base cell dep mutation does not change the base hash;
- uncovered base header dep mutation does not change the base hash;
- changing mask bytes still changes the base hash.

Existing positive and negative contract fixtures must continue to pass after
their signatures are regenerated with the new oracle.

## Non-Goals

- Do not change append masks or append scope semantics.
- Do not add new mask fields.
- Do not change OTX witness schemas.
- Do not change application contract behavior other than signatures being
  computed against the new base hash.
