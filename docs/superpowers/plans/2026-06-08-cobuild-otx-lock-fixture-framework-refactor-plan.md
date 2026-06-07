# Cobuild OTX Lock Fixture Framework Refactor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move reusable cobuild-otx-lock/UDT helper code out of `tests/tests/cobuild_otx_lock.rs`, internalize generic signing/contract/OTX primitives into `tests/src/framework`, and keep concrete test scenarios readable in the integration test.

**Architecture:** `tests/src/framework` owns reusable test primitives: contract deployment, cell creation, signing hashes, witness encoding, generic OTX construction helpers, and verification wrappers. `tests/src/fixtures` owns reusable cobuild-otx-lock and UDT fixture helpers, including malformed witnesses, bad seals, hash-oracle adapters, UDT input/output builders, and single-OTX UDT signing helpers. `tests/tests/cobuild_otx_lock.rs` keeps the concrete scenario composition, such as “100 UDT split into 2 outputs plus 300 UDT split into 3 outputs,” while delegating helper mechanics.

**Tech Stack:** Rust, `ckb-testtool`, `cobuild-types`, `cobuild-core`, `secp256k1`, cargo offline tests.

---

## File Structure

- Modify: `tests/tests/cobuild_otx_lock.rs`
  - Keep `#[test]` functions, `assert_lock_script_exit`, and concrete scenario composition such as `two_udt_transfer_otxs_case`.
  - Replace local `OtxFixtureInput`, `OtxFixtureOutput`, `create_udt_input`, `cell_input_for_output`, local hash/sign helpers, and reusable OTX/UDT builders with calls to `tests::fixtures` and `tests::framework`.

- Modify: `tests/src/fixtures/mod.rs`
  - Expose `pub mod udt;`.

- Create: `tests/src/fixtures/udt.rs`
  - Own reusable UDT fixture helpers:
    - `UdtTransferOtxParts`
    - `create_plain_locked_input`
    - `create_udt_input`
    - `udt_output`
    - `signed_udt_transfer_otx`
    - `full_output_masks`

- Modify: `tests/src/fixtures/cobuild_otx_lock.rs`
  - Own cobuild-otx-lock-specific case assembly for existing positive/negative lock contract tests.
  - Do not receive the concrete two-UDT-transfer scenario; that remains in `tests/tests/cobuild_otx_lock.rs`.
  - Use framework helpers for generic deployment, signing, cell creation, and witness encoding.

- Modify: `tests/src/fixtures/support.rs`
  - Keep only cobuild-otx-lock-specific support that does not belong in framework:
    - `Case`
    - malformed witness byte constructors
    - scenario-local OTX preimage structs if still needed by `fixtures/otx_hash.rs`
  - Remove duplicate `packed_hash_to_array`.

- Modify: `tests/src/fixtures/hashing.rs`
  - Delete after migrating generic hash/signing helpers to framework, or reduce to fixture-only helpers if any remain.

- Modify: `tests/src/fixtures/otx_hash.rs`
  - Keep lock-contract oracle hash logic for OTX base/append preimages.
  - Reuse shared framework length/hash helpers where appropriate.

- Create: `tests/src/framework/signing.rs`
  - Own generic signing helpers:
    - `fixed_secret_key`
    - `public_key_hash20`
    - `sign_recoverable`
    - `tx_without_message_hash`
    - `tx_without_message_hash_for_inputs`
    - `sighash_all_only_witness`

- Modify: `tests/src/framework/contracts.rs`
  - Keep generic deployment helpers only.
  - Do not add `deploy_cobuild_otx_lock`; fixtures should call `deploy_data2_script(context, "cobuild-otx-lock", args)` so the framework does not grow contract-specific wrappers.

- Modify: `tests/src/framework/cells.rs`
  - Add generic live input helpers that return both `CellInput` and resolved input metadata:
    - `TestResolvedInput`
    - `live_resolved_input`
    - `live_resolved_normal_input`
    - `live_resolved_typed_input`

- Modify: `tests/src/framework/cobuild.rs`
  - Add generic OTX seal and empty-message helpers:
    - `empty_message`
    - `seal_pair`
    - builder support for base outputs, masks, and seals where needed by fixtures.

