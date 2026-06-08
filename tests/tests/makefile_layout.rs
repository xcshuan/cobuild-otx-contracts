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
        "tests/contracts/test-input-type-proxy-lock",
    ] {
        assert!(
            stdout.contains(contract),
            "root Makefile must build test-only contract {contract}"
        );
    }
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
