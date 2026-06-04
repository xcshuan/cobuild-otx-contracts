# Cobuild Core Engine Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refactor `cobuild-core` into a protocol engine that emits lock and type validation plans while preserving Cobuild Core hash bytes and lock behavior.

**Architecture:** Add `plan.rs`, `engine.rs`, and `flow.rs` around a prepared transaction model. Move the chain-facing path to source-backed cursors and compact prepared metadata, then replace old context/query APIs with `LockValidationPlan` and `TypeValidationPlan`.

**Tech Stack:** Rust `no_std`, `alloc`, `cobuild_types::lazy_reader`, `ckb-std` in the lock crate only, `blake2b-ref`, ckb-testtool integration tests.

---

## File Structure

- Create `crates/cobuild-core/src/plan.rs`: public validation plan types: `LockValidationPlan`, `SigningRequirement`, `SignatureOrigin`, `TypeValidationPlan`, `RelatedMessage`, `MessageOrigin`, `OtxMessageLayout`, and `OtxTypeRelation`.
- Create `crates/cobuild-core/src/engine.rs`: public engine/prepared API and orchestration for lock/type planning.
- Create `crates/cobuild-core/src/flow.rs`: local flow selection helpers: lock group leading witness, OTX relevance, type relation, lock group coverage.
- Modify `crates/cobuild-core/src/source.rs`: replace scattered count methods with `TxCounts` and `HashInputSource`; keep `ClassifiedCursor` and test source.
- Modify `crates/cobuild-core/src/prepare.rs`: prepare compact metadata from a source; remove owned main-path transaction structures later in the plan.
- Modify `crates/cobuild-core/src/layout.rs`: change primary layout scanner from owned witness bytes to source/cursor-backed witness access.
- Modify `crates/cobuild-core/src/view.rs`: add `MessageView` and `ActionView`, keep views cursor-backed.
- Modify `crates/cobuild-core/src/hash.rs`: consume `HashInputSource`, then split repeated preimage helpers.
- Modify `crates/cobuild-core/src/lib.rs`: export new modules and remove old modules after migration.
- Modify `contracts/cobuild-otx-lock/src/entry.rs`: consume `LockValidationPlan`.
- Modify `contracts/cobuild-otx-lock/src/chain.rs`: implement `HashInputSource` and transaction/count cache.
- Modify tests under `crates/cobuild-core/tests`: replace query tests with engine plan tests, add type-plan and source behavior coverage.
- Modify `contracts/cobuild-otx-lock/tests` and `tests/tests/cobuild_otx_lock.rs` only when API migration requires it.

## Invariants

- Do not edit Molecule schemas.
- Do not change BLAKE2b personalization constants.
- Do not change hash preimage order or framing.
- Do not add legacy `WitnessArgs` fallback.
- Do not add `ckb-auth`.
- Do not add `ckb_std` to `crates/cobuild-core/src`.
- Do not add `cobuild_types::entity` to core or lock production paths.
- Do not introduce `unsafe`.

## Task 1: Add Plan Types

**Files:**
- Create: `crates/cobuild-core/src/plan.rs`
- Modify: `crates/cobuild-core/src/lib.rs`
- Test: `crates/cobuild-core/tests/plan.rs`

- [ ] **Step 1: Add the failing plan type smoke test**

Create `crates/cobuild-core/tests/plan.rs`:

```rust
use cobuild_core::{
    layout::Range,
    plan::{
        LockValidationPlan, MessageOrigin, OtxMessageLayout, OtxTypeRelation, SignatureOrigin,
        SigningRequirement, TypeValidationPlan,
    },
};

#[test]
fn lock_validation_plan_carries_required_signatures() {
    let requirement = SigningRequirement {
        origin: SignatureOrigin::TxLevel,
        carrier_witness_index: 0,
        seal: vec![7u8; 65],
        signing_message_hash: [9u8; 32],
    };
    let plan = LockValidationPlan {
        lock_script_hash: [1u8; 32],
        required_signatures: vec![requirement.clone()],
    };

    assert_eq!(plan.lock_script_hash, [1u8; 32]);
    assert_eq!(plan.required_signatures, vec![requirement]);
}

#[test]
fn type_validation_plan_origin_carries_otx_layout_and_relation() {
    let origin = MessageOrigin::Otx {
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
        relation: OtxTypeRelation {
            input_type_in_base: true,
            input_type_in_append: false,
            output_type_in_base: true,
            output_type_in_append: false,
        },
    };
    let plan = TypeValidationPlan {
        type_script_hash: [2u8; 32],
        related_messages: Vec::new(),
    };

    assert_eq!(plan.type_script_hash, [2u8; 32]);
    match origin {
        MessageOrigin::Otx {
            witness_index,
            otx_index,
            relation,
            ..
        } => {
            assert_eq!(witness_index, 4);
            assert_eq!(otx_index, 2);
            assert!(relation.input_type_in_base);
            assert!(relation.output_type_in_base);
        }
        MessageOrigin::TxLevel { .. } => panic!("expected otx origin"),
    }
}
```

- [ ] **Step 2: Run the failing test**

Run:

```bash
cargo test -p cobuild-core --offline --test plan
```

Expected: FAIL with unresolved import `cobuild_core::plan`.

- [ ] **Step 3: Add the plan module**

Create `crates/cobuild-core/src/plan.rs`:

```rust
use alloc::vec::Vec;

use crate::{layout::Range, view::MessageView};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LockValidationPlan {
    pub lock_script_hash: [u8; 32],
    pub required_signatures: Vec<SigningRequirement>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SigningRequirement {
    pub origin: SignatureOrigin,
    pub carrier_witness_index: usize,
    pub seal: Vec<u8>,
    pub signing_message_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SignatureOrigin {
    TxLevel,
    OtxBase,
    OtxAppend,
}

#[derive(Clone)]
pub struct TypeValidationPlan {
    pub type_script_hash: [u8; 32],
    pub related_messages: Vec<RelatedMessage>,
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
        relation: OtxTypeRelation,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OtxMessageLayout {
    pub base_inputs: Range,
    pub append_inputs: Range,
    pub base_outputs: Range,
    pub append_outputs: Range,
    pub base_cell_deps: Range,
    pub append_cell_deps: Range,
    pub base_header_deps: Range,
    pub append_header_deps: Range,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OtxTypeRelation {
    pub input_type_in_base: bool,
    pub input_type_in_append: bool,
    pub output_type_in_base: bool,
    pub output_type_in_append: bool,
}
```

Modify `crates/cobuild-core/src/lib.rs`:

```rust
pub mod plan;
```

- [ ] **Step 4: Add cursor-backed `MessageView` shell**

Modify `crates/cobuild-core/src/view.rs` by adding the type near the other view structs:

```rust
#[derive(Clone)]
pub struct MessageView {
    cursor: Cursor,
}

impl MessageView {
    pub fn new(cursor: Cursor) -> Self {
        Self { cursor }
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }
}
```

- [ ] **Step 5: Run the plan test**

Run:

```bash
cargo test -p cobuild-core --offline --test plan
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/cobuild-core/src/lib.rs crates/cobuild-core/src/plan.rs crates/cobuild-core/src/view.rs crates/cobuild-core/tests/plan.rs
git commit -m "feat: add cobuild validation plan types"
```

## Task 2: Introduce Source Counts And HashInputSource

**Files:**
- Modify: `crates/cobuild-core/src/source.rs`
- Modify: `crates/cobuild-core/src/hash.rs`
- Test: `crates/cobuild-core/tests/source.rs`
- Test: `crates/cobuild-core/tests/hash.rs`

- [ ] **Step 1: Add failing source count tests**

Append to `crates/cobuild-core/tests/source.rs`:

```rust
use cobuild_core::source::{HashInputSource, TxCounts};

#[test]
fn in_memory_source_exposes_counts_as_one_value() {
    let source = InMemorySource {
        raw_inputs: vec![Vec::new(), Vec::new()],
        raw_outputs: vec![Vec::new()],
        raw_cell_deps: vec![Vec::new(), Vec::new(), Vec::new()],
        raw_header_deps: vec![[0u8; 32]],
        witnesses: vec![Vec::new(), Vec::new(), Vec::new(), Vec::new()],
        ..InMemorySource::default()
    };

    assert_eq!(
        source.counts().unwrap(),
        TxCounts {
            inputs: 2,
            outputs: 1,
            cell_deps: 3,
            header_deps: 1,
            witnesses: 4,
        }
    );
}
```

- [ ] **Step 2: Run the failing source test**

Run:

```bash
cargo test -p cobuild-core --offline --test source in_memory_source_exposes_counts_as_one_value
```

Expected: FAIL with unresolved imports `HashInputSource` or `TxCounts`.

- [ ] **Step 3: Add `TxCounts` and `HashInputSource`**

Modify `crates/cobuild-core/src/source.rs`:

```rust
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TxCounts {
    pub inputs: usize,
    pub outputs: usize,
    pub cell_deps: usize,
    pub header_deps: usize,
    pub witnesses: usize,
}

pub trait HashInputSource: TransactionSource {
    fn counts(&self) -> Result<TxCounts, CoreError>;
    fn witness_cursor(&self, absolute_index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_input_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_output_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_cell_dep_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
    fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError>;
}
```

Move these methods out of `SigningDataSource` into `HashInputSource`, then make
`SigningDataSource` a compatibility alias while migration is in progress:

```rust
pub trait SigningDataSource: HashInputSource {}

impl<T: HashInputSource + ?Sized> SigningDataSource for T {}
```

Implement `HashInputSource for InMemorySource`:

```rust
impl HashInputSource for InMemorySource {
    fn counts(&self) -> Result<TxCounts, CoreError> {
        Ok(TxCounts {
            inputs: self.raw_inputs.len(),
            outputs: self.raw_outputs.len(),
            cell_deps: self.raw_cell_deps.len(),
            header_deps: self.raw_header_deps.len(),
            witnesses: self.witnesses.len(),
        })
    }

    fn witness_cursor(&self, absolute_index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_input_cursor(&self.witnesses, absolute_index)
    }

    fn raw_input_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_input_cursor(&self.raw_inputs, index)
    }

    fn raw_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_input_cursor(&self.raw_outputs, index)
    }

    fn raw_output_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_input_cursor(&self.raw_outputs_data, index)
    }

    fn raw_cell_dep_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        hash_input_cursor(&self.raw_cell_deps, index)
    }

    fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        self.raw_header_deps
            .get(index)
            .copied()
            .ok_or(CoreError::MissingHashInput)
    }
}
```

- [ ] **Step 4: Update hash functions to use `HashInputSource` counts**

Modify `crates/cobuild-core/src/hash.rs` imports:

```rust
use crate::source::HashInputSource;
```

Change generic bounds from `S: SigningDataSource` to `S: HashInputSource`.

Inside `tx_signing_hash`, replace count calls:

```rust
let counts = source.counts()?;
for index in 0..counts.inputs {
    let output = source.resolved_input_output_cursor(index)?;
    update_cursor_with_error(&mut hasher, &output.cursor, output.read_error())?;
    let data = source.resolved_input_data_cursor(index)?;
    update_len_prefixed_cursor(&mut hasher, &data.cursor, data.read_error())?;
}
for index in counts.inputs..counts.witnesses {
    let witness = source.witness_cursor(index)?;
    update_len_prefixed_cursor(&mut hasher, &witness.cursor, witness.read_error())?;
}
```

- [ ] **Step 5: Run source and hash tests**

Run:

```bash
cargo test -p cobuild-core --offline --test source
cargo test -p cobuild-core --offline --test hash
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/cobuild-core/src/source.rs crates/cobuild-core/src/hash.rs crates/cobuild-core/tests/source.rs crates/cobuild-core/tests/hash.rs
git commit -m "refactor: add hash input source counts"
```

## Task 3: Add Prepared Engine Skeleton

**Files:**
- Create: `crates/cobuild-core/src/engine.rs`
- Create: `crates/cobuild-core/src/flow.rs`
- Modify: `crates/cobuild-core/src/lib.rs`
- Test: `crates/cobuild-core/tests/engine.rs`

- [ ] **Step 1: Add failing engine tests for empty plans**

Create `crates/cobuild-core/tests/engine.rs`:

```rust
use cobuild_core::{
    engine::CobuildEngine,
    source::{InMemorySource, TxCounts},
};

#[test]
fn engine_returns_empty_lock_plan_when_lock_is_absent() {
    let source = InMemorySource {
        input_locks: vec![[1u8; 32]],
        input_types: vec![None],
        output_types: Vec::new(),
        raw_inputs: vec![Vec::new()],
        witnesses: vec![Vec::new()],
        ..InMemorySource::default()
    };

    let prepared = CobuildEngine::prepare(&source).unwrap();
    let plan = prepared.plan_lock_validation([2u8; 32], &source).unwrap();

    assert_eq!(plan.lock_script_hash, [2u8; 32]);
    assert!(plan.required_signatures.is_empty());
}

#[test]
fn engine_preparation_uses_source_counts() {
    let source = InMemorySource {
        input_locks: vec![[1u8; 32]],
        input_types: vec![None],
        output_types: Vec::new(),
        raw_inputs: vec![Vec::new()],
        witnesses: vec![Vec::new(), Vec::new()],
        ..InMemorySource::default()
    };

    let prepared = CobuildEngine::prepare(&source).unwrap();

    assert_eq!(
        prepared.counts(),
        TxCounts {
            inputs: 1,
            outputs: 0,
            cell_deps: 0,
            header_deps: 0,
            witnesses: 2,
        }
    );
}
```

- [ ] **Step 2: Run the failing engine test**

Run:

```bash
cargo test -p cobuild-core --offline --test engine
```

Expected: FAIL with unresolved import `cobuild_core::engine`.

- [ ] **Step 3: Add `flow.rs` helper for lock-group first input**

Create `crates/cobuild-core/src/flow.rs`:

```rust
use crate::context::ScriptHashIndex;

pub(crate) fn first_input_with_lock(
    script_hashes: &ScriptHashIndex,
    lock_hash: [u8; 32],
) -> Option<usize> {
    script_hashes
        .input_locks
        .iter()
        .position(|hash| *hash == lock_hash)
}
```

- [ ] **Step 4: Add engine skeleton**

Create `crates/cobuild-core/src/engine.rs`:

```rust
use alloc::vec::Vec;

use crate::{
    context::ScriptHashIndex,
    error::CoreError,
    flow::first_input_with_lock,
    layout::{scan_layout, LayoutTx, OtxLayoutScan},
    plan::{LockValidationPlan, TypeValidationPlan},
    source::{HashInputSource, TxCounts},
};

pub struct CobuildEngine;

pub struct PreparedCobuild {
    counts: TxCounts,
    script_hashes: ScriptHashIndex,
    layout_scan: OtxLayoutScan,
}

impl CobuildEngine {
    pub fn prepare<S: HashInputSource>(source: &S) -> Result<PreparedCobuild, CoreError> {
        let counts = source.counts()?;
        let mut input_locks = Vec::with_capacity(counts.inputs);
        let mut input_types = Vec::with_capacity(counts.inputs);
        for index in 0..counts.inputs {
            input_locks.push(source.input_lock_hash(index)?);
            input_types.push(source.input_type_hash(index)?);
        }

        let mut output_types = Vec::with_capacity(counts.outputs);
        for index in 0..counts.outputs {
            output_types.push(source.output_type_hash(index)?);
        }

        let script_hashes = ScriptHashIndex {
            input_locks,
            input_types,
            output_types,
        };

        let mut witnesses = Vec::with_capacity(counts.witnesses);
        for index in 0..counts.witnesses {
            let witness = source.witness_cursor(index)?;
            witnesses.push(crate::reader::cursor_bytes_with_error(
                &witness.cursor,
                witness.read_error(),
            )?);
        }

        let layout_scan = scan_layout(&LayoutTx {
            witnesses,
            input_count: counts.inputs,
            output_count: counts.outputs,
            cell_dep_count: counts.cell_deps,
            header_dep_count: counts.header_deps,
        });

        Ok(PreparedCobuild {
            counts,
            script_hashes,
            layout_scan,
        })
    }
}

impl PreparedCobuild {
    pub fn counts(&self) -> TxCounts {
        self.counts
    }

    pub fn plan_lock_validation<S: HashInputSource>(
        &self,
        lock_script_hash: [u8; 32],
        _source: &S,
    ) -> Result<LockValidationPlan, CoreError> {
        let _first_input = first_input_with_lock(&self.script_hashes, lock_script_hash);
        Ok(LockValidationPlan {
            lock_script_hash,
            required_signatures: Vec::new(),
        })
    }

    pub fn plan_type_validation<S: HashInputSource>(
        &self,
        type_script_hash: [u8; 32],
        _source: &S,
    ) -> Result<TypeValidationPlan, CoreError> {
        Ok(TypeValidationPlan {
            type_script_hash,
            related_messages: Vec::new(),
        })
    }
}
```

This skeleton still clones witnesses through the old layout path. That is
removed in Task 7 after the engine API has tests.

- [ ] **Step 5: Export modules**

Modify `crates/cobuild-core/src/lib.rs`:

```rust
pub mod engine;
mod flow;
```

- [ ] **Step 6: Run the engine test**

Run:

```bash
cargo test -p cobuild-core --offline --test engine
```

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/cobuild-core/src/engine.rs crates/cobuild-core/src/flow.rs crates/cobuild-core/src/lib.rs crates/cobuild-core/tests/engine.rs
git commit -m "feat: add cobuild engine skeleton"
```

## Task 4: Port Tx-Level Lock Planning

**Files:**
- Modify: `crates/cobuild-core/src/engine.rs`
- Modify: `crates/cobuild-core/src/flow.rs`
- Modify: `crates/cobuild-core/src/view.rs`
- Test: `crates/cobuild-core/tests/engine.rs`

- [ ] **Step 1: Add tx-level lock plan tests**

Append to `crates/cobuild-core/tests/engine.rs`:

```rust
use cobuild_core::plan::SignatureOrigin;