- Modify: `tests/src/framework/tx.rs`
  - Add helper for constructing `OtxStart` witness bytes with explicit start indices:
    - `otx_start_witness`
  - Existing `OtxTransactionBuilder` remains for common happy-path OTX transactions.

- Modify: `tests/src/framework/mod.rs`
  - Expose `pub mod signing;`.
  - Add tests for new framework helpers.

- Modify: `tests/src/tests.rs`
  - Add boundary tests preventing scenario helper types/functions from living in `tests/tests/cobuild_otx_lock.rs`.

---

### Task 1: Lock the Test/File Boundary

**Files:**
- Modify: `tests/src/tests.rs`
- Test: `cargo test -p tests cobuild_otx_lock_test_file_contains_no_fixture_helpers --offline`

- [ ] **Step 1: Write the failing boundary test**

Add this test to `tests/src/tests.rs`:

```rust
#[test]
fn cobuild_otx_lock_test_file_contains_no_fixture_helpers() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
    let test_file = repo.join("tests/cobuild_otx_lock.rs");
    let source = std::fs::read_to_string(&test_file).expect("read cobuild_otx_lock test file");

    for forbidden in [
        "struct OtxFixtureInput",
        "struct OtxFixtureOutput",
        "struct UdtTransferOtxParts",
        "struct OtxFixtureOutputPart",
        "struct OtxFixtureParts",
        "struct OtxHashFixture",
        "fn create_plain_locked_input",
        "fn create_udt_input",
        "fn cell_input_for_output",
        "fn udt_output",
        "fn signed_udt_transfer_otx",
        "fn empty_message_entity",
        "fn otx_base_hash",
        "fn otx_hash_inputs",
        "fn full_output_masks",
        "fn tx_without_message_hash_for_inputs",
        "fn sign_recoverable",
        "fn write_count",
        "fn write_len_prefixed_bytes",
        "fn checked_len_prefix",
        "fn packed_hash_to_array",
        "fn range",
    ] {
        assert!(
            !source.contains(forbidden),
            "`{forbidden}` belongs in fixtures/framework, not in tests/cobuild_otx_lock.rs"
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p tests cobuild_otx_lock_test_file_contains_no_fixture_helpers --offline
```

Expected: FAIL because `tests/tests/cobuild_otx_lock.rs` still contains `OtxFixtureInput`, `OtxFixtureOutput`, `create_udt_input`, and related helpers.

- [ ] **Step 3: Keep the failing result in the red/green notes**

Record:

```text
RED Task 1: cargo test -p tests cobuild_otx_lock_test_file_contains_no_fixture_helpers --offline
Expected failure: forbidden helper names are still present in tests/tests/cobuild_otx_lock.rs.
```

Do not change implementation in this task.

---

### Task 2: Add Framework Signing Helpers

**Files:**
- Create: `tests/src/framework/signing.rs`
- Modify: `tests/src/framework/mod.rs`
- Test: `cargo test -p tests framework::tests::signing_helpers_build_sighash_all_only_witness --offline`

- [ ] **Step 1: Write the failing framework test**

Add imports and test to the `#[cfg(test)] mod tests` block in `tests/src/framework/mod.rs`:

```rust
use super::signing::{
    fixed_secret_key, public_key_hash20, sighash_all_only_witness, sign_recoverable,
};

#[test]
fn signing_helpers_build_sighash_all_only_witness() {
    let secret_key = fixed_secret_key(1);
    let public_key_hash = public_key_hash20(&secret_key);
    assert_eq!(public_key_hash.len(), 20);

    let seal = sign_recoverable(&secret_key, [7u8; 32]);
    assert_eq!(seal.len(), 65);

    let witness = sighash_all_only_witness(seal.clone());
    assert!(witness.len() > seal.len());
    assert!(witness.windows(seal.len()).any(|window| window == seal.as_slice()));
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p tests framework::tests::signing_helpers_build_sighash_all_only_witness --offline
```

Expected: FAIL with unresolved import `super::signing`.

- [ ] **Step 3: Implement `tests/src/framework/signing.rs`**

Create `tests/src/framework/signing.rs`:

