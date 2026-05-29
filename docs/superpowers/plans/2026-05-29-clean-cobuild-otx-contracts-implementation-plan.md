# Clean Cobuild OTX Contracts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a clean, contract-first Cobuild OTX lock stack inside the isolated `cobuild-otx-contracts` sub-repository.

**Architecture:** The sub-repository is self-contained. `cobuild-types` owns Molecule schemas plus committed `lazy_reader` and `entity` outputs; `cobuild-core` is a no-std chain-facing protocol crate built on thin lazy-reader views; `cobuild-otx-lock` is a thin contract crate that consumes core tasks and calls a narrow verifier boundary.

**Tech Stack:** Rust 2021/2024 workspace, `ckb-script-template` workspace conventions, Molecule 0.9.2 codegen (`RustLazyReader` and `Rust`), `ckb-std`, `ckb-hash`, `blake2b-ref`, `secp256k1`, `ckb-testtool`, `cargo xtask`, `make`.

---

## Repository Boundary

All implementation work in this plan happens under:

`/home/xcshuan/contracts/ckb/cobuild-otx-contracts/cobuild-otx-contracts`

The parent repository and `../ref` paths are read-only references. Do not copy build glue, local runtime shims, or old implementation files from the parent workspace unless a task explicitly names a reference snippet.

The sub-repository owns its own:

- root `Cargo.toml`
- root `Makefile`
- `scripts/find_clang`
- `xtask`
- `crates/cobuild-types`
- `crates/cobuild-core`
- `contracts/cobuild-otx-lock`
- `tests`

## File Structure

### Workspace

- Modify: `Cargo.toml`
  - Add workspace members for `xtask`, `crates/cobuild-types`, `crates/cobuild-core`, `contracts/cobuild-otx-lock`, and `tests`.
  - Do not add a local `critical-section` patch.
- Modify: `Makefile`
  - Keep `ckb-script-template` conventions.
  - In `CONTRACT=...` builds, only build a native simulator when `native-simulators/$(CONTRACT)-sim` exists.
- Verify: `scripts/find_clang`
  - Keep this sub-repository-local script as the clang discovery path.

### `xtask`

- Create: `xtask/Cargo.toml`
- Create: `xtask/src/main.rs`
  - Commands:
    - `cargo run -p xtask -- codegen cobuild-types`
    - `cargo run -p xtask -- codegen cobuild-types --check`
  - Generate `lazy_reader` from Molecule `Language::RustLazyReader`.
  - Generate `entity` from Molecule `Language::Rust`.
  - Write module files for both output families.
  - In `--check`, generate into `target/xtask-codegen-check` and compare against committed files.

### `crates/cobuild-types`

- Modify: `crates/cobuild-types/Cargo.toml`
- Modify: `crates/cobuild-types/src/lib.rs`
- Replace: `crates/cobuild-types/src/generated/*`
- Create: `crates/cobuild-types/src/lazy_reader/{mod.rs,blockchain.rs,core.rs,witness.rs}`
- Create: `crates/cobuild-types/src/entity/{mod.rs,blockchain.rs,core.rs,witness.rs}`
- Keep: `crates/cobuild-types/schemas/{blockchain.mol,core.mol,witness.mol}`
- Tests:
  - `crates/cobuild-types/tests/generated_compile.rs`
  - `crates/cobuild-types/tests/lazy_reader_witness.rs`
  - `crates/cobuild-types/tests/entity_witness.rs`

### `crates/cobuild-core`

- Create: `crates/cobuild-core/Cargo.toml`
- Create: `crates/cobuild-core/src/lib.rs`
- Create: `crates/cobuild-core/src/error.rs`
- Create: `crates/cobuild-core/src/view.rs`
- Create: `crates/cobuild-core/src/witness.rs`
- Create: `crates/cobuild-core/src/layout.rs`
- Create: `crates/cobuild-core/src/hash.rs`
- Create: `crates/cobuild-core/src/context.rs`
- Create: `crates/cobuild-core/src/tasks.rs`
- Create: `crates/cobuild-core/src/loader.rs`
- Tests:
  - `crates/cobuild-core/tests/view.rs`
  - `crates/cobuild-core/tests/witness.rs`
  - `crates/cobuild-core/tests/layout.rs`
  - `crates/cobuild-core/tests/hash.rs`
  - `crates/cobuild-core/tests/tasks.rs`
  - `crates/cobuild-core/tests/no_entity_dependency.rs`

### `contracts/cobuild-otx-lock`

- Create with `ckb-script-template` contract conventions:
  - `contracts/cobuild-otx-lock/Cargo.toml`
  - `contracts/cobuild-otx-lock/Makefile`
  - `contracts/cobuild-otx-lock/src/main.rs`
  - `contracts/cobuild-otx-lock/src/lib.rs`
  - `contracts/cobuild-otx-lock/src/args.rs`
  - `contracts/cobuild-otx-lock/src/error.rs`
  - `contracts/cobuild-otx-lock/src/entry.rs`
  - `contracts/cobuild-otx-lock/src/runner.rs`
  - `contracts/cobuild-otx-lock/src/verify/mod.rs`
  - `contracts/cobuild-otx-lock/src/verify/local.rs`
- Tests:
  - `contracts/cobuild-otx-lock/tests/args.rs`
  - `contracts/cobuild-otx-lock/tests/error.rs`
  - `contracts/cobuild-otx-lock/tests/runner.rs`
  - `contracts/cobuild-otx-lock/tests/verifier.rs`

### Host Contract Tests

- Modify: `tests/Cargo.toml`
- Create or modify: `tests/src/lib.rs`
- Create: `tests/tests/cobuild_otx_lock.rs`
  - Build Data2 contract fixtures.
  - Cover tx-level only, OTX base+append, mixed tx+OTX, malformed witness, invalid args, bad seal, verifier backend failure.

## Task 1: Sub-Repository Workspace Bootstrap

**Files:**
- Modify: `Cargo.toml`
- Modify: `Makefile`
- Verify/modify: `scripts/find_clang`
- Create: `xtask/Cargo.toml`
- Create: `xtask/src/main.rs`

- [x] **Step 1: Write the failing workspace membership check**