#[test]
fn engine_lock_plan_uses_group_leading_sighash_all_only_witness() {
    let source = InMemorySource {
        input_locks: vec![[1u8; 32], [2u8; 32]],
        input_types: vec![None, None],
        output_types: Vec::new(),
        raw_inputs: vec![Vec::new(), Vec::new()],
        witnesses: vec![Vec::new(), sighash_all_only_witness(&[7u8; 65])],
        ..InMemorySource::default()
    };
    let prepared = CobuildEngine::prepare(&source).unwrap();

    assert!(prepared
        .plan_lock_validation([1u8; 32], &source)
        .unwrap()
        .required_signatures
        .is_empty());

    let plan = prepared.plan_lock_validation([2u8; 32], &source).unwrap();
    assert_eq!(plan.required_signatures.len(), 1);
    assert_eq!(plan.required_signatures[0].origin, SignatureOrigin::TxLevel);
    assert_eq!(plan.required_signatures[0].carrier_witness_index, 1);
    assert_eq!(plan.required_signatures[0].seal, vec![7u8; 65]);
}

#[test]
fn engine_lock_plan_rejects_duplicate_sighash_all_when_tx_level_relevant() {
    let message = empty_message();
    let source = InMemorySource {
        input_locks: vec![[1u8; 32], [2u8; 32]],
        input_types: vec![None, None],
        output_types: Vec::new(),
        raw_inputs: vec![Vec::new(), Vec::new()],
        witnesses: vec![
            sighash_all_witness(&[7u8; 65], &message),
            sighash_all_witness(&[8u8; 65], &message),
        ],
        ..InMemorySource::default()
    };
    let prepared = CobuildEngine::prepare(&source).unwrap();

    assert_eq!(
        prepared.plan_lock_validation([1u8; 32], &source),
        Err(cobuild_core::error::CoreError::DuplicateSighashAll)
    );
}
```

Copy these helper functions from `crates/cobuild-core/tests/signature_requests.rs`
into `crates/cobuild-core/tests/engine.rs`:

```rust
fn sighash_all_only_witness(seal: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0xFC, 0xFF, 0xFF, 0xFF];
    bytes.extend_from_slice(&sighash_all_only_table(seal));
    bytes
}

fn sighash_all_witness(seal: &[u8], message: &[u8]) -> Vec<u8> {
    let mut bytes = vec![0xFB, 0xFF, 0xFF, 0xFF];
    let seal_bytes = molecule_bytes(seal);
    let table_size = 12 + seal_bytes.len() as u32 + message.len() as u32;
    let mut item = Vec::new();
    item.extend_from_slice(&table_size.to_le_bytes());
    item.extend_from_slice(&12u32.to_le_bytes());
    item.extend_from_slice(&(12 + seal_bytes.len() as u32).to_le_bytes());
    item.extend_from_slice(&seal_bytes);
    item.extend_from_slice(message);
    bytes.extend_from_slice(&item);
    bytes
}

fn sighash_all_only_table(seal: &[u8]) -> Vec<u8> {
    let bytes_size = 4 + seal.len() as u32;
    let total_size = 8 + bytes_size;
    let mut item = Vec::new();
    item.extend_from_slice(&(total_size as u32).to_le_bytes());
    item.extend_from_slice(&8u32.to_le_bytes());
    item.extend_from_slice(&molecule_bytes(seal));
    item
}

fn empty_message() -> Vec<u8> {
    dynvec(&[])
}

fn molecule_bytes(raw: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(raw.len() as u32).to_le_bytes());
    bytes.extend_from_slice(raw);
    bytes
}