```rust
use blake2b_ref::Blake2bBuilder;
use ckb_testtool::ckb_types::bytes::Bytes;
use cobuild_types::entity::{core::SighashAllOnly, witness::WitnessLayout};
use secp256k1::{Message, PublicKey, Secp256k1, SecretKey};

const TX_WITHOUT_MESSAGE_PERSONAL: &[u8; 16] = b"ckbcb_tnm_core1\0";

pub fn fixed_secret_key(byte: u8) -> SecretKey {
    SecretKey::from_slice(&[byte; 32]).expect("fixed secret key")
}

pub fn public_key_hash20(secret_key: &SecretKey) -> [u8; 20] {
    let secp = Secp256k1::new();
    let public_key = PublicKey::from_secret_key(&secp, secret_key);
    let hash = ckb_hash::blake2b_256(public_key.serialize());
    let mut out = [0u8; 20];
    out.copy_from_slice(&hash[..20]);
    out
}

pub fn tx_without_message_hash(
    tx_hash: [u8; 32],
    input_count: usize,
    resolved_output: &[u8],
    witnesses: &[Vec<u8>],
) -> [u8; 32] {
    let inputs: Vec<(&[u8], &[u8])> = (0..input_count)
        .map(|_| (resolved_output, &[][..]))
        .collect();
    tx_without_message_hash_for_inputs(tx_hash, &inputs, witnesses)
}

pub fn tx_without_message_hash_for_inputs(
    tx_hash: [u8; 32],
    inputs: &[(&[u8], &[u8])],
    witnesses: &[Vec<u8>],
) -> [u8; 32] {
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32)
        .personal(TX_WITHOUT_MESSAGE_PERSONAL)
        .build();
    hasher.update(&tx_hash);
    for (resolved_output, data) in inputs {
        hasher.update(resolved_output);
        hasher.update(&checked_len_prefix(data.len()));
        hasher.update(data);
    }
    for witness in witnesses.iter().skip(inputs.len()) {
        hasher.update(&checked_len_prefix(witness.len()));
        hasher.update(witness);
    }
    hasher.finalize(&mut out);
    out
}

pub fn sign_recoverable(secret_key: &SecretKey, digest: [u8; 32]) -> Vec<u8> {
    let secp = Secp256k1::new();
    let message = Message::from_digest(digest);
    let signature = secp.sign_ecdsa_recoverable(&message, secret_key);
    let (recovery_id, compact) = signature.serialize_compact();
    let mut seal = Vec::with_capacity(65);
    seal.extend_from_slice(&compact);
    seal.push(i32::from(recovery_id) as u8);
    seal
}

pub fn sighash_all_only_witness(seal: Vec<u8>) -> Bytes {
    let witness = WitnessLayout::from(SighashAllOnly::new_builder().seal(seal).build());
    Bytes::copy_from_slice(witness.as_slice())
}

pub fn checked_len_prefix(len: usize) -> [u8; 4] {
    u32::try_from(len)
        .expect("fixture length fits u32")
        .to_le_bytes()
}
```

- [ ] **Step 4: Expose the module**

Add to `tests/src/framework/mod.rs`:

```rust
pub mod signing;
```

- [ ] **Step 5: Run the test to verify it passes**

Run:

```bash
cargo test -p tests framework::tests::signing_helpers_build_sighash_all_only_witness --offline
```

Expected: PASS.

---

### Task 3: Add Framework Cell Primitives and Keep Contract Deployment Generic

**Files:**
- Modify: `tests/src/framework/cells.rs`
- Modify: `tests/src/framework/contracts.rs`
- Modify: `tests/src/framework/mod.rs`
- Test: `cargo test -p tests framework::tests::resolved_input_helpers_preserve_cell_and_data --offline`

- [ ] **Step 1: Write the failing framework test**

Add this test to `tests/src/framework/mod.rs`:

