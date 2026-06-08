# Limit Order Type Test Script Spec

## Status

This spec defines the first Cobuild OTX test type script. The contract name is
`limit-order-type`. It is a test fixture under `tests`, not a production order
protocol.

The first implementation stage covers only full fills of an existing order cell. `CreateOrder`, partial fills, and cancellation are intentionally left for later stages.

## Goal

Validate the core Cobuild OTX intent + append settlement flow from a type
script:

- a base OTX consumes an order cell;
- a `FillOrder` action targets the order type as `input_type`;
- append outputs provide settlement to the order owner;
- the order type script verifies that the append settlement satisfies the limit
  price.

The order type does not validate the offered asset cell itself. Asset custody is
the lock layer's responsibility. For NFT-for-UDT tests, the offered NFT cell is
locked by an input-type proxy lock whose args identify the `limit-order-type`
script hash. The proxy lock permits the NFT input to be spent when a matching
`limit-order-type` input is present. The order type only checks that the owner
is paid enough requested asset inside the same OTX scope.

## Location

The contract lives under:

```text
tests/contracts/limit-order-type
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

`offered_asset_id` describes the asset the order maker intends to sell. In the
NFT-for-UDT fixture this can be the NFT type script hash, but the
`limit-order-type` script does not scan transaction inputs to match or validate
the NFT cell. This field remains part of the order data so the test order has a
complete description, and so later tests can add stronger per-asset settlement
binding without changing the base state shape.

## Settlement ABI

For the MVP, settlement assets are represented by ordinary test cells. The cell lock must hash to `owner_lock_hash`, and the cell data is:

```text
asset_id: [u8; 32]
amount: u64
```

The encoded length is 40 bytes.

This avoids coupling the first OTX type-script test to a full token standard. A later integration stage can replace or supplement this with `tests/contracts/test-udt`.

For the NFT-for-UDT stage, settlement can be represented by a `tests/contracts/test-udt`
output instead of the ordinary settlement cell:

```text
lock_hash: owner_lock_hash
type_hash: requested_asset_id
data: amount as little-endian u128
```

The `limit-order-type` script may count either supported settlement shape. It
must count only cells inside the current OTX settlement scope, and it must not
count unrelated tx-level outputs.

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

The script accepts only `FillOrder`. Unknown variants fail with an action
mismatch error.

## Action Target

`FillOrder` must target the current order type script with role `input_type`.

Cobuild Core is responsible for filtering actions by target hash and role. The
`limit-order-type` contract additionally requires the related action to be
OTX-level and to have an OTX relation where the current type appears in the base
input scope.

Tx-level `FillOrder` is rejected in this MVP. This keeps the first fixture focused on OTX append settlement.

## Asset Custody Boundary

The order type validates payment, not ownership transfer of the offered asset.
This is intentional:

- CKB inputs are authorized by lock scripts.
- A type script on the order cell cannot unlock an unrelated NFT or UDT cell by
  itself.
- To let an order authorize an offered NFT, the NFT cell must use a proxy lock
  that delegates unlock permission to the presence of a matching order type
  input.

The NFT-for-UDT fixture therefore uses this transaction shape:

```text
base inputs:
  order cell:
    type = limit-order-type
    lock = maker or test lock
  NFT cell:
    type = test-nft
    lock = input-type-proxy-lock(limit_order_type_hash)

append outputs:
  UDT payment to maker:
    type = test-udt
    lock hash = owner_lock_hash
    amount >= required payment