fn dynvec(items: &[Vec<u8>]) -> Vec<u8> {
    let header_size = 4 + items.len() * 4;
    let total_size = header_size + items.iter().map(Vec::len).sum::<usize>();
    let mut bytes = Vec::new();
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

- [ ] **Step 2: Run the failing tx-level tests**

Run:

```bash
cargo test -p cobuild-core --offline --test engine engine_lock_plan_uses_group_leading_sighash_all_only_witness
cargo test -p cobuild-core --offline --test engine engine_lock_plan_rejects_duplicate_sighash_all_when_tx_level_relevant
```

Expected: first test FAIL because the plan is empty; second test FAIL because duplicate `SighashAll` is not checked.

- [ ] **Step 3: Add unique SighashAll helper**

Add to `crates/cobuild-core/src/flow.rs`:

```rust
use cobuild_types::lazy_reader::support::Cursor;

use crate::{error::CoreError, view::WitnessLayoutView};

pub(crate) fn unique_sighash_all_message_from_witnesses(
    witnesses: &[Vec<u8>],
) -> Result<Option<Cursor>, CoreError> {
    let mut message = None;
    for witness in witnesses {
        if witness.is_empty() {
            continue;
        }
        let Ok(view) = WitnessLayoutView::from_slice(witness) else {
            continue;
        };
        if let Some(candidate) = view.sighash_all_message()? {
            if message.is_some() {
                return Err(CoreError::DuplicateSighashAll);
            }
            message = Some(candidate);
        }
    }
    Ok(message)
}
```

- [ ] **Step 4: Store prepared witness bytes temporarily**

Modify `PreparedCobuild` in `crates/cobuild-core/src/engine.rs`:

```rust
pub struct PreparedCobuild {
    counts: TxCounts,
    script_hashes: ScriptHashIndex,
    witnesses: Vec<Vec<u8>>,
    layout_scan: OtxLayoutScan,
}
```

In `prepare`, assign the field:

```rust
Ok(PreparedCobuild {
    counts,
    script_hashes,
    witnesses,
    layout_scan,
})
```

- [ ] **Step 5: Implement tx-level lock planning**

Replace `plan_lock_validation` in `crates/cobuild-core/src/engine.rs` with:

```rust
pub fn plan_lock_validation<S: HashInputSource>(
    &self,
    lock_script_hash: [u8; 32],
    source: &S,
) -> Result<LockValidationPlan, CoreError> {
    let mut required_signatures = Vec::new();

    if let Some(carrier_witness_index) =
        first_input_with_lock(&self.script_hashes, lock_script_hash)
    {
        if let Some(witness) = self.witnesses.get(carrier_witness_index) {
            if !witness.is_empty() {
                let view = crate::view::WitnessLayoutView::from_slice(witness)?;
                if let Some(layout) = view.sighash_all_witness_layout()? {
                    let tx_message =
                        crate::flow::unique_sighash_all_message_from_witnesses(&self.witnesses)?;
                    let (seal, signing_message_hash) = match layout {
                        crate::view::SighashAllWitnessView::WithMessage { seal, message } => {
                            let message = tx_message.as_ref().unwrap_or(&message);
                            crate::message::validate_message_targets(
                                message,
                                &self.script_hashes,
                            )?;
                            (
                                crate::reader::cursor_bytes(&seal)?,
                                crate::hash::tx_with_message_hash(message, source)?,
                            )
                        }
                        crate::view::SighashAllWitnessView::SealOnly { seal } => {
                            let signing_message_hash = match tx_message {
                                Some(message) => {
                                    crate::message::validate_message_targets(
                                        &message,
                                        &self.script_hashes,
                                    )?;
                                    crate::hash::tx_with_message_hash(&message, source)?
                                }
                                None => crate::hash::tx_without_message_hash(source)?,
                            };
                            (crate::reader::cursor_bytes(&seal)?, signing_message_hash)
                        }
                    };
                    required_signatures.push(crate::plan::SigningRequirement {
                        origin: crate::plan::SignatureOrigin::TxLevel,
                        carrier_witness_index,
                        seal,
                        signing_message_hash,
                    });
                }
            }
        }
    }

    Ok(LockValidationPlan {
        lock_script_hash,
        required_signatures,
    })
}
```

If `crate::message::validate_message_targets` does not exist publicly, extract
the current `LockScriptQuery::validate_message_targets` logic into
`crates/cobuild-core/src/message.rs` as:

```rust
pub(crate) fn validate_message_targets(
    message: &cobuild_types::lazy_reader::support::Cursor,
    script_hashes: &crate::context::ScriptHashIndex,
) -> Result<(), crate::error::CoreError> {
    for action in crate::view::message_actions(message)? {
        match action.script_role {
            0 => {
                if !script_hashes
                    .input_locks
                    .iter()
                    .any(|hash| *hash == action.script_hash)
                {
                    return Err(crate::error::CoreError::InvalidMessageTarget);
                }
            }
            1 => {
                if !script_hashes
                    .input_types
                    .iter()
                    .any(|hash| *hash == Some(action.script_hash))
                {
                    return Err(crate::error::CoreError::InvalidMessageTarget);
                }
            }
            2 => {
                if !script_hashes
                    .output_types
                    .iter()
                    .any(|hash| *hash == Some(action.script_hash))
                {
                    return Err(crate::error::CoreError::InvalidMessageTarget);
                }
            }
            _ => return Err(crate::error::CoreError::InvalidMessageTarget),
        }
    }
    Ok(())
}
```

- [ ] **Step 6: Run tx-level engine tests**

Run:

```bash
cargo test -p cobuild-core --offline --test engine
```

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/cobuild-core/src/engine.rs crates/cobuild-core/src/flow.rs crates/cobuild-core/src/message.rs crates/cobuild-core/tests/engine.rs
git commit -m "feat: plan tx-level cobuild lock validation"
```

## Task 5: Port OTX Lock Planning

**Files:**
- Modify: `crates/cobuild-core/src/engine.rs`
- Modify: `crates/cobuild-core/src/flow.rs`
- Modify: `crates/cobuild-core/src/seal.rs`
- Test: `crates/cobuild-core/tests/engine.rs`

- [ ] **Step 1: Add OTX lock plan tests**

Append to `crates/cobuild-core/tests/engine.rs`:

```rust
#[test]
fn engine_lock_plan_marks_otx_base_requirement() {
    let target_lock = [1u8; 32];
    let source = otx_source(
        vec![target_lock],
        vec![otx_start_witness(), otx_witness(&empty_message(), &[seal_pair(target_lock, 0, &[7u8; 65])])],
    );
    let prepared = CobuildEngine::prepare(&source).unwrap();

    let plan = prepared.plan_lock_validation(target_lock, &source).unwrap();

    assert_eq!(plan.required_signatures.len(), 1);
    assert_eq!(plan.required_signatures[0].origin, SignatureOrigin::OtxBase);
    assert_eq!(plan.required_signatures[0].carrier_witness_index, 1);
}

#[test]
fn engine_lock_plan_marks_otx_append_requirement() {
    let target_lock = [1u8; 32];
    let base_lock = [2u8; 32];
    let source = otx_source(
        vec![base_lock, target_lock],
        vec![
            otx_start_witness(),
            otx_append_witness(&[seal_pair(target_lock, 1, &[7u8; 65])]),
        ],
    );
    let prepared = CobuildEngine::prepare(&source).unwrap();

    let plan = prepared.plan_lock_validation(target_lock, &source).unwrap();

    assert_eq!(plan.required_signatures.len(), 1);
    assert_eq!(plan.required_signatures[0].origin, SignatureOrigin::OtxAppend);
    assert_eq!(plan.required_signatures[0].carrier_witness_index, 1);
}

#[test]
fn engine_lock_plan_rejects_missing_otx_seal_for_relevant_scope() {
    let target_lock = [1u8; 32];
    let source = otx_source(
        vec![target_lock],
        vec![otx_start_witness(), otx_witness(&empty_message(), &[])],
    );
    let prepared = CobuildEngine::prepare(&source).unwrap();

    assert_eq!(
        prepared.plan_lock_validation(target_lock, &source),
        Err(cobuild_core::error::CoreError::MissingSealPair)
    );
}
```

Copy the OTX helper functions from `crates/cobuild-core/tests/signature_requests.rs`
into `crates/cobuild-core/tests/engine.rs`: `otx_source`, `otx_start_witness`,
`otx_witness`, `otx_append_witness`, `otx_witness_custom`, `seal_pair`, `table`,
and reuse `dynvec`/`molecule_bytes`.

Use this `otx_source` helper:

```rust
fn otx_source(input_locks: Vec<[u8; 32]>, witnesses: Vec<Vec<u8>>) -> InMemorySource {
    let input_count = input_locks.len();
    InMemorySource {
        input_locks,
        input_types: vec![None; input_count],
        output_types: Vec::new(),
        raw_inputs: vec![Vec::new(); input_count],
        resolved_outputs: vec![Vec::new(); input_count],
        resolved_data: vec![Vec::new(); input_count],
        witnesses,
        ..InMemorySource::default()
    }
}
```

- [ ] **Step 2: Run failing OTX tests**

Run:

```bash
cargo test -p cobuild-core --offline --test engine engine_lock_plan_marks_otx_base_requirement
cargo test -p cobuild-core --offline --test engine engine_lock_plan_marks_otx_append_requirement
cargo test -p cobuild-core --offline --test engine engine_lock_plan_rejects_missing_otx_seal_for_relevant_scope
```

Expected: FAIL because OTX requirements are not emitted.

- [ ] **Step 3: Move seal lookup helpers to standalone functions**

In `crates/cobuild-core/src/seal.rs`, add:

```rust
use alloc::vec::Vec;

use crate::{
    error::CoreError,
    protocol::SealScope,
    reader::cursor_bytes,
    view::SealPairView,
};

pub(crate) fn unique_otx_seal_by_scope(
    script_hash: [u8; 32],
    seals: &[SealPairView],
    scope: SealScope,
) -> Result<Vec<u8>, CoreError> {
    let mut found = None;
    for seal in seals {
        let seal_scope = SealScope::try_from(seal.scope)?;
        if seal.script_hash == script_hash && seal_scope == scope {
            if found.is_some() {
                return Err(CoreError::DuplicateSealPair);
            }
            found = Some(cursor_bytes(&seal.seal)?);
        }
    }
    found.ok_or(CoreError::MissingSealPair)
}
```

Remove duplicate method logic from `impl LockScriptQuery` after all callers
move to the standalone helper.

- [ ] **Step 4: Add OTX relevance helper**

Add to `crates/cobuild-core/src/flow.rs`:

```rust
use crate::layout::Range;

pub(crate) fn script_in_input_range(
    input_locks: &[[u8; 32]],
    range: Range,
    script_hash: [u8; 32],
) -> bool {
    input_locks
        .iter()
        .skip(range.start)
        .take(range.count)
        .any(|hash| *hash == script_hash)
}

pub(crate) fn range_contains(range: Range, index: usize) -> bool {
    index >= range.start && index < range.start.saturating_add(range.count)
}
```

- [ ] **Step 5: Generate OTX signing requirements in the engine**

In `crates/cobuild-core/src/engine.rs`, after tx-level planning and before
returning `LockValidationPlan`, add:

```rust
match &self.layout_scan {
    OtxLayoutScan::None => {}
    OtxLayoutScan::Invalid { anchor, error } => {
        let relevant = anchor
            .as_ref()
            .map(|anchor| {
                self.script_hashes
                    .input_locks
                    .iter()
                    .skip(anchor.start_input_cell)
                    .any(|hash| *hash == lock_script_hash)
            })
            .unwrap_or(false);
        if relevant {
            return Err(error.clone());
        }
    }
    OtxLayoutScan::Complete(layout) => {
        for otx in &layout.otx_data {
            let base_relevant = crate::flow::script_in_input_range(
                &self.script_hashes.input_locks,
                otx.layout.base_inputs,
                lock_script_hash,
            );
            let append_relevant = crate::flow::script_in_input_range(
                &self.script_hashes.input_locks,
                otx.layout.append_inputs,
                lock_script_hash,
            );
            if !base_relevant && !append_relevant {
                continue;
            }

            crate::message::validate_message_targets(&otx.witness.message, &self.script_hashes)?;
            let base_hash = crate::hash::otx_base_hash(&otx.witness, &otx.layout, source)?;
            if base_relevant {
                let seal = crate::seal::unique_otx_seal_by_scope(
                    lock_script_hash,
                    &otx.witness.seals,
                    crate::protocol::SealScope::Base,
                )?;
                required_signatures.push(crate::plan::SigningRequirement {
                    origin: crate::plan::SignatureOrigin::OtxBase,
                    carrier_witness_index: otx.layout.witness_index,
                    seal,
                    signing_message_hash: base_hash,
                });
            }
            if append_relevant {
                let seal = crate::seal::unique_otx_seal_by_scope(
                    lock_script_hash,
                    &otx.witness.seals,
                    crate::protocol::SealScope::Append,
                )?;
                required_signatures.push(crate::plan::SigningRequirement {
                    origin: crate::plan::SignatureOrigin::OtxAppend,
                    carrier_witness_index: otx.layout.witness_index,
                    seal,
                    signing_message_hash: crate::hash::otx_append_hash(
                        &otx.witness,
                        &otx.layout,
                        source,
                        base_hash,
                    )?,
                });
            }
        }
    }
}
```

- [ ] **Step 6: Add lock group coverage helper**

Add to `crates/cobuild-core/src/flow.rs`:

```rust
pub(crate) fn lock_group_fully_covered_by_otx(
    input_locks: &[[u8; 32]],
    lock_script_hash: [u8; 32],
    otxs: &[crate::layout::OtxLayout],
) -> bool {
    input_locks.iter().enumerate().all(|(index, hash)| {
        if *hash != lock_script_hash {
            return true;
        }
        otxs.iter().any(|otx| {
            range_contains(otx.base_inputs, index) || range_contains(otx.append_inputs, index)
        })
    })
}
```

In `plan_lock_validation`, after OTX planning:

```rust
let has_tx_level = required_signatures
    .iter()
    .any(|requirement| requirement.origin == crate::plan::SignatureOrigin::TxLevel);
let has_otx = required_signatures.iter().any(|requirement| {
    matches!(
        requirement.origin,
        crate::plan::SignatureOrigin::OtxBase | crate::plan::SignatureOrigin::OtxAppend
    )
});
if has_otx && !has_tx_level {
    if let OtxLayoutScan::Complete(layout) = &self.layout_scan {
        if !crate::flow::lock_group_fully_covered_by_otx(
            &self.script_hashes.input_locks,
            lock_script_hash,
            &layout.otxs,
        ) {
            return Err(CoreError::MissingLockGroupCoverage);
        }
    }
}
```

- [ ] **Step 7: Run engine tests**

Run:

```bash
cargo test -p cobuild-core --offline --test engine
```

Expected: PASS.

- [ ] **Step 8: Commit**

Run:

```bash
git add crates/cobuild-core/src/engine.rs crates/cobuild-core/src/flow.rs crates/cobuild-core/src/seal.rs crates/cobuild-core/tests/engine.rs
git commit -m "feat: plan otx lock validation"
```

## Task 6: Add Type Validation Planning

**Files:**
- Modify: `crates/cobuild-core/src/engine.rs`
- Modify: `crates/cobuild-core/src/flow.rs`
- Modify: `crates/cobuild-core/src/view.rs`
- Test: `crates/cobuild-core/tests/type_plan.rs`

- [ ] **Step 1: Add failing type plan tests**

Create `crates/cobuild-core/tests/type_plan.rs`:

```rust
use cobuild_core::{
    engine::CobuildEngine,
    plan::MessageOrigin,
    source::InMemorySource,
};

#[test]
fn type_plan_exposes_tx_level_message_for_related_input_type() {
    let type_hash = [3u8; 32];
    let message = empty_message();
    let source = InMemorySource {
        input_locks: vec![[1u8; 32]],
        input_types: vec![Some(type_hash)],
        output_types: Vec::new(),
        raw_inputs: vec![Vec::new()],
        witnesses: vec![sighash_all_witness(&[7u8; 65], &message)],
        ..InMemorySource::default()
    };
    let prepared = CobuildEngine::prepare(&source).unwrap();

    let plan = prepared.plan_type_validation(type_hash, &source).unwrap();

    assert_eq!(plan.type_script_hash, type_hash);
    assert_eq!(plan.related_messages.len(), 1);
    assert!(matches!(
        plan.related_messages[0].origin,
        MessageOrigin::TxLevel {
            carrier_witness_index: 0
        }
    ));
}

#[test]
fn type_plan_exposes_otx_message_with_relation_flags() {
    let type_hash = [3u8; 32];
    let target_lock = [1u8; 32];
    let source = InMemorySource {
        input_locks: vec![target_lock],
        input_types: vec![Some(type_hash)],
        output_types: Vec::new(),
        raw_inputs: vec![Vec::new()],
        resolved_outputs: vec![Vec::new()],
        resolved_data: vec![Vec::new()],
        witnesses: vec![
            otx_start_witness(),
            otx_witness(&empty_message(), &[seal_pair(target_lock, 0, &[7u8; 65])]),
        ],
        ..InMemorySource::default()
    };
    let prepared = CobuildEngine::prepare(&source).unwrap();

    let plan = prepared.plan_type_validation(type_hash, &source).unwrap();

    assert_eq!(plan.related_messages.len(), 1);
    match plan.related_messages[0].origin {
        MessageOrigin::Otx {
            witness_index,
            otx_index,
            relation,
            ..
        } => {
            assert_eq!(witness_index, 1);
            assert_eq!(otx_index, 0);
            assert!(relation.input_type_in_base);
            assert!(!relation.input_type_in_append);
        }
        MessageOrigin::TxLevel { .. } => panic!("expected otx message"),
    }
}
```

Copy the same witness helper functions used in `engine.rs` tests into
`type_plan.rs`.

- [ ] **Step 2: Run failing type plan tests**

Run:

```bash
cargo test -p cobuild-core --offline --test type_plan
```

Expected: FAIL because `plan_type_validation` returns an empty plan.

- [ ] **Step 3: Add `MessageView` conversion**

Modify `crates/cobuild-core/src/view.rs`:

```rust
impl From<Cursor> for MessageView {
    fn from(cursor: Cursor) -> Self {
        Self::new(cursor)
    }
}
```

- [ ] **Step 4: Add type relation helpers**

Add to `crates/cobuild-core/src/flow.rs`:

```rust
pub(crate) fn type_hash_in_input_range(
    input_types: &[Option<[u8; 32]>],
    range: Range,
    type_hash: [u8; 32],
) -> bool {
    input_types
        .iter()
        .skip(range.start)
        .take(range.count)
        .any(|hash| *hash == Some(type_hash))
}

pub(crate) fn type_hash_in_output_range(
    output_types: &[Option<[u8; 32]>],
    range: Range,
    type_hash: [u8; 32],
) -> bool {
    output_types
        .iter()
        .skip(range.start)
        .take(range.count)
        .any(|hash| *hash == Some(type_hash))
}
```

- [ ] **Step 5: Implement `plan_type_validation`**

Replace the body of `plan_type_validation` in `crates/cobuild-core/src/engine.rs`:

```rust
pub fn plan_type_validation<S: HashInputSource>(
    &self,
    type_script_hash: [u8; 32],
    _source: &S,
) -> Result<TypeValidationPlan, CoreError> {
    let mut related_messages = Vec::new();

    match &self.layout_scan {
        OtxLayoutScan::Complete(layout) => {
            for (otx_index, otx) in layout.otx_data.iter().enumerate() {
                let relation = crate::plan::OtxTypeRelation {
                    input_type_in_base: crate::flow::type_hash_in_input_range(
                        &self.script_hashes.input_types,
                        otx.layout.base_inputs,
                        type_script_hash,
                    ),
                    input_type_in_append: crate::flow::type_hash_in_input_range(
                        &self.script_hashes.input_types,
                        otx.layout.append_inputs,
                        type_script_hash,
                    ),
                    output_type_in_base: crate::flow::type_hash_in_output_range(
                        &self.script_hashes.output_types,
                        otx.layout.base_outputs,
                        type_script_hash,
                    ),
                    output_type_in_append: crate::flow::type_hash_in_output_range(
                        &self.script_hashes.output_types,
                        otx.layout.append_outputs,
                        type_script_hash,
                    ),
                };
                let is_related = relation.input_type_in_base
                    || relation.input_type_in_append
                    || relation.output_type_in_base
                    || relation.output_type_in_append;
                if !is_related {
                    continue;
                }
                related_messages.push(crate::plan::RelatedMessage {
                    origin: crate::plan::MessageOrigin::Otx {
                        witness_index: otx.layout.witness_index,
                        otx_index,
                        layout: crate::plan::OtxMessageLayout {
                            base_inputs: otx.layout.base_inputs,
                            append_inputs: otx.layout.append_inputs,
                            base_outputs: otx.layout.base_outputs,
                            append_outputs: otx.layout.append_outputs,
                            base_cell_deps: otx.layout.base_cell_deps,
                            append_cell_deps: otx.layout.append_cell_deps,
                            base_header_deps: otx.layout.base_header_deps,
                            append_header_deps: otx.layout.append_header_deps,
                        },
                        relation,
                    },
                    message: crate::view::MessageView::new(otx.witness.message.clone()),
                });
            }
        }
        OtxLayoutScan::Invalid { error, .. } => return Err(error.clone()),
        OtxLayoutScan::None => {}
    }

    if related_messages.is_empty() {
        if let Some((carrier_witness_index, message)) =
            crate::flow::unique_sighash_all_message_with_index(&self.witnesses)?
        {
            let type_is_present = self
                .script_hashes
                .input_types
                .iter()
                .chain(self.script_hashes.output_types.iter())
                .any(|hash| *hash == Some(type_script_hash));
            if type_is_present {
                related_messages.push(crate::plan::RelatedMessage {
                    origin: crate::plan::MessageOrigin::TxLevel {
                        carrier_witness_index,
                    },
                    message: crate::view::MessageView::new(message),
                });
            }
        }
    }

    Ok(TypeValidationPlan {
        type_script_hash,
        related_messages,
    })
}
```

Add to `crates/cobuild-core/src/flow.rs`:

```rust
pub(crate) fn unique_sighash_all_message_with_index(
    witnesses: &[Vec<u8>],
) -> Result<Option<(usize, Cursor)>, CoreError> {
    let mut message = None;
    for (index, witness) in witnesses.iter().enumerate() {
        if witness.is_empty() {
            continue;
        }
        let Ok(view) = WitnessLayoutView::from_slice(witness) else {
            continue;
        };
        if let Some(candidate) = view.sighash_all_message()? {
            if message.is_some() {
                return Err(CoreError::DuplicateSighashAll);
            }
            message = Some((index, candidate));
        }
    }
    Ok(message)
}
```

- [ ] **Step 6: Run type plan tests**

Run:

```bash
cargo test -p cobuild-core --offline --test type_plan
```

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/cobuild-core/src/engine.rs crates/cobuild-core/src/flow.rs crates/cobuild-core/src/view.rs crates/cobuild-core/tests/type_plan.rs
git commit -m "feat: plan cobuild type validation"
```

## Task 7: Make Layout Scanning Source-Driven

**Files:**
- Modify: `crates/cobuild-core/src/layout.rs`
- Modify: `crates/cobuild-core/src/engine.rs`
- Test: `crates/cobuild-core/tests/layout.rs`
- Test: `crates/cobuild-core/tests/source.rs`

- [ ] **Step 1: Add failing source-driven layout test**

Append to `crates/cobuild-core/tests/layout.rs`:

```rust
use cobuild_core::{
    layout::{build_layout_from_witnesses, WitnessCursorSource},
    reader::cursor_from_slice,
    source::ClassifiedCursor,
};

struct TestWitnessSource {
    witnesses: Vec<Vec<u8>>,
}

impl WitnessCursorSource for TestWitnessSource {
    fn witness_count(&self) -> usize {
        self.witnesses.len()
    }

    fn witness_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.witnesses
            .get(index)
            .map(|witness| ClassifiedCursor::hash_input(cursor_from_slice(witness)))
            .ok_or(CoreError::MissingHashInput)
    }
}

#[test]
fn source_driven_layout_matches_owned_layout() {
    let witnesses = vec![otx_start_witness(), otx_witness()];
    let source = TestWitnessSource {
        witnesses: witnesses.clone(),
    };

    let source_layout = build_layout_from_witnesses(&source, 1, 0, 0, 0).unwrap();
    let owned_layout = build_layout(&LayoutTx {
        witnesses,
        input_count: 1,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    })
    .unwrap();

    assert_eq!(source_layout.otxs, owned_layout.otxs);
}
```

- [ ] **Step 2: Run the failing layout test**

Run:

```bash
cargo test -p cobuild-core --offline --test layout source_driven_layout_matches_owned_layout
```

Expected: FAIL with unresolved `build_layout_from_witnesses` and `WitnessCursorSource`.

- [ ] **Step 3: Add witness source trait and builder**

Add to `crates/cobuild-core/src/layout.rs`:

```rust
use crate::source::ClassifiedCursor;

pub trait WitnessCursorSource {
    fn witness_count(&self) -> usize;
    fn witness_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError>;
}

pub fn build_layout_from_witnesses<S: WitnessCursorSource>(
    source: &S,
    input_count: usize,
    output_count: usize,
    cell_dep_count: usize,
    header_dep_count: usize,
) -> Result<BuiltLayout, CoreError> {
    match scan_layout_from_witnesses(source, input_count, output_count, cell_dep_count, header_dep_count) {
        OtxLayoutScan::None => Ok(empty_layout()),
        OtxLayoutScan::Complete(layout) => Ok(layout),
        OtxLayoutScan::Invalid { error, .. } => Err(error),
    }
}
```

Add `scan_layout_from_witnesses` by copying `scan_layout` and replacing
`tx.witnesses[index]` reads with:

```rust
let classified = match source.witness_cursor(index) {
    Ok(cursor) => cursor,
    Err(error) => return invalid_layout(None, error),
};
let witness = match crate::reader::cursor_bytes_with_error(
    &classified.cursor,
    classified.read_error(),
) {
    Ok(bytes) => bytes,
    Err(error) => return invalid_layout(None, error),
};
```

Use `source.witness_count()` instead of `tx.witnesses.len()`, and use the count
arguments instead of fields from `LayoutTx`.

- [ ] **Step 4: Update engine prepare to use source-driven layout**

In `crates/cobuild-core/src/engine.rs`, create an adapter:

```rust
struct SourceWitnesses<'a, S> {
    source: &'a S,
    counts: TxCounts,
}

impl<S: HashInputSource> crate::layout::WitnessCursorSource for SourceWitnesses<'_, S> {
    fn witness_count(&self) -> usize {
        self.counts.witnesses
    }

    fn witness_cursor(&self, index: usize) -> Result<crate::source::ClassifiedCursor, CoreError> {
        self.source.witness_cursor(index)
    }
}
```

Replace the `LayoutTx` call in `prepare`:

```rust
let witness_source = SourceWitnesses { source, counts };
let layout_scan = crate::layout::scan_layout_from_witnesses(
    &witness_source,
    counts.inputs,
    counts.outputs,
    counts.cell_deps,
    counts.header_deps,
);
```

Keep temporary `witnesses: Vec<Vec<u8>>` for tx-level and type unique
`SighashAll` until Task 8 removes it.

- [ ] **Step 5: Run layout and engine tests**

Run:

```bash
cargo test -p cobuild-core --offline --test layout
cargo test -p cobuild-core --offline --test engine
cargo test -p cobuild-core --offline --test type_plan
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/cobuild-core/src/layout.rs crates/cobuild-core/src/engine.rs crates/cobuild-core/tests/layout.rs
git commit -m "refactor: scan otx layout from witness source"
```

## Task 8: Remove Prepared Witness Byte Copies

**Files:**
- Modify: `crates/cobuild-core/src/engine.rs`
- Modify: `crates/cobuild-core/src/flow.rs`
- Modify: `crates/cobuild-core/src/layout.rs`
- Test: `crates/cobuild-core/tests/source.rs`
- Test: `crates/cobuild-core/tests/engine.rs`
- Test: `crates/cobuild-core/tests/type_plan.rs`

- [ ] **Step 1: Add a source behavior test proving engine does not require owned witness collection**

Append to `crates/cobuild-core/tests/source.rs`:

```rust
use core::cell::Cell;
use cobuild_core::engine::CobuildEngine;

#[derive(Default)]
struct CountingSource {
    inner: InMemorySource,
    witness_reads: Cell<usize>,
}

impl TransactionSource for CountingSource {
    fn transaction_cursor(&self) -> Result<ClassifiedCursor, CoreError> {
        self.inner.transaction_cursor()
    }

    fn script_cursor(&self) -> Result<ClassifiedCursor, CoreError> {
        self.inner.script_cursor()
    }

    fn tx_hash(&self) -> Result<[u8; 32], CoreError> {
        self.inner.tx_hash()
    }

    fn input_lock_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        self.inner.input_lock_hash(index)
    }

    fn input_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        self.inner.input_type_hash(index)
    }

    fn output_type_hash(&self, index: usize) -> Result<Option<[u8; 32]>, CoreError> {
        self.inner.output_type_hash(index)
    }

    fn resolved_input_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.inner.resolved_input_output_cursor(index)
    }

    fn resolved_input_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.inner.resolved_input_data_cursor(index)
    }
}

impl HashInputSource for CountingSource {
    fn counts(&self) -> Result<TxCounts, CoreError> {
        self.inner.counts()
    }

    fn witness_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.witness_reads.set(self.witness_reads.get() + 1);
        self.inner.witness_cursor(index)
    }

    fn raw_input_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.inner.raw_input_cursor(index)
    }

    fn raw_output_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.inner.raw_output_cursor(index)
    }

    fn raw_output_data_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.inner.raw_output_data_cursor(index)
    }

    fn raw_cell_dep_cursor(&self, index: usize) -> Result<ClassifiedCursor, CoreError> {
        self.inner.raw_cell_dep_cursor(index)
    }

    fn raw_header_dep_hash(&self, index: usize) -> Result<[u8; 32], CoreError> {
        self.inner.raw_header_dep_hash(index)
    }
}

#[test]
fn engine_prepare_does_not_read_each_witness_twice() {
    let source = CountingSource {
        inner: InMemorySource {
            input_locks: vec![[1u8; 32]],
            input_types: vec![None],
            raw_inputs: vec![Vec::new()],
            witnesses: vec![Vec::new()],
            ..InMemorySource::default()
        },
        witness_reads: Cell::new(0),
    };

    let _prepared = CobuildEngine::prepare(&source).unwrap();

    assert_eq!(source.witness_reads.get(), 1);
}
```

- [ ] **Step 2: Run the source behavior test**

Run:

```bash
cargo test -p cobuild-core --offline --test source engine_prepare_does_not_read_each_witness_twice
```

Expected: FAIL while `prepare` reads witnesses once for layout and once for
temporary `self.witnesses`.

- [ ] **Step 3: Replace `witnesses: Vec<Vec<u8>>` with witness summaries**

Add to `crates/cobuild-core/src/engine.rs`:

```rust
#[derive(Clone)]
enum WitnessSummary {
    Empty,
    Other,
    SighashAll {
        message: cobuild_types::lazy_reader::support::Cursor,
    },
    SighashAllOnly,
}
```

Change `PreparedCobuild`:

```rust
pub struct PreparedCobuild {
    counts: TxCounts,
    script_hashes: ScriptHashIndex,
    witness_summaries: Vec<WitnessSummary>,
    layout_scan: OtxLayoutScan,
}
```

During `prepare`, read each witness once and build both layout input and summary:

```rust
let mut witness_summaries = Vec::with_capacity(counts.witnesses);
let mut witness_bytes_for_layout = Vec::with_capacity(counts.witnesses);
for index in 0..counts.witnesses {
    let witness = source.witness_cursor(index)?;
    let bytes = crate::reader::cursor_bytes_with_error(&witness.cursor, witness.read_error())?;
    let summary = if bytes.is_empty() {
        WitnessSummary::Empty
    } else {
        match crate::view::WitnessLayoutView::from_slice(&bytes) {
            Ok(view) => {
                if let Some(message) = view.sighash_all_message()? {
                    WitnessSummary::SighashAll { message }
                } else if view.sighash_all_witness_layout()?.is_some() {
                    WitnessSummary::SighashAllOnly
                } else {
                    WitnessSummary::Other
                }
            }
            Err(_) => WitnessSummary::Other,
        }
    };
    witness_summaries.push(summary);
    witness_bytes_for_layout.push(bytes);
}
```

Use `witness_bytes_for_layout` only to build the old `LayoutTx` while this task
keeps the scanner compatible. Task 7 already added source-driven layout; if it
is active, remove `witness_bytes_for_layout` and build the layout from the same
single-read cache.

- [ ] **Step 4: Replace unique SighashAll helpers with summary-based helpers**

Add to `crates/cobuild-core/src/engine.rs`:

```rust
fn unique_sighash_all_message_from_summaries(
    summaries: &[WitnessSummary],
) -> Result<Option<cobuild_types::lazy_reader::support::Cursor>, CoreError> {
    let mut message = None;
    for summary in summaries {
        if let WitnessSummary::SighashAll { message: candidate } = summary {
            if message.is_some() {
                return Err(CoreError::DuplicateSighashAll);
            }
            message = Some(candidate.clone());
        }
    }
    Ok(message)
}

fn unique_sighash_all_message_with_index_from_summaries(
    summaries: &[WitnessSummary],
) -> Result<Option<(usize, cobuild_types::lazy_reader::support::Cursor)>, CoreError> {
    let mut message = None;
    for (index, summary) in summaries.iter().enumerate() {
        if let WitnessSummary::SighashAll { message: candidate } = summary {
            if message.is_some() {
                return Err(CoreError::DuplicateSighashAll);
            }
            message = Some((index, candidate.clone()));
        }
    }
    Ok(message)
}
```

Replace calls to `unique_sighash_all_message_from_witnesses` and
`unique_sighash_all_message_with_index` with these summary helpers.

- [ ] **Step 5: For group-leading tx-level witness, re-read only the carrier witness**

In `plan_lock_validation`, when the summary at `carrier_witness_index` is
`SighashAll` or `SighashAllOnly`, read that one witness from `source`:

```rust
let carrier = source.witness_cursor(carrier_witness_index)?;
let carrier_bytes = crate::reader::cursor_bytes_with_error(
    &carrier.cursor,
    carrier.read_error(),
)?;
let view = crate::view::WitnessLayoutView::from_slice(&carrier_bytes)?;
```

This keeps prepare compact and only materializes the related carrier witness
when the current lock needs it.

- [ ] **Step 6: Run source, engine, type plan, and layout tests**

Run:

```bash
cargo test -p cobuild-core --offline --test source
cargo test -p cobuild-core --offline --test engine
cargo test -p cobuild-core --offline --test type_plan
cargo test -p cobuild-core --offline --test layout
```

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add crates/cobuild-core/src/engine.rs crates/cobuild-core/src/flow.rs crates/cobuild-core/src/layout.rs crates/cobuild-core/tests/source.rs crates/cobuild-core/tests/engine.rs crates/cobuild-core/tests/type_plan.rs
git commit -m "refactor: compact prepared witness metadata"
```

## Task 9: Move Lock Contract To LockValidationPlan

**Files:**
- Modify: `contracts/cobuild-otx-lock/src/entry.rs`
- Modify: `contracts/cobuild-otx-lock/src/chain.rs`
- Test: `tests/tests/cobuild_otx_lock.rs`
- Test: `contracts/cobuild-otx-lock/tests/verifier.rs`

- [ ] **Step 1: Add a lock entry compile expectation**

Run:

```bash
cargo test -p cobuild-otx-lock --offline --features library
```

Expected before changes: PASS. This establishes the current contract library
tests compile before API migration.

- [ ] **Step 2: Implement `HashInputSource` for `ChainSource`**

Modify imports in `contracts/cobuild-otx-lock/src/chain.rs`:

```rust
use cobuild_core::{
    engine::{CobuildEngine, PreparedCobuild},
    error::CoreError,
    source::{ClassifiedCursor, HashInputSource, TransactionSource, TxCounts},
};
```

Change `LoadedContext`:

```rust
pub(crate) struct LoadedContext {
    pub source: ChainSource,
    pub prepared: PreparedCobuild,
}
```

Change `load_prepared_context`:

```rust
pub(crate) fn load_prepared_context() -> Result<LoadedContext, Error> {
    let source = ChainSource::default();
    let prepared = CobuildEngine::prepare(&source)?;
    Ok(LoadedContext { source, prepared })
}
```

Add `Default`:

```rust
#[derive(Default)]
pub(crate) struct ChainSource;
```

Rename the existing `impl SigningDataSource for ChainSource` block to:

```rust
impl HashInputSource for ChainSource {
    fn counts(&self) -> Result<TxCounts, CoreError> {
        let raw = signing_raw_transaction()?;
        Ok(TxCounts {
            inputs: raw
                .inputs()
                .and_then(|inputs| inputs.len())
                .map_err(|_| CoreError::MissingHashInput)?,
            outputs: raw
                .outputs()
                .and_then(|outputs| outputs.len())
                .map_err(|_| CoreError::MissingHashInput)?,
            cell_deps: raw
                .cell_deps()
                .and_then(|cell_deps| cell_deps.len())
                .map_err(|_| CoreError::MissingHashInput)?,
            header_deps: raw
                .header_deps()
                .and_then(|header_deps| header_deps.len())
                .map_err(|_| CoreError::MissingHashInput)?,
            witnesses: signing_transaction_view()?
                .witnesses()
                .and_then(|witnesses| witnesses.len())
                .map_err(|_| CoreError::MissingHashInput)?,
        })
    }

    // keep witness_cursor/raw_* methods from the existing implementation
}
```

- [ ] **Step 3: Move entry to lock plan**

Modify `contracts/cobuild-otx-lock/src/entry.rs`:

```rust
pub fn main() -> Result<(), Error> {
    let auth = parse_auth_args(&load_current_script_args()?)?;
    let current_script_hash = load_script_hash()?;
    let loaded = load_prepared_context()?;
    let plan = loaded
        .prepared
        .plan_lock_validation(current_script_hash, &loaded.source)?;

    if plan.required_signatures.is_empty() {
        return Err(Error::LockSemanticFailure);
    }

    let verifier = LocalVerifier;
    for requirement in &plan.required_signatures {
        verifier.verify(
            &auth,
            &requirement.seal,
            &requirement.signing_message_hash,
        )?;
    }

    Ok(())
}
```

- [ ] **Step 4: Run lock crate tests**

Run:

```bash
cargo test -p cobuild-otx-lock --offline --features library
```

Expected: PASS.

- [ ] **Step 5: Run integration test**

Run:

```bash
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: PASS if the debug binary is current. If this fails because the binary
is stale, run the build command in Step 6 before rerunning.

- [ ] **Step 6: Rebuild contract and rerun integration test**

Run:

```bash
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```bash
git add contracts/cobuild-otx-lock/src/entry.rs contracts/cobuild-otx-lock/src/chain.rs tests/tests/cobuild_otx_lock.rs
git commit -m "refactor: use lock validation plan in otx lock"
```

## Task 10: Add Chain Source Cache

**Files:**
- Modify: `contracts/cobuild-otx-lock/src/chain.rs`
- Modify: `contracts/cobuild-otx-lock/src/chain/reader.rs`
- Test: `contracts/cobuild-otx-lock/tests/error.rs`
- Test: `tests/tests/cobuild_otx_lock.rs`

- [ ] **Step 1: Add a private cache API unit test**

Add this private unit test to `contracts/cobuild-otx-lock/src/chain.rs` under
`#[cfg(test)]`:

```rust
#[test]
fn cached_counts_are_returned_without_recomputing() {
    let counts = super::CachedTxCounts {
        inputs: 1,
        outputs: 2,
        cell_deps: 3,
        header_deps: 4,
        witnesses: 5,
    };
    let cache = super::ChainCache::default();

    cache.set_counts(counts);

    assert_eq!(cache.counts(), Some(counts));
}
```

- [ ] **Step 2: Run the failing cache test**

Run:

```bash
cargo test -p cobuild-otx-lock --offline --features library chain_cache
```

Expected: FAIL with unresolved `ChainCache` or `CachedTxCounts`.

- [ ] **Step 3: Add compact cache types**

In `contracts/cobuild-otx-lock/src/chain.rs`, add:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CachedTxCounts {
    inputs: usize,
    outputs: usize,
    cell_deps: usize,
    header_deps: usize,
    witnesses: usize,
}

#[derive(Default)]
struct ChainCache {
    counts: core::cell::Cell<Option<CachedTxCounts>>,
}

impl ChainCache {
    fn counts(&self) -> Option<CachedTxCounts> {
        self.counts.get()
    }

    fn set_counts(&self, counts: CachedTxCounts) {
        self.counts.set(Some(counts));
    }
}
```

Change `ChainSource`:

```rust
#[derive(Default)]
pub(crate) struct ChainSource {
    cache: ChainCache,
}
```

- [ ] **Step 4: Use the cache in `counts`**

In `impl HashInputSource for ChainSource`, start `counts` with:

```rust
if let Some(counts) = self.cache.counts() {
    return Ok(TxCounts {
        inputs: counts.inputs,
        outputs: counts.outputs,
        cell_deps: counts.cell_deps,
        header_deps: counts.header_deps,
        witnesses: counts.witnesses,
    });
}
```

After computing counts:

```rust
let counts = CachedTxCounts {
    inputs,
    outputs,
    cell_deps,
    header_deps,
    witnesses,
};
self.cache.set_counts(counts);
Ok(TxCounts {
    inputs,
    outputs,
    cell_deps,
    header_deps,
    witnesses,
})
```

- [ ] **Step 5: Run lock tests**

Run:

```bash
cargo test -p cobuild-otx-lock --offline --features library
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```bash
git add contracts/cobuild-otx-lock/src/chain.rs contracts/cobuild-otx-lock/src/chain/reader.rs contracts/cobuild-otx-lock/tests
git commit -m "refactor: cache cobuild chain source counts"
```

## Task 11: Split Hash Preimage Helpers

**Files:**
- Create: `crates/cobuild-core/src/hash/writer.rs`
- Modify: `crates/cobuild-core/src/hash.rs`
- Modify: `crates/cobuild-core/src/lib.rs`
- Test: `crates/cobuild-core/tests/hash.rs`

- [ ] **Step 1: Run current hash regression tests**

Run:

```bash
cargo test -p cobuild-core --offline --test hash
```

Expected: PASS. This locks the current hash bytes before refactoring.

- [ ] **Step 2: Create helper module**

Create `crates/cobuild-core/src/hash/writer.rs`:

```rust
use blake2b_ref::Blake2b;
use cobuild_types::lazy_reader::support::Cursor;

use crate::{
    error::CoreError,
    reader::{update_cursor_with_error, update_len_prefixed_cursor},
    source::ClassifiedCursor,
};

pub(crate) fn write_count(hasher: &mut Blake2b, count: usize) -> Result<(), CoreError> {
    let count = u32::try_from(count).map_err(|_| CoreError::HashInputTooLarge)?;
    hasher.update(&count.to_le_bytes());
    Ok(())
}

pub(crate) fn write_cursor(
    hasher: &mut Blake2b,
    cursor: &ClassifiedCursor,
) -> Result<(), CoreError> {
    update_cursor_with_error(hasher, &cursor.cursor, cursor.read_error())
}

pub(crate) fn write_len_prefixed_cursor_with_error(
    hasher: &mut Blake2b,
    cursor: &Cursor,
    error: CoreError,
) -> Result<(), CoreError> {
    update_len_prefixed_cursor(hasher, cursor, error)
}

pub(crate) fn write_len_prefixed_classified_cursor(
    hasher: &mut Blake2b,
    cursor: &ClassifiedCursor,
) -> Result<(), CoreError> {
    update_len_prefixed_cursor(hasher, &cursor.cursor, cursor.read_error())
}
```

- [ ] **Step 3: Convert `hash.rs` to module directory form**

Move `crates/cobuild-core/src/hash.rs` to `crates/cobuild-core/src/hash/mod.rs`.
At the top of `mod.rs`, add:

```rust
mod writer;
```

Update `crates/cobuild-core/src/lib.rs` only if module paths need explicit
directory support. `pub mod hash;` remains valid.

- [ ] **Step 4: Replace local helper calls**

In `crates/cobuild-core/src/hash/mod.rs`, replace:

```rust
fn update_count(hasher: &mut blake2b_ref::Blake2b, count: usize) -> Result<(), CoreError> {
    hasher.update(&checked_len_prefix(count)?);
    Ok(())
}
```

with calls to:

```rust
writer::write_count(&mut hasher, count)?;
```

Replace length-prefixed classified cursor calls with:

```rust
writer::write_len_prefixed_classified_cursor(&mut hasher, &cursor)?;
```

Replace direct classified cursor writes with:

```rust
writer::write_cursor(&mut hasher, &cursor)?;
```

Keep `checked_len_prefix` public for existing tests:

```rust
pub fn checked_len_prefix(len: usize) -> Result<[u8; 4], CoreError> {
    let len = u32::try_from(len).map_err(|_| CoreError::HashInputTooLarge)?;
    Ok(len.to_le_bytes())
}
```

- [ ] **Step 5: Run hash tests**

Run:

```bash
cargo test -p cobuild-core --offline --test hash
```

Expected: PASS with unchanged expected digests.

- [ ] **Step 6: Commit**

Run:

```bash
git add crates/cobuild-core/src/hash.rs crates/cobuild-core/src/hash crates/cobuild-core/src/lib.rs crates/cobuild-core/tests/hash.rs
git commit -m "refactor: split cobuild hash preimage writers"
```

## Task 12: Remove Old Query/Context API

**Files:**
- Modify: `crates/cobuild-core/src/lib.rs`
- Modify: `crates/cobuild-core/src/context.rs`
- Modify: `crates/cobuild-core/src/prepare.rs`
- Delete: `crates/cobuild-core/src/query.rs`
- Delete: `crates/cobuild-core/src/sighash.rs`
- Delete: `crates/cobuild-core/src/otx_request.rs`
- Delete or shrink: `crates/cobuild-core/src/signature.rs`
- Modify: `crates/cobuild-core/tests/signature_requests.rs`
- Test: `crates/cobuild-core/tests/engine.rs`
- Test: `crates/cobuild-core/tests/type_plan.rs`

- [ ] **Step 1: Add API removal guard test**

Create `crates/cobuild-core/tests/no_old_query_api.rs`:

```rust
use std::fs;
use std::path::PathBuf;

#[test]
fn core_no_longer_exports_old_lock_query_api() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src");
    let lib = fs::read_to_string(root.join("lib.rs")).expect("read lib.rs");

    assert!(!lib.contains("mod query"));
    assert!(!lib.contains("mod sighash"));
    assert!(!lib.contains("mod otx_request"));
    assert!(!lib.contains("pub mod signature"));
}
```

- [ ] **Step 2: Run failing API removal test**

Run:

```bash
cargo test -p cobuild-core --offline --test no_old_query_api
```

Expected: FAIL while old modules are exported.

- [ ] **Step 3: Port remaining signature request tests to engine tests**

Move behavior coverage from `crates/cobuild-core/tests/signature_requests.rs`
into `crates/cobuild-core/tests/engine.rs`. Port these existing tests by
renaming them and changing the assertion target from
`context.lock_query(hash).required_signatures(&source)` to
`prepared.plan_lock_validation(hash, &source)`:

- `otx_signature_rejects_message_action_target_absent_from_transaction` becomes
  `engine_lock_plan_rejects_message_action_target_absent_from_transaction`.
- `otx_signature_rejects_duplicate_required_seal_pair` becomes
  `engine_lock_plan_rejects_duplicate_required_otx_seal_pair`.
- `otx_signature_rejects_invalid_seal_scope` becomes
  `engine_lock_plan_rejects_invalid_otx_seal_scope`.
- `required_signatures_rejects_uncovered_lock_group_without_tx_level_witness`
  becomes
  `engine_lock_plan_rejects_uncovered_lock_group_without_tx_level_requirement`.
- `required_signatures_include_sighash_and_otx_requirements` becomes
  `engine_lock_plan_allows_combined_tx_level_and_otx_requirements`.

For each migrated test, construct `let prepared = CobuildEngine::prepare(&source).unwrap();`
and assert against `LockValidationPlan.required_signatures` or the returned
`CoreError`.

- [ ] **Step 4: Remove old modules from `lib.rs`**

Modify `crates/cobuild-core/src/lib.rs`:

```rust
pub mod context;
pub mod engine;
pub mod error;
pub mod hash;
pub mod layout;
mod message;
pub mod plan;
pub mod prepare;
pub mod protocol;
pub mod reader;
mod seal;
pub mod source;
pub mod view;
pub mod witness;
mod flow;
```

Remove:

```rust
mod otx_request;
mod query;
mod sighash;
pub mod signature;
```

- [ ] **Step 5: Remove obsolete public structs from prepare/context**

In `crates/cobuild-core/src/prepare.rs`, remove:

```text
TransactionInfo
PreparedContextInput
SourcePreparedContext
prepare_context
prepare_context_from_source
parse_transaction_info
```

Keep `script_args_from_slice` if tests or lock args still use it.

In `crates/cobuild-core/src/context.rs`, keep only `ScriptHashIndex` if engine
still uses it. Remove `CobuildContext`, `LockScriptQuery`, and
`PreparedContext` after all references are gone.

- [ ] **Step 6: Delete old files**

Delete:

```text
crates/cobuild-core/src/query.rs
crates/cobuild-core/src/sighash.rs
crates/cobuild-core/src/otx_request.rs
crates/cobuild-core/src/signature.rs
crates/cobuild-core/tests/signature_requests.rs
```

- [ ] **Step 7: Run core tests**

Run:

```bash
cargo test -p cobuild-core --offline
```

Expected: PASS.

- [ ] **Step 8: Commit**

Run:

```bash
git add crates/cobuild-core/src crates/cobuild-core/tests
git commit -m "refactor: remove old cobuild query api"
```

## Task 13: Full Boundary And Workspace Verification

**Files:**
- Modify: `crates/cobuild-core/src/*` only for compile or lint fixes found by this task.
- Modify: `contracts/cobuild-otx-lock/src/*` only for compile or lint fixes found by this task.
- Modify: `tests/tests/cobuild_otx_lock.rs` only for integration failure fixes found by this task.

- [ ] **Step 1: Run codegen check**

Run:

```bash
cargo run -p xtask --offline -- codegen cobuild-types --check
```

Expected: PASS.

- [ ] **Step 2: Run boundary checks**

Run:

```bash
rg -n "cobuild_types::entity|::entity::" crates/cobuild-core/src contracts/cobuild-otx-lock/src
rg -n "unsafe" crates/cobuild-core/src contracts/cobuild-otx-lock/src
rg -n "ckb_std" crates/cobuild-core/src
rg -n "critical-section|portable-atomic.*unsafe-assume-single-core|\\[patch.crates-io\\]" Cargo.toml crates contracts
```

Expected: no matches.

- [ ] **Step 3: Run clippy**

Run:

```bash
cargo clippy --workspace --all-targets --offline
```

Expected: PASS.

- [ ] **Step 4: Run workspace tests**

Run:

```bash
cargo test --workspace --offline
```

Expected: PASS.

- [ ] **Step 5: Build lock contract**

Run:

```bash
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
```

Expected: PASS.

- [ ] **Step 6: Run lock integration test**

Run:

```bash
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Run diff whitespace check**

Run:

```bash
git diff --check
```

Expected: no output.

- [ ] **Step 8: Commit verification fixes**

If any compile/test fixes were needed in this task, commit them:

```bash
git add crates contracts tests
git commit -m "test: verify cobuild engine refactor"
```

If no files changed, do not create an empty commit.
