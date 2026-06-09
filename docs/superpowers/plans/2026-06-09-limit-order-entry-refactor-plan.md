# Limit Order Entry Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split the tests-only limit-order contract `entry.rs` files into focused modules without changing ABI, error codes, validation semantics, fixtures, or integration-test expectations.

**Architecture:** Keep each contract's `entry.rs` as the small orchestration layer and move responsibility-specific logic into local modules. `validation.rs` owns create/fill flow, `otx.rs` owns Cobuild/OTX action plumbing, `settlement.rs` owns payment/NFT/proxy settlement checks, and lock-only `input.rs` owns current input discovery and NFT input verification.

**Tech Stack:** Rust no_std tests-only contract crates, `ckb-std`, `cobuild-core`, workspace Cargo tests run offline, Make-based debug contract builds when integration binaries need refreshing.

---

## Scope And Invariants

- Only modify files under `tests/contracts/limit-order-type/src/*`, `tests/contracts/limit-order-lock/src/*`, and implementation notes in this plan if execution records are filled in.
- Do not modify `crates/cobuild-core`, `contracts/cobuild-otx-lock`, `crates/cobuild-types`, public schema, fixtures, or integration tests unless a module split requires an import or test location adjustment.
- Preserve the 37-byte `FillOrder` action ABI: tag `2`, `payment_output_index: u32`, `buyer_lock_hash: [u8; 32]`.
- Preserve the `requested_amount` naming and all existing error mappings.
- Preserve fill validation: seller UDT payment must be at the specified OTX output, NFT delivery must be scanned in current OTX outputs, payment output indexes must be unique across same-OTX type+lock limit-order fill actions, and NFT output indexes remain unconstrained.
- Do not introduce shared limit-order modules between the two tests-only contract crates.

## Target File Structure

### `tests/contracts/limit-order-type/src`

- `lib.rs`: publish the new local modules with `pub mod otx;`, `pub mod settlement;`, and `pub mod validation;`.
- `entry.rs`: keep `OrderMode`, `order_mode`, `main`, `validate_order_type_id`, and `single_group_order`; build `CobuildContext` and `TypeValidationPlan`; dispatch to `validation::validate_create_order` or `validation::validate_fill_order`.
- `validation.rs`: parse the single group order and single related action; own create and fill rule order; call `otx` and `settlement`; expose:

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

- `otx.rs`: own OTX fill extraction, output range checks, same-OTX limit-order target collection, and duplicate payment output index rejection; expose:

```rust
pub struct TypeOtxFill {
    pub otx_index: usize,
    pub layout: OtxMessageLayout,
    pub action_data: Vec<u8>,
    pub action_target: [u8; 32],
}

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

- `settlement.rs`: own proxy lock hash computation, create NFT proxy output scan, bound UDT payment loading, and NFT delivery scan; expose:

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

### `tests/contracts/limit-order-lock/src`

- `lib.rs`: publish the new local modules with `pub mod input;`, `pub mod otx;`, `pub mod settlement;`, and `pub mod validation;`.
- `entry.rs`: load the current lock script, parse `OrderArgs`, build `CobuildContext`, and dispatch to `validation::validate_fill_order`.
- `input.rs`: own current group input validation, absolute input index discovery, and offered NFT input type verification; expose:

```rust
pub fn load_current_order_input(
    current_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<usize, Error>;
```

- `validation.rs`: own high-level lock fill flow; call `input`, `otx`, and `settlement`; expose:

```rust
pub fn validate_fill_order(
    context: &CobuildContext,
    order: &OrderArgs,
    current_lock_hash: [u8; 32],
) -> Result<(), Error>;
```

- `otx.rs`: own lock OTX fill extraction, range checks, same-OTX limit-order target collection, and duplicate payment output index rejection; expose:

```rust
pub struct LockOtxFill {
    pub otx_index: usize,
    pub layout: OtxMessageLayout,
    pub action_data: Vec<u8>,
    pub action_target: [u8; 32],
}

pub fn load_lock_otx_fill(
    context: &CobuildContext,
    input_index: usize,
) -> Result<LockOtxFill, Error>;

pub fn output_index_in_otx_outputs(
    layout: OtxMessageLayout,
    output_index: usize,
) -> Result<bool, Error>;

pub fn ensure_unique_payment_output_indexes(
    actions: &[ActionView],
    limit_order_targets: &[[u8; 32]],
) -> Result<(), Error>;
```

- `settlement.rs`: own bound UDT payment output loading and NFT delivery scan; expose:

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

## Task 1: Type Settlement Module Extraction

**Files:**
- Create: `tests/contracts/limit-order-type/src/settlement.rs`
- Modify: `tests/contracts/limit-order-type/src/lib.rs`
- Modify: `tests/contracts/limit-order-type/src/entry.rs`

- [ ] **Step 1: Add the module declaration and a failing settlement module test**

Add `pub mod settlement;` to `tests/contracts/limit-order-type/src/lib.rs`.

Create `tests/contracts/limit-order-type/src/settlement.rs` with this initial test module and no production helper implementations:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nft_delivery_match_accepts_buyer_lock_and_offered_nft_type() {
        assert!(nft_delivery_matches(
            [7; 32],
            Some([8; 32]),
            [7; 32],
            [8; 32]
        ));
    }
}
```

- [ ] **Step 2: Run the red command**

Run: `cargo test -p limit-order-type --offline nft_delivery_match_accepts_buyer_lock_and_offered_nft_type`

Expected: FAIL because `nft_delivery_matches` is not defined in `settlement.rs`.

**Red/Green Record:**
- Red command:
- Red result:
- Green command:
- Green result:

- [ ] **Step 3: Move settlement helpers and tests**

Move these helpers from `tests/contracts/limit-order-type/src/entry.rs` to `tests/contracts/limit-order-type/src/settlement.rs`:

```rust
fn expected_proxy_lock_hash(order_type_hash: [u8; 32]) -> [u8; 32]
fn has_nft_proxy_output(
    offered_nft_type_hash: [u8; 32],
    proxy_lock_hash: [u8; 32],
) -> Result<bool, Error>
fn load_udt_payment_output(index: usize) -> Result<SettlementCell, Error>
fn has_nft_delivery_output(
    layout: OtxMessageLayout,
    buyer_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<bool, Error>
fn nft_delivery_matches(
    lock_hash: [u8; 32],
    type_hash: Option<[u8; 32]>,
    buyer_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> bool
```

Add the public wrappers in `settlement.rs`:

```rust
pub fn ensure_create_nft_proxy_output(
    current_type_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<(), Error> {
    let proxy_lock_hash = expected_proxy_lock_hash(current_type_hash);
    if !has_nft_proxy_output(offered_nft_type_hash, proxy_lock_hash)? {
        return Err(Error::InvalidCobuild);
    }
    Ok(())
}

pub fn load_bound_payment(
    layout: OtxMessageLayout,
    payment_output_index: u32,
) -> Result<SettlementCell, Error> {
    let index = payment_output_index as usize;
    if !crate::entry::output_index_in_otx_outputs(layout, index)? {
        return Err(Error::InvalidCobuild);
    }
    load_udt_payment_output(index)
}

pub fn ensure_nft_delivered_to_buyer(
    layout: OtxMessageLayout,
    buyer_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<(), Error> {
    if !has_nft_delivery_output(layout, buyer_lock_hash, offered_nft_type_hash)? {
        return Err(Error::InvalidCobuild);
    }
    Ok(())
}
```

Change `entry.rs` `output_index_in_otx_outputs` visibility for this intermediate state:

```rust
pub(crate) fn output_index_in_otx_outputs(
    layout: OtxMessageLayout,
    output_index: usize,
) -> Result<bool, Error>
```

Move these tests from `entry.rs` into `settlement.rs`:

```rust
#[test]
fn nft_delivery_match_accepts_buyer_lock_and_offered_nft_type()

#[test]
fn nft_delivery_match_rejects_wrong_buyer_lock()

#[test]
fn nft_delivery_match_rejects_wrong_or_missing_nft_type()

#[test]
fn expected_proxy_lock_hash_changes_with_order_type_hash()
```

Replace the initial one-test `settlement.rs` test module from Step 1 with the moved test module so there is only one `nft_delivery_match_accepts_buyer_lock_and_offered_nft_type` test in the file.

Update `entry.rs` call sites:

```rust
let payment = crate::settlement::load_bound_payment(layout, action.payment_output_index)?;

crate::settlement::ensure_nft_delivered_to_buyer(
    layout,
    action.buyer_lock_hash,
    order.offered_nft_type_hash,
)?;

crate::settlement::ensure_create_nft_proxy_output(
    current_type_hash,
    order.offered_nft_type_hash,
)?;
```

- [ ] **Step 4: Run the green command**

Run: `cargo test -p limit-order-type --offline`

Expected: PASS for the `limit-order-type` crate.

- [ ] **Step 5: Commit**

```bash
git add tests/contracts/limit-order-type/src/lib.rs tests/contracts/limit-order-type/src/entry.rs tests/contracts/limit-order-type/src/settlement.rs
git commit -m "refactor: extract type settlement helpers"
```

## Task 2: Type OTX Module Extraction

**Files:**
- Create: `tests/contracts/limit-order-type/src/otx.rs`
- Modify: `tests/contracts/limit-order-type/src/lib.rs`
- Modify: `tests/contracts/limit-order-type/src/entry.rs`
- Modify: `tests/contracts/limit-order-type/src/settlement.rs`

- [ ] **Step 1: Add the module declaration and a failing OTX module test**

Add `pub mod otx;` to `tests/contracts/limit-order-type/src/lib.rs`.

Create `tests/contracts/limit-order-type/src/otx.rs` with this initial test module and no production helper implementations:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use cobuild_core::layout::Range;

    #[test]
    fn range_contains_accepts_start_and_last_index() {
        assert_eq!(range_contains(Range { start: 3, count: 2 }, 3), Ok(true));
        assert_eq!(range_contains(Range { start: 3, count: 2 }, 4), Ok(true));
        assert_eq!(range_contains(Range { start: 3, count: 2 }, 5), Ok(false));
    }
}
```

- [ ] **Step 2: Run the red command**

Run: `cargo test -p limit-order-type --offline range_contains_accepts_start_and_last_index`

Expected: FAIL because `range_contains` is not defined in `otx.rs`.

**Red/Green Record:**
- Red command:
- Red result:
- Green command:
- Green result:

- [ ] **Step 3: Move OTX helpers and tests**

Move these helpers from `entry.rs` to `otx.rs`:

```rust
pub fn otx_fill_layout(
    origin: &ActionOrigin,
    relation: Option<OtxTypeRelation>,
) -> Result<(usize, OtxMessageLayout), Error>

