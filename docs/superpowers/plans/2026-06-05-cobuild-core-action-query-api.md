# Cobuild Core Action Query API Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a public Cobuild Core API that lets lock and type scripts query `Action`s addressed to themselves without duplicating Message parsing.

**Architecture:** `MessageView` becomes the action-query surface and exposes cursor-backed `ActionView`s. Planning remains responsible for finding related messages and signature/message scopes; scripts call `actions_for` or `unique_action_for` to apply their own ABI policy. `MessageOrigin` is made role-neutral so both lock and type plans can carry related messages, while type-specific OTX relation data stays on type plan entries.

**Tech Stack:** Rust `no_std`, `alloc::vec::Vec`, `cobuild_types::lazy_reader`, existing `cobuild-core` plan/view/context modules, `cargo test -p cobuild-core --offline`.

---

## File Structure

- Modify `crates/cobuild-core/src/view.rs`
  - Add `ActionView`.
  - Move action parsing into `MessageView::actions`.
  - Add `MessageView::actions_for` and `MessageView::unique_action_for`.
  - Keep a crate-private helper for `TxScriptHashes::validate_message_targets` if useful, but make it call the public `MessageView` path.
- Modify `crates/cobuild-core/src/protocol.rs`
  - Keep `ScriptRole` as the public role enum used by action query APIs.
- Modify `crates/cobuild-core/src/error.rs`
  - Add `DuplicateMatchingAction`.
- Modify `crates/cobuild-core/src/plan.rs`
  - Add `LockValidationPlan.related_messages`.
  - Make `MessageOrigin::Otx` role-neutral by removing `relation`.
- Add `TypeRelatedMessage` so type plans keep optional `OtxTypeRelation` without forcing lock plans or tx-level type messages to carry type-only data.
- Modify `crates/cobuild-core/src/engine.rs`
  - Populate lock related messages on tx-level and OTX-relevant lock flows.
  - Populate type related messages using the new `TypeRelatedMessage` shape.
- Modify `crates/cobuild-core/src/context.rs`
  - Route target validation through `MessageView::actions`.
- Modify `crates/cobuild-core/tests/view.rs`
  - Add action query tests.
- Modify `crates/cobuild-core/tests/plan.rs`
  - Update plan construction tests for the new plan shapes.
- Modify `tests/tests/contract_template_layout.rs`
  - Update architecture guard expectations so action parsing is exposed on `MessageView` and not duplicated elsewhere.

---

### Task 1: Add `MessageView` Action Query API

**Files:**
- Modify: `crates/cobuild-core/src/view.rs`
- Modify: `crates/cobuild-core/tests/view.rs`

- [ ] **Step 1: Write failing tests for action parsing and filtering**

Append these tests and helpers to `crates/cobuild-core/tests/view.rs`:

```rust
use cobuild_core::protocol::ScriptRole;

#[test]
fn message_view_returns_action_views_with_cursor_backed_data() {
    let script_info_hash = [0x11u8; 32];
    let script_hash = [0x22u8; 32];
    let message = message_with_actions(&[action_bytes(
        script_info_hash,
        0,
        script_hash,
        &[0xaa, 0xbb],
    )]);
    let view = MessageView::new(cobuild_core::reader::cursor_from_slice(&message));

    let actions = view.actions().unwrap();

    assert_eq!(actions.len(), 1);
    assert_eq!(actions[0].index, 0);
    assert_eq!(actions[0].script_info_hash, script_info_hash);
    assert_eq!(actions[0].script_role, ScriptRole::InputLock);
    assert_eq!(actions[0].script_hash, script_hash);
    assert_eq!(
        cursor_bytes(&actions[0].data).unwrap(),
        vec![0xaa, 0xbb]
    );
}

#[test]
fn message_view_filters_actions_by_role_and_script_hash() {
    let lock_hash = [0x33u8; 32];
    let other_hash = [0x44u8; 32];
    let message = message_with_actions(&[
        action_bytes([0x01u8; 32], 0, lock_hash, &[0x10]),
        action_bytes([0x02u8; 32], 1, lock_hash, &[0x20]),
        action_bytes([0x03u8; 32], 0, other_hash, &[0x30]),
        action_bytes([0x04u8; 32], 0, lock_hash, &[0x40]),
    ]);
    let view = MessageView::new(cobuild_core::reader::cursor_from_slice(&message));

    let actions = view
        .actions_for(ScriptRole::InputLock, lock_hash)
        .unwrap();

    assert_eq!(actions.len(), 2);
    assert_eq!(actions[0].index, 0);
    assert_eq!(cursor_bytes(&actions[0].data).unwrap(), vec![0x10]);
    assert_eq!(actions[1].index, 3);
    assert_eq!(cursor_bytes(&actions[1].data).unwrap(), vec![0x40]);
}

#[test]
fn message_view_returns_empty_actions_for_role_mismatch() {
    let message = message_with_actions(&[action_bytes([0x01u8; 32], 2, [0x55u8; 32], &[0x99])]);
    let view = MessageView::new(cobuild_core::reader::cursor_from_slice(&message));

    let actions = view
        .actions_for(ScriptRole::InputLock, [0x55u8; 32])
        .unwrap();

    assert!(actions.is_empty());
}

fn message_with_actions(actions: &[Vec<u8>]) -> Vec<u8> {
    table_bytes(&[dynvec_bytes(actions)])
}

fn action_bytes(
    script_info_hash: [u8; 32],
    script_role: u8,
    script_hash: [u8; 32],
    data: &[u8],
) -> Vec<u8> {
    table_bytes(&[
        script_info_hash.to_vec(),
        vec![script_role],
        script_hash.to_vec(),
        molecule_bytes(data),
    ])
}

fn dynvec_bytes(items: &[Vec<u8>]) -> Vec<u8> {
    if items.is_empty() {
        return 4u32.to_le_bytes().to_vec();
    }
    let header_size = 4 + items.len() * 4;
    let total_size = header_size + items.iter().map(Vec::len).sum::<usize>();
    let mut bytes = Vec::with_capacity(total_size);
    bytes.extend_from_slice(&(total_size as u32).to_le_bytes());
    let mut offset = header_size;
    for item in items {
        bytes.extend_from_slice(&(offset as u32).to_le_bytes());
        offset += item.len();
    }
    for item in items {
        bytes.extend_from_slice(item);
    }
    bytes
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test -p cobuild-core --offline --test view message_view_ -- --nocapture
```

Expected: FAIL with unresolved imports or missing methods for `ScriptRole`, `actions`, and `actions_for`.

- [ ] **Step 3: Implement `ActionView` and query methods**

In `crates/cobuild-core/src/view.rs`, import `ScriptRole` and add `ActionView` near `MessageActionView`:

```rust
use crate::{
    error::CoreError,
    protocol::ScriptRole,
    reader::{cursor_bytes, cursor_from_slice},
};

#[derive(Clone)]
pub struct ActionView {
    pub index: usize,
    pub script_info_hash: [u8; 32],
    pub script_role: ScriptRole,
    pub script_hash: [u8; 32],
    pub data: Cursor,
}
```

Replace the current `MessageActionView` use with `ActionView` and add methods on `MessageView`:

```rust
impl MessageView {
    pub fn new(cursor: Cursor) -> Self {
        Self { cursor }
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    pub fn actions(&self) -> Result<Vec<ActionView>, CoreError> {
        message_actions(&self.cursor)
    }

    pub fn actions_for(
        &self,
        role: ScriptRole,
        script_hash: [u8; 32],
    ) -> Result<Vec<ActionView>, CoreError> {
        let mut out = Vec::new();
        for action in self.actions()? {
            if action.script_role == role && action.script_hash == script_hash {
                out.push(action);
            }
        }
        Ok(out)
    }
}
```

Update `message_actions`:

```rust
pub(crate) fn message_actions(message: &Cursor) -> Result<Vec<ActionView>, CoreError> {
    let message = Message::from(message.clone());
    message
        .verify(false)
        .map_err(|_| CoreError::InvalidOtxLayout)?;
    let actions = message.actions().map_err(|_| CoreError::MalformedCobuild)?;
    let action_count = actions.len().map_err(|_| CoreError::MalformedCobuild)?;
    let mut out = Vec::with_capacity(action_count);
    for index in 0..action_count {
        let action = actions
            .get(index)
            .map_err(|_| CoreError::MalformedCobuild)?;
        let raw_role = action
            .script_role()
            .map_err(|_| CoreError::MalformedCobuild)?;
        let data = action.data().map_err(|_| CoreError::MalformedCobuild)?;
        out.push(ActionView {
            index,
            script_info_hash: action
                .script_info_hash()
                .map_err(|_| CoreError::MalformedCobuild)?,
            script_role: ScriptRole::try_from(raw_role)?,
            script_hash: action
                .script_hash()
                .map_err(|_| CoreError::MalformedCobuild)?,
            data: data.cursor,
        });
    }
    Ok(out)
}
```

