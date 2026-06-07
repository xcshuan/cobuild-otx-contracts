# Cobuild OTX Test Fixture Framework Design

## Goal

Build a tests-only fixture framework under `tests/src/framework/` so Cobuild OTX integration tests can describe business scenarios without hand-writing Molecule witnesses, OTX layout counts, append permissions, cell deps, and full transaction assembly in every test.

This work keeps Limit Order behavior unchanged:

- valid append settlement passes
- insufficient append settlement fails with Limit Order exit code `11`

It does not add NFT-for-UDT swap semantics, Crowdfunding, NFT Minter, or AMM behavior.

## Context

`tests/tests/limit_order.rs` currently owns every layer of the setup:

- contract deployment
- lock/type script creation
- order and settlement cell data encoding
- Cobuild `Action`, `Message`, `OtxStart`, and `Otx` witness construction
- transaction assembly
- script exit assertion

The reference layout in `ref/repo/standard-udt-contracts/tests/src` separates common helpers from per-business fixtures. This repository should follow the same pattern, while adding Cobuild OTX-specific builders that match this project's core layout model.

`crates/cobuild-core/src/layout.rs` defines the OTX layout contract: `OtxStart` sets the starting input/output/cell_dep/header_dep indexes, and each `Otx` consumes base ranges before append ranges. `crates/cobuild-core/src/engine.rs` then derives lock/type validation plans from those ranges and Cobuild action targets. The framework should encode those defaults explicitly so tests remain readable and future fixtures can adjust them when needed.

## Architecture

Add `tests/src/framework/` and expose it from `tests/src/lib.rs` with `pub mod framework;`.

Modules:

- `contracts.rs`: deploy test contracts into a `Context`; return a `DeployedScript` containing `out_point`, `script`, `script_hash`, and `cell_dep`. Provide convenience helpers for `always-success` and `limit-order`, plus a generic `deploy_data2_script(name, args)` that future tests can use for `test-udt` and `test-nft` without adding their business semantics here.
- `scripts.rs`: small script utilities such as `script_hash()` and packed hash conversion.
- `cells.rs`: reusable cell helpers: normal/typed output builders, live input creation, Limit Order order data, settlement data, and a `TestCellOutput { cell, data }` wrapper so output cells and output data stay aligned.
- `cobuild.rs`: builders for Cobuild `Action`, `Message`, `OtxStart`, and `Otx`. Defaults should cover the current Limit Order shape: one base input, zero base outputs, one append output, no append inputs, and `allow_append_outputs()`.
- `tx.rs`: full transaction builder for common OTX test shapes. It should keep witness ordering and cell dep ordering outside business tests, calculate `OtxStart` indexes from the transaction segments it owns, and keep output data paired with outputs.
- `assertions.rs`: pass/fail assertions. Passing tests dump failed tx JSON on unexpected failure. Expected failure assertions do not dump by default; they dump only when `COBUILD_TEST_DUMP_EXPECTED_FAILURES=1`.
- `fixture.rs`: high-level `CobuildTestFixture` that owns `Context` and composes the lower-level builders.

## OTX Layout Rules

The transaction builder must preserve the layout invariants enforced by `cobuild-core`:

- exactly one `OtxStart` witness for a built OTX transaction
- at least one `Otx` witness after `OtxStart`
- `OtxStart` and all `Otx` witnesses are contiguous
- every `Otx` has at least one base input
- base ranges are emitted before append ranges for inputs, outputs, cell deps, and header deps
- computed ranges stay within the final transaction counts

For the current Limit Order tests, deployed contract cell deps are outside OTX scope. `start_cell_deps` is therefore the number of already-added contract deps, and the OTX itself has zero base/append cell deps. Future tests that need base/append cell deps should use explicit `base_cell_dep()` or `append_cell_dep()` APIs rather than relying on deployment order.

## API Shape

The exact names can follow Rust ergonomics, but `tests/tests/limit_order.rs` should end up close to:

```rust
let mut fixture = CobuildTestFixture::new();
let contracts = fixture.deploy_contracts(["limit-order", "always-success"]);

let owner = contracts.always_success.script.clone();
let order = fixture
    .limit_order()
    .owner(owner.clone())
    .offered_asset_id([3; 32])
    .requested_asset_id([4; 32])
    .offered_remaining(10)
    .min_requested_per_offered(3)
    .build_input(&contracts.limit_order.script);

let fill = fixture
    .cobuild()
    .input_type_action(contracts.limit_order.script_hash)
    .limit_order_fill([1; 32], [4; 32], 10, 30);

let otx = fixture
    .otx()
    .base_input(order)
    .append_output(settlement_output)
    .allow_append_outputs()
    .message(fill)
    .build();

let tx = fixture.tx().with_otx(otx).build();

fixture.assert_pass(&tx);
```

The framework does not need to force this exact spelling. The acceptance criterion is that the integration test no longer manually calls `Otx::new_builder()`, `OtxStart::new_builder()`, or `TransactionBuilder::default()` for the common case.

`settlement_output` in the example is a `TestCellOutput`, not a bare `CellOutput`; it carries both the output cell and the settlement data bytes.

## Dump Policy

`verify_and_dump_failed_tx()` remains available for unexpected failure debugging. New assertion helpers use two paths:

- `assert_pass`: verify with dump-on-failure
- `assert_type_script_exit`: verify directly, assert the expected input type script index and exit code, and skip dump unless `COBUILD_TEST_DUMP_EXPECTED_FAILURES=1`

This prevents known negative tests from continually writing `tests/failed_txs/*.json`.

## Scope Constraints

- Only modify `tests` code and necessary test/workspace manifests.
- Do not modify `contracts/cobuild-otx-lock` production code.
- Do not modify `cobuild-core` behavior for this task.
- Do not introduce a heavy offchain SDK.
- Preserve existing Limit Order behavior and OTX base/append scope coverage.