Create `tests/tests/workspace_layout.rs`:

```rust
use std::fs;

#[test]
fn workspace_declares_clean_cobuild_members() {
    let manifest = fs::read_to_string("Cargo.toml").expect("workspace manifest");
    for member in [
        "\"xtask\"",
        "\"crates/cobuild-types\"",
        "\"crates/cobuild-core\"",
        "\"contracts/cobuild-otx-lock\"",
        "\"tests\"",
    ] {
        assert!(manifest.contains(member), "missing workspace member {member}");
    }
    assert!(
        !manifest.contains("[patch.crates-io]\ncritical-section"),
        "clean workspace must not patch critical-section"
    );
}
```

- [x] **Step 2: Run the test and verify it fails**

Run:

```bash
cargo test -p tests --test workspace_layout --offline
```

Expected: FAIL because `crates/cobuild-core`, `contracts/cobuild-otx-lock`, and `xtask` are not workspace members yet.

- [x] **Step 3: Update root workspace manifest**

Update `Cargo.toml`:

```toml
[workspace]
resolver = "2"

members = [
  # Please don't remove the following line, we use it to automatically
  # detect insertion point for newly generated crates.
  # @@INSERTION_POINT@@
  "contracts/cobuild-otx-lock",
  "crates/cobuild-core",
  "crates/cobuild-types",
  "tests",
  "xtask",
]

[profile.release]
overflow-checks = true
strip = false
codegen-units = 1
debug = true
```

- [x] **Step 4: Add minimal crates so the workspace resolves**

Create `xtask/Cargo.toml`:

```toml
[package]
name = "xtask"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0"
molecule-codegen = "0.9.2"
```

Create `xtask/src/main.rs`:

```rust
fn main() {
    eprintln!("usage: cargo run -p xtask -- codegen cobuild-types [--check]");
    std::process::exit(2);
}
```

Create `crates/cobuild-core/Cargo.toml`:

```toml
[package]
name = "cobuild-core"
version = "0.1.0"
edition = "2021"

[dependencies]
cobuild-types = { path = "../cobuild-types" }
```

Create `crates/cobuild-core/src/lib.rs`:

```rust
#![no_std]

pub fn bootstrap_marker() -> bool {
    true
}
```

Create `contracts/cobuild-otx-lock/Cargo.toml`:

```toml
[package]
name = "cobuild-otx-lock"
version = "0.1.0"
edition = "2021"

[dependencies]
cobuild-core = { path = "../../crates/cobuild-core" }
```

Create `contracts/cobuild-otx-lock/src/lib.rs`:

```rust
#![no_std]

pub fn bootstrap_marker() -> bool {
    cobuild_core::bootstrap_marker()
}
```

Create `contracts/cobuild-otx-lock/src/main.rs`:

```rust
fn main() {}
```

- [x] **Step 5: Fix the single-contract Makefile branch**

In root `Makefile`, replace the `else` branch under `build:` with:

```make
	else \
		$(MAKE) -e -C contracts/$(CONTRACT) build; \
		if [ -d "native-simulators/$(CONTRACT)-sim" ]; then \
			cargo build -p $(CONTRACT)-sim $(CARGO_ARGS); \
		fi; \
	fi;
```

- [x] **Step 6: Run workspace checks**

Run:

```bash
cargo test -p tests --test workspace_layout --offline
cargo check --workspace --offline
```

Expected: both PASS.

- [x] **Step 7: Commit**

```bash
git add Cargo.toml Makefile xtask crates/cobuild-core contracts/cobuild-otx-lock tests/tests/workspace_layout.rs
git commit -m "chore: bootstrap clean cobuild workspace"
```

## Task 2: `cobuild-types` Dual Codegen

**Files:**
- Modify: `xtask/src/main.rs`
- Modify: `crates/cobuild-types/Cargo.toml`
- Modify: `crates/cobuild-types/src/lib.rs`
- Replace: `crates/cobuild-types/src/generated/*`
- Create: `crates/cobuild-types/src/lazy_reader/*`
- Create: `crates/cobuild-types/src/entity/*`
- Test: `crates/cobuild-types/tests/generated_compile.rs`
- Test: `crates/cobuild-types/tests/lazy_reader_witness.rs`
- Test: `crates/cobuild-types/tests/entity_witness.rs`

- [x] **Step 1: Write failing public module tests**

Replace `crates/cobuild-types/tests/generated_compile.rs` with:

```rust
use cobuild_types::{entity, lazy_reader};

#[test]
fn exposes_lazy_reader_and_entity_modules() {
    let _ = core::any::type_name::<lazy_reader::witness::WitnessLayout>();
    let _ = core::any::type_name::<entity::witness::WitnessLayout>();
    let _ = core::any::type_name::<lazy_reader::core::Otx>();
    let _ = core::any::type_name::<entity::core::Otx>();
}
```

Create `crates/cobuild-types/tests/lazy_reader_witness.rs`:

```rust
use cobuild_types::lazy_reader::witness::WitnessLayout;
use molecule::lazy_reader::Cursor;

#[test]
fn lazy_reader_witness_rejects_empty_cursor() {
    let cursor = Cursor::new(0, Box::new(&[][..]));
    assert!(WitnessLayout::try_from(cursor).is_err());
}
```

Create `crates/cobuild-types/tests/entity_witness.rs`:

```rust
use cobuild_types::entity::witness::WitnessLayout;
use molecule::prelude::Entity as _;

#[test]
fn entity_witness_default_serializes() {
    let bytes = WitnessLayout::default().as_slice();
    assert!(!bytes.is_empty());
}
```

- [x] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test -p cobuild-types --offline
```

Expected: FAIL because `lazy_reader` and `entity` modules are not exposed yet.

- [x] **Step 3: Implement `xtask` dual codegen**

Replace `xtask/src/main.rs` with:

```rust
use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{bail, Context, Result};
use molecule_codegen::{Compiler, Language};

const SCHEMAS: &[&str] = &["blockchain.mol", "core.mol", "witness.mol"];

