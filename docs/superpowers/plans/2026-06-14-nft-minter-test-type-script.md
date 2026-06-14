# NFT Minter Test Type Script Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build test-only `nft-minter-type` and `minted-nft-type` contracts that validate Cobuild-driven minter creation, counter-based NFT minting, rarity derivation, NFT self-checking creation, transfer, and burn.

**Architecture:** Add two small test contracts under `tests/contracts`. `nft-minter-type` owns counter state and binds `CreateMinter` / `MintNft` actions to expected outputs; `minted-nft-type` owns each NFT cell and self-validates creation against a minter counter transition. Integration tests use the existing `tests/src/framework` transaction, Cobuild message, OTX, and assertion helpers.

**Tech Stack:** Rust 2024, `ckb-std` 1.1, `cobuild-core`, `ckb-testtool`, existing repository Makefiles, fixed-width test ABI bytes.

---

## File Structure

Create:

- `tests/contracts/nft-minter-type/Cargo.toml` - contract manifest with `ckb-std`, `cobuild-core`, and type-id feature.
- `tests/contracts/nft-minter-type/Makefile` - copy of existing test contract Makefile without proxy-lock hash generation.
- `tests/contracts/nft-minter-type/README.md` - short test-only contract notes.
- `tests/contracts/nft-minter-type/src/main.rs` - `no_std` binary entry.
- `tests/contracts/nft-minter-type/src/lib.rs` - library entry for tests.
- `tests/contracts/nft-minter-type/src/error.rs` - stable exit codes.
- `tests/contracts/nft-minter-type/src/types.rs` - fixed-width minter state, NFT data, action parsing, rarity/hash helpers.
- `tests/contracts/nft-minter-type/src/entry.rs` - group-shape dispatch and Cobuild plan loading.
- `tests/contracts/nft-minter-type/src/validation.rs` - create and mint update validation.
- `tests/contracts/minted-nft-type/Cargo.toml` - contract manifest with `ckb-std`.
- `tests/contracts/minted-nft-type/Makefile` - copy of test contract Makefile.
- `tests/contracts/minted-nft-type/README.md` - short test-only contract notes.
- `tests/contracts/minted-nft-type/src/main.rs` - `no_std` binary entry.
- `tests/contracts/minted-nft-type/src/lib.rs` - library entry for tests.
- `tests/contracts/minted-nft-type/src/error.rs` - stable exit codes.
- `tests/contracts/minted-nft-type/src/types.rs` - NFT data, minter state reader, hash helpers.
- `tests/contracts/minted-nft-type/src/entry.rs` - creation, transfer, burn validation.
- `tests/src/fixtures/nft_minter/mod.rs` - fixture module exports.
- `tests/src/fixtures/nft_minter/state.rs` - host-side state/action encoders matching contract ABI.
- `tests/src/fixtures/nft_minter/scenarios.rs` - transaction scenario builders.
- `tests/src/fixtures/nft_minter/errors.rs` - expected exit outcome helpers.
- `tests/tests/nft_minter_type.rs` - integration test runner.

Modify:

- `Cargo.toml` - add both new test contract workspace members.
- `tests/src/fixtures/mod.rs` - export `nft_minter`.
- `tests/src/fixtures/common/contracts.rs` - deploy helpers for `nft-minter-type` and `minted-nft-type`.
- `tests/tests/workspace_layout.rs` - assert new test contracts live under `tests/contracts`.

Do not modify production contracts or `cobuild-types`.

### Task 1: Workspace And Contract Scaffolds

**Files:**
- Create: all files under `tests/contracts/nft-minter-type`
- Create: all files under `tests/contracts/minted-nft-type`
- Modify: `Cargo.toml`
- Modify: `tests/tests/workspace_layout.rs`

- [ ] **Step 1: Add failing workspace layout coverage**

Edit `tests/tests/workspace_layout.rs` so `workspace_declares_clean_cobuild_members` includes:

```rust
for member in [
    "\"xtask\"",
    "\"crates/cobuild-types\"",
    "\"crates/cobuild-core\"",
    "\"contracts/cobuild-otx-lock\"",
    "\"tests/contracts/test-nft\"",
    "\"tests/contracts/test-udt\"",
    "\"tests/contracts/nft-minter-type\"",
    "\"tests/contracts/minted-nft-type\"",
    "\"tests\"",
] {
    assert!(
        manifest.contains(member),
        "missing workspace member {member}"
    );
}
```

Edit `test_asset_contracts_live_under_tests_directory` to include:

```rust
for contract in ["test-udt", "test-nft", "nft-minter-type", "minted-nft-type"] {
    let test_contract_dir = workspace_root.join("tests/contracts").join(contract);
    assert!(
        test_contract_dir.join("Cargo.toml").is_file(),
        "missing test-only contract manifest for {contract}"
    );
    assert!(
        test_contract_dir.join("Makefile").is_file(),
        "missing test-only contract Makefile for {contract}"
    );
    assert!(
        !workspace_root.join("contracts").join(contract).exists(),
        "{contract} must stay under tests/contracts, not production contracts"
    );
}
```

- [ ] **Step 2: Run layout test and confirm failure**

Run:

```bash
cargo test -p tests --test workspace_layout --offline
```

Expected: FAIL, reporting missing workspace members and missing test-only contract manifests for `nft-minter-type` and `minted-nft-type`.

- [ ] **Step 3: Add workspace members**

Edit root `Cargo.toml` members:

```toml
  "tests/contracts/minted-nft-type",
  "tests/contracts/nft-minter-type",
```

Place them next to the other `tests/contracts/*` members.

- [ ] **Step 4: Create `nft-minter-type` scaffold**

Create `tests/contracts/nft-minter-type/Cargo.toml`:

```toml
[package]
name = "nft-minter-type"
version = "0.1.0"
edition = "2024"

[dependencies]
ckb-std = { version = "1.1", default-features = false, features = ["allocator", "calc-hash", "ckb-types", "dummy-atomic"] }
cobuild-core = { path = "../../../crates/cobuild-core" }

[features]
library = []
native-simulator = ["library", "ckb-std/native-simulator"]
type-id = ["ckb-std/type-id"]
```

Create `tests/contracts/nft-minter-type/src/lib.rs`:

```rust
#![cfg_attr(not(feature = "library"), no_std)]

extern crate alloc;

pub mod entry;
pub mod error;
pub mod types;
pub mod validation;
```

Create `tests/contracts/nft-minter-type/src/main.rs`:

```rust
#![cfg_attr(not(any(test, feature = "library")), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
ckb_std::entry!(program_entry);
#[cfg(not(test))]
ckb_std::default_alloc!();

#[cfg(not(test))]
fn program_entry() -> i8 {
    match nft_minter_type::entry::main() {
        Ok(()) => 0,
        Err(error) => error.into(),
    }
}
```

Create `tests/contracts/nft-minter-type/src/error.rs`:

```rust
use ckb_std::error::SysError;

#[repr(i8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Syscall = 5,
    InvalidArgs = 10,
    TypeId = 11,
    InvalidMinterData = 12,
    InvalidAction = 13,
    InvalidCobuild = 14,
    InvalidMintedNft = 15,
    Counter = 16,
    SupplyCap = 17,
    InvalidShape = 18,
}

impl From<SysError> for Error {
    fn from(error: SysError) -> Self {
        match error {
            SysError::TypeIDError => Self::TypeId,
            _ => Self::Syscall,
        }
    }
}

impl From<cobuild_core::error::CoreError> for Error {
    fn from(_: cobuild_core::error::CoreError) -> Self {
        Self::InvalidCobuild
    }
}

impl From<Error> for i8 {
    fn from(error: Error) -> Self {
        error as i8
    }
}
```

Create minimal compileable files:

```rust
// tests/contracts/nft-minter-type/src/types.rs
pub const MINTER_DATA_LEN: usize = 16;
```

```rust
// tests/contracts/nft-minter-type/src/validation.rs
```

```rust
// tests/contracts/nft-minter-type/src/entry.rs
use crate::error::Error;

pub fn main() -> Result<(), Error> {
    Ok(())
}
```

Create `tests/contracts/nft-minter-type/README.md`:

```markdown
# nft-minter-type

Test-only Cobuild type script for counter-based NFT minting.
```

Copy `tests/contracts/test-nft/Makefile` to `tests/contracts/nft-minter-type/Makefile`.

- [ ] **Step 5: Create `minted-nft-type` scaffold**

Create `tests/contracts/minted-nft-type/Cargo.toml`:

```toml
[package]
name = "minted-nft-type"
version = "0.1.0"
edition = "2024"

[dependencies]
ckb-std = { version = "1.1", default-features = false, features = ["allocator", "calc-hash", "ckb-types", "dummy-atomic"] }

[features]
library = []
native-simulator = ["library", "ckb-std/native-simulator"]
```

Create `tests/contracts/minted-nft-type/src/lib.rs`:

```rust
#![cfg_attr(not(feature = "library"), no_std)]

extern crate alloc;

pub mod entry;
pub mod error;
pub mod types;
```

Create `tests/contracts/minted-nft-type/src/main.rs`:

```rust
#![cfg_attr(not(any(test, feature = "library")), no_std)]
#![cfg_attr(not(test), no_main)]

#[cfg(not(test))]
ckb_std::entry!(program_entry);
#[cfg(not(test))]
ckb_std::default_alloc!();

#[cfg(not(test))]
fn program_entry() -> i8 {
    match minted_nft_type::entry::main() {
        Ok(()) => 0,
        Err(error) => error.into(),
    }
}
```

Create `tests/contracts/minted-nft-type/src/error.rs`:

```rust
use ckb_std::error::SysError;

#[repr(i8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    Syscall = 5,
    InvalidArgs = 10,
    InvalidNftData = 11,
    InvalidMinterTransition = 12,
    InvalidShape = 13,
}

impl From<SysError> for Error {
    fn from(_: SysError) -> Self {
        Self::Syscall
    }
}

impl From<Error> for i8 {
    fn from(error: Error) -> Self {
        error as i8
    }
}
```

Create minimal compileable files:

```rust
// tests/contracts/minted-nft-type/src/types.rs
pub const NFT_DATA_LEN: usize = 73;
```

```rust
// tests/contracts/minted-nft-type/src/entry.rs
use crate::error::Error;

pub fn main() -> Result<(), Error> {
    Ok(())
}
```

Create `tests/contracts/minted-nft-type/README.md`:

```markdown
# minted-nft-type

Test-only NFT type script for cells minted by `nft-minter-type`.
```

Copy `tests/contracts/test-nft/Makefile` to `tests/contracts/minted-nft-type/Makefile`.

- [ ] **Step 6: Run layout and compile checks**

Run:

```bash
cargo test -p tests --test workspace_layout --offline
cargo check -p nft-minter-type --features library --offline
cargo check -p minted-nft-type --features library --offline
```

Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add Cargo.toml tests/tests/workspace_layout.rs tests/contracts/nft-minter-type tests/contracts/minted-nft-type
git commit -m "test: scaffold nft minter contracts"
```

### Task 2: Fixed-Width ABI And Pure Helpers

**Files:**
- Modify: `tests/contracts/nft-minter-type/src/types.rs`
- Modify: `tests/contracts/minted-nft-type/src/types.rs`
- Test: unit tests inside those files

- [ ] **Step 1: Add failing minter helper tests**

Add to `tests/contracts/nft-minter-type/src/types.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rarity_treats_zero_as_rare3() {
        assert_eq!(rarity_for_serial(0), 3);
        assert_eq!(rarity_for_serial(6), 0);
        assert_eq!(rarity_for_serial(7), 1);
        assert_eq!(rarity_for_serial(11), 2);
        assert_eq!(rarity_for_serial(77), 3);
    }

    #[test]
    fn minter_state_round_trips() {
        let state = MinterState {
            mint_counter: 6,
            supply_cap: 100,
        };
        assert_eq!(parse_minter_state(&encode_minter_state(state)), Ok(state));
    }

    #[test]
    fn actions_parse_create_and_mint() {
        assert_eq!(
            parse_action(&create_minter_action_data(10)),
            Ok(NftMinterAction::CreateMinter { supply_cap: 10 })
        );
        assert_eq!(
            parse_action(&mint_nft_action_data([7; 32])),
            Ok(NftMinterAction::MintNft {
                metadata_seed: [7; 32]
            })
        );
    }
}
```

- [ ] **Step 2: Run tests and confirm failure**

Run:

```bash
cargo test -p nft-minter-type --features library --offline types::tests -- --nocapture
```

Expected: FAIL because `MinterState`, `rarity_for_serial`, and action helpers are undefined.

- [ ] **Step 3: Implement minter helpers**

Replace `tests/contracts/nft-minter-type/src/types.rs` with:

```rust
use ckb_std::ckb_types::util::hash::blake2b_256;

use crate::error::Error;

