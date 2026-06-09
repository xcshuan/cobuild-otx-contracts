# Limit Order Entry Refactor Design

## Status

This spec covers an internal refactor for the tests-only contracts:

- `tests/contracts/limit-order-type`
- `tests/contracts/limit-order-lock`

It does not change action ABI, order ABI, lock args ABI, error codes, validation
semantics, fixtures, or integration-test expectations.

The current `entry.rs` files are too large:

- `limit-order-type/src/entry.rs`: about 567 lines
- `limit-order-lock/src/entry.rs`: about 424 lines

The goal is to split them by responsibility and improve internal interfaces
where that makes call sites clearer.

## Goals

1. Keep `entry.rs` as the contract entry orchestration layer.
2. Move OTX/Cobuild action plumbing out of `entry.rs`.
3. Move settlement output loading and validation helpers out of `entry.rs`.
4. Move lock-input-only helpers out of `limit-order-lock/src/entry.rs`.
5. Preserve all externally observable behavior.
6. Keep modules local to each tests-only contract crate. Do not introduce a
   shared crate or public schema.

## Non-Goals

- No ABI changes.
- No new validation behavior.
- No fixture or integration-test scenario changes.
- No changes to `contracts/cobuild-otx-lock`.
- No changes to `crates/cobuild-types`.
- No production order protocol work.

## Module Shape

### `limit-order-type`

Add:

```text
tests/contracts/limit-order-type/src/otx.rs
tests/contracts/limit-order-type/src/settlement.rs
tests/contracts/limit-order-type/src/validation.rs
```

Keep:

```text
tests/contracts/limit-order-type/src/entry.rs
```

`entry.rs` responsibilities:

- load current type hash;
- build `CobuildContext`;
- build `TypeValidationPlan`;
- determine `OrderMode`;
- dispatch to `validation::validate_create_order` or
  `validation::validate_fill_order`.

`validation.rs` responsibilities:

- own the high-level create/fill flow;
- parse the single group order;
- parse the single related action;
- call OTX and settlement helpers in rule order.

Expected public-in-crate functions:

```rust
pub fn validate_create_order(
    current_type_hash: [u8; 32],
    plan: &TypeValidationPlan,
) -> Result<(), Error>;

pub fn validate_fill_order(
    context: &CobuildContext,
    plan: &TypeValidationPlan,
) -> Result<(), Error>;
```

`otx.rs` responsibilities:

- extract and validate OTX fill layout for type orders;
- check whether output indexes are inside current OTX settlement outputs;
- collect same-OTX limit-order targets;
- reject duplicate payment output indexes.

The interface should make the call site read as OTX intent rather than raw
plumbing. A small context struct is preferred:

```rust
pub struct TypeOtxFill {
    pub otx_index: usize,
    pub layout: OtxMessageLayout,
    pub related_action_data: Cursor,
    pub related_action_target: [u8; 32],
}
```

The concrete data type for `related_action_data` may follow existing
`cobuild-core` cursor types. If using `Cursor` in the struct adds lifetime or
type noise, expose a helper function that returns `Vec<u8>` instead. Favor
clarity over abstract reuse.

Expected public-in-crate functions:

```rust
pub fn load_type_otx_fill(
    context: &CobuildContext,
    plan: &TypeValidationPlan,
) -> Result<TypeOtxFill, Error>;

pub fn output_index_in_otx_outputs(
    layout: OtxMessageLayout,
    output_index: usize,
) -> Result<bool, Error>;

pub fn ensure_unique_payment_output_indexes(
    actions: &[ActionView],
    limit_order_targets: &[[u8; 32]],
) -> Result<(), Error>;
```

`settlement.rs` responsibilities:

- compute expected proxy lock hash for create-order validation;
- scan for NFT proxy output during create;
- load a bound UDT payment output;
- require seller payment output to be inside current OTX settlement outputs;
- scan current OTX settlement outputs for buyer NFT delivery.

Expected public-in-crate functions:

```rust
pub fn ensure_create_nft_proxy_output(
    current_type_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<(), Error>;

pub fn load_bound_payment(
    layout: OtxMessageLayout,
    payment_output_index: u32,
) -> Result<SettlementCell, Error>;

pub fn ensure_nft_delivered_to_buyer(
    layout: OtxMessageLayout,
    buyer_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<(), Error>;
```

### `limit-order-lock`

Add:

```text
tests/contracts/limit-order-lock/src/input.rs
tests/contracts/limit-order-lock/src/otx.rs
tests/contracts/limit-order-lock/src/settlement.rs
tests/contracts/limit-order-lock/src/validation.rs
```

Keep:

```text
tests/contracts/limit-order-lock/src/entry.rs
```

`entry.rs` responsibilities:

- load current lock script and args;
- parse order args;
- build `CobuildContext`;
- dispatch to `validation::validate_fill_order`.

`input.rs` responsibilities:

- require one current lock group input;
- find the absolute input index for the current lock;
- verify the offered NFT input type hash.

Expected public-in-crate function:

```rust
pub fn load_current_order_input(
    current_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<usize, Error>;
```

`validation.rs` responsibilities:

- own lock-order fill flow;
- load current order input context;
- parse related action;
- call OTX and settlement helpers in rule order.

Expected public-in-crate function:

```rust
pub fn validate_fill_order(
    context: &CobuildContext,
    order: &OrderArgs,
    current_lock_hash: [u8; 32],
) -> Result<(), Error>;
```

`otx.rs` responsibilities:

- extract and validate OTX fill layout for lock orders;
- collect same-OTX limit-order targets;
- reject duplicate payment output indexes;
- expose output range helper for settlement.

`settlement.rs` responsibilities:

- load a bound UDT payment output;
- require seller payment output to be inside current OTX settlement outputs;
- scan current OTX settlement outputs for buyer NFT delivery.

Expected public-in-crate functions mirror the type contract with local payment
types:

```rust
pub fn load_bound_payment(
    layout: OtxMessageLayout,
    payment_output_index: u32,
) -> Result<UdtPayment, Error>;

pub fn ensure_nft_delivered_to_buyer(
    layout: OtxMessageLayout,
    buyer_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<(), Error>;
```

## Interface Guidance

Internal interfaces may change when it improves readability. This refactor
should prefer semantic helpers over exposing raw syscall plumbing at call sites.

Good call-site shape:

```rust
let fill = otx::load_type_otx_fill(context, plan)?;
let action = parse_fill_action(fill.action_data()?)?;
let payment = settlement::load_bound_payment(fill.layout, action.payment_output_index)?;
validate_fill(&order, payment)?;
settlement::ensure_nft_delivered_to_buyer(
    fill.layout,
    action.buyer_lock_hash,
    order.offered_nft_type_hash,
)?;
```

Avoid over-abstracting into shared traits or generic helpers. The two contracts
are separate no-std fixture contracts; clear local duplication is acceptable.

## Tests

Move existing unit tests with the helper they exercise:

- OTX layout, range, target, and duplicate-payment tests move to `otx.rs`.
- NFT delivery predicate tests and payment loading helper tests move to
  `settlement.rs` where possible.
- Lock input tests move to `input.rs` if they are pure or can stay near the
  helper.
- `entry.rs` keeps only tests for `order_mode` or high-level dispatch if needed.

No test assertions should be weakened. Existing integration tests must continue
to pass unchanged.

## Verification

Required verification after implementation:

```bash
cargo fmt
cargo test -p limit-order-type --offline
cargo test -p limit-order-lock --offline
cargo test -p tests --test limit_order_type --offline
cargo test -p tests --test limit_order_lock --offline
cargo test -p tests --lib --offline
cargo test --workspace --offline
cargo clippy --workspace --offline --all-targets
cargo fmt --check
git diff --check
git status --short
```

If contract integration tests use stale debug binaries after source movement,
rebuild the affected tests-only contracts with their existing Makefile commands.