pub fn output_index_in_otx_outputs(
    layout: OtxMessageLayout,
    output_index: usize,
) -> Result<bool, Error>

fn range_contains(range: Range, index: usize) -> Result<bool, Error>

pub fn ensure_unique_payment_output_indexes(
    actions: &[ActionView],
    limit_order_targets: &[[u8; 32]],
) -> Result<(), Error>

fn limit_order_target_hashes(
    actions: &[ActionView],
    current_target: [u8; 32],
) -> Result<Vec<[u8; 32]>, Error>

fn is_limit_order_role(role: ScriptRole) -> bool
```

Add the OTX fill context in `otx.rs`:

```rust
pub struct TypeOtxFill {
    pub otx_index: usize,
    pub layout: OtxMessageLayout,
    pub action_data: Vec<u8>,
    pub action_target: [u8; 32],
}

pub fn load_type_otx_fill(
    context: &CobuildContext,
    plan: &TypeValidationPlan,
) -> Result<TypeOtxFill, Error> {
    if plan.related_actions.len() != 1 {
        return Err(Error::InvalidCobuild);
    }
    let related = &plan.related_actions[0];
    let (otx_index, layout) = otx_fill_layout(
        &related.action.origin,
        related.otx_type_scope.in_otx_scope(),
    )?;
    let actions = context.otx_actions(otx_index)?;
    let targets = limit_order_target_hashes(&actions, related.action.action.script_hash)?;
    ensure_unique_payment_output_indexes(&actions, &targets)?;
    Ok(TypeOtxFill {
        otx_index,
        layout,
        action_data: cursor_bytes(&related.action.action.data)?,
        action_target: related.action.action.script_hash,
    })
}
```

Move these tests from `entry.rs` into `otx.rs`:

```rust
#[test]
fn otx_fill_context_accepts_base_input_relation()

#[test]
fn output_index_in_otx_outputs_accepts_base_and_append_outputs()

#[test]
fn output_index_in_otx_outputs_rejects_out_of_range_output()

#[test]
fn otx_fill_context_rejects_tx_level_action()

#[test]
fn otx_fill_context_rejects_non_base_input_relation()

#[test]
fn otx_fill_context_rejects_append_input_relation_only()

#[test]
fn duplicate_payment_output_index_accepts_unique_indexes()

#[test]
fn duplicate_payment_output_index_rejects_duplicate_indexes()

#[test]
fn duplicate_payment_output_index_rejects_mixed_type_lock_duplicate()

#[test]
fn limit_order_target_hashes_rejects_malformed_tag_two_in_selected_role()

