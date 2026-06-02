use std::{fs, path::Path};

#[test]
fn cobuild_otx_lock_uses_ckb_contract_template_scaffold() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let contract_dir = workspace_root.join("contracts/cobuild-otx-lock");

    for path in [".gitignore", "README.md", "Makefile", "Cargo.toml"] {
        assert!(
            contract_dir.join(path).is_file(),
            "missing template file {path}"
        );
    }

    let manifest = fs::read_to_string(contract_dir.join("Cargo.toml")).expect("contract manifest");
    assert!(
        manifest.contains("ckb-std"),
        "contract must depend on ckb-std"
    );
    assert!(
        manifest.contains("native-simulator"),
        "contract must expose native-simulator feature"
    );

    let main_rs = fs::read_to_string(contract_dir.join("src/main.rs")).expect("main.rs");
    assert!(
        main_rs.contains("ckb_std::entry!(program_entry);"),
        "contract main must use ckb_std entry macro"
    );
    assert!(
        main_rs.contains("ckb_std::default_alloc!"),
        "contract main must configure ckb-std allocator"
    );
}

#[test]
fn cobuild_otx_lock_entry_owns_contract_flow() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
    let contract_src = workspace_root.join("contracts/cobuild-otx-lock/src");

    assert!(
        !contract_src.join("runner.rs").exists(),
        "runner.rs must not own the contract entry flow"
    );

    let lib_rs = fs::read_to_string(contract_src.join("lib.rs")).expect("lib.rs");
    assert!(
        lib_rs.contains("mod loader"),
        "contract crate should keep chain-loading helpers in loader.rs"
    );
    assert!(
        !lib_rs.contains("mod chain"),
        "contract crate should not use the less precise chain module name"
    );
    assert!(
        !lib_rs.contains("pub mod runner"),
        "contract crate must not export a runner module"
    );

    let entry_rs = fs::read_to_string(contract_src.join("entry.rs")).expect("entry.rs");
    assert!(
        !entry_rs.contains("runner::run"),
        "entry.rs must not delegate the full contract flow to runner::run"
    );
    for expected in [
        "parse_auth_args",
        "load_current_script_args",
        "load_script_hash",
        "load_prepared_context",
        "lock_query",
        "LocalVerifier",
    ] {
        assert!(
            entry_rs.contains(expected),
            "entry.rs should expose the high-level contract flow via {expected}"
        );
    }
}
