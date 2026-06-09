# Limit Order Payment Output Binding Design

## Status

This spec covers a tests-only breaking change for:

- `tests/contracts/limit-order-type`
- `tests/contracts/limit-order-lock`
- their limit-order fixtures and integration tests

It does not change production contracts, `contracts/cobuild-otx-lock`,
`crates/cobuild-types`, or any public production action schema.

The goal is to remove the Fill-time UDT payment output reuse bug. Today both
fixture contracts scan the current OTX settlement output range and sum every
matching UDT payment for:

```text
owner_lock_hash + requested_asset_id
```

That lets two same-price orders in one OTX count the same payment output. The
new design makes each `FillOrder` action bind to one explicit absolute
transaction output index, and rejects any duplicate payment output index among
limit-order Fill actions in the same OTX.

## Required Design Decisions

### FillOrder ABI

`limit-order-type` and `limit-order-lock` must use the same FillOrder action ABI:

```text
tag: u8 = 2
requested_asset_id: [u8; 32]
min_requested_amount: u64
payment_output_index: u32
```

The encoded length is 45 bytes. All integer fields are little-endian.

The old 41-byte FillOrder ABI is not retained. These contracts are tests-only
fixtures, so fixtures and tests will migrate in place. A tag `2` action with
any length other than 45 bytes fails closed as `InvalidActionData` in both
contracts.

`payment_output_index` is an absolute transaction output index. It is not
relative to the OTX, not relative to base outputs, and not relative to append
outputs.

### Payment Output Binding

Fill validation must stop scanning and summing all matching outputs. Each order
validates only `action.payment_output_index`.

For the current order, the script must require:

1. The action comes from `ActionOrigin::Otx`.
2. The current order input is in the OTX base-input scope required by the
   existing fixture rules:
   - `limit-order-type`: current type relation has `input_type_in_base`.
   - `limit-order-lock`: current input index is in `layout.base_inputs`.
3. `action.payment_output_index` is inside `layout.base_outputs` or
   `layout.append_outputs`.
4. `action.requested_asset_id == order.requested_asset_id`.
5. `action.min_requested_amount >= order.min_requested_amount`.
6. The referenced output lock hash equals `order.owner_lock_hash`.
7. The referenced output type hash exists and equals `order.requested_asset_id`.
8. The referenced output data is exactly a 16-byte little-endian UDT amount that
   fits in `u64`.
9. The referenced amount is at least `action.min_requested_amount`.

The type fixture's legacy 40-byte settlement-cell format is no longer counted
for Fill validation. Fill payment is a UDT output selected by
`payment_output_index`.

### Same-OTX Duplicate Rejection

Each Fill script must call:

```rust
context.otx_actions(otx_index)?
```

using `otx_index` from the current related action's `ActionOrigin::Otx`.

The script then checks the returned actions for duplicate `payment_output_index`
among limit-order-looking Fill actions in the same OTX. A duplicate is
`InvalidCobuild`.

The duplicate check covers mixed type and lock fills in the same OTX. This is
required; otherwise a type-order and a lock-order could still reuse the same
UDT payment output.

## Action Filtering Boundary

`otx_actions(otx_index)` returns all actions in the OTX message, including
actions unrelated to these test fixtures. The duplicate check therefore cannot
treat every action whose first byte is `2` as a limit-order action; unrelated
schemas may use the same first byte.

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
means the shared 45-byte tests-only `FillOrder` ABI and must be parsed for
`payment_output_index`. Actions with script hashes outside the target set are
ignored by duplicate checking, even when their first byte is `2`.

Malformed handling is fail-closed within this boundary:

- If an action is in the limit-order target set for the current validation and
  its first byte is tag `2`, it must have exactly 45 bytes and parse correctly.
- If such an action is tag `2` but has the old 41-byte length or any other
  malformed length, validation returns `InvalidActionData` or `InvalidCobuild`
  through the local contract error mapping.
- Actions outside the limit-order target set are ignored by duplicate checking,
  even if their first byte is `2`.