#[test]
fn limit_order_target_hashes_ignores_unrelated_non_fill_actions()
```

Update `settlement.rs`:

```rust
if !crate::otx::output_index_in_otx_outputs(layout, index)? {
    return Err(Error::InvalidCobuild);
}
```

Update `entry.rs` fill flow to use:

```rust
let fill = crate::otx::load_type_otx_fill(context, plan)?;
let LimitOrderAction::Fill(action) = parse_limit_order_action(&fill.action_data)? else {
    return Err(Error::UnsupportedAction);
};
let payment = crate::settlement::load_bound_payment(fill.layout, action.payment_output_index)?;
validate_fill(&order, payment)?;
crate::settlement::ensure_nft_delivered_to_buyer(
    fill.layout,
    action.buyer_lock_hash,
    order.offered_nft_type_hash,
)?;
```

- [ ] **Step 4: Run the green command**

Run: `cargo test -p limit-order-type --offline`

Expected: PASS for the `limit-order-type` crate.

- [ ] **Step 5: Commit**

```bash
git add tests/contracts/limit-order-type/src/lib.rs tests/contracts/limit-order-type/src/entry.rs tests/contracts/limit-order-type/src/settlement.rs tests/contracts/limit-order-type/src/otx.rs
git commit -m "refactor: extract type otx helpers"
```

## Task 3: Type Validation Module Extraction

**Files:**
- Create: `tests/contracts/limit-order-type/src/validation.rs`
- Modify: `tests/contracts/limit-order-type/src/lib.rs`
- Modify: `tests/contracts/limit-order-type/src/entry.rs`

- [ ] **Step 1: Add the module declaration and a failing dispatch call**

Add `pub mod validation;` to `tests/contracts/limit-order-type/src/lib.rs`.

Create `tests/contracts/limit-order-type/src/validation.rs` with imports only:

```rust
use ckb_std::ckb_constants::Source;
use cobuild_core::{engine::CobuildContext, plan::TypeValidationPlan};

use crate::error::Error;
```

Update `entry.rs` dispatch before adding the functions:

```rust
match order_mode(input_count, output_count)? {
    OrderMode::Create => crate::validation::validate_create_order(current_type_hash, &plan),
    OrderMode::Fill => crate::validation::validate_fill_order(&context, &plan),
}
```

- [ ] **Step 2: Run the red command**

Run: `cargo test -p limit-order-type --offline`

Expected: FAIL to compile because `crate::validation::validate_create_order` and `crate::validation::validate_fill_order` do not exist yet.

**Red/Green Record:**
- Red command:
- Red result:
- Green command:
- Green result:

- [ ] **Step 3: Move high-level validation helpers**

Move these helpers from `entry.rs` to `validation.rs`:

```rust
fn validate_fill_entry(
    context: &CobuildContext,
    plan: &TypeValidationPlan,
) -> Result<(), Error>

fn validate_create_entry(
    current_type_hash: [u8; 32],
    plan: &TypeValidationPlan,
) -> Result<(), Error>

fn single_create_action(plan: &TypeValidationPlan) -> Result<CreateOrderAction, Error>
```

Rename the moved public entry points:

```rust
pub fn validate_create_order(
    current_type_hash: [u8; 32],
    plan: &TypeValidationPlan,
) -> Result<(), Error> {
    crate::entry::validate_order_type_id()?;
    let order = crate::entry::single_group_order(Source::GroupOutput)?;
    let action = single_create_action(plan)?;
    validate_create(&order, &action)?;
    crate::settlement::ensure_create_nft_proxy_output(
        current_type_hash,
        order.offered_nft_type_hash,
    )
}

pub fn validate_fill_order(
    context: &CobuildContext,
    plan: &TypeValidationPlan,
) -> Result<(), Error> {
    let order = crate::entry::single_group_order(Source::GroupInput)?;
    let fill = crate::otx::load_type_otx_fill(context, plan)?;
    let LimitOrderAction::Fill(action) = parse_limit_order_action(&fill.action_data)? else {
        return Err(Error::UnsupportedAction);
    };
    let payment = crate::settlement::load_bound_payment(fill.layout, action.payment_output_index)?;
    validate_fill(&order, payment)?;
    crate::settlement::ensure_nft_delivered_to_buyer(
        fill.layout,
        action.buyer_lock_hash,
        order.offered_nft_type_hash,
    )
}
```

Keep `validate_order_type_id` and `single_group_order` in `entry.rs`, and change their visibility to `pub(crate)` so `validation.rs` can use them:

```rust
pub(crate) fn single_group_order(source: Source) -> Result<crate::types::OrderState, Error>

pub(crate) fn validate_order_type_id() -> Result<(), Error>
```

Update `entry.rs` dispatch:

```rust
match order_mode(input_count, output_count)? {
    OrderMode::Create => crate::validation::validate_create_order(current_type_hash, &plan),
    OrderMode::Fill => crate::validation::validate_fill_order(&context, &plan),
}
```

Move this test from `entry.rs` into `validation.rs`:

```rust
#[test]
fn create_action_context_accepts_any_origin_with_single_create_action()
```

Keep these tests in `entry.rs`:

```rust
#[test]
fn order_mode_accepts_create_shape()

#[test]
fn order_mode_accepts_fill_shape()

#[test]
fn order_mode_rejects_update_or_empty_shapes()