fn main() -> Result<()> {
    let args = env::args().skip(1).collect::<Vec<_>>();
    match args.as_slice() {
        [cmd, target] if cmd == "codegen" && target == "cobuild-types" => codegen(false),
        [cmd, target, flag] if cmd == "codegen" && target == "cobuild-types" && flag == "--check" => {
            codegen(true)
        }
        _ => bail!("usage: cargo run -p xtask -- codegen cobuild-types [--check]"),
    }
}

fn codegen(check: bool) -> Result<()> {
    let root = workspace_root()?;
    let schema_dir = root.join("crates/cobuild-types/schemas");
    let checked_in = root.join("crates/cobuild-types/src");
    let output_root = if check {
        root.join("target/xtask-codegen-check/cobuild-types/src")
    } else {
        checked_in.clone()
    };

    generate_family(&schema_dir, &output_root.join("lazy_reader"), Language::RustLazyReader)?;
    generate_family(&schema_dir, &output_root.join("entity"), Language::Rust)?;

    if check {
        compare_dirs(&checked_in.join("lazy_reader"), &output_root.join("lazy_reader"))?;
        compare_dirs(&checked_in.join("entity"), &output_root.join("entity"))?;
    }

    Ok(())
}

fn workspace_root() -> Result<PathBuf> {
    Ok(PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .context("xtask must live under workspace root")?
        .to_path_buf())
}

fn generate_family(schema_dir: &Path, out_dir: &Path, language: Language) -> Result<()> {
    fs::create_dir_all(out_dir)
        .with_context(|| format!("failed to create {}", out_dir.display()))?;
    prune_rs_files(out_dir)?;

    for schema in SCHEMAS {
        Compiler::new()
            .generate_code(language)
            .input_schema_file(schema_dir.join(schema))
            .output_dir(out_dir)
            .run()
            .map_err(anyhow::Error::msg)
            .with_context(|| format!("failed to generate {schema}"))?;
        run_rustfmt(&out_dir.join(schema).with_extension("rs"))?;
    }

    fs::write(out_dir.join("mod.rs"), module_file())
        .with_context(|| format!("failed to write {}", out_dir.join("mod.rs").display()))?;
    run_rustfmt(&out_dir.join("mod.rs"))?;
    Ok(())
}