This boundary is intentionally tests-only. The implementation must not introduce
a global registry of limit-order script hashes and must not add these action
schemas to `crates/cobuild-types`.

## Implementation Shape

Both contracts keep their current high-level entry flow but retain the
`CobuildContext` instead of discarding it after planning.

### `limit-order-type`

Fill entry flow:

1. Load the order from `Source::GroupInput`.
2. Build `CobuildContext::build(CurrentScript::Type(current_type_hash))?`.
3. Call `plan_type_validation()`.
4. Require exactly one related action.
5. Extract `ActionOrigin::Otx { otx_index, layout, .. }`.
6. Require the current type relation is in OTX scope and has
   `input_type_in_base`.
7. Parse the related action as the new 45-byte `FillOrder`.
8. Require `payment_output_index` to be in `layout.base_outputs` or
   `layout.append_outputs`.
9. Call `context.otx_actions(otx_index)?` and reject duplicate payment indexes
   among same-OTX limit-order Fill actions.
10. Load the referenced output's lock hash, type hash, and data from
    `Source::Output`.
11. Validate that exact payment output against the order and action.

CreateOrder behavior stays unchanged.

### `limit-order-lock`

Fill entry flow:

1. Parse lock args as the order.
2. Require a single current lock input and verify its NFT type.
3. Build `CobuildContext::build(CurrentScript::InputLock(current_lock_hash))?`.
4. Call `plan_lock_validation()`.
5. Require exactly one related action.
6. Extract `ActionOrigin::Otx { otx_index, layout, .. }`.
7. Require the current lock input index is in `layout.base_inputs`.
8. Parse the related action as the new 45-byte `FillOrder`.
9. Require `payment_output_index` to be in `layout.base_outputs` or
   `layout.append_outputs`.
10. Call `context.otx_actions(otx_index)?` and reject duplicate payment indexes
    among same-OTX limit-order Fill actions, including mixed type+lock fills.
11. Load and validate the exact referenced UDT payment output.

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
- `validate_fill(order, action, payment)` where `payment` is the single bound
  payment, not a list to be summed

`FillOrderAction` must include:

```rust
pub struct FillOrderAction {
    pub requested_asset_id: [u8; 32],
    pub min_requested_amount: u64,
    pub payment_output_index: u32,
}
```

When indexing outputs through CKB syscalls, convert `u32` to `usize`.

## Fixtures

The shared test fixture action builder must change from:

```rust
limit_order_fill(requested_asset_id, min_requested_amount)
```

to a form that includes the absolute payment output index, for example:

```rust
limit_order_fill(requested_asset_id, min_requested_amount, payment_output_index)
```

Scenarios with one base NFT output and one append UDT payment output should use
`payment_output_index = 1`, because transaction outputs are ordered as:

1. all base outputs,
2. all append outputs,
3. tx-level remainder outputs.

Scenarios that need payment outside the OTX settlement range should point the
action at a tx-level remainder output or another OTX output and expect
`InvalidCobuild` for out-of-range binding rather than relying on scanned sums.

New multi-order fixtures may extend `CobuildMessageBuilder` to build messages
with more than one action. This should stay in the test framework and not touch
public schemas.

## Test Coverage

### Parser and Unit Tests

Both contracts should cover:

- new 45-byte FillOrder parses successfully;
- old 41-byte FillOrder is rejected;
- empty action data is rejected;
- unknown action tag is rejected;
- `payment_output_index` is parsed little-endian;
- base output range accepts start and last index;
- append output range accepts start and last index;
- output range rejects an index outside both OTX output ranges;
- overflowing range arithmetic fails closed;
- duplicate payment index helper accepts unique Fill actions;
- duplicate payment index helper rejects duplicate Fill actions;
- malformed in-scope tag `2` actions fail closed;
- unrelated non-limit-order actions do not affect duplicate checking.

### `limit-order-type` Integration Tests