Remove the old `MessageActionView` struct once all references are updated.

- [ ] **Step 4: Run tests to verify action query passes**

Run:

```bash
cargo test -p cobuild-core --offline --test view message_view_ -- --nocapture
```

Expected: PASS for the new `message_view_*` tests.

- [ ] **Step 5: Commit**

```bash
git add crates/cobuild-core/src/view.rs crates/cobuild-core/tests/view.rs
git commit -m "feat: expose message action queries"
```

---

### Task 2: Add Unique Action Query Error

**Files:**
- Modify: `crates/cobuild-core/src/error.rs`
- Modify: `crates/cobuild-core/src/view.rs`
- Modify: `crates/cobuild-core/tests/view.rs`

- [ ] **Step 1: Write failing tests for unique action behavior**

Append to `crates/cobuild-core/tests/view.rs`:

```rust
use cobuild_core::error::CoreError;

#[test]
fn unique_action_for_distinguishes_zero_one_and_many_matches() {
    let target_hash = [0x66u8; 32];
    let empty_view = MessageView::new(cobuild_core::reader::cursor_from_slice(
        &message_with_actions(&[]),
    ));
    assert!(empty_view
        .unique_action_for(ScriptRole::InputLock, target_hash)
        .unwrap()
        .is_none());

    let one_view = MessageView::new(cobuild_core::reader::cursor_from_slice(
        &message_with_actions(&[action_bytes([0x01u8; 32], 0, target_hash, &[0x10])]),
    ));
    let one = one_view
        .unique_action_for(ScriptRole::InputLock, target_hash)
        .unwrap()
        .unwrap();
    assert_eq!(one.index, 0);

    let many_view = MessageView::new(cobuild_core::reader::cursor_from_slice(
        &message_with_actions(&[
            action_bytes([0x01u8; 32], 0, target_hash, &[0x10]),
            action_bytes([0x02u8; 32], 0, target_hash, &[0x20]),
        ]),
    ));
    assert_eq!(
        many_view
            .unique_action_for(ScriptRole::InputLock, target_hash)
            .err(),
        Some(CoreError::DuplicateMatchingAction)
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```bash
cargo test -p cobuild-core --offline --test view unique_action_for_distinguishes_zero_one_and_many_matches -- --nocapture
```

Expected: FAIL with missing `DuplicateMatchingAction` or missing `unique_action_for`.

- [ ] **Step 3: Implement error and method**

In `crates/cobuild-core/src/error.rs`, add:

```rust
DuplicateMatchingAction,
```

In `MessageView` implementation in `crates/cobuild-core/src/view.rs`, add:

```rust
pub fn unique_action_for(
    &self,
    role: ScriptRole,
    script_hash: [u8; 32],
) -> Result<Option<ActionView>, CoreError> {
    let mut matches = self.actions_for(role, script_hash)?;
    match matches.len() {
        0 => Ok(None),
        1 => Ok(matches.pop()),
        _ => Err(CoreError::DuplicateMatchingAction),
    }
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```bash
cargo test -p cobuild-core --offline --test view unique_action_for_distinguishes_zero_one_and_many_matches -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cobuild-core/src/error.rs crates/cobuild-core/src/view.rs crates/cobuild-core/tests/view.rs
git commit -m "feat: add unique action query"
```

---

### Task 3: Refactor Plan Message Types for Lock and Type Reuse

**Files:**
- Modify: `crates/cobuild-core/src/plan.rs`
- Modify: `crates/cobuild-core/src/engine.rs`
- Modify: `crates/cobuild-core/tests/plan.rs`

- [ ] **Step 1: Write failing plan shape tests**

Update `crates/cobuild-core/tests/plan.rs` so the lock test constructs `related_messages`, and add a type-related-message relation assertion:

```rust
use cobuild_core::reader::cursor_from_slice;

#[test]
fn lock_validation_plan_carries_required_signatures_and_related_messages() {
    let requirement = SigningRequirement {
        origin: SignatureOrigin::TxLevel,
        carrier_witness_index: 0,
        seal: vec![7u8; 65],
        signing_message_hash: [9u8; 32],
    };
    let message = RelatedMessage {
        origin: MessageOrigin::TxLevel {
            carrier_witness_index: 0,
        },
        message: cursor_from_slice(&[4, 0, 0, 0]).into(),
    };
    let plan = LockValidationPlan {
        lock_script_hash: [1u8; 32],
        required_signatures: vec![requirement.clone()],
        related_messages: vec![message.clone()],
    };

    assert_eq!(plan.lock_script_hash, [1u8; 32]);
    assert_eq!(plan.required_signatures, vec![requirement]);
    assert_eq!(plan.related_messages.len(), 1);
    assert!(matches!(
        plan.related_messages[0].origin,
        MessageOrigin::TxLevel {
            carrier_witness_index: 0
        }
    ));
}
```

Update the existing type test to expect a type-specific entry:

```rust
let message = RelatedMessage {
    origin: MessageOrigin::Otx {
        witness_index: 4,
        otx_index: 2,
        layout: OtxMessageLayout {
            base_inputs: Range { start: 1, count: 2 },
            append_inputs: Range { start: 3, count: 1 },
            base_outputs: Range { start: 0, count: 1 },
            append_outputs: Range { start: 1, count: 0 },
            base_cell_deps: Range { start: 0, count: 0 },
            append_cell_deps: Range { start: 0, count: 0 },
            base_header_deps: Range { start: 0, count: 0 },
            append_header_deps: Range { start: 0, count: 0 },
        },
    },
    message: cursor_from_slice(&[4, 0, 0, 0]).into(),
};
let related = TypeRelatedMessage {
    message,
    otx_relation: Some(OtxTypeRelation {
        input_type_in_base: true,
        input_type_in_append: false,
        output_type_in_base: true,
        output_type_in_base_covered: true,
        output_type_in_append: false,
    }),
};
let plan = TypeValidationPlan {
    type_script_hash: [2u8; 32],
    related_messages: vec![related],
};

assert!(plan.related_messages[0]
    .otx_relation
    .unwrap()
    .input_type_in_base);
```

- [ ] **Step 2: Run plan tests to verify they fail**

Run:

```bash
cargo test -p cobuild-core --offline --test plan -- --nocapture
```

Expected: FAIL because `LockValidationPlan.related_messages` and
`TypeRelatedMessage` do not exist, and `MessageOrigin::Otx` is not yet
role-neutral.

- [ ] **Step 3: Update plan types**

In `crates/cobuild-core/src/plan.rs`, change the plan types to:

```rust
#[derive(Clone)]
pub struct LockValidationPlan {
    pub lock_script_hash: [u8; 32],
    pub required_signatures: Vec<SigningRequirement>,
    pub related_messages: Vec<RelatedMessage>,
}

#[derive(Clone)]
pub struct TypeValidationPlan {
    pub type_script_hash: [u8; 32],
    pub related_messages: Vec<TypeRelatedMessage>,
}

#[derive(Clone)]
pub struct TypeRelatedMessage {
    pub message: RelatedMessage,
    pub otx_relation: Option<OtxTypeRelation>,
}

#[derive(Clone)]
pub struct RelatedMessage {
    pub origin: MessageOrigin,
    pub message: MessageView,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageOrigin {
    TxLevel {
        carrier_witness_index: usize,
    },
    Otx {
        witness_index: usize,
        otx_index: usize,
        layout: OtxMessageLayout,
    },
}
```

Keep `OtxMessageLayout` and `OtxTypeRelation` unchanged.

- [ ] **Step 4: Update engine type plan construction**

In `crates/cobuild-core/src/engine.rs`, import `TypeRelatedMessage` and wrap type messages:

```rust
use crate::plan::{
    LockValidationPlan, MessageOrigin, OtxMessageLayout, RelatedMessage, SignatureOrigin,
    SigningRequirement, TypeRelatedMessage, TypeValidationPlan,
};
```

In `TypePlanBuilder`, change the field:

```rust
related_messages: Vec<TypeRelatedMessage>,
```

When pushing OTX type messages, use:

```rust
self.related_messages.push(TypeRelatedMessage {
    message: RelatedMessage {
        origin: MessageOrigin::Otx {
            witness_index: otx.layout.witness_index,
            otx_index,
            layout: OtxMessageLayout {
                base_inputs: otx.layout.base_inputs,
                append_inputs: otx.layout.append_inputs,
                base_outputs: otx.layout.base_outputs,
                append_outputs: otx.layout.append_outputs,
                base_cell_deps: otx.layout.base_cell_deps,
                append_cell_deps: otx.layout.append_cell_deps,
                base_header_deps: otx.layout.base_header_deps,
                append_header_deps: otx.layout.append_header_deps,
            },
        },
        message: otx.witness.message.clone().into(),
    },
    otx_relation: Some(relation),
});
```

When pushing tx-level type messages, use no OTX relation:

```rust
self.related_messages.push(TypeRelatedMessage {
    message: RelatedMessage {
        origin: MessageOrigin::TxLevel {
            carrier_witness_index,
        },
        message: message.into(),
    },
    otx_relation: None,
});
```

- [ ] **Step 5: Run plan tests to verify they pass**

Run:

```bash
cargo test -p cobuild-core --offline --test plan -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/cobuild-core/src/plan.rs crates/cobuild-core/src/engine.rs crates/cobuild-core/tests/plan.rs
git commit -m "refactor: separate message origins from type relations"
```

---

### Task 4: Populate Lock Related Messages

**Files:**
- Modify: `crates/cobuild-core/src/engine.rs`
- Modify: `crates/cobuild-core/tests/plan.rs`
- Modify: `tests/tests/contract_template_layout.rs`

- [ ] **Step 1: Write failing functional and architecture tests**

Add this assertion to `crates/cobuild-core/tests/plan.rs` in `lock_validation_plan_carries_required_signatures_and_related_messages`:

```rust
assert_eq!(plan.related_messages.len(), 1);
```

Add this test to `tests/tests/contract_template_layout.rs`:

```rust
#[test]
fn cobuild_core_lock_plan_exposes_related_messages() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let core_src = workspace_root.join("crates/cobuild-core/src");
    let plan_rs = fs::read_to_string(core_src.join("plan.rs")).expect("plan.rs");
    let engine_rs = fs::read_to_string(core_src.join("engine.rs")).expect("engine.rs");

    assert!(
        plan_rs.contains("pub related_messages: Vec<RelatedMessage>"),
        "LockValidationPlan should expose related messages for input_lock actions"
    );
    assert!(
        engine_rs.contains("related_messages: Vec<RelatedMessage>"),
        "LockPlanBuilder should collect lock related messages"
    );
    assert!(
        engine_rs.contains("self.related_messages.push(RelatedMessage"),
        "lock planning should push tx-level or OTX related messages"
    );
}
```

Add these unit tests to `crates/cobuild-core/src/engine.rs` under a
`#[cfg(test)] mod tests` block:

```rust
#[test]
fn lock_related_tx_message_preserves_origin_and_message_cursor() {
    let message_bytes = [4u8, 0, 0, 0];
    let message = crate::reader::cursor_from_slice(&message_bytes);
    let related = related_tx_message(2, message.clone());

    assert!(matches!(
        related.origin,
        MessageOrigin::TxLevel {
            carrier_witness_index: 2
        }
    ));
    assert_eq!(
        crate::reader::cursor_bytes(related.message.cursor()).unwrap(),
        message_bytes.to_vec()
    );
}

#[test]
fn lock_related_otx_message_preserves_origin_layout_and_message_cursor() {
    let message_bytes = [4u8, 0, 0, 0];
    let otx = crate::layout::OtxLayoutEntry {
        layout: crate::layout::OtxLayout {
            witness_index: 7,
            base_inputs: crate::layout::Range { start: 1, count: 2 },
            append_inputs: crate::layout::Range { start: 3, count: 1 },
            base_outputs: crate::layout::Range { start: 0, count: 1 },
            append_outputs: crate::layout::Range { start: 1, count: 0 },
            base_cell_deps: crate::layout::Range { start: 0, count: 0 },
            append_cell_deps: crate::layout::Range { start: 0, count: 0 },
            base_header_deps: crate::layout::Range { start: 0, count: 0 },
            append_header_deps: crate::layout::Range { start: 0, count: 0 },
        },
        witness: crate::view::OtxView {
            message: crate::reader::cursor_from_slice(&message_bytes),
            append_permissions: 0,
            base_input_cells: 1,
            base_input_masks: crate::view::MaskView::new(vec![0]),
            base_output_cells: 0,
            base_output_masks: crate::view::MaskView::new(Vec::new()),
            base_cell_deps: 0,
            base_cell_dep_masks: crate::view::MaskView::new(Vec::new()),
            base_header_deps: 0,
            base_header_dep_masks: crate::view::MaskView::new(Vec::new()),
            append_input_cells: 0,
            append_output_cells: 0,
            append_cell_deps: 0,
            append_header_deps: 0,
            seals: Vec::new(),
        },
    };

    let related = related_otx_message(3, &otx);

    match related.origin {
        MessageOrigin::Otx {
            witness_index,
            otx_index,
            layout,
        } => {
            assert_eq!(witness_index, 7);
            assert_eq!(otx_index, 3);
            assert_eq!(layout.base_inputs.start, 1);
            assert_eq!(layout.append_inputs.start, 3);
        }
        MessageOrigin::TxLevel { .. } => panic!("expected OTX message origin"),
    }
    assert_eq!(
        crate::reader::cursor_bytes(related.message.cursor()).unwrap(),
        message_bytes.to_vec()
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```bash
cargo test --workspace --offline cobuild_core_lock_plan_exposes_related_messages -- --nocapture
```

Expected: FAIL until `LockPlanBuilder` owns `related_messages` and the
`related_tx_message` / `related_otx_message` helpers exist.

- [ ] **Step 3: Add lock related message collection**

In `crates/cobuild-core/src/engine.rs`, update `LockPlanBuilder`:

```rust
struct LockPlanBuilder<'a> {
    context: &'a CobuildContext,
    lock_script_hash: [u8; 32],
    required_signatures: Vec<SigningRequirement>,
    related_messages: Vec<RelatedMessage>,
}
```

Initialize it:

```rust
related_messages: Vec::new(),
```

Return it:

```rust
Ok(LockValidationPlan {
    lock_script_hash: self.lock_script_hash,
    required_signatures: self.required_signatures,
    related_messages: self.related_messages,
})
```

In `add_tx_level_requirement`, track the message used for signing:

```rust
let mut related_message = None;
let (seal, signing_message_hash) = match sighash_all_witness_layout {
    SighashAllWitnessView::WithMessage { seal, message } => {
        let message = tx_message.as_ref().unwrap_or(&message);
        self.context
            .script_hashes
            .validate_message_targets(message)?;
        related_message = Some(message.clone());
        let signing_message_hash = tx_with_message_hash(message, &self.context.tx)?;
        (cursor_bytes(&seal)?, signing_message_hash)
    }
    SighashAllWitnessView::SealOnly { seal } => {
        let signing_message_hash = match tx_message {
            Some(message) => {
                self.context
                    .script_hashes
                    .validate_message_targets(&message)?;
                related_message = Some(message.clone());
                tx_with_message_hash(&message, &self.context.tx)?
            }
            None => tx_without_message_hash(&self.context.tx)?,
        };
        (cursor_bytes(&seal)?, signing_message_hash)
    }
};
```

After pushing the signature requirement, add:

```rust
if let Some(message) = related_message {
    self.related_messages
        .push(related_tx_message(carrier_witness_index, message));
}
```

In `add_otx_requirements`, after target validation and before seal lookup, push one OTX related message per relevant OTX:

```rust
self.related_messages
    .push(related_otx_message(otx_index, otx));