#[test]
fn type_id_sys_error_maps_to_stable_exit_code()
```

- [ ] **Step 4: Run the green command**

Run: `cargo test -p limit-order-type --offline`

Expected: PASS for the `limit-order-type` crate.

- [ ] **Step 5: Commit**

```bash
git add tests/contracts/limit-order-type/src/lib.rs tests/contracts/limit-order-type/src/entry.rs tests/contracts/limit-order-type/src/validation.rs
git commit -m "refactor: extract type validation flow"
```

## Task 4: Lock Input Module Extraction

**Files:**
- Create: `tests/contracts/limit-order-lock/src/input.rs`
- Modify: `tests/contracts/limit-order-lock/src/lib.rs`
- Modify: `tests/contracts/limit-order-lock/src/entry.rs`

- [ ] **Step 1: Add the module declaration and a failing input module call**

Add `pub mod input;` to `tests/contracts/limit-order-lock/src/lib.rs`.

Create `tests/contracts/limit-order-lock/src/input.rs` with imports only:

```rust
use ckb_std::{
    ckb_constants::Source,
    high_level::{QueryIter, load_cell_data, load_cell_lock_hash, load_cell_type_hash},
};

use crate::error::Error;
```

Update `entry.rs` temporarily:

```rust
let input_index = crate::input::load_current_order_input(
    current_lock_hash,
    order.offered_nft_type_hash,
)?;
```

- [ ] **Step 2: Run the red command**

Run: `cargo test -p limit-order-lock --offline`

Expected: FAIL to compile because `crate::input::load_current_order_input` does not exist yet.

**Red/Green Record:**
- Red command:
- Red result:
- Green command:
- Green result:

- [ ] **Step 3: Move input helpers**

Move these helpers from `entry.rs` to `input.rs`:

```rust
fn single_group_input_index(current_lock_hash: [u8; 32]) -> Result<usize, Error>

