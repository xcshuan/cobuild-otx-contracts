# Limit Order NFT Delivery Binding Design

## Status

This spec covers a tests-only breaking change for:

- `tests/contracts/limit-order-type`
- `tests/contracts/limit-order-lock`
- their limit-order fixtures and integration tests

It extends the previous payment-output binding work by also requiring Fill
settlement to deliver the offered NFT to the buyer. It does not change
production contracts, `contracts/cobuild-otx-lock`, `crates/cobuild-types`, or
any public production action schema.

No legacy ABI compatibility is required. Existing fixture state and action
builders will migrate in place.

## Goal

Fill validation must prove both sides of the trade in the current OTX
settlement:

1. The seller receives the requested UDT in the explicitly bound payment output.
2. The buyer receives the offered NFT in an output belonging to the same OTX.

The current UDT payment output binding remains necessary because UDT payment
outputs can otherwise be reused by multiple Fill actions. NFT delivery does not
need an explicit output index or same-OTX uniqueness check in the limit-order
script because the NFT's own type script and cell spend rules prevent duplicating
the same NFT. The limit-order script only needs to require that the current OTX
contains the NFT output locked to the buyer.

## Order ABI Rename

The order state currently names the requested amount as `min_requested_amount`.
For this fixture, there is no separate action-level price negotiation. The order
state is the source of truth for the exact requested amount threshold.

Rename the field to:

```text
requested_amount: u64
```

This applies to both fixture shapes:

- `limit-order-type` order cell data
- `limit-order-lock` lock args

The order ABIs remain fixed-width and little-endian. The encoded size does not
change; only the local field name, helper names, test names, and documentation
change.

## FillOrder ABI

`limit-order-type` and `limit-order-lock` must use the same FillOrder action ABI:

```text
tag: u8 = 2
payment_output_index: u32
buyer_lock_hash: [u8; 32]
```

The encoded length is 37 bytes. All integer fields are little-endian.

`payment_output_index` is an absolute transaction output index. It is not
relative to the OTX, not relative to base outputs, and not relative to append
outputs.

`buyer_lock_hash` is the lock hash that must receive the offered NFT.

The previous 41-byte and 45-byte FillOrder ABIs are not retained. A tag `2`
action with any length other than 37 bytes fails closed as `InvalidActionData`
or the local equivalent error mapping.

The action no longer carries `requested_asset_id` or requested amount. Those
values are loaded from the order state:

- `requested_asset_id`
- `requested_amount`
- `offered_nft_type_hash`
- `owner_lock_hash`

This avoids duplicating order commitments in action data and removes inconsistent
state/action price branches.

## Fill Payment Validation

Fill validation continues to check one explicit UDT payment output.

For the current order, the script must require:

1. The related action comes from `ActionOrigin::Otx`.
2. The current order input is in the OTX base-input scope required by the
   existing fixture rules:
   - `limit-order-type`: current type relation has `input_type_in_base`.
   - `limit-order-lock`: current input index is in `layout.base_inputs`.
3. `action.payment_output_index` is inside `layout.base_outputs` or
   `layout.append_outputs`.
4. The referenced output lock hash equals `order.owner_lock_hash`.
5. The referenced output type hash exists and equals `order.requested_asset_id`.
6. The referenced output data is exactly a 16-byte little-endian UDT amount that
   fits in `u64`.
7. The referenced amount is at least `order.requested_amount`.

The script must not scan and sum all matching UDT outputs for Fill payment.
Payment in tx-level remainder outputs, outputs outside the current OTX, or
another OTX must not count.

## NFT Delivery Validation

Fill validation must scan the current OTX settlement output range and require at
least one output that delivers the offered NFT to the buyer.

The scan covers:

- `layout.base_outputs`
- `layout.append_outputs`

The script must accept the Fill only if it finds an output where:

```text
lock_hash == action.buyer_lock_hash
type_hash == order.offered_nft_type_hash
```

No NFT amount field is parsed by the limit-order scripts. The fixture relies on
the `test-nft` type script and normal CKB cell-spend semantics to prevent NFT
duplication and preserve NFT-specific invariants.

The limit-order scripts do not need to enforce NFT output index uniqueness among
Fill actions. If two orders claim the same NFT delivery output but only one NFT
cell actually exists, the corresponding NFT type/input validation prevents both
distinct offered NFTs from being validly delivered through that single output.

## Same-OTX Payment Duplicate Rejection

Each Fill script must continue to call:

```rust
context.otx_actions(otx_index)?
```

using `otx_index` from the current related action's `ActionOrigin::Otx`.

The script checks the returned actions for duplicate `payment_output_index`
among limit-order-looking Fill actions in the same OTX. A duplicate is
`InvalidCobuild`.

The duplicate payment check covers mixed type and lock fills in the same OTX.
This remains required because a type-order and a lock-order could otherwise
reuse the same UDT payment output.

There is no same-OTX duplicate check for NFT output indexes because Fill actions
do not carry NFT output indexes.

## Action Filtering Boundary

`otx_actions(otx_index)` returns all actions in the OTX message, including
actions unrelated to these test fixtures. The duplicate payment scan therefore
cannot treat every action whose first byte is `2` as a limit-order action.

The duplicate scan should parse actions only when their target is in the
tests-only limit-order target set for the current OTX. That set is built by the
fixture scenario and contains the script hashes for the type and lock
limit-order contracts participating in that OTX. The current related action's
target must always be included. Mixed type+lock scenarios include both the
`limit-order-type` type hash and the `limit-order-lock` lock hash.

Within that target set, the duplicate helper considers these roles:

- `InputType`
- `OutputType`
- `InputLock`

For actions with one of those roles and a script hash in the target set, tag `2`
means the shared 37-byte tests-only `FillOrder` ABI and must be parsed for
`payment_output_index`. Actions with script hashes outside the target set are
ignored by duplicate checking, even when their first byte is `2`.

Malformed handling is fail-closed within this boundary:

- If an action is in the limit-order target set for the current validation and
  its first byte is tag `2`, it must have exactly 37 bytes and parse correctly.
- If such an action is tag `2` but has any old or malformed length, validation
  returns `InvalidActionData` or `InvalidCobuild` through the local contract
  error mapping.
- Actions outside the limit-order target set are ignored by duplicate checking,
  even if their first byte is `2`.

This boundary is intentionally tests-only. The implementation must not introduce
a global registry of limit-order script hashes and must not add these action
schemas to `crates/cobuild-types`.

## Implementation Shape

Both contracts keep their current high-level entry flow but update their local
types and validation helpers.

### `limit-order-type`

CreateOrder behavior stays unchanged except for the order field rename from
`min_requested_amount` to `requested_amount`.

Fill entry flow:

1. Load the order from `Source::GroupInput`.
2. Build `CobuildContext::build(CurrentScript::Type(current_type_hash))?`.
3. Call `plan_type_validation()`.
4. Require exactly one related action.
5. Extract `ActionOrigin::Otx { otx_index, layout, .. }`.
6. Require the current type relation is in OTX scope and has
   `input_type_in_base`.
7. Parse the related action as the new 37-byte `FillOrder`.
8. Require `payment_output_index` to be in `layout.base_outputs` or
   `layout.append_outputs`.
9. Call `context.otx_actions(otx_index)?` and reject duplicate payment indexes
   among same-OTX limit-order Fill actions.
10. Load the referenced output's lock hash, type hash, and data from
    `Source::Output`.
11. Validate that exact payment output against the order.
12. Scan the same OTX output ranges for an NFT output locked to
    `action.buyer_lock_hash` with type hash `order.offered_nft_type_hash`.

### `limit-order-lock`

Fill entry flow:

1. Parse lock args as the order.
2. Require a single current lock input and verify its NFT type.
3. Build `CobuildContext::build(CurrentScript::InputLock(current_lock_hash))?`.
4. Call `plan_lock_validation()`.
5. Require exactly one related action.
6. Extract `ActionOrigin::Otx { otx_index, layout, .. }`.
7. Require the current lock input index is in `layout.base_inputs`.
8. Parse the related action as the new 37-byte `FillOrder`.
9. Require `payment_output_index` to be in `layout.base_outputs` or
   `layout.append_outputs`.
10. Call `context.otx_actions(otx_index)?` and reject duplicate payment indexes
    among same-OTX limit-order Fill actions, including mixed type+lock fills.
11. Load and validate the exact referenced UDT payment output against the order.
12. Scan the same OTX output ranges for an NFT output locked to
    `action.buyer_lock_hash` with type hash `order.offered_nft_type_hash`.

The lock fixture remains actionless for order creation.

## Shared Local Helpers

The two contract crates are separate no-std tests-only contracts, so code may be
duplicated locally if introducing a shared test contract crate would add more
surface area than it removes.