```

Use `for (otx_index, otx) in layout.otx_entries.iter().enumerate()` in the loop so `otx_index` is available.

Add these private helpers near the bottom of `engine.rs`:

```rust
fn related_tx_message(carrier_witness_index: usize, message: Cursor) -> RelatedMessage {
    RelatedMessage {
        origin: MessageOrigin::TxLevel {
            carrier_witness_index,
        },
        message: message.into(),
    }
}

fn related_otx_message(otx_index: usize, otx: &crate::layout::OtxLayoutEntry) -> RelatedMessage {
    RelatedMessage {
        origin: MessageOrigin::Otx {
            witness_index: otx.layout.witness_index,
            otx_index,
            layout: OtxMessageLayout {
                base_inputs: otx.layout.base_inputs,
                append_inputs: otx.layout.append_inputs,
                base_outputs: otx.layout.base_outputs,
                append_outputs: otx.layout.append_outputs,
                base_cell_deps: otx.layout.base_cell_deps,
                append_cell_deps: otx.layout.append_cell_deps,
                base_header_deps: otx.layout.base_header_deps,
                append_header_deps: otx.layout.append_header_deps,
            },
        },
        message: otx.witness.message.clone().into(),
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test --workspace --offline cobuild_core_lock_plan_exposes_related_messages -- --nocapture
cargo test -p cobuild-core --offline engine::tests::lock_related_ -- --nocapture
cargo test -p cobuild-core --offline --test plan -- --nocapture
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cobuild-core/src/engine.rs crates/cobuild-core/tests/plan.rs tests/tests/contract_template_layout.rs
git commit -m "feat: expose lock related messages"
```

---

### Task 5: Route Target Validation Through `MessageView`

**Files:**
- Modify: `crates/cobuild-core/src/context.rs`
- Modify: `tests/tests/contract_template_layout.rs`

- [ ] **Step 1: Write failing architecture guard**

Update `tests/tests/contract_template_layout.rs` in the existing view/core guard to assert:

```rust
for expected in [
    "pub struct ActionView",
    "pub fn actions(&self) -> Result<Vec<ActionView>, CoreError>",
    "pub fn actions_for(",
    "pub fn unique_action_for(",
] {
    assert!(
        view_rs.contains(expected),
        "MessageView should expose action query API {expected}"
    );
}
```

Add:

```rust
let context_rs = fs::read_to_string(core_src.join("context.rs")).expect("context.rs");
assert!(
    context_rs.contains("MessageView") && context_rs.contains(".actions()?"),
    "message target validation should reuse MessageView action parsing"
);
assert!(
    !context_rs.contains("message_actions"),
    "context.rs should not parse message actions through the old helper"
);
```

- [ ] **Step 2: Run guard to verify it fails before context routing**

Run:

```bash
cargo test -p tests --offline --test contract_template_layout cobuild_core_view_is_cursor_backed_protocol_boundary -- --nocapture
```

Expected: FAIL until `context.rs` uses `MessageView::actions`.

- [ ] **Step 3: Update context target validation**

In `crates/cobuild-core/src/context.rs`, change imports:

```rust
view::MessageView,
```

Update `validate_message_targets`:

```rust
pub(crate) fn validate_message_targets(&self, message: &Cursor) -> Result<(), CoreError> {
    for action in MessageView::new(message.clone()).actions()? {
        let target_exists = match action.script_role {
            ScriptRole::InputLock => self.input_locks.contains(&action.script_hash),
            ScriptRole::InputType => self
                .input_types
                .iter()
                .flatten()
                .any(|hash| *hash == action.script_hash),
            ScriptRole::OutputType => self
                .output_types
                .iter()
                .flatten()
                .any(|hash| *hash == action.script_hash),
        };
        if !target_exists {
            return Err(CoreError::InvalidMessageTarget);
        }
    }
    Ok(())
}
```

Remove the direct `message_actions` import.

- [ ] **Step 4: Run tests to verify they pass**

Run:

```bash
cargo test -p tests --offline --test contract_template_layout cobuild_core_view_is_cursor_backed_protocol_boundary -- --nocapture
cargo test -p cobuild-core --offline
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/cobuild-core/src/context.rs tests/tests/contract_template_layout.rs
git commit -m "refactor: validate targets through message view"
```

---

### Task 6: Final Verification

**Files:**
- Verify all modified files.

- [ ] **Step 1: Format**

Run:

```bash
cargo fmt --all
```

Expected: command exits 0.

- [ ] **Step 2: Run package tests**

Run:

```bash
cargo test -p cobuild-core --offline
```

Expected: all `cobuild-core` unit, integration, and doc tests pass.

- [ ] **Step 3: Run workspace tests**

Run:

```bash
cargo test --workspace --offline
```

Expected: all workspace tests pass.

- [ ] **Step 4: Run focused architecture scans**

Run:

```bash
rg -n "message_actions\\(" crates/cobuild-core/src
rg -n "pub struct ActionData|ActionData" crates/cobuild-core/src
rg -n "Message::from|\\.actions\\(" contracts/cobuild-otx-lock/src
rg -n "unsafe" crates/cobuild-core/src contracts/cobuild-otx-lock/src
```

Expected:

- `message_actions(` appears only as the helper definition or is removed entirely.
- `ActionData` has no matches.
- `Message::from` and direct `.actions(` parsing have no matches in the lock
  contract.
- `unsafe` has no matches.

- [ ] **Step 5: Commit final formatting if needed**

If `cargo fmt` changed files after the previous commits:

```bash
git add crates/cobuild-core tests
git commit -m "chore: format action query api"
```

If no files changed, do not create an empty commit.
