# Cobuild Core Action Query API Design

## Status

Proposed implementation design for `crates/cobuild-core`.

This design extends the existing Cobuild Core planning API so lock and type
scripts can find the `Action`s addressed to themselves without reimplementing
Message parsing.

## Context

Cobuild Core already parses `Message.actions` internally to validate that each
action target points to a real script position in the full transaction.

The current public planning API is incomplete for action consumers:

- `TypeValidationPlan` exposes `related_messages`, but scripts still have to
  parse each `Message` again to find matching actions.
- `LockValidationPlan` exposes signature requirements, but not the related
  messages that may contain `input_lock` actions for the current lock.
- `message_actions` is currently crate-private and only returns target metadata,
  not action data.

Both lock and type scripts can receive action requests because `Action` targets
are distinguished by `script_role`:

- `input_lock`
- `input_type`
- `output_type`

Core should provide a shared action-query API for all three roles.

## Goals

- Let lock and type scripts query actions addressed to themselves.
- Keep action parsing in Cobuild Core instead of duplicating it in contracts.
- Preserve current planning responsibilities: plans identify relevant messages
  and signature/message scopes; scripts decide how to interpret action data.
- Support both single-action and multi-action script ABIs.
- Keep the API explicit at call sites.

## Non-Goals

- Core will not parse application-specific `Action.data`.
- Core will not require every lock or type script to consume actions.
- Core will not universally reject multiple matching actions.
- Core will not validate off-chain `ScriptInfo` beyond exposing fields needed by
  scripts and wallets.

## API Shape

### `ScriptRole`

The existing `ScriptRole` enum should remain the role type used by the public
query API:

```rust
pub enum ScriptRole {
    InputLock,
    InputType,
    OutputType,
}
```

If it is currently awkward for downstream contracts to import, expose it through
the same public module path used by the plan API.

### `ActionView`

Add an owned/lightweight view for one action:

```rust
#[derive(Clone)]
pub struct ActionView {
    pub index: usize,
    pub script_info_hash: [u8; 32],
    pub script_role: ScriptRole,
    pub script_hash: [u8; 32],
    pub data: Cursor,
}
```

`index` is the action's index inside the containing `Message.actions` vector.
`script_info_hash` is exposed as signed metadata for off-chain tooling and for
scripts that choose to apply additional policy, but Core does not validate the
corresponding off-chain `ScriptInfo`.
`data` remains cursor-backed so Core does not copy application payloads.

### `MessageView` Query Methods

`MessageView` should become the public action-query surface:

```rust
impl MessageView {
    pub fn actions(&self) -> Result<Vec<ActionView>, CoreError>;

    pub fn actions_for(
        &self,
        role: ScriptRole,
        script_hash: [u8; 32],
    ) -> Result<Vec<ActionView>, CoreError>;

    pub fn unique_action_for(
        &self,
        role: ScriptRole,
        script_hash: [u8; 32],
    ) -> Result<Option<ActionView>, CoreError>;
}
```

`actions()` parses all actions and validates `script_role`.

`actions_for()` filters by exact `(role, script_hash)`.

`unique_action_for()` is a convenience API for scripts whose ABI expects at most
one matching action. It returns:

- `Ok(None)` when no matching action exists;
- `Ok(Some(action))` when exactly one matching action exists;
- `Err(CoreError::DuplicateMatchingAction)` when more than one matching action
  exists.

Scripts that support multi-action semantics should use `actions_for()` instead.

## Plan API Changes

### Lock Plans

Extend `LockValidationPlan`:

```rust
pub struct LockValidationPlan {
    pub lock_script_hash: [u8; 32],
    pub required_signatures: Vec<SigningRequirement>,
    pub related_messages: Vec<RelatedMessage>,
}
```

`related_messages` contains messages that may carry `input_lock` actions for
the current lock:

- the tx-level message when tx-level remainder validation is relevant and a
  tx-level message exists;
- each relevant OTX message where the current lock appears in base or append
  input scope.

This field does not mean the lock must interpret `Action.data`. It only exposes
messages that are relevant to this lock under Core flow selection.

### Type Plans

Keep `TypeValidationPlan.related_messages`, but scripts should use
`MessageView::actions_for()` or `unique_action_for()` to extract:

- `InputType` actions for the current type hash;
- `OutputType` actions for the current type hash.

No pre-filtered action vector is added to `RelatedMessage`. Keeping filtering
on `MessageView` avoids duplicating policy in the plan and lets different
scripts choose single-action or multi-action semantics.

## Data Flow

Lock script flow:

1. Build `CobuildContext`.
2. Call `plan_lock_validation(current_lock_hash)`.
3. Verify every `SigningRequirement`.
4. For each `RelatedMessage`, call:

```rust
message.actions_for(ScriptRole::InputLock, current_lock_hash)
```

5. Interpret returned `ActionView.data` according to the lock's own ABI, if the
   lock defines action semantics.

Type script flow:

1. Build `CobuildContext`.
2. Call `plan_type_validation(current_type_hash)`.
3. For each `RelatedMessage`, call:

```rust
message.actions_for(ScriptRole::InputType, current_type_hash)
message.actions_for(ScriptRole::OutputType, current_type_hash)
```

4. Interpret returned `ActionView.data` according to the type script's own ABI
   and validate the relevant cells from `MessageOrigin`.

## Error Handling

Add:

```rust
DuplicateMatchingAction
```

to `CoreError`.

Parsing failures in `MessageView::actions()` should use existing malformed
message errors consistently with current `message_actions` behavior.

Invalid `script_role` values should remain Core errors because Core v1 defines
the valid role set.

`actions_for()` should not fail because no matching action exists. Absence of a
matching action is a script policy decision.

## Testing

Add focused tests for:

- `MessageView::actions()` returns action index, `script_info_hash`, role,
  hash, and data cursor.
- `actions_for()` returns only exact role/hash matches.
- `actions_for()` returns an empty vector when no action matches.
- `unique_action_for()` returns `None`, one action, or
  `DuplicateMatchingAction` for zero, one, or many matches.
- `plan_lock_validation()` exposes related messages for tx-level and OTX lock
  relevance.
- Existing type planning behavior still exposes related messages without
  requiring actions to exist.

Architecture guards should assert that action parsing remains in
`cobuild-core::view` / `MessageView`, not duplicated in lock contracts.

## Compatibility

This is an additive API change except for adding a field to
`LockValidationPlan`. Existing constructors in tests must be updated to include
`related_messages`.

No molecule schema changes are required.

No signature hash behavior changes are required.
