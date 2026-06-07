# Limit Order Test Type Script Spec

## Status

This spec defines the first Cobuild OTX test type script. It is a test fixture under `tests`, not a production order protocol.

The first implementation stage covers only full fills of an existing order cell. `CreateOrder`, partial fills, and cancellation are intentionally left for later stages.

## Goal

Validate the core Cobuild OTX intent + append settlement flow from a type script:

- a base OTX consumes an order cell;
- a `FillOrder` action targets the order type as `input_type`;
- append outputs provide settlement to the order owner;
- the order type script verifies that the append settlement satisfies the limit price.

## Location

The contract lives under:

```text
tests/contracts/limit-order
```

It is part of the test workspace and build matrix only. It must not add features, error variants, or dependencies to production contracts.

## State ABI

Order cell data is fixed-width little-endian data:

```text
order_id: [u8; 32]
owner_lock_hash: [u8; 32]
offered_asset_id: [u8; 32]
requested_asset_id: [u8; 32]
offered_remaining: u64
min_requested_per_offered: u64
nonce: u64
```

The encoded length is 152 bytes.

`min_requested_per_offered` is an integer ratio in test units. A full fill requires:

```text
requested_paid_to_owner >= offered_remaining * min_requested_per_offered
```

Overflow fails closed.

## Settlement Cell ABI

For the MVP, settlement assets are represented by ordinary test cells. The cell lock must hash to `owner_lock_hash`, and the cell data is:

```text
asset_id: [u8; 32]
amount: u64
```

The encoded length is 40 bytes.

This avoids coupling the first OTX type-script test to a full token standard. A later integration stage can replace or supplement this with `tests/contracts/test-udt`.

## Action ABI

`Action.data` uses a test Molecule-style envelope owned by this fixture:

```text
variant: u8
payload: variant-specific bytes
```

MVP variant:

```text
1 = FillOrder

FillOrder payload:
order_id: [u8; 32]
requested_asset_id: [u8; 32]
offered_amount: u64
min_requested_amount: u64
```

The first byte is a union discriminant, matching the intended generated Molecule union shape. The MVP keeps the on-chain reader local and fixed-width; once the shared test action schema is introduced, this ABI should move to generated Molecule readers without changing the semantic fields.

The script accepts only `FillOrder`. Unknown variants fail with an action mismatch error.

## Action Target

`FillOrder` must target the current order type script with role `input_type`.

Cobuild Core is responsible for filtering actions by target hash and role. The Limit Order contract additionally requires the related action to be OTX-level and to have an OTX relation where the current type appears in the base input scope.

Tx-level `FillOrder` is rejected in this MVP. This keeps the first fixture focused on OTX append settlement.

## Validation Rules

For each transaction group using the Limit Order type:

1. There must be exactly one group input order cell.
2. There must be no group output order cell. The MVP is full-fill only.
3. There must be exactly one related `FillOrder` action.
4. The `FillOrder` action must originate from an OTX message.
5. The current order type must be present in the OTX base input relation.
6. The action `order_id`, `requested_asset_id`, and `offered_amount` must match the input order state.
7. The action `min_requested_amount` must be at least the order state's required amount.
8. Settlement outputs counted for the fill are limited to that OTX's `base_outputs` and `append_outputs`.
9. Counted settlement outputs must be locked to `owner_lock_hash` and carry `requested_asset_id`.
10. The counted settlement amount must be at least `min_requested_amount`.

Unrelated actions are ignored by Cobuild Core because their target does not match the current type hash. If an action targets this type with an unsupported variant, the script fails closed.

## Required Passing Cases

- A full fill where the append output pays the owner at exactly the required amount.
- A full fill where the append output pays the owner more than required.
- A message that also contains unrelated actions targeting another script hash.

## Required Failing Cases

- Price too low: settlement to owner is below the required amount.
- Output goes to the wrong lock hash.
- Output uses the wrong requested asset id.
- `FillOrder` appears as tx-level action.
- `FillOrder` target role is `output_type`.
- The order output remains present, which would imply a partial fill or incorrect full-fill state.
- Action data is malformed or uses an unsupported variant.
- Required amount multiplication overflows.

## Cobuild Semantics Covered

This fixture proves:

- `input_type` action target selection reaches the order type script.
- OTX-level action origin is visible to type scripts.
- OTX `base_*` and `append_*` ranges can constrain settlement visibility.
- base intent can constrain append outputs.
- tx-level remainder is not accidentally counted as OTX settlement.
- unsupported or malformed actions fail at the script boundary.

## Out of Scope

- Production-grade matching or order book behavior.
- Partial fills and remaining amount updates.
- Owner authorization for cancellation.
- Complete token standard enforcement.
- Off-chain solver SDKs.
- Multi-order batching.