```rust
use super::{
    cells::{live_resolved_typed_input, TestResolvedInput},
    contracts::deploy_data2_script,
};

#[test]
fn resolved_input_helpers_preserve_cell_and_data() {
    let mut fixture = CobuildTestFixture::new();
    let lock = fixture.deploy_always_success();
    let type_script = fixture.deploy_always_success();
    let (_input, resolved): (_, TestResolvedInput) = live_resolved_typed_input(
        fixture.context_mut(),
        lock.script.clone(),
        type_script.script.clone(),
        1_000,
        vec![1, 2, 3],
    );

    assert!(!resolved.raw_input.is_empty());
    assert!(!resolved.resolved_output.is_empty());
    assert_eq!(resolved.data, vec![1, 2, 3]);

    let deployed = deploy_data2_script(
        fixture.context_mut(),
        "cobuild-otx-lock",
        vec![0u8; 21],
    );
    assert_eq!(deployed.script.args().raw_data().len(), 21);
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p tests framework::tests::resolved_input_helpers_preserve_cell_and_data --offline
```

Expected: FAIL with unresolved imports `TestResolvedInput` or `live_resolved_typed_input`.

- [ ] **Step 3: Add resolved input helpers**

Add to `tests/src/framework/cells.rs`:

```rust
#[derive(Clone, Debug)]
pub struct TestResolvedInput {
    pub raw_input: Vec<u8>,
    pub resolved_output: Vec<u8>,
    pub data: Vec<u8>,
}

pub fn live_resolved_input(
    context: &mut Context,
    output: CellOutput,
    data: impl Into<Bytes>,
) -> (CellInput, TestResolvedInput) {
    let data = data.into();
    let previous_output = context.create_cell(output.clone(), data.clone());
    let input = CellInput::new_builder()
        .previous_output(previous_output)
        .build();
    let resolved = TestResolvedInput {
        raw_input: input.as_slice().to_vec(),
        resolved_output: output.as_slice().to_vec(),
        data: data.to_vec(),
    };
    (input, resolved)
}

pub fn live_resolved_normal_input(
    context: &mut Context,
    lock: Script,
    capacity: u64,
    data: impl Into<Bytes>,
) -> (CellInput, TestResolvedInput) {
    live_resolved_input(context, normal_output(lock, capacity), data)
}

pub fn live_resolved_typed_input(
    context: &mut Context,
    lock: Script,
    type_script: Script,
    capacity: u64,
    data: impl Into<Bytes>,
) -> (CellInput, TestResolvedInput) {
    live_resolved_input(context, typed_output(lock, type_script, capacity), data)
}
```

- [ ] **Step 4: Confirm contract deployment stays generic**

Do not add a `deploy_cobuild_otx_lock` wrapper to `tests/src/framework/contracts.rs`.
Use the existing generic helper at call sites:

```rust
let deployed = deploy_data2_script(context, "cobuild-otx-lock", args);
```

This keeps the framework from accumulating contract-specific catalog functions.

- [ ] **Step 5: Run the test to verify it passes**

Run:

```bash
cargo test -p tests framework::tests::resolved_input_helpers_preserve_cell_and_data --offline
```

Expected: PASS.

---

### Task 4: Add Generic OTX Witness Helpers

**Files:**
- Modify: `tests/src/framework/cobuild.rs`
- Modify: `tests/src/framework/tx.rs`
- Modify: `tests/src/framework/mod.rs`
- Test: `cargo test -p tests framework::tests::otx_witness_helpers_encode_start_and_seal --offline`

- [ ] **Step 1: Write the failing framework test**

Add this test to `tests/src/framework/mod.rs`:

```rust
use super::{
    cobuild::{empty_message, seal_pair},
    tx::otx_start_witness,
};

#[test]
fn otx_witness_helpers_encode_start_and_seal() {
    let message = empty_message();
    assert_eq!(message.actions().len(), 0);

    let seal = seal_pair([9u8; 32], 0, vec![1, 2, 3]);
    assert_eq!(seal.script_hash().raw_data().as_ref(), &[9u8; 32]);

    let witness = otx_start_witness(1, 2, 3, 4);
    assert!(!witness.is_empty());
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p tests framework::tests::otx_witness_helpers_encode_start_and_seal --offline
```

Expected: FAIL with unresolved imports `empty_message`, `seal_pair`, or `otx_start_witness`.

- [ ] **Step 3: Add generic cobuild helpers**

Add to `tests/src/framework/cobuild.rs`:

```rust
pub fn empty_message() -> CobuildMessage {
    CobuildMessage::new_builder()
        .actions(ActionVec::new_builder().build())
        .build()
}

pub fn seal_pair(script_hash: [u8; 32], scope: u8, seal: Vec<u8>) -> cobuild_types::entity::core::SealPair {
    cobuild_types::entity::core::SealPair::new_builder()
        .script_hash(script_hash)
        .scope(scope)
        .seal(seal)
        .build()
}
```

- [ ] **Step 4: Add OtxStart witness helper**

Add to `tests/src/framework/tx.rs`:

```rust
pub fn otx_start_witness(
    start_input_cell: u32,
    start_output_cell: u32,
    start_cell_deps: u32,
    start_header_deps: u32,
) -> Bytes {
    let witness = WitnessLayout::from(
        OtxStart::new_builder()
            .start_input_cell(start_input_cell.to_le_bytes())
            .start_output_cell(start_output_cell.to_le_bytes())
            .start_cell_deps(start_cell_deps.to_le_bytes())
            .start_header_deps(start_header_deps.to_le_bytes())
            .build(),
    );
    Bytes::copy_from_slice(witness.as_slice())
}
```

- [ ] **Step 5: Run the test to verify it passes**

Run:

```bash
cargo test -p tests framework::tests::otx_witness_helpers_encode_start_and_seal --offline
```

Expected: PASS.

---

### Task 5: Extract UDT Transfer Helpers While Keeping Scenarios In Integration Tests

**Files:**
- Create: `tests/src/fixtures/udt.rs`
- Modify: `tests/src/fixtures/mod.rs`
- Modify: `tests/tests/cobuild_otx_lock.rs`
- Modify: `tests/src/tests.rs`
- Test: `cargo test -p tests cobuild_otx_lock_test_file_contains_no_fixture_helpers --offline`
- Test: `cargo test -p tests --test cobuild_otx_lock contract_accepts_two_udt_transfer_otxs_in_one_transaction --offline`
- Test: `cargo test -p tests --test cobuild_otx_lock contract_accepts_two_udt_transfer_otxs_with_sighash_all_fee_input --offline`

- [ ] **Step 1: Keep concrete scenario functions in the integration test**

The two tests in `tests/tests/cobuild_otx_lock.rs` must continue to express the scenario locally:

```rust
#[test]
fn contract_accepts_two_udt_transfer_otxs_in_one_transaction() {
    let (context, tx) = two_udt_transfer_otxs_case(false);
    let result = context.verify_tx(&tx, 50_000_000);
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_two_udt_transfer_otxs_with_sighash_all_fee_input() {
    let (context, tx) = two_udt_transfer_otxs_case(true);
    let result = context.verify_tx(&tx, 50_000_000);
    assert!(result.is_ok(), "{result:?}");
}
```

`two_udt_transfer_otxs_case` is allowed to remain in the integration test because it is the concrete scenario being tested. The helpers it uses should move out.

- [ ] **Step 2: Run the boundary test to verify it fails before the refactor**

Run:

```bash
cargo test -p tests cobuild_otx_lock_test_file_contains_no_fixture_helpers --offline
```

Expected: FAIL while `tests/tests/cobuild_otx_lock.rs` still defines helper types/functions such as `OtxFixtureInput`, `create_udt_input`, `cell_input_for_output`, `signed_udt_transfer_otx`, and hash/signing helpers.

- [ ] **Step 3: Create abstract UDT/OTX helper module**

Create `tests/src/fixtures/udt.rs` with reusable helpers that do not encode the two-OTX concrete scenario:

