# cobuild-otx-contracts

Clean Cobuild OTX lock contract workspace for CKB.

## Layout

- `crates/cobuild-types`: Molecule schemas plus generated `lazy_reader` and `entity` modules.
- `crates/cobuild-core`: `no_std` Cobuild protocol logic, lazy-reader views, hashing, layout scanning, and task generation.
- `contracts/cobuild-otx-lock`: thin lock contract runner, args, verifier boundary, and exit code mapping.
- `tests`: CKB testtool integration fixtures for tx-level, OTX, mixed, and negative flows.
- `tests/contracts`: test-only asset contracts used by integration fixtures.
- `xtask`: local codegen entrypoint for `cobuild-types`.

## Common Commands

```bash
cargo run -p xtask --offline -- codegen cobuild-types --check
cargo test --workspace --offline
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
MODE=debug cargo test -p tests --offline --test cobuild_otx_lock -- --nocapture
```

To regenerate committed Cobuild type outputs:

```bash
cargo run -p xtask --offline -- codegen cobuild-types
```

## Contract Build

The root `Makefile` builds CKB RISC-V binaries into `build/<mode>/`.

```bash
make build CONTRACT=cobuild-otx-lock MODE=debug CARGO_ARGS=--offline
make build CONTRACT=cobuild-otx-lock MODE=release CARGO_ARGS=--offline
```

The integration test loader defaults to `build/debug` when `MODE` is not set, matching the debug build used by the test workflow.

## Design Boundaries

- `cobuild-types` keeps the crate name and exposes both `cobuild_types::lazy_reader` and `cobuild_types::entity`.
- Chain-facing `cobuild-core` and `cobuild-otx-lock` code must not depend on `cobuild_types::entity`.
- `cobuild-core` owns Cobuild witness parsing, OTX layout scanning, signing hash construction, and task generation.
- `cobuild-otx-lock` stays contract-specific: script args, runner orchestration, verifier boundary, and exit code mapping.
- The contract fixtures use `ScriptHashType::Data2` for `cobuild-otx-lock`.
- This workspace does not use a local `critical-section` shim or `portable-atomic` single-core assumption.

## References

- Main implementation spec: `docs/superpowers/specs/2026-05-29-clean-cobuild-otx-contracts-design.md`
- Implementation plan: `docs/superpowers/plans/2026-05-29-clean-cobuild-otx-contracts-implementation-plan.md`

This project was bootstrapped with [ckb-script-templates].

[ckb-script-templates]: https://github.com/cryptape/ckb-script-templates