Keep existing create-order tests. Update existing Fill tests to use the new
action ABI and add:

- happy path succeeds when the action points at the append UDT payment output;
- action points outside current OTX settlement range and is rejected;
- action points at wrong UDT and is rejected;
- action points at wrong owner and is rejected;
- action points at insufficient amount and is rejected;
- two type orders in one OTX reuse one payment output index and are rejected;
- two type orders in one OTX use different payment output indexes and succeed,
  or at least both order validations pass if the fixture can only assert per
  script execution cleanly.

### `limit-order-lock` Integration Tests

Update existing Fill tests to use the new action ABI and add:

- happy path succeeds when the action points at the append UDT payment output;
- action points outside current OTX settlement range and is rejected;
- action points at wrong UDT and is rejected;
- action points at wrong owner and is rejected;
- action points at insufficient amount and is rejected;
- two lock orders in one OTX reuse one payment output index and are rejected,
  including the case where lock args differ;
- two lock orders in one OTX use different payment output indexes and succeed,
  or at least both order validations pass if the fixture can only assert per
  script execution cleanly.

### Mixed Type + Lock Integration Test

Add one OTX with:

- one `limit-order-type` Fill action;
- one `limit-order-lock` Fill action;
- both actions referencing the same UDT payment output index.

The transaction must be rejected. This test is the main proof that duplicate
checking is same-OTX and not limited to the current script's related action.

If fixture construction proves too large during implementation, the plan must
split this into its own task rather than dropping it. The target behavior remains
required by this spec.

## Error Expectations

Existing error codes should be reused:

- malformed Fill ABI: `InvalidActionData`;
- unsupported Fill tag: `UnsupportedAction`;
- requested asset mismatch: `ActionMismatch`;
- amount below order minimum or insufficient bound payment:
  `InsufficientPayment`;
- payment output outside the current OTX output range: `InvalidCobuild`;
- duplicate same-OTX payment output index: `InvalidCobuild`;
- malformed Cobuild, invalid OTX layout, or `otx_actions` failure:
  `InvalidCobuild`;
- payment amount overflow: `AmountOverflow`.

Integration tests should assert the current originating script and stable exit
code. Expected-failure tests must not create tracked files under
`tests/failed_txs` unless `COBUILD_TEST_DUMP_EXPECTED_FAILURES=1`.

## Alternatives Considered

### Alternative A: Keep scanning and divide outputs by action order

This would preserve the old ABI but relies on implicit ordering and still makes
it hard to prove which payment belongs to which order. It is rejected because it
does not create an explicit order-to-payment binding.

### Alternative B: Bind by full output hash instead of index

This would avoid absolute index concerns but increases action size and requires
hashing or serializing output fields in each script. It is unnecessary for these
tests-only fixtures.

### Alternative C: Explicit absolute output index

This is the selected design. It is small, deterministic, easy to test, and
matches the existing OTX layout model where output ranges are absolute
transaction indexes.

## Out of Scope

- Partial fills.
- Multiple payment outputs per order.
- Order cancellation.
- Production order-book semantics.
- Public limit-order schemas in `cobuild-types`.
- Changes to `contracts/cobuild-otx-lock`.
- Changes to `crates/cobuild-core` beyond using the already-added
  `CobuildContext::otx_actions` API.

## Verification Plan

The implementation plan must require TDD per task and record Red/Green results.
Final verification must include at least:

```bash
cargo fmt
make -e -C tests/contracts/limit-order-type build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/limit-order-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-udt build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-nft build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
cargo test -p limit-order-type --offline
cargo test -p limit-order-lock --offline
cargo test -p cobuild-core --lib --offline
cargo test -p tests --test limit_order_type --offline
cargo test -p tests --test limit_order_lock --offline
cargo test -p tests --lib --offline
cargo test --workspace --offline
cargo fmt --check
git diff --check
git status --short
find tests/failed_txs -maxdepth 1 -type f 2>/dev/null | wc -l
git status --short --ignored tests/failed_txs
```
