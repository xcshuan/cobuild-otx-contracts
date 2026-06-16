# Cobuild Extension Helper API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add low-risk Cobuild Core helper APIs that make Standard Extensions easier to build, then update existing test contracts to use them.

**Architecture:** Keep the Core v1 Molecule schema and signing rules unchanged. Add Rust-only helpers for canonical action references, origin/layout classification, and action enumeration with origins. Update limit-order and nft-minter test contracts to remove duplicated range/origin plumbing.

**Tech Stack:** Rust 1.92, `cobuild-core`, CKB contract test crates, cargo workspace tests.

---

## File Map

- Modify `crates/cobuild-core/src/layout.rs`
  - Add `Range::is_empty`, `Range::contains`, and `Range::local_index`.
- Modify `crates/cobuild-core/src/plan.rs`
  - Add `ActionRef`.
  - Add `OtxPart`.
  - Add `ActionOrigin` convenience methods.
  - Add `RelatedAction::action_ref`.
  - Add `OtxMessageLayout::classify_*` helpers.
  - Add `OtxTypeRelation` helper predicates.
  - Add `unique_related_action` helpers for lock/type plans.
- Modify `crates/cobuild-core/src/engine.rs`
  - Add `CobuildContext::tx_level_actions`.
  - Update `CobuildContext::otx_actions` to return validated related actions.
  - Add `CobuildContext::all_actions`.
- Modify `crates/cobuild-core/tests/plan.rs`
  - Test range/layout/origin/action-ref helpers.
- Modify `crates/cobuild-core/src/engine.rs` tests
  - Test new action enumeration methods.
- Modify `tests/contracts/limit-order-lock/src/otx.rs`
  - Replace local `range_contains` and bare `otx_actions` usage with Core helpers.
- Modify `tests/contracts/limit-order-type/src/otx.rs`
  - Replace manual origin matching and bare `otx_actions` usage with Core helpers.
- Modify `tests/contracts/nft-minter-type/src/validation/mint.rs`
  - Use `ActionRef` for action identity/sorting.
- Modify `docs/superpowers/specs/2026-05-28-cobuild-core-community-redraft-design.md`
  - Document extension hash framing and OTX extension signing constraints.

## Task 1: Plan-Level Helper Tests

**Files:**
- Modify: `crates/cobuild-core/tests/plan.rs`

- [ ] **Step 1: Add failing tests for helper behavior**

Append tests covering:

```rust
#[test]
fn range_contains_and_local_index_are_non_panicking() {
    let range = Range { start: 5, count: 3 };

    assert!(!range.is_empty());
    assert_eq!(range.contains(4), false);
    assert_eq!(range.contains(5), true);
    assert_eq!(range.contains(7), true);
    assert_eq!(range.contains(8), false);
    assert_eq!(range.local_index(4), None);
    assert_eq!(range.local_index(5), Some(0));
    assert_eq!(range.local_index(7), Some(2));
    assert_eq!(range.local_index(8), None);

    let overflowing = Range {
        start: usize::MAX,
        count: 2,
    };
    assert_eq!(overflowing.contains(usize::MAX), false);
    assert_eq!(overflowing.local_index(usize::MAX), None);
}

#[test]
fn action_origin_exposes_canonical_action_refs() {
    let tx = ActionOrigin::TxLevel { witness_index: 9 };
    assert_eq!(tx.witness_index(), 9);
    assert_eq!(tx.otx_index(), None);
    assert!(tx.is_tx_level());
    assert_eq!(
        tx.action_ref(2),
        ActionRef::TxLevel {
            witness_index: 9,
            action_index: 2,
        }
    );

    let layout = OtxMessageLayout {
        base_inputs: Range { start: 1, count: 2 },
        append_inputs: Range { start: 3, count: 1 },
        base_outputs: Range { start: 10, count: 1 },
        append_outputs: Range { start: 11, count: 2 },
        base_cell_deps: Range { start: 0, count: 0 },
        append_cell_deps: Range { start: 0, count: 0 },
        base_header_deps: Range { start: 0, count: 0 },
        append_header_deps: Range { start: 0, count: 0 },
    };
    let otx = ActionOrigin::Otx {
        witness_index: 4,
        otx_index: 1,
        layout,
    };

    assert_eq!(otx.witness_index(), 4);
    assert_eq!(otx.otx_index(), Some(1));
    assert_eq!(otx.otx_layout(), Some(layout));
    assert!(otx.is_otx());
    assert_eq!(
        otx.action_ref(3),
        ActionRef::Otx {
            witness_index: 4,
            otx_index: 1,
            action_index: 3,
        }
    );
}

#[test]
fn otx_message_layout_classifies_base_and_append_indices() {
    let layout = OtxMessageLayout {
        base_inputs: Range { start: 1, count: 2 },
        append_inputs: Range { start: 3, count: 1 },
        base_outputs: Range { start: 10, count: 1 },
        append_outputs: Range { start: 11, count: 2 },
        base_cell_deps: Range { start: 20, count: 1 },
        append_cell_deps: Range { start: 21, count: 1 },
        base_header_deps: Range { start: 30, count: 1 },
        append_header_deps: Range { start: 31, count: 0 },
    };

    assert_eq!(layout.classify_input(1), Some((OtxPart::Base, 0)));
    assert_eq!(layout.classify_input(3), Some((OtxPart::Append, 0)));
    assert_eq!(layout.classify_input(4), None);
    assert_eq!(layout.classify_output(10), Some((OtxPart::Base, 0)));
    assert_eq!(layout.classify_output(12), Some((OtxPart::Append, 1)));
    assert_eq!(layout.classify_cell_dep(21), Some((OtxPart::Append, 0)));
    assert_eq!(layout.classify_header_dep(30), Some((OtxPart::Base, 0)));
}
```

- [ ] **Step 2: Run failing tests**

Run: `cargo test -p cobuild-core --test plan --offline`

Expected: compile failure for missing `ActionRef`, `OtxPart`, `Range` helpers, and helper methods.

- [ ] **Step 3: Implement minimal helper APIs**

Add the helpers in `layout.rs` and `plan.rs`. Keep them pure and no-alloc.

- [ ] **Step 4: Run plan tests**

Run: `cargo test -p cobuild-core --test plan --offline`

Expected: tests pass.

## Task 2: Context Action Enumeration APIs

**Files:**
- Modify: `crates/cobuild-core/src/engine.rs`

- [ ] **Step 1: Add failing tests in `engine.rs` test module**

Add tests for:

```rust
#[test]
fn otx_actions_return_origin_layout_and_validate_targets() {
    let message = message_with_actions(&[
        action_bytes(1, [0x55; 32], &[0x20]),
        action_bytes(2, [0x66; 32], &[0x30]),
    ]);
    let context = test_context_with_otx_entries(vec![test_otx(&message, 1, 2)]);

    let actions = context.otx_actions(0).unwrap();

    assert_eq!(actions.len(), 2);
    assert_eq!(
        actions[0].action_ref(),
        crate::plan::ActionRef::Otx {
            witness_index: 7,
            otx_index: 0,
            action_index: 0,
        }
    );
    assert!(matches!(
        actions[0].origin,
        ActionOrigin::Otx {
            witness_index: 7,
            otx_index: 0,
            ..
        }
    ));
}

#[test]
fn all_actions_returns_otx_actions_with_canonical_origin() {
    let first_message = message_with_actions(&[action_bytes(0, [0x44; 32], &[0x10])]);
    let second_message = message_with_actions(&[action_bytes(1, [0x55; 32], &[0x20])]);
    let context = test_context_with_otx_entries(vec![
        test_otx(&first_message, 0, 1),
        test_otx(&second_message, 1, 2),
    ]);

    let actions = context.all_actions().unwrap();

    assert_eq!(actions.len(), 2);
    assert_eq!(
        actions[0].action_ref(),
        crate::plan::ActionRef::Otx {
            witness_index: 7,
            otx_index: 0,
            action_index: 0,
        }
    );
    assert_eq!(
        actions[1].action_ref(),
        crate::plan::ActionRef::Otx {
            witness_index: 7,
            otx_index: 1,
            action_index: 0,
        }
    );
}
```

