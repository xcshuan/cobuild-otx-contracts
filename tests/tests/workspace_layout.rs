use std::fs;
use std::path::Path;

#[test]
fn workspace_declares_clean_cobuild_members() {
    let manifest_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../Cargo.toml");
    let manifest = fs::read_to_string(manifest_path).expect("workspace manifest");
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
    assert!(
        !manifest.contains("[patch.crates-io]\ncritical-section"),
        "clean workspace must not patch critical-section"
    );
}

#[test]
fn test_asset_contracts_live_under_tests_directory() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
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
}

#[test]
fn proxy_locks_live_under_tests_vendor_submodule() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let vendor_dir = workspace_root.join("tests/vendor/ckb-proxy-locks");
    let input_type_proxy_lock = vendor_dir.join("contracts/input-type-proxy-lock");

    assert!(
        vendor_dir.join(".git").exists() || vendor_dir.join(".git").is_file(),
        "ckb-proxy-locks must be checked out as a tests/vendor submodule"
    );
    assert!(
        input_type_proxy_lock.join("Cargo.toml").is_file(),
        "missing vendored input-type-proxy-lock manifest"
    );
    assert!(
        input_type_proxy_lock.join("Makefile").is_file(),
        "missing vendored input-type-proxy-lock Makefile"
    );
    assert!(
        !workspace_root
            .join("tests/contracts/test-input-type-proxy-lock")
            .exists(),
        "input-type-proxy-lock must be reused from tests/vendor/ckb-proxy-locks"
    );
}
