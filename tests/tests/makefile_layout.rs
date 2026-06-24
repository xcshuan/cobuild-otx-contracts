use std::{path::Path, process::Command};

#[test]
fn root_makefile_dry_run_handles_contract_shells_without_makefile() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let output = Command::new("make")
        .arg("-n")
        .arg("build")
        .current_dir(&workspace_root)
        .output()
        .expect("run make -n build");
    assert!(
        output.status.success(),
        "make -n build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn root_makefile_builds_test_only_contracts() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let output = Command::new("make")
        .arg("-n")
        .arg("build")
        .current_dir(&workspace_root)
        .output()
        .expect("run make -n build");
    assert!(
        output.status.success(),
        "make -n build failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    for contract in [
        "tests/contracts/limit-order-type",
        "tests/contracts/test-nft",
        "tests/contracts/test-udt",
    ] {
        assert!(
            stdout.contains(contract),
            "root Makefile must build test-only contract {contract}"
        );
    }
}

#[test]
fn root_makefile_builds_release_input_type_proxy_lock_before_contracts() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let makefile = std::fs::read_to_string(workspace_root.join("Makefile")).expect("Makefile");

    assert!(
        makefile.contains("test: build"),
        "root make test must prepare contract binaries before running cargo tests"
    );
    assert!(
        makefile.contains("tests/vendor/ckb-proxy-locks"),
        "root Makefile must build the vendored proxy lock dependency"
    );
    assert!(
        makefile.contains("CONTRACT=input-type-proxy-lock"),
        "root Makefile must build the input-type-proxy-lock contract"
    );
    assert!(
        makefile.contains("MODE=release"),
        "input-type-proxy-lock must always be built in release mode"
    );
    assert!(
        makefile.contains("BUILD_DIR=../../../build/release"),
        "vendored proxy lock must be copied to the root release build directory"
    );
    assert!(
        makefile.contains("CLEAN_BUILD_DIR_FIRST=false"),
        "vendored proxy lock build must not clean the root build directory"
    );
}

#[test]
fn limit_order_type_contract_build_uses_release_proxy_lock_hash() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let contract_dir = workspace_root.join("tests/contracts/limit-order-type");
    let manifest = std::fs::read_to_string(contract_dir.join("Cargo.toml")).expect("manifest");
    let makefile = std::fs::read_to_string(contract_dir.join("Makefile")).expect("Makefile");
    let xtask = std::fs::read_to_string(workspace_root.join("xtask/src/main.rs")).expect("xtask");

    assert!(
        !manifest.contains("default = [\"type-id\"]"),
        "limit-order-type must not enable ckb-std/type-id by default for workspace tests"
    );
    assert!(
        makefile.contains("CONTRACT_FEATURES := --features type-id"),
        "limit-order-type contract build must enable official ckb-std type-id validation"
    );
    assert!(
        makefile.contains("$(CONTRACT_FEATURES)"),
        "limit-order-type Makefile must pass contract features to cargo build"
    );
    assert!(
        makefile.contains("build/release/input-type-proxy-lock"),
        "limit-order-type Makefile must generate proxy lock hash from the release proxy lock"
    );
    assert!(
        makefile.contains(
            "PROXY_LOCK_MODE=\"release\" cargo run --offline -p xtask -- proxy-lock-code-hash limit-order-type"
        ),
        "limit-order-type Makefile must force release proxy lock hash generation"
    );
    assert!(
        !makefile.contains("build/debug/input-type-proxy-lock"),
        "limit-order-type Makefile must not hard-code debug proxy lock hash generation"
    );
    assert!(
        !makefile.contains("build/$(MODE)/input-type-proxy-lock"),
        "limit-order-type Makefile must not derive the proxy lock hash from active MODE"
    );
    assert!(
        xtask.contains("env::var(\"PROXY_LOCK_MODE\").unwrap_or_else(|_| \"release\".to_owned())"),
        "xtask proxy-lock-code-hash must default to release proxy lock mode"
    );
}

#[test]
fn fixture_loads_input_type_proxy_lock_from_release_build() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let fixture_contracts =
        std::fs::read_to_string(workspace_root.join("tests/src/fixtures/common/contracts.rs"))
            .expect("fixture contracts");

    assert!(
        fixture_contracts.contains("TestEnv::Release"),
        "input-type-proxy-lock fixture must load the release proxy lock binary"
    );
    assert!(
        fixture_contracts.contains("\"input-type-proxy-lock\""),
        "fixture must still deploy the vendored input-type-proxy-lock"
    );
}

#[test]
fn root_makefile_generate_handles_nested_destinations() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let makefile = std::fs::read_to_string(workspace_root.join("Makefile")).expect("Makefile");

    assert!(
        !makefile.contains("$(DESTINATION)\\/$(CRATE)"),
        "generate target must not put DESTINATION into a slash-delimited sed replacement"
    );
    assert!(
        makefile.contains(r#"s,$$,\n  "$(DESTINATION)/$(CRATE)",,"#),
        "generate target should use a delimiter-safe replacement for nested destinations"
    );
}
