# Limit Order Lock Design

## Status

This spec defines a new tests-only `limit-order-lock` fixture contract. It is a
simpler sibling of `tests/contracts/limit-order-type`, not a replacement for the
type-cell order fixture and not a production protocol.

The design decision for this stage is:

- creating an order does not require a Cobuild action;
- filling an order must be authorized by an OTX-level `FillOrder` action
  targeting the current input lock.

## Goal

Add a lock-script-shaped NFT-for-UDT limit order fixture where the order is the
NFT cell lock itself. The maker creates an order by transferring the NFT cell to:

```text
limit-order-lock(args = order_info)
```

The lock args carry all order information. There is no order type cell, no order
cell data, and no mutable on-chain order state. When the NFT input is spent, the
lock script validates that the current OTX pays the maker enough requested UDT.

## Architecture

`limit-order-lock` is a tests-only input lock under:

```text
tests/contracts/limit-order-lock
```

On every unlock, the script:

1. Parses the current lock args as fixed-width order data.
2. Verifies the current lock group contains exactly one input.
3. Verifies that input's type hash equals the offered NFT type hash from args.
4. Builds a Cobuild lock validation plan with:

```rust
CobuildContext::build(CurrentScript::InputLock(current_lock_hash))?
    .plan_lock_validation()?
```

5. Requires exactly one related `FillOrder` action targeting the current input
   lock.
6. Requires that action to come from an OTX witness.
7. Requires the current lock input to be inside that OTX's base input range.
8. Counts payment only from that OTX's `base_outputs` and `append_outputs`.
9. Verifies the owner receives at least the requested amount of the requested
   UDT.

The script does not verify Cobuild signatures. This lock is itself the order
authorization rule for the NFT input, not a maker signature lock.

## CreateOrder Semantics

CreateOrder is intentionally actionless.

Creating an order means constructing a transaction that outputs the NFT cell
with:

```text
type_hash == offered_nft_type_hash
lock      == limit-order-lock(args = order_info)
```

No `CreateOrder` Cobuild action is required because there is no separate order
state cell to bind to an action. The NFT cell's lock args are the order. This
keeps the fixture focused on lock-side settlement validation and avoids copying
the type-cell fixture's state/action model into the lock version.

Tests should still cover creation shape at the fixture/scenario level:

- the NFT output is locked by `limit-order-lock`;
- the NFT output type hash matches `offered_nft_type_hash`;
- malformed lock args are rejected when the cell is later spent.

The create transaction may be a normal SighashAll-style transaction assembled by
the maker, but `limit-order-lock` does not run on outputs, so it cannot enforce
create semantics at creation time.

## Lock Args ABI

The lock args are fixed-width little-endian data:

```text
owner_lock_hash: [u8; 32]
offered_nft_type_hash: [u8; 32]
requested_asset_id: [u8; 32]
min_requested_amount: u64
```

The encoded length is 104 bytes.

Rules:

- exactly 104 bytes are accepted;
- shorter or longer args fail closed;
- `owner_lock_hash` is the lock hash that must receive payment;
- `offered_nft_type_hash` is the type hash of the NFT input protected by this
  lock;
- `requested_asset_id` is the requested `test-udt` type hash;
- `min_requested_amount` is the minimum UDT amount as `u64`.

## Action ABI

`Action.data` is a tests-only local fixed-width ABI that mirrors the current
`limit-order-type` fill ABI:

```text
variant: u8
payload: variant-specific bytes
```

Supported variant:

```text
2 = FillOrder
```

`FillOrder` payload:

```text
requested_asset_id: [u8; 32]
min_requested_amount: u64
```

The encoded action data length is 41 bytes.

Rules:

- tag `2` with exactly 40 payload bytes is accepted;
- unknown tags fail closed;
- malformed payload lengths fail closed;
- tag `1` / `CreateOrder` is not accepted by this lock contract.

## Fill Validation

For the current lock group, the script must:

1. Parse lock args.
2. Require exactly one group input using the current lock.
3. Require the single group input to have a type script.
4. Require the input type hash to equal `offered_nft_type_hash`.
5. Build the Cobuild lock validation plan for the current input lock.
6. Require exactly one related action.
7. Require the related action origin to be `ActionOrigin::Otx`.
8. Require the current lock input index to be inside that OTX layout's
   `base_inputs` range.
9. Parse the related action as `FillOrder`.
10. Require `action.requested_asset_id == args.requested_asset_id`.
11. Require `action.min_requested_amount >= args.min_requested_amount`.
12. Count settlement only from the same OTX layout's `base_outputs` and
    `append_outputs`.
13. Sum only `test-udt` payment cells whose:

```text
type_hash == requested_asset_id
lock_hash == owner_lock_hash
data      == 16-byte little-endian u128 amount that fits in u64
```

14. Require the summed amount to be at least `action.min_requested_amount`.