pub const MINTER_DATA_LEN: usize = 16;
pub const NFT_DATA_LEN: usize = 73;
pub const CREATE_MINTER_TAG: u8 = 1;
pub const MINT_NFT_TAG: u8 = 2;
pub const CREATE_ACTION_LEN: usize = 1 + 8;
pub const MINT_ACTION_LEN: usize = 1 + 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MinterState {
    pub mint_counter: u64,
    pub supply_cap: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintedNftData {
    pub minter_type_hash: [u8; 32],
    pub serial: u64,
    pub rarity: u8,
    pub attributes_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NftMinterAction {
    CreateMinter { supply_cap: u64 },
    MintNft { metadata_seed: [u8; 32] },
}

pub fn parse_minter_state(data: &[u8]) -> Result<MinterState, Error> {
    if data.len() != MINTER_DATA_LEN {
        return Err(Error::InvalidMinterData);
    }
    let mut counter = [0u8; 8];
    counter.copy_from_slice(&data[0..8]);
    let mut cap = [0u8; 8];
    cap.copy_from_slice(&data[8..16]);
    Ok(MinterState {
        mint_counter: u64::from_le_bytes(counter),
        supply_cap: u64::from_le_bytes(cap),
    })
}

pub fn encode_minter_state(state: MinterState) -> [u8; MINTER_DATA_LEN] {
    let mut out = [0u8; MINTER_DATA_LEN];
    out[0..8].copy_from_slice(&state.mint_counter.to_le_bytes());
    out[8..16].copy_from_slice(&state.supply_cap.to_le_bytes());
    out
}

pub fn parse_minted_nft_data(data: &[u8]) -> Result<MintedNftData, Error> {
    if data.len() != NFT_DATA_LEN {
        return Err(Error::InvalidMintedNft);
    }
    let mut minter_type_hash = [0u8; 32];
    minter_type_hash.copy_from_slice(&data[0..32]);
    let mut serial = [0u8; 8];
    serial.copy_from_slice(&data[32..40]);
    let rarity = data[40];
    let mut attributes_hash = [0u8; 32];
    attributes_hash.copy_from_slice(&data[41..73]);
    Ok(MintedNftData {
        minter_type_hash,
        serial: u64::from_le_bytes(serial),
        rarity,
        attributes_hash,
    })
}

pub fn encode_minted_nft_data(data: MintedNftData) -> [u8; NFT_DATA_LEN] {
    let mut out = [0u8; NFT_DATA_LEN];
    out[0..32].copy_from_slice(&data.minter_type_hash);
    out[32..40].copy_from_slice(&data.serial.to_le_bytes());
    out[40] = data.rarity;
    out[41..73].copy_from_slice(&data.attributes_hash);
    out
}

pub fn parse_action(data: &[u8]) -> Result<NftMinterAction, Error> {
    match data.first().copied() {
        Some(CREATE_MINTER_TAG) if data.len() == CREATE_ACTION_LEN => {
            let mut cap = [0u8; 8];
            cap.copy_from_slice(&data[1..9]);
            Ok(NftMinterAction::CreateMinter {
                supply_cap: u64::from_le_bytes(cap),
            })
        }
        Some(MINT_NFT_TAG) if data.len() == MINT_ACTION_LEN => {
            let mut metadata_seed = [0u8; 32];
            metadata_seed.copy_from_slice(&data[1..33]);
            Ok(NftMinterAction::MintNft { metadata_seed })
        }
        _ => Err(Error::InvalidAction),
    }
}

pub fn create_minter_action_data(supply_cap: u64) -> [u8; CREATE_ACTION_LEN] {
    let mut out = [0u8; CREATE_ACTION_LEN];
    out[0] = CREATE_MINTER_TAG;
    out[1..9].copy_from_slice(&supply_cap.to_le_bytes());
    out
}

pub fn mint_nft_action_data(metadata_seed: [u8; 32]) -> [u8; MINT_ACTION_LEN] {
    let mut out = [0u8; MINT_ACTION_LEN];
    out[0] = MINT_NFT_TAG;
    out[1..33].copy_from_slice(&metadata_seed);
    out
}

pub fn rarity_for_serial(serial: u64) -> u8 {
    if serial % 77 == 0 {
        3
    } else if serial % 11 == 0 {
        2
    } else if serial % 7 == 0 {
        1
    } else {
        0
    }
}

pub fn nft_id(minter_type_hash: [u8; 32], serial: u64) -> [u8; 32] {
    let mut input = [0u8; 40];
    input[0..32].copy_from_slice(&minter_type_hash);
    input[32..40].copy_from_slice(&serial.to_le_bytes());
    blake2b_256(input)
}

pub fn attributes_hash(
    minter_type_hash: [u8; 32],
    serial: u64,
    rarity: u8,
    metadata_seed: [u8; 32],
) -> [u8; 32] {
    let mut input = [0u8; 73];
    input[0..32].copy_from_slice(&minter_type_hash);
    input[32..40].copy_from_slice(&serial.to_le_bytes());
    input[40] = rarity;
    input[41..73].copy_from_slice(&metadata_seed);
    blake2b_256(input)
}
```

Keep the tests from Step 1 at the bottom of the file.

- [ ] **Step 4: Add minted NFT helper tests**

Add to `tests/contracts/minted-nft-type/src/types.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nft_data_round_trips() {
        let data = MintedNftData {
            minter_type_hash: [1; 32],
            serial: 77,
            rarity: 3,
            attributes_hash: [2; 32],
        };
        assert_eq!(parse_nft_data(&encode_nft_data(data)), Ok(data));
    }

    #[test]
    fn nft_id_uses_minter_hash_and_serial() {
        assert_eq!(nft_id([1; 32], 7), nft_id([1; 32], 7));
        assert_ne!(nft_id([1; 32], 7), nft_id([1; 32], 8));
    }
}
```

- [ ] **Step 5: Implement minted NFT helpers**

Replace `tests/contracts/minted-nft-type/src/types.rs` with:

```rust
use ckb_std::ckb_types::util::hash::blake2b_256;

use crate::error::Error;

pub const NFT_DATA_LEN: usize = 73;
pub const MINTER_DATA_LEN: usize = 16;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintedNftData {
    pub minter_type_hash: [u8; 32],
    pub serial: u64,
    pub rarity: u8,
    pub attributes_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MinterState {
    pub mint_counter: u64,
    pub supply_cap: u64,
}

pub fn parse_nft_data(data: &[u8]) -> Result<MintedNftData, Error> {
    if data.len() != NFT_DATA_LEN {
        return Err(Error::InvalidNftData);
    }
    let mut minter_type_hash = [0u8; 32];
    minter_type_hash.copy_from_slice(&data[0..32]);
    let mut serial = [0u8; 8];
    serial.copy_from_slice(&data[32..40]);
    let rarity = data[40];
    let mut attributes_hash = [0u8; 32];
    attributes_hash.copy_from_slice(&data[41..73]);
    Ok(MintedNftData {
        minter_type_hash,
        serial: u64::from_le_bytes(serial),
        rarity,
        attributes_hash,
    })
}

pub fn encode_nft_data(data: MintedNftData) -> [u8; NFT_DATA_LEN] {
    let mut out = [0u8; NFT_DATA_LEN];
    out[0..32].copy_from_slice(&data.minter_type_hash);
    out[32..40].copy_from_slice(&data.serial.to_le_bytes());
    out[40] = data.rarity;
    out[41..73].copy_from_slice(&data.attributes_hash);
    out
}

pub fn parse_minter_state(data: &[u8]) -> Result<MinterState, Error> {
    if data.len() != MINTER_DATA_LEN {
        return Err(Error::InvalidMinterTransition);
    }
    let mut counter = [0u8; 8];
    counter.copy_from_slice(&data[0..8]);
    let mut cap = [0u8; 8];
    cap.copy_from_slice(&data[8..16]);
    Ok(MinterState {
        mint_counter: u64::from_le_bytes(counter),
        supply_cap: u64::from_le_bytes(cap),
    })
}

pub fn nft_id(minter_type_hash: [u8; 32], serial: u64) -> [u8; 32] {
    let mut input = [0u8; 40];
    input[0..32].copy_from_slice(&minter_type_hash);
    input[32..40].copy_from_slice(&serial.to_le_bytes());
    blake2b_256(input)
}
```

Keep the tests from Step 4 at the bottom.

- [ ] **Step 6: Run helper tests**

Run:

```bash
cargo test -p nft-minter-type --features library --offline types::tests -- --nocapture
cargo test -p minted-nft-type --features library --offline types::tests -- --nocapture
```

Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add tests/contracts/nft-minter-type/src/types.rs tests/contracts/minted-nft-type/src/types.rs
git commit -m "test: add nft minter abi helpers"
```

### Task 3: Minter Create Validation

**Files:**
- Modify: `tests/contracts/nft-minter-type/src/entry.rs`
- Modify: `tests/contracts/nft-minter-type/src/validation.rs`
- Test: unit tests in both files

- [ ] **Step 1: Add failing create validation unit tests**

Add to `tests/contracts/nft-minter-type/src/validation.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{MinterState, NftMinterAction};

    #[test]
    fn create_requires_zero_counter_and_matching_cap() {
        let state = MinterState {
            mint_counter: 0,
            supply_cap: 10,
        };
        let action = NftMinterAction::CreateMinter { supply_cap: 10 };

        assert_eq!(validate_create_state(state, action), Ok(()));
    }

    #[test]
    fn create_rejects_non_zero_counter_or_cap_mismatch() {
        assert_eq!(
            validate_create_state(
                MinterState {
                    mint_counter: 1,
                    supply_cap: 10,
                },
                NftMinterAction::CreateMinter { supply_cap: 10 },
            ),
            Err(Error::Counter)
        );
        assert_eq!(
            validate_create_state(
                MinterState {
                    mint_counter: 0,
                    supply_cap: 9,
                },
                NftMinterAction::CreateMinter { supply_cap: 10 },
            ),
            Err(Error::SupplyCap)
        );
    }
}
```

- [ ] **Step 2: Run and confirm failure**

Run:

```bash
cargo test -p nft-minter-type --features library --offline validation::tests -- --nocapture
```

Expected: FAIL because `validate_create_state` is undefined.

- [ ] **Step 3: Implement create validation helpers and entry shape**

Replace `tests/contracts/nft-minter-type/src/validation.rs` with:

```rust
use ckb_std::{
    ckb_constants::Source,
    high_level::{QueryIter, load_cell_data},
};
use cobuild_core::{plan::TypeValidationPlan, reader::cursor_bytes};

use crate::{
    error::Error,
    types::{MinterState, NftMinterAction, parse_action, parse_minter_state},
};

pub fn validate_create(plan: &TypeValidationPlan) -> Result<(), Error> {
    crate::entry::validate_minter_type_id()?;
    let output = single_group_state(Source::GroupOutput)?;
    let action = single_action(plan)?;
    validate_create_state(output, action)
}

pub fn validate_create_state(
    output: MinterState,
    action: NftMinterAction,
) -> Result<(), Error> {
    let NftMinterAction::CreateMinter { supply_cap } = action else {
        return Err(Error::InvalidAction);
    };
    if output.mint_counter != 0 {
        return Err(Error::Counter);
    }
    if output.supply_cap != supply_cap {
        return Err(Error::SupplyCap);
    }
    Ok(())
}

pub fn single_action(plan: &TypeValidationPlan) -> Result<NftMinterAction, Error> {
    if plan.related_actions.len() != 1 {
        return Err(Error::InvalidCobuild);
    }
    let action_data = cursor_bytes(&plan.related_actions[0].action.action.data)?;
    parse_action(&action_data)
}

pub fn single_group_state(source: Source) -> Result<MinterState, Error> {
    let mut cells = QueryIter::new(load_cell_data, source);
    let Some(data) = cells.next() else {
        return Err(Error::InvalidMinterData);
    };
    if cells.next().is_some() {
        return Err(Error::InvalidShape);
    }
    parse_minter_state(&data)
}
```

Replace `tests/contracts/nft-minter-type/src/entry.rs` with:

```rust
#[cfg(not(feature = "type-id"))]
use ckb_std::high_level::load_script;
#[cfg(feature = "type-id")]
use ckb_std::type_id::check_type_id;
use ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, prelude::*},
    high_level::{QueryIter, load_cell_data, load_script_hash},
};
use cobuild_core::{context::CurrentScript, engine::CobuildContext};

use crate::error::Error;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MinterMode {
    Create,
    Mint,
    Burn,
}

pub fn minter_mode(input_count: usize, output_count: usize) -> Result<MinterMode, Error> {
    match (input_count, output_count) {
        (0, 1) => Ok(MinterMode::Create),
        (1, 1) => Ok(MinterMode::Mint),
        (1, 0) => Ok(MinterMode::Burn),
        _ => Err(Error::InvalidShape),
    }
}

pub fn main() -> Result<(), Error> {
    let current_type_hash = load_script_hash()?;
    let context = CobuildContext::build(CurrentScript::Type(current_type_hash))?;
    let plan = context.plan_type_validation()?;

    let input_count = QueryIter::new(load_cell_data, Source::GroupInput).count();
    let output_count = QueryIter::new(load_cell_data, Source::GroupOutput).count();

    match minter_mode(input_count, output_count)? {
        MinterMode::Create => crate::validation::validate_create(&plan),
        MinterMode::Mint => Err(Error::InvalidAction),
        MinterMode::Burn => Err(Error::InvalidShape),
    }
}

#[cfg(feature = "type-id")]
pub(crate) fn validate_minter_type_id() -> Result<(), Error> {
    check_type_id(0, 32).map_err(Error::from)
}

#[cfg(not(feature = "type-id"))]
pub(crate) fn validate_minter_type_id() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();
    if args.len() != 32 {
        return Err(Error::InvalidArgs);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minter_mode_detects_create_mint_and_burn() {
        assert_eq!(minter_mode(0, 1), Ok(MinterMode::Create));
        assert_eq!(minter_mode(1, 1), Ok(MinterMode::Mint));
        assert_eq!(minter_mode(1, 0), Ok(MinterMode::Burn));
        assert_eq!(minter_mode(0, 0), Err(Error::InvalidShape));
        assert_eq!(minter_mode(2, 1), Err(Error::InvalidShape));
    }
}
```

- [ ] **Step 4: Run create validation unit tests**

Run:

```bash
cargo test -p nft-minter-type --features library --offline entry::tests -- --nocapture
cargo test -p nft-minter-type --features library --offline validation::tests -- --nocapture
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/contracts/nft-minter-type/src/entry.rs tests/contracts/nft-minter-type/src/validation.rs
git commit -m "test: validate nft minter creation"
```

### Task 4: Minter Mint Update Validation

**Files:**
- Modify: `tests/contracts/nft-minter-type/src/entry.rs`
- Modify: `tests/contracts/nft-minter-type/src/validation.rs`
- Test: unit tests in `validation.rs`

- [ ] **Step 1: Add failing pure mint transition tests**

Append to `tests/contracts/nft-minter-type/src/validation.rs` tests:

```rust
#[test]
fn mint_transition_requires_counter_increment_and_fixed_cap() {
    let input = MinterState {
        mint_counter: 6,
        supply_cap: 10,
    };
    let output = MinterState {
        mint_counter: 8,
        supply_cap: 10,
    };
    assert_eq!(validate_mint_state(input, output, 2), Ok(()));
}

#[test]
fn mint_transition_rejects_wrong_counter_cap_and_over_cap() {
    assert_eq!(
        validate_mint_state(
            MinterState {
                mint_counter: 6,
                supply_cap: 10,
            },
            MinterState {
                mint_counter: 7,
                supply_cap: 10,
            },
            2,
        ),
        Err(Error::Counter)
    );
    assert_eq!(
        validate_mint_state(
            MinterState {
                mint_counter: 6,
                supply_cap: 10,
            },
            MinterState {
                mint_counter: 8,
                supply_cap: 11,
            },
            2,
        ),
        Err(Error::SupplyCap)
    );
    assert_eq!(
        validate_mint_state(
            MinterState {
                mint_counter: 9,
                supply_cap: 10,
            },
            MinterState {
                mint_counter: 11,
                supply_cap: 10,
            },
            2,
        ),
        Err(Error::SupplyCap)
    );
}
```

- [ ] **Step 2: Run and confirm failure**

Run:

```bash
cargo test -p nft-minter-type --features library --offline validation::tests -- --nocapture
```

Expected: FAIL because `validate_mint_state` is undefined.

- [ ] **Step 3: Implement mint state helper**

Add to `tests/contracts/nft-minter-type/src/validation.rs`:

```rust
pub fn validate_mint_state(
    input: MinterState,
    output: MinterState,
    mint_action_count: usize,
) -> Result<(), Error> {
    if input.supply_cap != output.supply_cap {
        return Err(Error::SupplyCap);
    }
    let increment: u64 = mint_action_count.try_into().map_err(|_| Error::Counter)?;
    let expected = input
        .mint_counter
        .checked_add(increment)
        .ok_or(Error::Counter)?;
    if output.mint_counter != expected {
        return Err(Error::Counter);
    }
    if output.mint_counter > output.supply_cap {
        return Err(Error::SupplyCap);
    }
    Ok(())
}
```

- [ ] **Step 4: Implement action sorting and mint action extraction**

Add to `tests/contracts/nft-minter-type/src/validation.rs`:

```rust
use alloc::vec::Vec;
use cobuild_core::plan::ActionOrigin;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintActionFact {
    pub witness_index: usize,
    pub action_index: usize,
    pub metadata_seed: [u8; 32],
}

pub fn mint_actions(plan: &TypeValidationPlan) -> Result<Vec<MintActionFact>, Error> {
    let mut facts = Vec::new();
    for related in &plan.related_actions {
        let action_data = cursor_bytes(&related.action.action.data)?;
        let NftMinterAction::MintNft { metadata_seed } = parse_action(&action_data)? else {
            return Err(Error::InvalidAction);
        };
        let witness_index = match related.action.origin {
            ActionOrigin::TxLevel { witness_index } => witness_index,
            ActionOrigin::Otx { witness_index, .. } => witness_index,
        };
        facts.push(MintActionFact {
            witness_index,
            action_index: related.action.action.index,
            metadata_seed,
        });
    }
    facts.sort_by_key(|fact| (fact.witness_index, fact.action_index));
    Ok(facts)
}
```

Ensure `tests/contracts/nft-minter-type/src/lib.rs` keeps `extern crate alloc;`.

- [ ] **Step 5: Run unit tests**

Run:

```bash
cargo test -p nft-minter-type --features library --offline validation::tests -- --nocapture
cargo check -p nft-minter-type --features library --offline
```

Expected: all PASS.

- [ ] **Step 6: Commit**

```bash
git add tests/contracts/nft-minter-type/src/entry.rs tests/contracts/nft-minter-type/src/validation.rs
git commit -m "test: validate nft minter counter updates"
```

### Task 5: Minted NFT Type Validation

**Files:**
- Modify: `tests/contracts/minted-nft-type/src/entry.rs`
- Test: unit tests in `entry.rs`

- [ ] **Step 1: Add failing mode and pure helper tests**

Add to `tests/contracts/minted-nft-type/src/entry.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::MinterState;

    #[test]
    fn nft_mode_detects_creation_transfer_and_burn() {
        assert_eq!(nft_mode(0, 1), Ok(NftMode::Create));
        assert_eq!(nft_mode(1, 1), Ok(NftMode::Transfer));
        assert_eq!(nft_mode(1, 0), Ok(NftMode::Burn));
        assert_eq!(nft_mode(0, 0), Err(Error::InvalidShape));
        assert_eq!(nft_mode(2, 1), Err(Error::InvalidShape));
    }

    #[test]
    fn serial_range_requires_counter_increment_covering_serial() {
        assert_eq!(
            serial_is_minted(
                MinterState {
                    mint_counter: 6,
                    supply_cap: 10,
                },
                MinterState {
                    mint_counter: 8,
                    supply_cap: 10,
                },
                7,
            ),
            Ok(())
        );
        assert_eq!(
            serial_is_minted(
                MinterState {
                    mint_counter: 6,
                    supply_cap: 10,
                },
                MinterState {
                    mint_counter: 6,
                    supply_cap: 10,
                },
                6,
            ),
            Err(Error::InvalidMinterTransition)
        );
    }
}
```

- [ ] **Step 2: Run and confirm failure**

Run:

```bash
cargo test -p minted-nft-type --features library --offline entry::tests -- --nocapture
```

Expected: FAIL because `NftMode`, `nft_mode`, and `serial_is_minted` are undefined.

- [ ] **Step 3: Implement minted NFT entry**

Replace `tests/contracts/minted-nft-type/src/entry.rs` with:

```rust
use ckb_std::{
    ckb_constants::Source,
    ckb_types::{bytes::Bytes, prelude::*},
    high_level::{QueryIter, load_cell_data, load_cell_type_hash, load_script},
};

use crate::{
    error::Error,
    types::{MinterState, nft_id, parse_minter_state, parse_nft_data},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NftMode {
    Create,
    Transfer,
    Burn,
}

pub fn nft_mode(input_count: usize, output_count: usize) -> Result<NftMode, Error> {
    match (input_count, output_count) {
        (0, 1) => Ok(NftMode::Create),
        (1, 1) => Ok(NftMode::Transfer),
        (1, 0) => Ok(NftMode::Burn),
        _ => Err(Error::InvalidShape),
    }
}

pub fn main() -> Result<(), Error> {
    validate_args_len()?;
    let input_count = QueryIter::new(load_cell_data, Source::GroupInput).count();
    let output_count = QueryIter::new(load_cell_data, Source::GroupOutput).count();
    match nft_mode(input_count, output_count)? {
        NftMode::Create => validate_create(),
        NftMode::Transfer => validate_transfer(),
        NftMode::Burn => Ok(()),
    }
}

fn validate_args_len() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();
    if args.len() != 32 {
        return Err(Error::InvalidArgs);
    }
    Ok(())
}

fn validate_create() -> Result<(), Error> {
    let script = load_script()?;
    let args: Bytes = script.args().unpack();
    let output = single_group_data(Source::GroupOutput)?;
    let nft = parse_nft_data(&output)?;
    if args.as_ref() != nft_id(nft.minter_type_hash, nft.serial) {
        return Err(Error::InvalidArgs);
    }
    let (input, output) = find_minter_transition(nft.minter_type_hash)?;
    serial_is_minted(input, output, nft.serial)
}

fn validate_transfer() -> Result<(), Error> {
    let input = single_group_data(Source::GroupInput)?;
    let output = single_group_data(Source::GroupOutput)?;
    parse_nft_data(&input)?;
    parse_nft_data(&output)?;
    if input != output {
        return Err(Error::InvalidNftData);
    }
    Ok(())
}

fn single_group_data(source: Source) -> Result<Bytes, Error> {
    let mut cells = QueryIter::new(load_cell_data, source);
    let Some(data) = cells.next() else {
        return Err(Error::InvalidNftData);
    };
    if cells.next().is_some() {
        return Err(Error::InvalidShape);
    }
    Ok(data.into())
}

fn find_minter_transition(
    minter_type_hash: [u8; 32],
) -> Result<(MinterState, MinterState), Error> {
    let input = find_one_minter_state(Source::Input, minter_type_hash)?;
    let output = find_one_minter_state(Source::Output, minter_type_hash)?;
    Ok((input, output))
}

fn find_one_minter_state(source: Source, minter_type_hash: [u8; 32]) -> Result<MinterState, Error> {
    let mut found = None;
    for (index, type_hash) in QueryIter::new(load_cell_type_hash, source).enumerate() {
        if type_hash != Some(minter_type_hash) {
            continue;
        }
        if found.is_some() {
            return Err(Error::InvalidMinterTransition);
        }
        let data = load_cell_data(index, source)?;
        found = Some(parse_minter_state(&data)?);
    }
    found.ok_or(Error::InvalidMinterTransition)
}

pub fn serial_is_minted(
    input: MinterState,
    output: MinterState,
    serial: u64,
) -> Result<(), Error> {
    if input.supply_cap != output.supply_cap {
        return Err(Error::InvalidMinterTransition);
    }
    if output.mint_counter <= input.mint_counter {
        return Err(Error::InvalidMinterTransition);
    }
    if serial < input.mint_counter || serial >= output.mint_counter {
        return Err(Error::InvalidMinterTransition);
    }
    Ok(())
}
```

- [ ] **Step 4: Run unit tests and check**

Run:

```bash
cargo test -p minted-nft-type --features library --offline entry::tests -- --nocapture
cargo check -p minted-nft-type --features library --offline
```

Expected: all PASS.

- [ ] **Step 5: Commit**

```bash
git add tests/contracts/minted-nft-type/src/entry.rs
git commit -m "test: validate minted nft lifecycle"
```

### Task 6: Host Fixture Module And Deploy Helpers

**Files:**
- Modify: `tests/src/fixtures/mod.rs`
- Modify: `tests/src/fixtures/common/contracts.rs`
- Create: `tests/src/fixtures/nft_minter/mod.rs`
- Create: `tests/src/fixtures/nft_minter/state.rs`
- Create: `tests/src/fixtures/nft_minter/errors.rs`

- [ ] **Step 1: Add fixture module and deploy helpers**

Edit `tests/src/fixtures/mod.rs`:

```rust
pub mod cobuild_otx_lock;
pub mod common;
pub mod limit_order;
pub mod nft_minter;
```

Add to `tests/src/fixtures/common/contracts.rs`:

```rust
pub fn deploy_nft_minter_type(context: &mut Context, args: Vec<u8>) -> DeployedScript {
    deploy_loader_binary(context, "nft-minter-type", ScriptHashType::Data2, args)
}

pub fn deploy_minted_nft_type(context: &mut Context, nft_id: [u8; 32]) -> DeployedScript {
    deploy_loader_binary(context, "minted-nft-type", ScriptHashType::Data2, nft_id.to_vec())
}
```

- [ ] **Step 2: Add host-side state encoders**

Create `tests/src/fixtures/nft_minter/mod.rs`:

```rust
pub mod errors;
pub mod scenarios;
pub mod state;
```

Create `tests/src/fixtures/nft_minter/state.rs`:

```rust
use ckb_hash::blake2b_256;

pub const CREATE_MINTER_TAG: u8 = 1;
pub const MINT_NFT_TAG: u8 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MinterState {
    pub mint_counter: u64,
    pub supply_cap: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MintedNftData {
    pub minter_type_hash: [u8; 32],
    pub serial: u64,
    pub rarity: u8,
    pub attributes_hash: [u8; 32],
}

pub fn minter_data(state: MinterState) -> Vec<u8> {
    let mut out = Vec::with_capacity(16);
    out.extend_from_slice(&state.mint_counter.to_le_bytes());
    out.extend_from_slice(&state.supply_cap.to_le_bytes());
    out
}

pub fn minted_nft_data(data: MintedNftData) -> Vec<u8> {
    let mut out = Vec::with_capacity(73);
    out.extend_from_slice(&data.minter_type_hash);
    out.extend_from_slice(&data.serial.to_le_bytes());
    out.push(data.rarity);
    out.extend_from_slice(&data.attributes_hash);
    out
}

pub fn create_minter_action_data(supply_cap: u64) -> Vec<u8> {
    let mut out = Vec::with_capacity(9);
    out.push(CREATE_MINTER_TAG);
    out.extend_from_slice(&supply_cap.to_le_bytes());
    out
}

pub fn mint_nft_action_data(metadata_seed: [u8; 32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(33);
    out.push(MINT_NFT_TAG);
    out.extend_from_slice(&metadata_seed);
    out
}

pub fn rarity_for_serial(serial: u64) -> u8 {
    if serial % 77 == 0 {
        3
    } else if serial % 11 == 0 {
        2
    } else if serial % 7 == 0 {
        1
    } else {
        0
    }
}

pub fn nft_id(minter_type_hash: [u8; 32], serial: u64) -> [u8; 32] {
    let mut input = [0u8; 40];
    input[0..32].copy_from_slice(&minter_type_hash);
    input[32..40].copy_from_slice(&serial.to_le_bytes());
    blake2b_256(input)
}

pub fn attributes_hash(
    minter_type_hash: [u8; 32],
    serial: u64,
    rarity: u8,
    metadata_seed: [u8; 32],
) -> [u8; 32] {
    let mut input = [0u8; 73];
    input[0..32].copy_from_slice(&minter_type_hash);
    input[32..40].copy_from_slice(&serial.to_le_bytes());
    input[40] = rarity;
    input[41..73].copy_from_slice(&metadata_seed);
    blake2b_256(input)
}
```

Create `tests/src/fixtures/nft_minter/errors.rs`:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NftMinterExpected {
    Pass,
    MinterType(i8),
    MintedNftType(i8),
}
```

- [ ] **Step 3: Run fixture compile**

Run:

```bash
cargo check -p tests --offline
```

Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add tests/src/fixtures/mod.rs tests/src/fixtures/common/contracts.rs tests/src/fixtures/nft_minter
git commit -m "test: add nft minter fixture helpers"
```

### Task 7: Integration Happy Paths And Minter Output Binding

**Files:**
- Create: `tests/src/fixtures/nft_minter/scenarios.rs`
- Create: `tests/tests/nft_minter_type.rs`
- Modify: `tests/contracts/nft-minter-type/src/validation.rs`

- [ ] **Step 1: Add failing integration test runner**

Create `tests/tests/nft_minter_type.rs`:

```rust
use tests::fixtures::nft_minter::scenarios::{
    create_minter_case, mint_first_nft_case, mint_from_counter_six_case,
};

#[test]
fn nft_minter_happy_paths_pass() {
    for case in [
        create_minter_case(),
        mint_first_nft_case(),
        mint_from_counter_six_case(),
    ] {
        case.assert_expected();
    }
}
```

- [ ] **Step 2: Build contracts and run test to confirm failure**

Run:

```bash
make build MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test nft_minter_type -- --nocapture
```

Expected: FAIL because `tests::fixtures::nft_minter::scenarios` and the imported case builders do not exist yet.

- [ ] **Step 3: Implement scenario builders**

Replace `tests/src/fixtures/nft_minter/scenarios.rs` with concrete builders using existing framework primitives:

```rust
use ckb_testtool::ckb_types::{
    bytes::Bytes,
    core::ScriptHashType,
    packed::{CellOutput, Script},
    prelude::*,
};

use crate::{
    fixtures::{
        common::contracts::{
            deploy_always_success, deploy_minted_nft_type, deploy_nft_minter_type,
            rebuild_data2_script,
        },
        nft_minter::state::{
            MinterState, MintedNftData, attributes_hash, create_minter_action_data, minter_data,
            mint_nft_action_data, minted_nft_data, nft_id, rarity_for_serial,
        },
    },
    framework::{
        cells::{TestCellOutput, live_resolved_facts, typed_output},
        cobuild::{ActionRole, CobuildMessageBuilder},
        fixture::CobuildTestFixture,
        scripts::script_hash,
        tx::TxShape,
    },
};

pub struct NftMinterCase {
    pub name: &'static str,
    pub fixture: CobuildTestFixture,
    pub tx: ckb_testtool::ckb_types::core::TransactionView,
}

impl NftMinterCase {
    pub fn assert_expected(&self) {
        self.fixture.assert_pass(&self.tx);
    }
}

pub fn create_minter_case() -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_code.script);
    let output = TestCellOutput::new(
        typed_output(lock.script.clone(), minter_code.script.clone(), 200_000_000_000),
        minter_data(MinterState {
            mint_counter: 0,
            supply_cap: 10,
        }),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    shape.push_remainder_output(output);
    shape.tx_level_message(
        CobuildMessageBuilder::new()
            .output_type_action(minter_hash)
            .action_data(create_minter_action_data(10))
            .build(),
    );
    let built = shape.build();
    let tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase {
        name: "create_minter",
        fixture,
        tx,
    }
}

pub fn mint_first_nft_case() -> NftMinterCase {
    mint_from_counter_case("mint_first_nft", 0, 1, [9u8; 32])
}

pub fn mint_from_counter_six_case() -> NftMinterCase {
    mint_from_counter_case("mint_from_counter_six", 6, 7, [6u8; 32])
}

fn mint_from_counter_case(
    name: &'static str,
    old_counter: u64,
    new_counter: u64,
    seed: [u8; 32],
) -> NftMinterCase {
    let mut fixture = CobuildTestFixture::new();
    let lock = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let minter_code = deploy_nft_minter_type(fixture.context_mut(), [1u8; 32].to_vec());
    let minter_hash = script_hash(&minter_code.script);
    let serial = old_counter;
    let rarity = rarity_for_serial(serial);
    let nft_id = nft_id(minter_hash, serial);
    let nft_code = deploy_minted_nft_type(fixture.context_mut(), nft_id);
    let nft_data = minted_nft_data(MintedNftData {
        minter_type_hash: minter_hash,
        serial,
        rarity,
        attributes_hash: attributes_hash(minter_hash, serial, rarity, seed),
    });
    let minter_input_output = typed_output(
        lock.script.clone(),
        minter_code.script.clone(),
        200_000_000_000,
    );
    let minter_input = live_resolved_facts(
        fixture.context_mut(),
        minter_input_output.clone(),
        minter_data(MinterState {
            mint_counter: old_counter,
            supply_cap: 100,
        }),
    );
    let minter_output = TestCellOutput::new(
        minter_input_output,
        minter_data(MinterState {
            mint_counter: new_counter,
            supply_cap: 100,
        }),
    );
    let nft_output = TestCellOutput::new(
        typed_output(lock.script.clone(), nft_code.script.clone(), 200_000_000_000),
        nft_data,
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(minter_code.cell_dep.clone());
    shape.push_prefix_cell_dep(nft_code.cell_dep.clone());
    shape.push_prefix_input(minter_input);
    shape.push_remainder_output(minter_output);
    shape.push_remainder_output(nft_output);
    shape.tx_level_message(
        CobuildMessageBuilder::new()
            .input_type_action(minter_hash)
            .action_data(mint_nft_action_data(seed))
            .build(),
    );
    let built = shape.build();
    let tx = fixture.context_mut().complete_tx(built.tx);
    NftMinterCase { name, fixture, tx }
}
```

- [ ] **Step 4: Implement minter output binding and wire mint mode**

Change the `MinterMode::Mint` arm in `tests/contracts/nft-minter-type/src/entry.rs`:

```rust
MinterMode::Mint => crate::validation::validate_mint(current_type_hash, &plan),
```

Add to `tests/contracts/nft-minter-type/src/validation.rs`:

```rust
pub fn validate_mint(
    current_type_hash: [u8; 32],
    plan: &TypeValidationPlan,
) -> Result<(), Error> {
    let input = single_group_state(Source::GroupInput)?;
    let output = single_group_state(Source::GroupOutput)?;
    let actions = mint_actions(plan)?;
    validate_mint_state(input, output, actions.len())?;
    validate_expected_outputs(current_type_hash, input.mint_counter, &actions)
}
```

In `tests/contracts/nft-minter-type/src/validation.rs`, implement `validate_expected_outputs` using CKB syscalls:

```rust
use ckb_std::{
    ckb_constants::Source,
    high_level::{QueryIter, load_cell_data, load_cell_type},
};

fn validate_expected_outputs(
    current_type_hash: [u8; 32],
    old_counter: u64,
    actions: &[MintActionFact],
) -> Result<(), Error> {
    for (offset, action) in actions.iter().enumerate() {
        let offset: u64 = offset.try_into().map_err(|_| Error::Counter)?;
        let serial = old_counter.checked_add(offset).ok_or(Error::Counter)?;
        let rarity = crate::types::rarity_for_serial(serial);
        let expected_id = crate::types::nft_id(current_type_hash, serial);
        let expected_attributes =
            crate::types::attributes_hash(current_type_hash, serial, rarity, action.metadata_seed);
        let mut matches = 0usize;
        for (index, type_script) in QueryIter::new(load_cell_type, Source::Output).enumerate() {
            let Some(type_script) = type_script else {
                continue;
            };
            let args: alloc::vec::Vec<u8> = type_script.args().unpack();
            if args.as_slice() != expected_id {
                continue;
            }
            let data = load_cell_data(index, Source::Output)?;
            let nft = crate::types::parse_minted_nft_data(&data)?;
            if nft.minter_type_hash != current_type_hash
                || nft.serial != serial
                || nft.rarity != rarity
                || nft.attributes_hash != expected_attributes
            {
                return Err(Error::InvalidMintedNft);
            }
            matches += 1;
        }
        if matches != 1 {
            return Err(Error::InvalidMintedNft);
        }
    }
    Ok(())
}
```

Remove duplicate imports introduced by the snippet.

- [ ] **Step 5: Run happy path integration**

Run:

```bash
make build MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test nft_minter_type -- --nocapture
```

Expected: all happy path cases PASS.

- [ ] **Step 6: Commit**

```bash
git add tests/tests/nft_minter_type.rs tests/src/fixtures/nft_minter/scenarios.rs tests/contracts/nft-minter-type/src/validation.rs
git commit -m "test: cover nft minter happy paths"
```

### Task 8: Negative Cases

**Files:**
- Modify: `tests/src/fixtures/nft_minter/scenarios.rs`
- Modify: `tests/tests/nft_minter_type.rs`

- [ ] **Step 1: Extend case expected outcomes**

Change `NftMinterCase` in `scenarios.rs`:

```rust
pub enum Expected {
    Pass,
    MinterOutput(i8),
    MinterInput(i8),
    MintedNftOutput(i8),
    MintedNftInput(i8),
}

pub struct NftMinterCase {
    pub name: &'static str,
    pub fixture: CobuildTestFixture,
    pub tx: ckb_testtool::ckb_types::core::TransactionView,
    pub expected: Expected,
}

impl NftMinterCase {
    pub fn assert_expected(&self) {
        match self.expected {
            Expected::Pass => self.fixture.assert_pass(&self.tx),
            Expected::MinterOutput(code) => self.fixture.assert_output_type_script_exit(&self.tx, 0, code),
            Expected::MinterInput(code) => self.fixture.assert_type_script_exit(&self.tx, 0, code),
            Expected::MintedNftOutput(code) => self.fixture.assert_output_type_script_exit(&self.tx, 1, code),
            Expected::MintedNftInput(code) => self.fixture.assert_type_script_exit(&self.tx, 0, code),
        }
    }
}
```

Update happy path builders to set `expected: Expected::Pass`.

- [ ] **Step 2: Add negative case list to test runner**

Extend `tests/tests/nft_minter_type.rs`:

```rust
use tests::fixtures::nft_minter::scenarios::{
    create_minter_case, create_minter_missing_action_case, create_minter_non_zero_counter_case,
    create_minter_supply_cap_mismatch_case, forged_nft_creation_case, minter_burn_case,
    mint_first_nft_case, mint_from_counter_six_case, mint_missing_nft_output_case,
    mint_wrong_attributes_case, mint_wrong_counter_case, nft_transfer_mutates_data_case,
};

#[test]
fn nft_minter_cases_match_expected_outcomes() {
    for case in [
        create_minter_case(),
        mint_first_nft_case(),
        mint_from_counter_six_case(),
        create_minter_missing_action_case(),
        create_minter_non_zero_counter_case(),
        create_minter_supply_cap_mismatch_case(),
        mint_wrong_counter_case(),
        mint_missing_nft_output_case(),
        mint_wrong_attributes_case(),
        forged_nft_creation_case(),
        nft_transfer_mutates_data_case(),
        minter_burn_case(),
    ] {
        case.assert_expected();
    }
}
```

Remove the earlier `nft_minter_happy_paths_pass` test to avoid duplicate loops.

- [ ] **Step 3: Implement negative scenario builders**

In `scenarios.rs`, implement each named builder by reusing the happy path construction and mutating one field:

```rust
pub fn create_minter_missing_action_case() -> NftMinterCase {
    let mut case = create_minter_case();
    case.name = "create_minter_missing_action";
    case.expected = Expected::MinterOutput(14);
    case
}
```

Build negative cases through small local builder variants instead of editing serialized bytes:

```rust
fn create_minter_case_with(counter: u64, output_cap: u64, action_cap: Option<u64>) -> NftMinterCase
```

Use these expected codes from contract `Error` enums:

```rust
const MINTER_INVALID_COBUILD: i8 = 14;
const MINTER_COUNTER: i8 = 16;
const MINTER_SUPPLY_CAP: i8 = 17;
const MINTER_INVALID_MINTED_NFT: i8 = 15;
const MINTER_INVALID_SHAPE: i8 = 18;
const NFT_INVALID_DATA: i8 = 11;
const NFT_INVALID_MINTER_TRANSITION: i8 = 12;
```

Map cases:

```rust
create_minter_missing_action_case -> MINTER_INVALID_COBUILD
create_minter_non_zero_counter_case -> MINTER_COUNTER
create_minter_supply_cap_mismatch_case -> MINTER_SUPPLY_CAP
mint_wrong_counter_case -> MINTER_COUNTER
mint_missing_nft_output_case -> MINTER_INVALID_MINTED_NFT
mint_wrong_attributes_case -> MINTER_INVALID_MINTED_NFT
forged_nft_creation_case -> NFT_INVALID_MINTER_TRANSITION
nft_transfer_mutates_data_case -> NFT_INVALID_DATA
minter_burn_case -> MINTER_INVALID_SHAPE
```

- [ ] **Step 4: Run negative matrix**

Run:

```bash
make build MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test nft_minter_type -- --nocapture
```

Expected: all cases PASS and each negative case reports its expected exit code.

- [ ] **Step 5: Commit**

```bash
git add tests/tests/nft_minter_type.rs tests/src/fixtures/nft_minter/scenarios.rs
git commit -m "test: cover nft minter failure cases"
```

### Task 9: Multi-Action, Rarity, OTX, Transfer, And Burn Coverage

**Files:**
- Modify: `tests/src/fixtures/nft_minter/scenarios.rs`
- Modify: `tests/tests/nft_minter_type.rs`

- [ ] **Step 1: Add coverage case names**

Extend `nft_minter_cases_match_expected_outcomes` with:

```rust
mint_serial_seven_case(),
mint_serial_eleven_case(),
mint_serial_seventy_seven_case(),
mint_two_actions_tx_level_case(),
mint_mixed_tx_and_otx_order_case(),
nft_burn_case(),
```

- [ ] **Step 2: Implement rarity cases**

Add builders:

```rust
pub fn mint_serial_seven_case() -> NftMinterCase {
    mint_from_counter_case("mint_serial_seven", 7, 8, [7u8; 32])
}

pub fn mint_serial_eleven_case() -> NftMinterCase {
    mint_from_counter_case("mint_serial_eleven", 11, 12, [11u8; 32])
}

pub fn mint_serial_seventy_seven_case() -> NftMinterCase {
    mint_from_counter_case("mint_serial_seventy_seven", 77, 78, [77u8; 32])
}
```

These pass only if rarity derivation is correct in both fixtures and contract.

- [ ] **Step 3: Implement two tx-level action case**

Add `mint_two_actions_tx_level_case()` that creates one minter input with `old_counter = 6`, minter output with `new_counter = 8`, two `MintNft` actions in one tx-level message, and two NFT outputs for serials `6` and `7`. Build the message with:

```rust
shape.tx_level_message(
    CobuildMessageBuilder::new()
        .push_action(
            ActionRole::InputType.into(),
            minter_hash,
            mint_nft_action_data([6u8; 32]),
        )
        .push_action(
            ActionRole::InputType.into(),
            minter_hash,
            mint_nft_action_data([7u8; 32]),
        )
        .build(),
);
```

Expected: PASS.

- [ ] **Step 4: Implement mixed tx-level and OTX ordering case**

Add `mint_mixed_tx_and_otx_order_case()`:

- minter input counter `6`, output counter `8`;
- tx-level message contains `MintNft(seed=[6;32])`;
- one OTX segment contains `MintNft(seed=[7;32])`;
- expected serial order follows `(witness_index, action.index)`;
- put NFT outputs for serial `6` and `7`.

Use `TxShape::push_otx(OtxSegment { ... })` with the minter input in the OTX base input and the second NFT output in the OTX append output. Keep the tx-level action witness before `OtxStart`; the canonical order remains `(witness_index, action.index)`.

Expected: PASS for the supported mixed-origin shape.

- [ ] **Step 5: Implement NFT burn case**

Add `nft_burn_case()`:

- create a live `minted-nft-type` input with valid data;
- no output with that NFT type;
- no Cobuild action;
- lock is always-success.

Expected: PASS.

- [ ] **Step 6: Run coverage matrix**

Run:

```bash
make build MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test nft_minter_type -- --nocapture
```

Expected: all PASS.

- [ ] **Step 7: Commit**

```bash
git add tests/tests/nft_minter_type.rs tests/src/fixtures/nft_minter/scenarios.rs
git commit -m "test: cover nft minter ordering and lifecycle"
```

### Task 10: Final Verification And Documentation

**Files:**
- Modify: `docs/superpowers/specs/2026-06-14-nft-minter-test-type-script-design.md` only if implementation discovered an intentional spec correction.
- Modify: `docs/CobuildAgentDevelopGuide.md` only if adding new common commands is useful.

- [ ] **Step 1: Run contract unit tests**

Run:

```bash
cargo test -p nft-minter-type --features library --offline
cargo test -p minted-nft-type --features library --offline
```

Expected: all PASS.

- [ ] **Step 2: Run workspace layout and targeted integration**

Run:

```bash
cargo test -p tests --test workspace_layout --offline
make build MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test nft_minter_type -- --nocapture
```

Expected: all PASS.

- [ ] **Step 3: Run boundary checks**

Run:

```bash
rg -n "cobuild_types::entity|::entity::" crates/cobuild-core/src contracts/cobuild-otx-lock/src
rg -n "critical-section|portable-atomic.*unsafe-assume-single-core|\\[patch.crates-io\\]" Cargo.toml crates contracts tests/contracts/nft-minter-type tests/contracts/minted-nft-type
rg -n "CobuildEngine|PreparedCobuild|ScriptHashIndex|ChainSource|source\\.rs|prepare\\.rs|flow\\.rs|message\\.rs" crates contracts tests docs/CobuildAgentDevelopGuide.md
```

Expected: first two commands print no matches. The removed-name command may match historical guard text but must not find new production uses.

- [ ] **Step 4: Run broader workspace tests**

Run:

```bash
cargo test --workspace --offline
```

Expected: PASS.

- [ ] **Step 5: Commit final docs corrections when files changed**

When implementation changes either listed docs file, commit those corrections:

```bash
git add docs/superpowers/specs/2026-06-14-nft-minter-test-type-script-design.md docs/CobuildAgentDevelopGuide.md
git commit -m "docs: update nft minter implementation notes"
```

When neither listed docs file changed, record in the implementation notes that no docs correction was needed.

## Self-Review

Spec coverage:

- Minter create with `CreateMinter` output-type action: Tasks 3, 7, 8.
- Minter mint with `MintNft` input-type action: Tasks 4, 7, 8, 9.
- Counter increment, supply cap, and overflow behavior: Tasks 2, 4, 8.
- Rarity for serials `0`, `7`, `11`, `77`: Tasks 2, 7, 9.
- NFT args/data binding to `minter_type_hash`, serial, rarity, seed: Tasks 2, 7, 8.
- NFT self-checking creation against minter transition: Task 5 and forged creation test in Task 8.
- NFT transfer and burn: Tasks 5, 8, 9.
- Tx-level, OTX-level, and action ordering: Tasks 4, 7, 9.
- Spore-like boundary where fake NFT types are outside protocol: Task 7 binding and Task 5 self-checks.

Placeholder scan:

- The plan contains no open-ended implementation placeholders.
- No task uses open-ended "add tests" instructions without named tests or expected outcomes.

Type consistency:

- Host and contract ABI use the same tag values: `1 = CreateMinter`, `2 = MintNft`.
- Minter state is always 16 bytes; minted NFT data is always 73 bytes.
- Exit codes referenced in negative tests match the proposed `Error` enums.