fn prune_rs_files(out_dir: &Path) -> Result<()> {
    if !out_dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(out_dir)? {
        let entry = entry?;
        if entry.path().extension().is_some_and(|ext| ext == "rs") {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}

fn module_file() -> &'static str {
    "#![allow(dead_code)]\n#![allow(clippy::all)]\npub mod blockchain;\npub mod core;\npub mod witness;\n"
}

fn run_rustfmt(path: &Path) -> Result<()> {
    let status = Command::new("rustfmt").arg("--edition").arg("2021").arg(path).status()?;
    if !status.success() {
        bail!("rustfmt failed for {}", path.display());
    }
    Ok(())
}

fn compare_dirs(expected: &Path, actual: &Path) -> Result<()> {
    for name in ["mod.rs", "blockchain.rs", "core.rs", "witness.rs"] {
        let expected_text = fs::read_to_string(expected.join(name))
            .with_context(|| format!("missing {}", expected.join(name).display()))?;
        let actual_text = fs::read_to_string(actual.join(name))
            .with_context(|| format!("missing {}", actual.join(name).display()))?;
        if expected_text != actual_text {
            bail!("generated output differs for {}", expected.join(name).display());
        }
    }
    Ok(())
}
```

- [x] **Step 4: Update `cobuild-types` module exports**

Replace `crates/cobuild-types/src/lib.rs` with:

```rust
#![no_std]

pub mod entity;
pub mod lazy_reader;
```

Ensure `crates/cobuild-types/Cargo.toml` contains:

```toml
[package]
name = "cobuild-types"
version = "0.1.0"
edition = "2021"

[dependencies]
molecule = { version = "0.9.2", default-features = false }
```

- [x] **Step 5: Generate committed outputs**

Run:

```bash
cargo run -p xtask --offline -- codegen cobuild-types
cargo run -p xtask --offline -- codegen cobuild-types --check
```

Expected: both PASS.

- [x] **Step 6: Run `cobuild-types` tests**

Run:

```bash
cargo test -p cobuild-types --offline
```

Expected: PASS.

- [x] **Step 7: Commit**

```bash
git add xtask crates/cobuild-types
git commit -m "feat: add dual cobuild type codegen"
```

## Task 3: `cobuild-core` View And Witness Boundary

**Files:**
- Modify: `crates/cobuild-core/Cargo.toml`
- Modify: `crates/cobuild-core/src/lib.rs`
- Create: `crates/cobuild-core/src/error.rs`
- Create: `crates/cobuild-core/src/view.rs`
- Create: `crates/cobuild-core/src/witness.rs`
- Test: `crates/cobuild-core/tests/view.rs`
- Test: `crates/cobuild-core/tests/witness.rs`
- Test: `crates/cobuild-core/tests/no_entity_dependency.rs`

- [x] **Step 1: Write failing view and witness tests**

Create `crates/cobuild-core/tests/view.rs`:

```rust
use cobuild_core::view::WitnessLayoutView;

#[test]
fn empty_witness_is_not_a_cobuild_layout() {
    assert!(WitnessLayoutView::from_slice(&[]).is_err());
}
```

Create `crates/cobuild-core/tests/witness.rs`:

```rust
use cobuild_core::witness::{parse_witness, ParsedWitness};

#[test]
fn non_cobuild_witness_returns_none() {
    assert!(matches!(parse_witness(&[0, 1, 2, 3]), Ok(ParsedWitness::None)));
}
```

Create `crates/cobuild-core/tests/no_entity_dependency.rs`:

```rust
#[test]
fn core_source_does_not_import_entity_module() {
    for path in ["src/lib.rs", "src/view.rs", "src/witness.rs"] {
        let text = std::fs::read_to_string(format!("crates/cobuild-core/{path}")).unwrap();
        assert!(
            !text.contains("cobuild_types::entity"),
            "{path} must not import cobuild_types::entity"
        );
    }
}
```

- [x] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test -p cobuild-core --offline
```

Expected: FAIL because `view` and `witness` modules do not exist yet.

- [x] **Step 3: Implement core errors**

Create `crates/cobuild-core/src/error.rs`:

```rust
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoreError {
    MalformedCobuild,
    InvalidLayout,
    InvalidMessageTarget,
    MissingHashParts,
    MissingSealPair,
    DuplicateSealPair,
}
```

- [x] **Step 4: Implement thin witness view**

Create `crates/cobuild-core/src/view.rs`:

```rust
use cobuild_types::lazy_reader::witness::WitnessLayout;
use molecule::lazy_reader::{Cursor, Read};

use crate::error::CoreError;

pub struct SliceReader<'a> {
    data: &'a [u8],
}

impl<'a> SliceReader<'a> {
    pub const fn new(data: &'a [u8]) -> Self {
        Self { data }
    }
}

impl Read for SliceReader<'_> {
    fn read(&self, buf: &mut [u8], offset: usize) -> Result<usize, molecule::lazy_reader::Error> {
        if offset > self.data.len() {
            return Err(molecule::lazy_reader::Error::OutOfBound(offset, self.data.len()));
        }
        let len = core::cmp::min(buf.len(), self.data.len() - offset);
        buf[..len].copy_from_slice(&self.data[offset..offset + len]);
        Ok(len)
    }
}

pub struct WitnessLayoutView {
    inner: WitnessLayout,
}

impl WitnessLayoutView {
    pub fn from_slice(data: &[u8]) -> Result<Self, CoreError> {
        let cursor = Cursor::new(data.len(), Box::new(SliceReader::new(data)));
        let inner = WitnessLayout::try_from(cursor).map_err(|_| CoreError::MalformedCobuild)?;
        Ok(Self { inner })
    }

    pub fn inner(&self) -> &WitnessLayout {
        &self.inner
    }
}
```

- [x] **Step 5: Implement witness parsing facade**

Create `crates/cobuild-core/src/witness.rs`:

```rust
use crate::{error::CoreError, view::WitnessLayoutView};

pub enum ParsedWitness {
    None,
    Cobuild(WitnessLayoutView),
}

pub fn parse_witness(data: &[u8]) -> Result<ParsedWitness, CoreError> {
    match WitnessLayoutView::from_slice(data) {
        Ok(view) => Ok(ParsedWitness::Cobuild(view)),
        Err(CoreError::MalformedCobuild) => Ok(ParsedWitness::None),
        Err(err) => Err(err),
    }
}
```

Replace `crates/cobuild-core/src/lib.rs` with:

```rust
#![no_std]
extern crate alloc;

pub mod error;
pub mod view;
pub mod witness;
```

Update `crates/cobuild-core/Cargo.toml`:

```toml
[package]
name = "cobuild-core"
version = "0.1.0"
edition = "2021"

[dependencies]
cobuild-types = { path = "../cobuild-types" }
molecule = { version = "0.9.2", default-features = false }
```

- [x] **Step 6: Run tests**

Run:

```bash
cargo test -p cobuild-core --offline
```

Expected: PASS.

- [x] **Step 7: Commit**

```bash
git add crates/cobuild-core
git commit -m "feat: add cobuild core reader boundary"
```

## Task 4: Core Layout, Hash, Context, And Tasks

**Files:**
- Create: `crates/cobuild-core/src/layout.rs`
- Create: `crates/cobuild-core/src/hash.rs`
- Create: `crates/cobuild-core/src/context.rs`
- Create: `crates/cobuild-core/src/tasks.rs`
- Create: `crates/cobuild-core/src/loader.rs`
- Modify: `crates/cobuild-core/src/lib.rs`
- Tests:
  - `crates/cobuild-core/tests/layout.rs`
  - `crates/cobuild-core/tests/hash.rs`
  - `crates/cobuild-core/tests/tasks.rs`

- [x] **Step 1: Write failing layout/hash/task tests**

Create `crates/cobuild-core/tests/layout.rs`:

```rust
use cobuild_core::layout::{build_layout, LayoutTx};

#[test]
fn empty_tx_has_no_otx_layouts() {
    let layout = build_layout(&LayoutTx {
        witnesses: Vec::new(),
        input_count: 0,
        output_count: 0,
        cell_dep_count: 0,
        header_dep_count: 0,
    })
    .unwrap();
    assert!(layout.otxs.is_empty());
}
```

Create `crates/cobuild-core/tests/hash.rs`:

```rust
use cobuild_core::hash::{tx_without_message_hash, TxHashParts};

#[test]
fn tx_without_message_hash_is_deterministic() {
    let parts = TxHashParts {
        tx_hash: [7u8; 32],
        resolved_inputs: Vec::new(),
        trailing_witnesses: Vec::new(),
    };
    assert_eq!(
        tx_without_message_hash(&parts).unwrap(),
        tx_without_message_hash(&parts).unwrap()
    );
}
```

Create `crates/cobuild-core/tests/tasks.rs`:

```rust
use cobuild_core::{
    context::{CobuildContext, TxScriptHashes},
    layout::LayoutTx,
};

#[test]
fn lock_query_without_matching_lock_has_no_tasks() {
    let context = CobuildContext::new(
        LayoutTx {
            witnesses: Vec::new(),
            input_count: 1,
            output_count: 0,
            cell_dep_count: 0,
            header_dep_count: 0,
        },
        TxScriptHashes {
            input_locks: vec![[1u8; 32]],
            input_types: vec![None],
            output_types: Vec::new(),
        },
    )
    .unwrap();
    assert!(context.lock_query([2u8; 32]).tx_tasks().unwrap().is_empty());
}
```

- [x] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test -p cobuild-core --offline
```

Expected: FAIL because layout/hash/context/task modules do not exist yet.

- [x] **Step 3: Add minimal layout API**

Create `crates/cobuild-core/src/layout.rs`:

```rust
use alloc::vec::Vec;

use crate::error::CoreError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LayoutTx {
    pub witnesses: Vec<Vec<u8>>,
    pub input_count: usize,
    pub output_count: usize,
    pub cell_dep_count: usize,
    pub header_dep_count: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Range {
    pub start: usize,
    pub count: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OtxLayout {
    pub witness_index: usize,
    pub base_inputs: Range,
    pub append_inputs: Range,
    pub base_outputs: Range,
    pub append_outputs: Range,
    pub base_cell_deps: Range,
    pub append_cell_deps: Range,
    pub base_header_deps: Range,
    pub append_header_deps: Range,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BuiltLayout {
    pub otxs: Vec<OtxLayout>,
}

pub fn build_layout(tx: &LayoutTx) -> Result<BuiltLayout, CoreError> {
    if tx.witnesses.is_empty() {
        return Ok(BuiltLayout { otxs: Vec::new() });
    }
    Ok(BuiltLayout { otxs: Vec::new() })
}
```

- [x] **Step 4: Add minimal hash API**

Create `crates/cobuild-core/src/hash.rs`:

```rust
use alloc::vec::Vec;

use blake2b_ref::Blake2bBuilder;

use crate::error::CoreError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TxHashParts {
    pub tx_hash: [u8; 32],
    pub resolved_inputs: Vec<ResolvedInputHashPart>,
    pub trailing_witnesses: Vec<Vec<u8>>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResolvedInputHashPart {
    pub output: Vec<u8>,
    pub data: Vec<u8>,
}

pub fn tx_without_message_hash(parts: &TxHashParts) -> Result<[u8; 32], CoreError> {
    let mut out = [0u8; 32];
    let mut hasher = Blake2bBuilder::new(32)
        .personal(b"ckbcb_tnm_core1\0")
        .build();
    hasher.update(&parts.tx_hash);
    for input in &parts.resolved_inputs {
        update_len_prefixed(&mut hasher, &input.output);
        update_len_prefixed(&mut hasher, &input.data);
    }
    for witness in &parts.trailing_witnesses {
        update_len_prefixed(&mut hasher, witness);
    }
    hasher.finalize(&mut out);
    Ok(out)
}

fn update_len_prefixed(hasher: &mut blake2b_ref::Blake2b, bytes: &[u8]) {
    hasher.update(&(bytes.len() as u32).to_le_bytes());
    hasher.update(bytes);
}
```

Update `crates/cobuild-core/Cargo.toml` dependencies:

```toml
blake2b-ref = "0.3.1"
```

- [x] **Step 5: Add minimal context/task API**

Create `crates/cobuild-core/src/tasks.rs`:

```rust
use alloc::vec::Vec;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TxLevelLockTask {
    pub script_hash: [u8; 32],
    pub carrier_witness_index: usize,
    pub seal: Vec<u8>,
    pub signing_message_hash: [u8; 32],
}
```

Create `crates/cobuild-core/src/context.rs`:

```rust
use alloc::vec::Vec;

use crate::{error::CoreError, layout::LayoutTx, tasks::TxLevelLockTask};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TxScriptHashes {
    pub input_locks: Vec<[u8; 32]>,
    pub input_types: Vec<Option<[u8; 32]>>,
    pub output_types: Vec<Option<[u8; 32]>>,
}

pub struct CobuildContext {
    tx: LayoutTx,
    script_hashes: TxScriptHashes,
}

pub struct LockScriptQuery<'a> {
    context: &'a CobuildContext,
    script_hash: [u8; 32],
}

impl CobuildContext {
    pub fn new(tx: LayoutTx, script_hashes: TxScriptHashes) -> Result<Self, CoreError> {
        if script_hashes.input_locks.len() != tx.input_count {
            return Err(CoreError::InvalidLayout);
        }
        if script_hashes.input_types.len() != tx.input_count {
            return Err(CoreError::InvalidLayout);
        }
        if script_hashes.output_types.len() != tx.output_count {
            return Err(CoreError::InvalidLayout);
        }
        Ok(Self { tx, script_hashes })
    }

    pub fn lock_query(&self, script_hash: [u8; 32]) -> LockScriptQuery<'_> {
        LockScriptQuery {
            context: self,
            script_hash,
        }
    }
}

impl LockScriptQuery<'_> {
    pub fn tx_tasks(&self) -> Result<Vec<TxLevelLockTask>, CoreError> {
        if !self
            .context
            .script_hashes
            .input_locks
            .iter()
            .any(|hash| *hash == self.script_hash)
        {
            return Ok(Vec::new());
        }
        Ok(Vec::new())
    }
}
```

Create `crates/cobuild-core/src/loader.rs`:

```rust
#![allow(dead_code)]

pub struct LoaderSession;
```

Update `crates/cobuild-core/src/lib.rs`:

```rust
#![no_std]
extern crate alloc;

pub mod context;
pub mod error;
pub mod hash;
pub mod layout;
pub mod loader;
pub mod tasks;
pub mod view;
pub mod witness;
```

- [x] **Step 6: Run tests**

Run:

```bash
cargo test -p cobuild-core --offline
```

Expected: PASS for the initial minimal API tests.

- [x] **Step 7: Commit**

```bash
git add crates/cobuild-core
git commit -m "feat: add initial cobuild core protocol surface"
```

## Task 5: `cobuild-otx-lock` Contract Crate

**Files:**
- Modify: `contracts/cobuild-otx-lock/Cargo.toml`
- Modify: `contracts/cobuild-otx-lock/Makefile`
- Modify: `contracts/cobuild-otx-lock/src/*`
- Tests:
  - `contracts/cobuild-otx-lock/tests/args.rs`
  - `contracts/cobuild-otx-lock/tests/error.rs`
  - `contracts/cobuild-otx-lock/tests/verifier.rs`
  - `contracts/cobuild-otx-lock/tests/runner.rs`

- [x] **Step 1: Write failing lock unit tests**

Create `contracts/cobuild-otx-lock/tests/args.rs`:

```rust
use cobuild_otx_lock::args::{parse_auth_args, AUTH_KIND_SECP256K1_BLAKE160};

#[test]
fn parses_auth_kind_and_identity() {
    let mut args = vec![AUTH_KIND_SECP256K1_BLAKE160];
    args.extend_from_slice(&[7u8; 20]);
    let auth = parse_auth_args(&args).unwrap();
    assert_eq!(auth.kind, AUTH_KIND_SECP256K1_BLAKE160);
    assert_eq!(auth.identity, [7u8; 20]);
}

#[test]
fn rejects_wrong_arg_length() {
    assert!(parse_auth_args(&[AUTH_KIND_SECP256K1_BLAKE160]).is_err());
}
```

Create `contracts/cobuild-otx-lock/tests/error.rs`:

```rust
use cobuild_otx_lock::error::ExitCode;

#[test]
fn exit_codes_are_stable() {
    assert_eq!(ExitCode::InvalidArgs as i8, 1);
    assert_eq!(ExitCode::MalformedCobuild as i8, 2);
    assert_eq!(ExitCode::LockSemanticFailure as i8, 3);
    assert_eq!(ExitCode::VerifyFailure as i8, 4);
    assert_eq!(ExitCode::SyscallFailure as i8, 5);
    assert_eq!(ExitCode::InternalFailure as i8, 6);
}
```

Create `contracts/cobuild-otx-lock/tests/verifier.rs`:

```rust
use cobuild_otx_lock::{
    args::{AuthContext, AUTH_KIND_SECP256K1_BLAKE160},
    verify::{LockVerifier, VerifyError},
};

struct FailingVerifier;

impl LockVerifier for FailingVerifier {
    fn verify(
        &self,
        _auth: &AuthContext,
        _seal: &[u8],
        _signing_message_hash: &[u8; 32],
    ) -> Result<(), VerifyError> {
        Err(VerifyError::VerificationFailed)
    }
}

#[test]
fn verifier_trait_returns_verify_error() {
    let auth = AuthContext {
        kind: AUTH_KIND_SECP256K1_BLAKE160,
        identity: [0u8; 20],
    };
    assert_eq!(
        FailingVerifier.verify(&auth, &[0u8; 65], &[1u8; 32]),
        Err(VerifyError::VerificationFailed)
    );
}
```

- [x] **Step 2: Run tests and verify they fail**

Run:

```bash
cargo test -p cobuild-otx-lock --offline
```

Expected: FAIL because args/error/verify modules are not implemented yet.

- [x] **Step 3: Implement args, errors, verifier boundary**

Create `contracts/cobuild-otx-lock/src/args.rs`:

```rust
pub const AUTH_KIND_SECP256K1_BLAKE160: u8 = 0;
pub const AUTH_ARGS_LEN: usize = 21;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AuthContext {
    pub kind: u8,
    pub identity: [u8; 20],
}

pub fn parse_auth_args(args: &[u8]) -> Result<AuthContext, crate::error::Error> {
    if args.len() != AUTH_ARGS_LEN {
        return Err(crate::error::Error::InvalidArgs);
    }
    if args[0] != AUTH_KIND_SECP256K1_BLAKE160 {
        return Err(crate::error::Error::InvalidArgs);
    }
    let mut identity = [0u8; 20];
    identity.copy_from_slice(&args[1..]);
    Ok(AuthContext {
        kind: args[0],
        identity,
    })
}
```

Create `contracts/cobuild-otx-lock/src/error.rs`:

```rust
#[repr(i8)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExitCode {
    InvalidArgs = 1,
    MalformedCobuild = 2,
    LockSemanticFailure = 3,
    VerifyFailure = 4,
    SyscallFailure = 5,
    InternalFailure = 6,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Error {
    InvalidArgs,
    MalformedCobuild,
    LockSemanticFailure,
    VerifyFailure,
    SyscallFailure,
    InternalFailure,
}

impl Error {
    pub fn exit_code(&self) -> i8 {
        match self {
            Self::InvalidArgs => ExitCode::InvalidArgs as i8,
            Self::MalformedCobuild => ExitCode::MalformedCobuild as i8,
            Self::LockSemanticFailure => ExitCode::LockSemanticFailure as i8,
            Self::VerifyFailure => ExitCode::VerifyFailure as i8,
            Self::SyscallFailure => ExitCode::SyscallFailure as i8,
            Self::InternalFailure => ExitCode::InternalFailure as i8,
        }
    }
}
```

Create `contracts/cobuild-otx-lock/src/verify/mod.rs`:

```rust
pub mod local;

use crate::args::AuthContext;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifyError {
    InvalidSealEncoding,
    VerificationFailed,
    BackendUnavailable,
}

pub trait LockVerifier {
    fn verify(
        &self,
        auth: &AuthContext,
        seal: &[u8],
        signing_message_hash: &[u8; 32],
    ) -> Result<(), VerifyError>;
}
```

Create `contracts/cobuild-otx-lock/src/verify/local.rs`:

```rust
#[derive(Default)]
pub struct LocalVerifier;

impl crate::verify::LockVerifier for LocalVerifier {
    fn verify(
        &self,
        _auth: &crate::args::AuthContext,
        seal: &[u8],
        _signing_message_hash: &[u8; 32],
    ) -> Result<(), crate::verify::VerifyError> {
        if seal.len() != 65 {
            return Err(crate::verify::VerifyError::InvalidSealEncoding);
        }
        Err(crate::verify::VerifyError::BackendUnavailable)
    }
}
```

Replace `contracts/cobuild-otx-lock/src/lib.rs`:

```rust
#![no_std]
extern crate alloc;

pub mod args;
pub mod error;
pub mod verify;
```

- [x] **Step 4: Run unit tests**

Run:

```bash
cargo test -p cobuild-otx-lock --offline
```

Expected: PASS for args/error/verifier tests.

- [x] **Step 5: Add ckb-script-template contract Makefile**

Create `contracts/cobuild-otx-lock/Makefile`:

```make
RUSTFLAGS := -C target-feature=+zba,+zbb,+zbc,+zbs,-a $(CUSTOM_RUSTFLAGS)

build:
	RUSTFLAGS="$(RUSTFLAGS)" TARGET_CC="$(CLANG)" TARGET_AR="llvm-ar" \
		cargo build --target=riscv64imac-unknown-none-elf $(MODE_ARGS) $(CARGO_ARGS)
	cp ../../target/riscv64imac-unknown-none-elf/$(MODE)/cobuild-otx-lock $(BUILD_DIR)/
```

Replace `contracts/cobuild-otx-lock/src/main.rs`:

```rust
#![no_std]
#![no_main]

ckb_std::entry!(program_entry);

fn program_entry() -> i8 {
    match cobuild_otx_lock::entry::main() {
        Ok(()) => 0,
        Err(err) => err.exit_code(),
    }
}

ckb_std::default_alloc!();
```

Create `contracts/cobuild-otx-lock/src/entry.rs`:

```rust
pub fn main() -> Result<(), crate::error::Error> {
    Err(crate::error::Error::LockSemanticFailure)
}
```

Update `contracts/cobuild-otx-lock/src/lib.rs`:

```rust
#![no_std]
extern crate alloc;

pub mod args;
pub mod entry;
pub mod error;
pub mod verify;
```

Update `contracts/cobuild-otx-lock/Cargo.toml` dependencies:

```toml
ckb-std = { version = "0.16.4", default-features = false, features = ["allocator", "ckb-types", "dummy-atomic"] }
```

- [x] **Step 6: Build contract**

Run:

```bash
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
```

Expected: PASS and produce `build/debug/cobuild-otx-lock`.

- [x] **Step 7: Commit**

```bash
git add contracts/cobuild-otx-lock
git commit -m "feat: add cobuild otx lock contract shell"
```

## Task 6: Host Integration Harness

**Files:**
- Modify: `tests/Cargo.toml`
- Create/modify: `tests/src/lib.rs`
- Create: `tests/tests/cobuild_otx_lock.rs`

- [x] **Step 1: Write failing contract tests**

Create `tests/tests/cobuild_otx_lock.rs`:

```rust
use tests::fixtures;

#[test]
fn contract_rejects_invalid_args() {
    let result = fixtures::invalid_args_case().verify();
    assert!(result.is_err(), "{result:?}");
}

#[test]
fn contract_rejects_without_relevant_task() {
    let result = fixtures::no_relevant_task_case().verify();
    assert!(result.is_err(), "{result:?}");
}
```

Create `tests/src/lib.rs`:

```rust
pub mod fixtures {
    use ckb_testtool::{
        builtin::ALWAYS_SUCCESS,
        ckb_types::{
            bytes::Bytes,
            core::{ScriptHashType, TransactionBuilder},
            packed::{CellDep, CellInput, CellOutput, OutPoint, Script},
            prelude::*,
        },
        context::Context,
    };

    pub struct Case {
        context: Context,
        tx: ckb_testtool::ckb_types::core::TransactionView,
    }

    impl Case {
        pub fn verify(mut self) -> Result<u64, ckb_testtool::context::VerifyError> {
            self.context.verify_tx(&self.tx, 10_000_000)
        }
    }

    pub fn invalid_args_case() -> Case {
        build_case(Bytes::from(vec![0u8]))
    }

    pub fn no_relevant_task_case() -> Case {
        let mut args = vec![0u8];
        args.extend_from_slice(&[1u8; 20]);
        build_case(Bytes::from(args))
    }

    fn build_case(args: Bytes) -> Case {
        let mut context = Context::default();
        let contract_bin = std::fs::read("build/debug/cobuild-otx-lock")
            .expect("run make build CONTRACT=cobuild-otx-lock MODE=debug first");
        let contract_out_point = context.deploy_cell(contract_bin.into());
        let contract_dep = CellDep::new_builder()
            .out_point(contract_out_point.clone())
            .build();
        let lock = Script::new_builder()
            .code_hash(context.calc_data_hash(&contract_bin.into()))
            .hash_type(ScriptHashType::Data2.into())
            .args(args.pack())
            .build();
        let input_out_point = context.create_cell(
            CellOutput::new_builder()
                .capacity(1_000u64.pack())
                .lock(lock.clone())
                .build(),
            Bytes::new(),
        );
        let output = CellOutput::new_builder()
            .capacity(900u64.pack())
            .lock(always_success_script(&mut context))
            .build();
        let tx = TransactionBuilder::default()
            .cell_dep(contract_dep)
            .input(CellInput::new_builder().previous_output(input_out_point).build())
            .output(output)
            .output_data(Bytes::new().pack())
            .witness(Bytes::new().pack())
            .build();
        Case { context, tx }
    }

    fn always_success_script(context: &mut Context) -> Script {
        let out_point = context.deploy_cell(ALWAYS_SUCCESS.to_vec().into());
        Script::new_builder()
            .code_hash(context.calc_data_hash(&ALWAYS_SUCCESS.to_vec().into()))
            .hash_type(ScriptHashType::Data.into())
            .args(Bytes::new().pack())
            .build()
    }
}
```

- [x] **Step 2: Run tests and verify initial failure mode**

Run:

```bash
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: FAIL if the contract binary has not been built; after Task 5 build, tests should execute and return script errors.

- [x] **Step 3: Compile the fixture and keep Data2**

Run:

```bash
cargo test -p tests --offline --no-run
```

Expected: PASS. If this fails, fix only concrete compiler errors in `tests/src/lib.rs` while preserving these exact script hash type expressions:

```rust
ScriptHashType::Data2
ScriptHashType::Data
```

`ScriptHashType::Data2` is used for `cobuild-otx-lock`. `ScriptHashType::Data` is used for `always-success`.

- [x] **Step 4: Run invalid/no-task tests**

Run:

```bash
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: PASS for the two fail-closed tests.

- [x] **Step 5: Commit**

```bash
git add tests
git commit -m "test: add cobuild otx lock integration harness"
```

## Task 7: Complete Protocol Behavior Incrementally

**Files:**
- Modify: `crates/cobuild-core/src/{view.rs,witness.rs,layout.rs,hash.rs,context.rs,tasks.rs,loader.rs,error.rs}`
- Modify: `contracts/cobuild-otx-lock/src/{entry.rs,runner.rs,verify/local.rs,error.rs}`
- Modify: `tests/src/lib.rs`
- Modify: `tests/tests/cobuild_otx_lock.rs`

- [x] **Step 1: Add tx-level positive contract test**

Extend `tests/tests/cobuild_otx_lock.rs`:

```rust
#[test]
fn contract_accepts_tx_level_cobuild_signature() {
    let result = fixtures::signed_tx_level_case().verify();
    assert!(result.is_ok(), "{result:?}");
}
```

Add `fixtures::signed_tx_level_case()` to `tests/src/lib.rs` using `entity` helpers only in host tests to build a `SighashAllOnly` witness with a 65-byte seal.

- [x] **Step 2: Run and verify failure**

Run:

```bash
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock contract_accepts_tx_level_cobuild_signature -- --nocapture
```

Expected: FAIL because core does not yet generate tx-level tasks and local verifier does not verify signatures.

- [x] **Step 3: Implement tx-level task path**

Implement these concrete APIs:

```rust
// cobuild-core
pub struct PreparedContext { /* chain-facing context */ }
pub struct TxLevelLockTask {
    pub script_hash: [u8; 32],
    pub carrier_witness_index: usize,
    pub seal: Vec<u8>,
    pub signing_message_hash: [u8; 32],
}
impl CobuildContext {
    pub fn lock_query(&self, script_hash: [u8; 32]) -> LockScriptQuery<'_>;
}
impl LockScriptQuery<'_> {
    pub fn tx_tasks(&self, parts: &TxHashParts) -> Result<Vec<TxLevelLockTask>, CoreError>;
}
```

Update the lock runner so `entry` loads args/script hash, builds core context, queries tasks, and verifies them.

- [x] **Step 4: Implement local secp256k1 verifier**

Update `contracts/cobuild-otx-lock/src/verify/local.rs` to:

- reject seals not exactly 65 bytes;
- parse recovery id from byte 64;
- recover secp256k1 pubkey from `signing_message_hash`;
- compute `ckb_hash::blake2b_256(pubkey.serialize())`;
- compare the first 20 bytes to `AuthContext.identity`.

- [x] **Step 5: Run tx-level positive and negative tests**

Run:

```bash
cargo test -p cobuild-core --offline
cargo test -p cobuild-otx-lock --offline
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock contract_accepts_tx_level_cobuild_signature -- --nocapture
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock contract_rejects_invalid_args -- --nocapture
```

Expected: all PASS.

- [x] **Step 6: Commit tx-level behavior**

```bash
git add crates/cobuild-core contracts/cobuild-otx-lock tests
git commit -m "feat: verify tx-level cobuild lock tasks"
```

- [x] **Step 7: Add OTX and mixed tests**

Extend `tests/tests/cobuild_otx_lock.rs`:

```rust
#[test]
fn contract_accepts_otx_base_and_append_signatures() {
    let result = fixtures::signed_otx_dual_scope_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_accepts_mixed_tx_level_and_otx_tasks() {
    let result = fixtures::mixed_tx_and_otx_case().verify();
    assert!(result.is_ok(), "{result:?}");
}

#[test]
fn contract_rejects_bad_seal() {
    let result = fixtures::bad_seal_case().verify();
    assert!(result.is_err(), "{result:?}");
}

#[test]
fn contract_rejects_malformed_cobuild_witness() {
    let result = fixtures::malformed_cobuild_witness_case().verify();
    assert!(result.is_err(), "{result:?}");
}
```

- [x] **Step 8: Implement OTX layout/hash/task path**

Implement:

- OTX start scan;
- contiguous OTX witness scan;
- base and append range calculation;
- seal pair collection by scope;
- base hash and append hash;
- mixed tx-level plus OTX task verification.

Keep all OTX parsing in `cobuild-core`; the lock crate only consumes tasks.

- [x] **Step 9: Run full matrix**

Run:

```bash
cargo test -p cobuild-types --offline
cargo test -p cobuild-core --offline
cargo test -p cobuild-otx-lock --offline
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: all PASS.

- [x] **Step 10: Commit OTX behavior**

```bash
git add crates/cobuild-core contracts/cobuild-otx-lock tests
git commit -m "feat: verify otx cobuild lock tasks"
```

## Task 8: Final Review And Verification

**Files:**
- Modify only if verification reveals issues:
  - `Cargo.toml`
  - `Makefile`
  - `xtask`
  - `crates/*`
  - `contracts/*`
  - `tests/*`

- [x] **Step 1: Run codegen check**

Run:

```bash
cargo run -p xtask --offline -- codegen cobuild-types --check
```

Expected: PASS.

- [x] **Step 2: Run all Rust tests**

Run:

```bash
cargo test --workspace --offline
```

Expected: PASS.

- [x] **Step 3: Run contract build and integration tests**

Run:

```bash
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

Expected: PASS.

- [x] **Step 4: Check dependency boundary**

Run:

```bash
rg -n "cobuild_types::entity|::entity::" crates/cobuild-core contracts/cobuild-otx-lock
rg -n "critical-section|portable-atomic.*unsafe-assume-single-core|\\[patch.crates-io\\]" Cargo.toml crates contracts
```

Expected:

- First command prints no matches.
- Second command prints no matches unless a dependency lockfile mentions transitive packages outside source manifests.

- [x] **Step 5: Final commit**

```bash
git status --short
git add .
git commit -m "chore: verify clean cobuild otx contracts"
```

Only make this commit if there are verification fixes or generated files not already committed by earlier tasks.

## Self-Review

Spec coverage:

- Workspace autonomy is covered by Task 1.
- `cobuild-types` `lazy_reader` plus `entity` generation is covered by Task 2.
- `xtask` local codegen and check mode are covered by Task 2 and Task 8.
- `cobuild-core` reader boundary is covered by Task 3.
- Core layout/hash/context/task responsibilities are covered by Task 4 and Task 7.
- `cobuild-otx-lock` args/verifier/runner/exit codes are covered by Task 5 and Task 7.
- Contract integration matrix is covered by Task 6 and Task 7.
- No local `critical-section` shim and no `entity` in chain-facing core are covered by Task 8.

Placeholder scan:

- This plan does not use TODO/TBD placeholders.
- Task 7 groups protocol completion behind explicit contract tests and verification commands. When executing it, expand compiler-discovered generated reader method names into the implementation before committing that task.

Type consistency:

- Public module names are consistently `cobuild_types::lazy_reader` and `cobuild_types::entity`.
- Contract error codes are consistently represented by `ExitCode`.
- The verifier boundary uses `AuthContext`, `seal`, and `[u8; 32]` signing message hashes throughout.