fn verify_offered_nft_input(
    input_index: usize,
    offered_nft_type_hash: [u8; 32],
) -> Result<(), Error>
```

Implement the public wrapper:

```rust
pub fn load_current_order_input(
    current_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<usize, Error> {
    let input_index = single_group_input_index(current_lock_hash)?;
    verify_offered_nft_input(input_index, offered_nft_type_hash)?;
    Ok(input_index)
}
```

Remove the direct `verify_offered_nft_input` call from `entry.rs`.

- [ ] **Step 4: Run the green command**

Run: `cargo test -p limit-order-lock --offline`

Expected: PASS for the `limit-order-lock` crate.

- [ ] **Step 5: Commit**

```bash
git add tests/contracts/limit-order-lock/src/lib.rs tests/contracts/limit-order-lock/src/entry.rs tests/contracts/limit-order-lock/src/input.rs
git commit -m "refactor: extract lock input helpers"
```

## Task 5: Lock Settlement Module Extraction

**Files:**
- Create: `tests/contracts/limit-order-lock/src/settlement.rs`
- Modify: `tests/contracts/limit-order-lock/src/lib.rs`
- Modify: `tests/contracts/limit-order-lock/src/entry.rs`

- [ ] **Step 1: Add the module declaration and a failing settlement module test**

Add `pub mod settlement;` to `tests/contracts/limit-order-lock/src/lib.rs`.

Create `tests/contracts/limit-order-lock/src/settlement.rs` with this initial test module and no production helper implementations:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nft_delivery_match_accepts_buyer_lock_and_offered_nft_type() {
        assert!(nft_delivery_matches(
            [7; 32],
            Some([8; 32]),
            [7; 32],
            [8; 32]
        ));
    }
}
```

- [ ] **Step 2: Run the red command**

Run: `cargo test -p limit-order-lock --offline nft_delivery_match_accepts_buyer_lock_and_offered_nft_type`

Expected: FAIL because `nft_delivery_matches` is not defined in `settlement.rs`.

**Red/Green Record:**
- Red command:
- Red result:
- Green command:
- Green result:

- [ ] **Step 3: Move settlement helpers and tests**

Move these helpers from `entry.rs` to `settlement.rs`:

```rust
fn load_udt_payment_output(index: usize) -> Result<UdtPayment, Error>

fn has_nft_delivery_output(
    layout: OtxMessageLayout,
    buyer_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<bool, Error>

fn nft_delivery_matches(
    lock_hash: [u8; 32],
    type_hash: Option<[u8; 32]>,
    buyer_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> bool
```

Add the public wrappers:

```rust
pub fn load_bound_payment(
    layout: OtxMessageLayout,
    payment_output_index: u32,
) -> Result<UdtPayment, Error> {
    let index = payment_output_index as usize;
    if !crate::entry::output_index_in_otx_outputs(layout, index)? {
        return Err(Error::InvalidCobuild);
    }
    load_udt_payment_output(index)
}

pub fn ensure_nft_delivered_to_buyer(
    layout: OtxMessageLayout,
    buyer_lock_hash: [u8; 32],
    offered_nft_type_hash: [u8; 32],
) -> Result<(), Error> {
    if !has_nft_delivery_output(layout, buyer_lock_hash, offered_nft_type_hash)? {
        return Err(Error::InvalidCobuild);
    }
    Ok(())
}
```

Change `entry.rs` `output_index_in_otx_outputs` visibility for this intermediate state:

```rust
pub(crate) fn output_index_in_otx_outputs(
    layout: OtxMessageLayout,
    output_index: usize,
) -> Result<bool, Error>
```

Move these tests from `entry.rs` into `settlement.rs`:

```rust
#[test]
fn nft_delivery_match_accepts_buyer_lock_and_offered_nft_type()

#[test]
fn nft_delivery_match_rejects_wrong_buyer_lock()

#[test]
fn nft_delivery_match_rejects_wrong_or_missing_nft_type()
```

Replace the initial one-test `settlement.rs` test module from Step 1 with the moved test module so there is only one `nft_delivery_match_accepts_buyer_lock_and_offered_nft_type` test in the file.

Update `entry.rs` call sites:

```rust
let payment = crate::settlement::load_bound_payment(layout, action.payment_output_index)?;

crate::settlement::ensure_nft_delivered_to_buyer(
    layout,
    action.buyer_lock_hash,
    order.offered_nft_type_hash,
)?;
```

- [ ] **Step 4: Run the green command**

Run: `cargo test -p limit-order-lock --offline`

Expected: PASS for the `limit-order-lock` crate.

- [ ] **Step 5: Commit**

```bash
git add tests/contracts/limit-order-lock/src/lib.rs tests/contracts/limit-order-lock/src/entry.rs tests/contracts/limit-order-lock/src/settlement.rs
git commit -m "refactor: extract lock settlement helpers"
```

## Task 6: Lock OTX Module Extraction

**Files:**
- Create: `tests/contracts/limit-order-lock/src/otx.rs`
- Modify: `tests/contracts/limit-order-lock/src/lib.rs`
- Modify: `tests/contracts/limit-order-lock/src/entry.rs`
- Modify: `tests/contracts/limit-order-lock/src/settlement.rs`

- [ ] **Step 1: Add the module declaration and a failing OTX module test**

Add `pub mod otx;` to `tests/contracts/limit-order-lock/src/lib.rs`.

Create `tests/contracts/limit-order-lock/src/otx.rs` with this initial test module and no production helper implementations:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use cobuild_core::layout::Range;

    #[test]
    fn range_contains_accepts_start_and_last_index() {
        assert_eq!(range_contains(Range { start: 3, count: 2 }, 3), Ok(true));
        assert_eq!(range_contains(Range { start: 3, count: 2 }, 4), Ok(true));
        assert_eq!(range_contains(Range { start: 3, count: 2 }, 5), Ok(false));
    }
}
```

- [ ] **Step 2: Run the red command**

Run: `cargo test -p limit-order-lock --offline range_contains_accepts_start_and_last_index`

Expected: FAIL because `range_contains` is not defined in `otx.rs`.

**Red/Green Record:**
- Red command:
- Red result:
- Green command:
- Green result:

- [ ] **Step 3: Move OTX helpers and tests**

Move these helpers from `entry.rs` to `otx.rs`:

```rust
pub fn otx_fill_layout(
    origin: &ActionOrigin,
    input_index: usize,
) -> Result<(usize, OtxMessageLayout), Error>

pub fn output_index_in_otx_outputs(
    layout: OtxMessageLayout,
    output_index: usize,
) -> Result<bool, Error>

fn range_contains(range: Range, index: usize) -> Result<bool, Error>

fn ensure_unique_payment_output_indexes(
    actions: &[ActionView],
    limit_order_targets: &[[u8; 32]],
) -> Result<(), Error>

fn limit_order_target_hashes(
    actions: &[ActionView],
    current_target: [u8; 32],
) -> Result<Vec<[u8; 32]>, Error>

fn is_limit_order_role(role: ScriptRole) -> bool
```

Add the OTX fill context:

```rust
pub struct LockOtxFill {
    pub otx_index: usize,
    pub layout: OtxMessageLayout,
    pub action_data: Vec<u8>,
    pub action_target: [u8; 32],
}

