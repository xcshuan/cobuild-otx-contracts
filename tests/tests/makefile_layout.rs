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
    for contract in ["tests/contracts/test-udt", "tests/contracts/test-nft"] {
        assert!(
            stdout.contains(contract),
            "root Makefile must build test-only contract {contract}"
        );
    }
}
