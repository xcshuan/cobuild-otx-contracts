# Limit Order CreateOrder Design

## Status

This spec extends the tests-only `limit-order-type` fixture. It supersedes the
earlier fill-only order ABI for the next implementation stage.

The goal is to add order creation for the NFT-for-UDT scenario while keeping the
contract tests-only and scoped to one NFT order. It does not define a production
order book, cancel flow, partial fill, or multi-asset protocol.

## Goal

Add a `CreateOrder` action that validates creation of a unique order cell and
checks that the offered NFT has been transferred into the input-type proxy lock
for that order.

After creation, the existing fill path remains payment-focused:

- create path proves the NFT is held by the proxy lock for this order;
- fill path proves the maker receives enough requested asset;
- the NFT cell itself is unlocked by `input-type-proxy-lock` when the matching
  order type input is present.

## Architecture

`limit-order-type` becomes a dual-mode type script. The active mode is selected
from the current type group shape:

```text
0 group inputs, 1 group output  -> CreateOrder validation
1 group input, 0 group outputs  -> FillOrder validation
anything else                   -> fail closed
```

Each order cell uses type-id args following `ckb-std` type-id creation rules.
That means each order is a distinct type group and the order identity is the
order cell's type args/script hash. The ABI must not carry a separate
`order_id`.

The fill path continues to use Cobuild OTX settlement scope. The create path is
not an append protocol: it is expected to be created by the maker using a
SighashAll transaction, though the script does not hard-code tx-level origin.

## State ABI

Order cell data is fixed-width little-endian data:

```text
owner_lock_hash: [u8; 32]
offered_nft_type_hash: [u8; 32]
requested_asset_id: [u8; 32]
min_requested_amount: u64
```

The encoded length is 104 bytes.

There is no `order_id`: order identity is the current `limit-order-type` type-id
args/script hash. There is no `offered_remaining`: this stage only supports
selling one type-id NFT. There is no `nonce`: type-id args provide uniqueness.

## Action ABI

`Action.data` remains a tests-only local fixed-width ABI:

```text
variant: u8
payload: variant-specific bytes
```

Variants:

```text
1 = CreateOrder
2 = FillOrder
```

`CreateOrder` payload mirrors the order state:

```text
owner_lock_hash: [u8; 32]
offered_nft_type_hash: [u8; 32]
requested_asset_id: [u8; 32]
min_requested_amount: u64
```

`FillOrder` payload:

```text
requested_asset_id: [u8; 32]
min_requested_amount: u64
```

`FillOrder` no longer carries `order_id` or `offered_amount`. The order is the
current input type group, and the offered NFT amount is fixed to one.

## CreateOrder Validation

In create mode, the type script must:

1. Build the Cobuild type validation plan for the current type hash.
2. Require exactly one related `CreateOrder` action.
3. Parse the single group output order state.
4. Verify output type args satisfy `ckb-std` type-id creation rules.
5. Verify the order state exactly matches the `CreateOrder` payload.
6. Compute the expected proxy lock hash for:

```text
input-type-proxy-lock(args = current_order_type_hash)
```

7. Scan all transaction outputs and require at least one NFT output where:

```text
type_hash == offered_nft_type_hash
lock_hash == expected_proxy_lock_hash
```

The create path intentionally does not require the NFT output to be inside an
OTX range. The maker is creating the order with SighashAll, so the whole
transaction is their construction choice.

The create path does not check NFT data. The NFT type script hash is type-id
based and globally unique for this test fixture, so it identifies the NFT.

## Proxy Lock Hash Source

The `limit-order-type` script needs to know which proxy lock code hash is valid.
For this tests-only fixture, the proxy lock code hash is a hardcoded or
compile-time-written constant for the vendored
`tests/vendor/ckb-proxy-locks/contracts/input-type-proxy-lock` Data2 script.

This is not a production upgrade mechanism. If the vendored proxy lock binary
changes, the tests or build step must update the constant. The order state,
action payload, and type args do not include the proxy lock code hash.

## FillOrder Validation

In fill mode, the type script must:

1. Require exactly one related `FillOrder` action.
2. Require the action to be OTX-level and related to the current type in the OTX
   base input scope.
3. Parse the single group input order state.
4. Verify action `requested_asset_id == state.requested_asset_id`.
5. Verify action `min_requested_amount >= state.min_requested_amount`.
6. Count settlement only from the current OTX `base_outputs` and
   `append_outputs`.
7. Verify the owner receives at least `action.min_requested_amount` of the
   requested asset.

The fill path continues not to scan or match the NFT. The NFT was already
escrowed into the proxy lock during create, and the proxy lock authorizes the
NFT input when this order type input is present.

## Fail-Closed Rules

The script must fail for:

- group shape other than `0 input, 1 output` or `1 input, 0 output`;
- missing or multiple related actions for the active mode;
- `CreateOrder` action/state mismatch;
- invalid type-id args in create mode;
- missing NFT output locked to the expected proxy lock;
- NFT output with wrong type hash;
- NFT output locked to a proxy for a different order type hash;
- `FillOrder` action/state mismatch;
- `FillOrder` below the order's `min_requested_amount`;
- insufficient payment in the current OTX settlement scope;
- tx-level remainder or another OTX paying enough while the current OTX does not.

## Test Coverage

CreateOrder passing case:

- SighashAll transaction creates one order output whose type args satisfy
  type-id rules.
- Same transaction creates or transfers an NFT output with:

```text
type = offered_nft_type_hash
lock = input-type-proxy-lock(args = current_order_type_hash)
```

- `CreateOrder` action matches the order output state.

CreateOrder failing cases:

- missing NFT proxy output;
- wrong NFT type hash;
- NFT locked by a proxy for another order type hash;
- order state/action mismatch;
- invalid type-id args;
- unsupported group shape such as input plus output.

FillOrder regression cases:

- update existing fill fixtures to the new state/action ABI;
- valid NFT-for-UDT fill still passes;
- insufficient UDT, wrong UDT, wrong owner, tx-level remainder, payment in
  another OTX, and action mismatches still fail closed.

## Out Of Scope

- production-grade order protocol;
- cancel;
- partial fill;
- multi-order settlement binding;
- generic offered asset quantities;
- including the proxy lock code hash in public action/state ABI;
- modifying `contracts/cobuild-otx-lock`, `cobuild-core`, or public
  `cobuild-types` schemas.