- [ ] **Step 2: Run failing test**

Run: `cargo test -p cobuild-core --lib --offline otx_actions`

Expected: compile failure for missing methods.

- [ ] **Step 3: Implement context methods**

Add `tx_level_actions`, validated `otx_actions`, and `all_actions` to `CobuildContext`.

- [ ] **Step 4: Run targeted library tests**

Run: `cargo test -p cobuild-core --lib --offline otx_actions all_actions`

Expected: tests pass.

## Task 3: Optimize Existing Test Contracts

**Files:**
- Modify: `tests/contracts/limit-order-lock/src/otx.rs`
- Modify: `tests/contracts/limit-order-type/src/otx.rs`
- Modify: `tests/contracts/nft-minter-type/src/validation/mint.rs`

- [ ] **Step 1: Run existing contract unit tests as baseline**

Run:

```bash
cargo test -p limit-order-lock --offline
cargo test -p limit-order-type --offline
cargo test -p nft-minter-type --offline
```

Expected: baseline result is recorded before edits.

- [ ] **Step 2: Update limit-order lock**

Change `load_lock_otx_fill` to use `context.otx_actions(otx_index)?`.
Change `otx_fill_layout` to use `origin.otx_index()`, `origin.otx_layout()`,
and `layout.classify_input(input_index) == Some((OtxPart::Base, _))`.
Delete the local `range_contains` helper and its tests.

- [ ] **Step 3: Update limit-order type**

Change `load_type_otx_fill` to use `context.otx_actions(otx_index)?`.
Change `otx_fill_layout` to use `origin.otx_index()` and `origin.otx_layout()`.
Use `relation.input_type_in_base()` after adding helper methods.

- [ ] **Step 4: Update nft-minter**

Change `MintActionFact` to carry `action_ref: ActionRef` instead of separate
`witness_index` and `action_index`. Sort by `action_ref`.

- [ ] **Step 5: Run contract tests**

Run:

```bash
cargo test -p limit-order-lock --offline
cargo test -p limit-order-type --offline
cargo test -p nft-minter-type --offline
```

Expected: tests pass.

## Task 4: Document Extension Guidance

**Files:**
- Modify: `docs/superpowers/specs/2026-05-28-cobuild-core-community-redraft-design.md`

- [ ] **Step 1: Add extension helper guidance**

In the Standard Extension Boundary section, add:

- Canonical `ActionRef` is derived from message origin plus action index.
- Standard extensions must use injective hash framing.
- OTX extension actions must be finalized before the signed message is signed.
- Intent-before-trace flows must use intent commitment plus separate proof, not mutation of a signed message.

- [ ] **Step 2: Scan docs for contradictions**

Run:

```bash
rg -n "ActionRef|extension hash|signed message|OTX message" docs/superpowers/specs/2026-05-28-cobuild-core-community-redraft-design.md
```

Expected: new guidance appears in Standard Extension Boundary or nearby section.

## Task 5: Final Verification

**Files:**
- All modified files.

- [ ] **Step 1: Run focused tests**

Run:

```bash
cargo test -p cobuild-core --test plan --offline
cargo test -p cobuild-core --lib --offline
cargo test -p limit-order-lock --offline
cargo test -p limit-order-type --offline
cargo test -p nft-minter-type --offline
```

Expected: all pass.

- [ ] **Step 2: Run workspace tests if focused tests pass**

Run: `cargo test --workspace --offline`

Expected: all pass or report pre-existing failures clearly.

- [ ] **Step 3: Check git status**

Run: `git status --short`

Expected: modified plan, core files, docs, and optimized test-contract files only, plus any pre-existing untracked docs.