pub fn load_lock_otx_fill(
    context: &CobuildContext,
    input_index: usize,
) -> Result<LockOtxFill, Error> {
    let plan = context.plan_lock_validation()?;
    if plan.related_actions.len() != 1 {
        return Err(Error::InvalidCobuild);
    }
    let related = &plan.related_actions[0];
    let (otx_index, layout) = otx_fill_layout(&related.origin, input_index)?;
    let actions = context.otx_actions(otx_index)?;
    let targets = limit_order_target_hashes(&actions, related.action.script_hash)?;
    ensure_unique_payment_output_indexes(&actions, &targets)?;
    Ok(LockOtxFill {
        otx_index,
        layout,
        action_data: cursor_bytes(&related.action.data)?,
        action_target: related.action.script_hash,
    })
}
```

Move these tests from `entry.rs` into `otx.rs`:

```rust
#[test]
fn otx_fill_layout_accepts_current_input_in_base_scope()

#[test]
fn otx_fill_layout_rejects_tx_level_action()

#[test]
fn otx_fill_layout_rejects_append_only_current_input()

#[test]
fn range_contains_accepts_start_and_last_index()

#[test]
fn output_index_in_otx_outputs_accepts_base_and_append_outputs()

#[test]
fn output_index_in_otx_outputs_rejects_out_of_range_output()

#[test]
fn range_contains_rejects_overflowing_range()

#[test]
fn duplicate_payment_output_index_accepts_unique_indexes()

#[test]
fn duplicate_payment_output_index_rejects_duplicate_indexes()

#[test]
fn duplicate_payment_output_index_rejects_mixed_type_lock_duplicate()

#[test]
fn limit_order_target_hashes_rejects_malformed_tag_two_in_selected_role()

#[test]
fn limit_order_target_hashes_ignores_unrelated_non_fill_actions()
```

Update `settlement.rs`:

```rust
if !crate::otx::output_index_in_otx_outputs(layout, index)? {
    return Err(Error::InvalidCobuild);
}
```

Update `entry.rs` fill flow to use:

```rust
let fill = crate::otx::load_lock_otx_fill(&context, input_index)?;
let action = parse_fill_order_action(&fill.action_data)?;
let payment = crate::settlement::load_bound_payment(fill.layout, action.payment_output_index)?;
validate_fill(&order, payment)?;
crate::settlement::ensure_nft_delivered_to_buyer(
    fill.layout,
    action.buyer_lock_hash,
    order.offered_nft_type_hash,
)?;
```

- [ ] **Step 4: Run the green command**

Run: `cargo test -p limit-order-lock --offline`

Expected: PASS for the `limit-order-lock` crate.

- [ ] **Step 5: Commit**

```bash
git add tests/contracts/limit-order-lock/src/lib.rs tests/contracts/limit-order-lock/src/entry.rs tests/contracts/limit-order-lock/src/settlement.rs tests/contracts/limit-order-lock/src/otx.rs
git commit -m "refactor: extract lock otx helpers"
```

## Task 7: Lock Validation Module Extraction

**Files:**
- Create: `tests/contracts/limit-order-lock/src/validation.rs`
- Modify: `tests/contracts/limit-order-lock/src/lib.rs`
- Modify: `tests/contracts/limit-order-lock/src/entry.rs`

- [ ] **Step 1: Add the module declaration and a failing validation module call**

Add `pub mod validation;` to `tests/contracts/limit-order-lock/src/lib.rs`.

Create `tests/contracts/limit-order-lock/src/validation.rs` with imports only:

```rust
use cobuild_core::engine::CobuildContext;