Payment in tx-level remainder outputs, outputs outside the current OTX, or
another OTX must not count.

## Settlement ABI

The lock fixture only needs the NFT-for-UDT payment shape:

```text
lock_hash: owner_lock_hash
type_hash: requested_asset_id
data: amount as little-endian u128
```

The script may reuse local parser style from `limit-order-type`, but it does not
need to support the older ordinary 40-byte settlement-cell ABI. Keeping only the
UDT shape matches this fixture's purpose and avoids an extra test-only payment
format.

If a matching UDT payment amount does not fit into `u64`, the script fails
closed with amount overflow.

## Fail-Closed Rules

The script must fail for:

- lock args shorter or longer than 104 bytes;
- current lock group with zero or multiple inputs;
- current input missing a type script;
- current input type hash not equal to `offered_nft_type_hash`;
- missing related action;
- multiple related actions;
- tx-level `FillOrder`;
- OTX action target not being the current input lock;
- current lock input not in the OTX base input range;
- unknown action tag;
- malformed action payload;
- `requested_asset_id` mismatch;
- `action.min_requested_amount < args.min_requested_amount`;
- insufficient owner payment in the current OTX settlement range;
- wrong UDT type;
- wrong owner lock;
- payment only in tx-level remainder;
- payment only in another OTX;
- malformed Cobuild witness or invalid OTX layout.

All unexpected parser, syscall, overflow, and Cobuild errors fail closed.

## Test Coverage

Contract unit tests:

- lock args parser accepts exactly 104 bytes;
- lock args parser rejects short and long byte slices;
- action parser accepts `FillOrder` tag `2` with a 41-byte payload;
- action parser rejects unknown tag, empty data, short payload, and long payload;
- settlement validation accepts exact and over-limit UDT payment;
- settlement validation rejects requested asset mismatch, minimum below order
  minimum, insufficient payment, wrong owner, wrong UDT, and overflow.

Integration tests:

- happy path: NFT input locked by `limit-order-lock`, OTX fill unlocks it, and
  owner receives enough requested UDT in the same OTX settlement scope;
- lock args malformed;
- input NFT type hash does not match `offered_nft_type_hash`;
- tx-level `FillOrder` rejected;
- OTX action target is not current lock;
- current lock input is only in append input scope;
- `requested_asset_id` mismatch;
- fill amount below order minimum;
- insufficient UDT;
- wrong UDT;
- wrong owner;
- payment in tx-level remainder only does not count;
- payment in another OTX does not count;
- malformed Cobuild/action rejected.

Expected failure tests must not create tracked files in `tests/failed_txs` unless
`COBUILD_TEST_DUMP_EXPECTED_FAILURES=1`.

## Fixture And File Boundaries

Planned implementation files:

- `tests/contracts/limit-order-lock/Cargo.toml`
- `tests/contracts/limit-order-lock/Makefile`
- `tests/contracts/limit-order-lock/src/main.rs`
- `tests/contracts/limit-order-lock/src/lib.rs`
- `tests/contracts/limit-order-lock/src/entry.rs`
- `tests/contracts/limit-order-lock/src/error.rs`
- `tests/contracts/limit-order-lock/src/types.rs`
- `Cargo.toml`
- `tests/src/fixtures/limit_order.rs`
- `tests/src/fixtures/limit_order/lock_nft_for_udt.rs`
- `tests/tests/limit_order_lock.rs`

Potential helper additions may go in `tests/src/framework` only if they are
general Cobuild/CKB testing utilities. Scenario-specific builders belong under
`tests/src/fixtures/limit_order*`.

The root `Makefile` already builds every `tests/contracts/*` directory with a
Makefile, so adding the new contract directory is enough for the default build
matrix. The workspace member list still needs the new contract crate.

## Out Of Scope

- production-grade order protocol;
- order book;
- partial fills;
- cancel;
- order state cells;
- type-id order identity;
- adding public schemas to `crates/cobuild-types`;
- changing `contracts/cobuild-otx-lock`;
- changing `crates/cobuild-core` unless later implementation proves a concrete
  lock-plan gap and the user approves it first;
- generic offered assets beyond one type-id NFT;
- settlement formats other than `test-udt` payment cells.

## Alternatives Considered

### Recommended: Actionless Create, OTX Fill Action

This matches the lock-shaped protocol: order state is the NFT lock args, and the
only cross-party intent binding needed by the script is at unlock time. It keeps
the fixture small and exercises `plan_lock_validation()`.

### Require `CreateOrder` Action

This would mirror `limit-order-type`, but it would not be enforceable by the
lock script at output creation time because lock scripts do not run on outputs.
It would add ceremony without improving the lock-side invariant.

### Move Action ABI Into Shared Cobuild Types

This would make the ABI more formal, but the action is explicitly tests-only and
specific to this fixture. Keeping local fixed-width parsers avoids leaking test
protocol into public schema crates.