Each contract should expose small pure helpers for unit tests:

- `parse_fill_order_action(data: &[u8]) -> Result<FillOrderAction, Error>`
- `range_contains(range: Range, index: usize) -> Result<bool, Error>`
- `output_index_in_otx_outputs(layout: OtxMessageLayout, index: usize) -> Result<bool, Error>`
- `duplicate_payment_output_index(actions: &[ActionView]) -> Result<bool, Error>` or an
  equivalent helper that returns an error on malformed in-scope actions
- `validate_fill_payment(order, payment)`
- `find_nft_delivery(layout, buyer_lock_hash, offered_nft_type_hash)` or an
  equivalent helper around syscall output scanning

`FillOrderAction` must include:

```rust
pub struct FillOrderAction {
    pub payment_output_index: u32,
    pub buyer_lock_hash: [u8; 32],
}
```

When indexing outputs through CKB syscalls, convert `u32` to `usize`.

## Fixtures

The shared test fixture action builder must change from the previous payment
binding form to a smaller action:

```rust
limit_order_fill(payment_output_index, buyer_lock_hash)
```

The fixture order builders should rename amount parameters and fields from
`min_requested_amount` to `requested_amount`.

Integration fixtures must construct both settlement outputs:

- seller UDT payment output at the action's absolute `payment_output_index`;
- buyer NFT output in the same OTX `base_outputs` or `append_outputs` range.

Tests may continue to use helper defaults for the buyer lock hash, but negative
tests must be able to override it.

## Error Handling

Malformed Fill action data fails as `InvalidActionData` or the contract's local
mapping from action parsing errors.

Invalid payment output binding fails as the existing payment error path where
appropriate:

- output outside current OTX settlement range: `InvalidCobuild`;
- wrong owner lock, wrong UDT type, malformed amount, or insufficient amount:
  `InsufficientPayment` or the existing local equivalent.

Invalid NFT delivery fails closed as `InvalidCobuild` or a dedicated local
settlement error if one is already available. The implementation should not
reuse `InsufficientPayment` for missing NFT delivery because the failure is on
the buyer side of settlement, not seller payment.

## Test Coverage

Contract unit tests:

- FillOrder parser accepts exactly 37 bytes.
- FillOrder parser rejects old 41-byte and 45-byte action data.
- `payment_output_index` parses as little-endian `u32`.
- `buyer_lock_hash` parses at the expected offset.
- duplicate payment index check accepts unique payment indexes.
- duplicate payment index check rejects reused payment indexes.
- duplicate payment index check fails closed for malformed in-scope tag `2`
  actions.
- OTX output range helper covers both base outputs and append outputs.
- payment validation uses `order.requested_amount`.

`limit-order-type` integration tests:

- happy path succeeds with a seller UDT payment output and buyer NFT output in
  the current OTX.
- missing buyer NFT output is rejected.
- buyer NFT output with wrong lock hash is rejected.
- buyer NFT output with wrong NFT type hash is rejected.
- payment output index outside the current OTX settlement range is rejected.
- wrong UDT, wrong owner, and insufficient payment remain rejected.
- two type orders reusing one payment output index in the same OTX are rejected.
- two type orders using different payment output indexes and delivering their
  NFTs to the buyer succeed, subject to fixture capability.

`limit-order-lock` integration tests:

- happy path succeeds with a seller UDT payment output and buyer NFT output in
  the current OTX.
- missing buyer NFT output is rejected.
- buyer NFT output with wrong lock hash is rejected.
- buyer NFT output with wrong NFT type hash is rejected.
- payment output index outside the current OTX settlement range is rejected.
- wrong UDT, wrong owner, and insufficient payment remain rejected.
- two lock orders reusing one payment output index in the same OTX are rejected,
  including when lock args differ.
- two lock orders using different payment output indexes and delivering their
  NFTs to the buyer succeed, subject to fixture capability.

Mixed integration tests:

- one `limit-order-type` fill and one `limit-order-lock` fill in the same OTX
  reusing the same payment output index are rejected.
- mixed fills with distinct payment output indexes and valid NFT deliveries are
  accepted or individually validated, subject to fixture capability.

## Non-Goals

- No production order-book semantics.
- No partial fill support.
- No cancel flow changes.
- No public Cobuild action schema changes.
- No global registry of limit-order action targets.
- No NFT output index action field.
- No NFT output index uniqueness check in the limit-order scripts.
- No compatibility for old 41-byte or 45-byte FillOrder action data.