use crate::{error::Error, types::OrderArgs};
```

Update `entry.rs`:

```rust
let context = CobuildContext::build(CurrentScript::InputLock(current_lock_hash))?;
crate::validation::validate_fill_order(&context, &order, current_lock_hash)
```

- [ ] **Step 2: Run the red command**

Run: `cargo test -p limit-order-lock --offline`

Expected: FAIL to compile because `crate::validation::validate_fill_order` does not exist yet.

**Red/Green Record:**
- Red command:
- Red result:
- Green command:
- Green result:

- [ ] **Step 3: Move high-level fill flow**

Move lock fill orchestration out of `entry.rs` and implement:

```rust
pub fn validate_fill_order(
    context: &CobuildContext,
    order: &OrderArgs,
    current_lock_hash: [u8; 32],
) -> Result<(), Error> {
    let input_index = crate::input::load_current_order_input(
        current_lock_hash,
        order.offered_nft_type_hash,
    )?;
    let fill = crate::otx::load_lock_otx_fill(context, input_index)?;
    let action = parse_fill_order_action(&fill.action_data)?;
    let payment = crate::settlement::load_bound_payment(fill.layout, action.payment_output_index)?;
    validate_fill(order, payment)?;
    crate::settlement::ensure_nft_delivered_to_buyer(
        fill.layout,
        action.buyer_lock_hash,
        order.offered_nft_type_hash,
    )
}
```

After this step, `entry.rs` should contain only:

```rust
pub fn main() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();
    let order = parse_order_args(&args)?;

    let current_lock_hash = load_script_hash()?;
    let context = CobuildContext::build(CurrentScript::InputLock(current_lock_hash))?;
    crate::validation::validate_fill_order(&context, &order, current_lock_hash)
}
```

- [ ] **Step 4: Run the green command**

Run: `cargo test -p limit-order-lock --offline`

Expected: PASS for the `limit-order-lock` crate.

- [ ] **Step 5: Commit**

```bash
git add tests/contracts/limit-order-lock/src/lib.rs tests/contracts/limit-order-lock/src/entry.rs tests/contracts/limit-order-lock/src/validation.rs
git commit -m "refactor: extract lock validation flow"
```

## Task 8: Final Verification And Execution Record

**Files:**
- Modify only if execution records are filled in: `docs/superpowers/plans/2026-06-09-limit-order-entry-refactor-plan.md`

- [ ] **Step 1: Run formatting**

Run: `cargo fmt`

Expected: command exits 0.

- [ ] **Step 2: Run focused contract crate tests**

Run:

```bash
cargo test -p limit-order-type --offline
cargo test -p limit-order-lock --offline
```

Expected: both commands exit 0.

- [ ] **Step 3: Run limit-order integration tests**

Run:

```bash
cargo test -p tests --test limit_order_type --offline
cargo test -p tests --test limit_order_lock --offline
```

Expected: both commands exit 0.

- [ ] **Step 4: Run broader workspace verification**

Run:

```bash
cargo test -p tests --lib --offline
cargo test --workspace --offline
cargo clippy --workspace --offline --all-targets
cargo fmt --check
git diff --check
git status --short
find tests/failed_txs -maxdepth 1 -type f 2>/dev/null | wc -l
git status --short --ignored tests/failed_txs
```

Expected:
- Cargo test, clippy, fmt, and diff-check commands exit 0.
- `git status --short` only shows intended implementation-plan execution record edits if this file was updated during execution.
- `find tests/failed_txs -maxdepth 1 -type f 2>/dev/null | wc -l` prints `0`.
- `git status --short --ignored tests/failed_txs` shows no unexpected tracked or ignored failed transaction artifacts.

- [ ] **Step 5: Rebuild debug contract binaries only if integration tests indicate stale binaries**

Run these commands only if `cargo test -p tests --test limit_order_type --offline` or `cargo test -p tests --test limit_order_lock --offline` fails due to stale or missing debug contract binaries:

```bash
make -e -C tests/contracts/limit-order-type build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/limit-order-lock build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-udt build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
make -e -C tests/contracts/test-nft build MODE=debug TOP=/home/xcshuan/contracts/ckb/cobuild-otx-contracts BUILD_DIR=build/debug CARGO_ARGS=--offline
```

Expected: each required build command exits 0. Re-run the failed integration command after rebuilding and expect it to pass.

- [ ] **Step 6: Record final verification results**

Fill in this record with the exact commands that were run and the observed result:

**Final Verification Record:**
- `cargo fmt`:
- `cargo test -p limit-order-type --offline`:
- `cargo test -p limit-order-lock --offline`:
- `cargo test -p tests --test limit_order_type --offline`:
- `cargo test -p tests --test limit_order_lock --offline`:
- `cargo test -p tests --lib --offline`:
- `cargo test --workspace --offline`:
- `cargo clippy --workspace --offline --all-targets`:
- `cargo fmt --check`:
- `git diff --check`:
- `git status --short`:
- `find tests/failed_txs -maxdepth 1 -type f 2>/dev/null | wc -l`:
- `git status --short --ignored tests/failed_txs`:
- Debug rebuild commands, if any:

- [ ] **Step 7: Commit final execution record if the plan file was updated**

If Step 6 changes this plan file, commit the record:

```bash
git add docs/superpowers/plans/2026-06-09-limit-order-entry-refactor-plan.md
git commit -m "docs: record limit order entry refactor verification"
```

## Plan Self-Review

- Spec coverage: The plan maps the design spec to local modules for `limit-order-type` and `limit-order-lock`, keeps `entry.rs` as orchestration, moves OTX plumbing, settlement checks, lock input helpers, and high-level validation flows, and preserves the tests-only local crate boundary.
- Placeholder scan: The plan contains no unfinished marker text, deferred edge-handling instructions, or references to an earlier task as a substitute for explicit steps.
- Type and function consistency: Public interfaces use `TypeOtxFill`, `LockOtxFill`, `SettlementCell`, `UdtPayment`, `OrderArgs`, `TypeValidationPlan`, `CobuildContext`, `OtxMessageLayout`, `ActionView`, `validate_create_order`, `validate_fill_order`, `load_bound_payment`, and `ensure_nft_delivered_to_buyer` consistently with the design spec and current source names.
