use crate::{TestEnv, default_test_env};

#[test]
fn default_test_env_defaults_to_debug_build_when_mode_is_unset() {
    assert_eq!(default_test_env(), TestEnv::Debug);
}

#[test]
fn fixtures_live_in_dedicated_module_files() {
    let lib_rs = std::fs::read_to_string("src/lib.rs").expect("read tests src/lib.rs");

    assert!(
        !lib_rs.contains("pub mod fixtures {"),
        "fixtures should stay split under tests/src/fixtures/"
    );
    assert!(
        std::path::Path::new("src/fixtures/mod.rs").exists(),
        "fixtures module entry should exist"
    );
    let fixture_entries =
        std::fs::read_dir("src/fixtures").expect("read tests src/fixtures directory");
    for entry in fixture_entries {
        let path = entry.expect("fixture entry").path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        let line_count = std::fs::read_to_string(&path)
            .expect("read fixture file")
            .lines()
            .count();
        assert!(
            line_count <= 450,
            "fixture file {path:?} has {line_count} lines; split by responsibility"
        );
    }
}

#[test]
fn cobuild_otx_lock_test_file_contains_no_fixture_helpers() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let test_file = repo.join("tests/tests/cobuild_otx_lock.rs");
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
        "fn empty_message_entity",
        "fn always_success_script",
        "const TX_WITHOUT_MESSAGE_PERSONAL",
    ] {
        assert!(
            !source.contains(forbidden),
            "`{forbidden}` should be imported from tests::framework"
        );
    }
}