```rust
use ckb_testtool::{
    ckb_types::{
        bytes::Bytes,
        packed::{CellOutput, Script},
        prelude::*,
    },
    context::Context,
};
use cobuild_types::entity::core::Otx;
use secp256k1::SecretKey;

use crate::framework::{
    cells::{TestCellOutput, TestResolvedInput, live_resolved_normal_input, live_resolved_typed_input},
    cobuild::{empty_message, seal_pair},
    signing::sign_recoverable,
};

#[derive(Clone)]
pub struct UdtTransferOtxParts {
    pub start_input: usize,
    pub start_output: usize,
    pub input: TestResolvedInput,
    pub outputs: Vec<TestCellOutput>,
}

pub fn create_plain_locked_input(
    context: &mut Context,
    lock: Script,
    capacity: u64,
    data: impl Into<Bytes>,
) -> (ckb_testtool::ckb_types::packed::CellInput, TestResolvedInput) {
    live_resolved_normal_input(context, lock, capacity, data)
}

pub fn create_udt_input(
    context: &mut Context,
    lock: Script,
    type_script: Script,
    amount: u128,
) -> (ckb_testtool::ckb_types::packed::CellInput, TestResolvedInput) {
    live_resolved_typed_input(
        context,
        lock,
        type_script,
        100_000_000_000,
        amount.to_le_bytes().to_vec(),
    )
}

pub fn udt_output(lock: Script, type_script: Script, amount: u128) -> TestCellOutput {
    TestCellOutput::new(
        CellOutput::new_builder()
            .capacity(90_000_000_000u64)
            .lock(lock)
            .type_(Some(type_script).pack())
            .build(),
        amount.to_le_bytes().to_vec(),
    )
}

pub fn full_output_masks(output_count: usize) -> Vec<u8> {
    let bits = output_count * 4;
    let bytes = bits.div_ceil(8);
    let mut masks = vec![0xff; bytes];
    let extra_bits = bytes * 8 - bits;
    if extra_bits > 0 {
        let keep_bits = 8 - extra_bits;
        let last = masks.last_mut().expect("non-empty output mask");
        *last = (1u8 << keep_bits) - 1;
    }
    masks
}
```

Also move a reusable `signed_udt_transfer_otx(...)` helper into this module. It should accept `UdtTransferOtxParts`, `lock_hash`, and `secret_key`, build a single base-scope UDT transfer OTX, and use:

```rust
empty_message()
seal_pair(lock_hash, 0, base_seal)
sign_recoverable(secret_key, base_hash)
full_output_masks(parts.outputs.len())
```

Keep the helper generic: it must not know that one test uses 100 UDT split into 2 outputs or 300 UDT split into 3 outputs.

- [ ] **Step 4: Expose the UDT helper module**

Modify `tests/src/fixtures/mod.rs`:

```rust
pub mod udt;
```

- [ ] **Step 5: Refactor the integration test to use abstract helpers**

In `tests/tests/cobuild_otx_lock.rs`, keep `two_udt_transfer_otxs_case(include_fee_input: bool)` and the two concrete tests, but replace local helper definitions and calls:

```text
create_plain_locked_input(...)
  -> fixtures::udt::create_plain_locked_input(...)

create_udt_input(...)
  -> fixtures::udt::create_udt_input(...)

udt_output(...)
  -> fixtures::udt::udt_output(...)

UdtTransferOtxParts
  -> fixtures::udt::UdtTransferOtxParts

signed_udt_transfer_otx(...)
  -> fixtures::udt::signed_udt_transfer_otx(...)

full_output_masks(...)
  -> fixtures::udt::full_output_masks(...) only if still needed locally; prefer keeping it inside `signed_udt_transfer_otx`.
```

Use framework helpers for generic signing/witness pieces:

```text
empty_message_entity()
  -> framework::cobuild::empty_message()

OtxStart::new_builder()...
  -> framework::tx::otx_start_witness(start_input as u32, 0, 2, 0)

SighashAllOnly::new_builder()...
  -> framework::signing::sighash_all_only_witness(tx_seal)

tx_without_message_hash_for_inputs(...)
  -> framework::signing::tx_without_message_hash_for_inputs(...)

sign_recoverable(&secp, &secret_key, hash)
  -> framework::signing::sign_recoverable(&secret_key, hash)
```

The integration test should still preserve and visibly encode these facts:

```text
OTX A input: 100 UDT
OTX A outputs: 40 UDT + 60 UDT
OTX B input: 300 UDT
OTX B outputs: 100 UDT + 100 UDT + 100 UDT
With fee input: first input is plain CKB locked by cobuild-otx-lock and signed by SighashAllOnly.
All locks use contracts/cobuild-otx-lock.
The test UDT type script arg uses the cobuild-otx-lock script hash.
```

- [ ] **Step 6: Delete helper definitions from the integration test**

Remove from `tests/tests/cobuild_otx_lock.rs`:

```rust
struct OtxFixtureInput
struct OtxFixtureOutput
struct UdtTransferOtxParts
struct OtxFixtureOutputPart
struct OtxFixtureParts
struct OtxHashFixture
fn create_plain_locked_input(...)
fn create_udt_input(...)
fn cell_input_for_output(...)
fn udt_output(...)
fn signed_udt_transfer_otx(...)
fn empty_message_entity(...)
fn otx_base_hash(...)
fn otx_hash_inputs(...)
fn full_output_masks(...)
fn tx_without_message_hash_for_inputs(...)
fn sign_recoverable(...)
fn write_count(...)
fn write_len_prefixed_bytes(...)
fn checked_len_prefix(...)
fn packed_hash_to_array(...)
fn range(...)
```

Do not remove `two_udt_transfer_otxs_case(include_fee_input: bool)`; it is the concrete scenario composition and should remain in the integration test.

- [ ] **Step 7: Run the targeted tests to verify they pass**

Run:

```bash
cargo test -p tests cobuild_otx_lock_test_file_contains_no_fixture_helpers --offline
cargo test -p tests --test cobuild_otx_lock contract_accepts_two_udt_transfer_otxs_in_one_transaction --offline
cargo test -p tests --test cobuild_otx_lock contract_accepts_two_udt_transfer_otxs_with_sighash_all_fee_input --offline
```

Expected: all PASS.

---

### Task 6: Remove Duplicate Hash and Script Helpers From Fixtures

**Files:**
- Modify: `tests/src/fixtures/support.rs`
- Modify: `tests/src/fixtures/hashing.rs`
- Modify: `tests/src/fixtures/otx_hash.rs`
- Modify: `tests/src/fixtures/cobuild_otx_lock.rs`
- Test: `cargo test -p tests --test cobuild_otx_lock --offline`

- [ ] **Step 1: Write the duplicate-helper boundary test**

Add to `tests/src/tests.rs`:

```rust
#[test]
fn fixtures_do_not_redefine_framework_helpers() {
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/fixtures");
    let mut source = String::new();
    for entry in std::fs::read_dir(&fixture_dir).expect("read fixtures dir") {
        let path = entry.expect("dir entry").path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            source.push_str(&std::fs::read_to_string(path).expect("read fixture file"));
        }
    }

    for forbidden in [
        "fn packed_hash_to_array",
        "fn sign_recoverable",
        "fn tx_without_message_hash",
        "fn tx_without_message_hash_for_inputs",
        "const TX_WITHOUT_MESSAGE_PERSONAL",
    ] {
        assert!(
            !source.contains(forbidden),
            "`{forbidden}` should be imported from tests::framework"
        );
    }
}
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```bash
cargo test -p tests fixtures_do_not_redefine_framework_helpers --offline
```

Expected: FAIL while fixtures still define duplicate signing/hash helpers.

- [ ] **Step 3: Replace duplicate helpers with framework imports**

Use these imports where needed:

```rust
use crate::framework::{
    scripts::packed_hash_to_array,
    signing::{sign_recoverable, tx_without_message_hash, tx_without_message_hash_for_inputs},
};
```

Use `crate::framework::signing::checked_len_prefix` in `fixtures/otx_hash.rs` and delete local `checked_len_prefix` if it only exists for hash length prefixes.
Update all `sign_recoverable` call sites from the fixture-local signature:

```rust
sign_recoverable(&secp, &secret_key, digest)
```

to the framework signature:

```rust
sign_recoverable(&secret_key, digest)
```

- [ ] **Step 4: Delete `tests/src/fixtures/hashing.rs` if empty**

If all functions in `tests/src/fixtures/hashing.rs` moved to framework, remove:

```rust
mod hashing;
```

from `tests/src/fixtures/mod.rs`, and delete `tests/src/fixtures/hashing.rs`.

- [ ] **Step 5: Run the boundary and cobuild-otx-lock tests**

Run:

```bash
cargo test -p tests fixtures_do_not_redefine_framework_helpers --offline
cargo test -p tests --test cobuild_otx_lock --offline
```

Expected: both PASS.

---

### Task 7: Keep OTX Hash Oracle Fixture-Local and Document the Boundary

**Files:**
- Modify: `tests/src/fixtures/otx_hash.rs`
- Modify: `tests/src/fixtures/support.rs`
- Test: `cargo test -p tests fixtures_live_in_dedicated_module_files --offline`

- [ ] **Step 1: Add a short boundary comment**

Add near the top of `tests/src/fixtures/otx_hash.rs`:

```rust
// This module is intentionally fixture-local: it mirrors the cobuild-otx-lock
// preimage hashing contract for lock verification tests. General signing and
// witness helpers belong in tests::framework instead.
```

- [ ] **Step 2: Rename scenario-local structs if needed**

If `OtxFixtureInput`, `OtxFixtureOutput`, or `OtxFixtureParts` remain in `tests/src/fixtures/support.rs`, keep them `pub(super)` and document that they are the lock-contract hash oracle input model, not general framework data.

Use this comment above `OtxFixtureParts`:

```rust
// Input model for the cobuild-otx-lock hash oracle. This stays out of
// framework because it encodes lock-verification preimage details.
```

- [ ] **Step 3: Run the fixture boundary test**

Run:

```bash
cargo test -p tests fixtures_live_in_dedicated_module_files --offline
```

Expected: PASS.

---

### Task 8: Final Regression and Cleanup

**Files:**
- All modified files
- Test: targeted and workspace commands

- [ ] **Step 1: Run targeted tests**

Run:

```bash
cargo test -p tests fixtures_live_in_dedicated_module_files --offline
cargo test -p tests cobuild_otx_lock_test_file_contains_no_fixture_helpers --offline
cargo test -p tests fixtures_do_not_redefine_framework_helpers --offline
cargo test -p tests framework --offline
cargo test -p tests --test cobuild_otx_lock --offline
cargo test -p tests --test limit_order --offline
```

Expected:

```text
All commands exit 0.
No warnings from unused imports.
```

- [ ] **Step 2: Run format and diff checks**

Run:

```bash
cargo fmt --check
git diff --check
```

Expected:

```text
Both commands exit 0.
```

- [ ] **Step 3: Inspect remaining helper placement**

Run:

```bash
rg -n "struct OtxFixtureInput|struct OtxFixtureOutput|fn create_udt_input|fn cell_input_for_output|fn signed_udt_transfer_otx|fn sign_recoverable|fn tx_without_message_hash_for_inputs" tests/tests/cobuild_otx_lock.rs tests/src/fixtures tests/src/framework
```

Expected:

```text
tests/tests/cobuild_otx_lock.rs has no matches.
tests/src/framework/signing.rs owns sign_recoverable and tx_without_message_hash_for_inputs.
tests/src/framework/cells.rs owns generic live input helpers.
tests/src/fixtures/cobuild_otx_lock.rs owns signed_udt_transfer_otx only if it remains scenario-specific.
tests/src/fixtures/support.rs owns OtxFixture* structs only if they feed the lock-contract hash oracle.
```

- [ ] **Step 4: Record red/green notes**

Add a concise note in the implementation summary, not necessarily in docs:

```text
Task 1 RED: boundary test failed before moving helpers out of tests/tests/cobuild_otx_lock.rs.
Task 2 RED/GREEN: signing helper test failed before framework::signing existed, passed after implementation.
Task 3 RED/GREEN: resolved input/contract helper test failed before framework helpers existed, passed after implementation.
Task 4 RED/GREEN: OTX witness helper test failed before helper exports existed, passed after implementation.
Task 5 RED/GREEN: UDT transfer tests failed after switching to fixture functions, passed after moving scenario implementation.
Task 6 RED/GREEN: duplicate-helper boundary test failed while fixtures redefined framework helpers, passed after imports.
Task 7 GREEN: fixture boundary test passed with hash oracle kept fixture-local.
```

---

## Self-Review

- The plan explicitly covers moving `OtxFixtureInput`, `OtxFixtureOutput`, `create_udt_input`, and `cell_input_for_output` out of `tests/tests/cobuild_otx_lock.rs`.
- It separates generic framework helpers from cobuild-otx-lock scenario fixtures.
- It avoids moving lock-contract hash oracle code wholesale into framework.
- It keeps Limit Order-specific code separate in `tests/src/framework/limit_order.rs`.
- It uses targeted red/green tests before implementation changes.
- It does not require changes to contracts or `cobuild-core` behavior.