```

The buyer or solver is responsible for including the NFT input and a desired NFT
output. If they omit the NFT transfer, they may still pay the maker and fill the
order, but they do not receive the NFT. This is a transaction construction
failure by the buyer, not a failure the order type must detect.

Wrong NFT cases belong to the proxy-lock or NFT-transfer test layer. For
example, an NFT locked by a proxy whose args do not match the `limit-order-type`
hash must fail in that NFT input's lock script. It must not be modeled as a
`limit-order-type` validation failure.

## Settlement Reuse Boundary

The MVP deliberately avoids multi-order batching. A plain payment output locked
to the maker cannot prove by itself that it was uniquely allocated to one order.
If several order types in the same visible scope all count the same output, they
could all consider themselves paid.

To keep the fixture sound for its intended Cobuild semantics, the test protocol
uses these constraints:

1. One OTX fills one order.
2. One order input appears in the current type group.
3. One related `FillOrder` action targets the current order type.
4. Settlement is counted only from the current OTX's `base_outputs` and
   `append_outputs`.
5. Tx-level remainder and other OTXs' outputs are never counted.

This prevents tx-level remainder reuse and cross-OTX reuse as long as Cobuild
Core enforces non-overlapping OTX ranges. It does not claim to solve production
multi-order settlement binding.

A later multi-order protocol must add a per-order binding mechanism, such as a
receipt cell, marker type, or payment lock whose args include `order_id` or the
order type hash. Merely pointing an action at an output index is insufficient,
because another type script could still point at the same output.

## Validation Rules

For each transaction group using the `limit-order-type` script:

1. There must be exactly one group input order cell.
2. There must be no group output order cell. The MVP is full-fill only.
3. There must be exactly one related `FillOrder` action.
4. The `FillOrder` action must originate from an OTX message.
5. The current order type must be present in the OTX base input relation.
6. The action `order_id`, `requested_asset_id`, and `offered_amount` must match the input order state.
7. The action `min_requested_amount` must be at least the order state's required amount.
8. Settlement outputs counted for the fill are limited to that OTX's
   `base_outputs` and `append_outputs`.
9. Counted settlement outputs must be locked to `owner_lock_hash` and carry
   `requested_asset_id`, either as the ordinary settlement cell ABI or as a
   `test-udt` output in the NFT-for-UDT stage.
10. The counted settlement amount must be at least `min_requested_amount`.
11. The script does not require, scan, or match the offered NFT cell. That check
    is outside this type script's responsibility.

Unrelated actions are ignored by Cobuild Core because their target does not match the current type hash. If an action targets this type with an unsupported variant, the script fails closed.

## Required Passing Cases

- A full fill where the append output pays the owner at exactly the required amount.
- A full fill where the append output pays the owner more than required.
- A message that also contains unrelated actions targeting another script hash.
- An NFT-for-UDT fill where the order input and NFT input are in the OTX base
  inputs, the NFT is unlocked by an input-type proxy lock, and the append scope
  pays enough `test-udt` to the owner.

## Required Failing Cases

- Price too low: settlement to owner is below the required amount.
- Output goes to the wrong lock hash.
- Output uses the wrong requested asset id.
- NFT-for-UDT payment uses the wrong UDT type hash.
- `FillOrder` appears as tx-level action.
- `FillOrder` target role is `output_type`.
- The order output remains present, which would imply a partial fill or incorrect full-fill state.
- Action data is malformed or uses an unsupported variant.
- Required amount multiplication overflows.
- A tx-level remainder output pays the owner enough, but the current OTX
  settlement scope does not.

Wrong NFT is not a required `limit-order-type` failing case. It should fail in
the offered asset lock layer when the NFT cell is not locked by a proxy that
delegates to the current order type, or it should be treated as a buyer-side
transaction construction error.

## Cobuild Semantics Covered

This fixture proves:

- `input_type` action target selection reaches the order type script.
- OTX-level action origin is visible to type scripts.
- OTX `base_*` and `append_*` ranges can constrain settlement visibility.
- base intent can constrain append outputs.
- tx-level remainder is not accidentally counted as OTX settlement.
- unsupported or malformed actions fail at the script boundary.
- payment validation can be tested independently from asset custody, while the
  offered NFT is authorized through a proxy lock.

## Out of Scope

- Production-grade matching or order book behavior.
- Partial fills and remaining amount updates.
- Owner authorization for cancellation.
- Complete token standard enforcement.
- Off-chain solver SDKs.
- Multi-order batching.
- Production-grade per-order receipt or settlement binding.
