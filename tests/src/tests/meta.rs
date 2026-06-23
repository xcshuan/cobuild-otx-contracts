use super::*;
fn rust_files_under(root: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(path) = stack.pop() {
        if path.is_dir() {
            for entry in std::fs::read_dir(&path).expect("read directory") {
                stack.push(entry.expect("directory entry").path());
            }
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    files
}

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
fn business_fixtures_do_not_live_in_framework_modules() {
    assert!(
        !std::path::Path::new("src/framework/limit_order.rs").exists(),
        "limit-order helpers are business fixtures and must live under tests/src/fixtures/"
    );
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
        "TransactionBuilder::default()",
        "Loader::default().load_binary",
        "build_script_with_hash_type",
        "WitnessLayout::from(SighashAllOnly",
        "fn write_count",
        "fn write_len_prefixed_bytes",
        "fn checked_len_prefix",
        "fn packed_hash_to_array",
        "fn range",
        "fn assert_lock_script_exit",
    ] {
        assert!(
            !source.contains(forbidden),
            "`{forbidden}` belongs in fixtures/framework, not in tests/cobuild_otx_lock.rs"
        );
    }
}

#[test]
fn limit_order_test_file_contains_no_fixture_scenario_builder() {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    let test_file = repo.join("tests/tests/limit_order_type.rs");
    let source = std::fs::read_to_string(&test_file).expect("read limit_order_type test file");

    for forbidden in ["fn limit_order_case", "fn failed_txs_count"] {
        assert!(
            !source.contains(forbidden),
            "`{forbidden}` belongs in fixtures/framework, not in tests/limit_order_type.rs"
        );
    }
}

#[test]
fn fixtures_do_not_redefine_framework_helpers() {
    let fixture_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/fixtures");
    let mut source = String::new();
    for path in rust_files_under(&fixture_dir) {
        source.push_str(&std::fs::read_to_string(path).expect("read fixture file"));
    }

    for forbidden in [
        "fn packed_hash_to_array",
        "fn sign_recoverable",
        "fn tx_without_message_hash",
        "fn tx_without_message_hash_for_inputs",
        "fn failed_txs_count",
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

#[test]
fn framework_does_not_depend_on_fixtures_or_named_test_contracts() {
    let framework_dir = std::path::Path::new("src/framework");
    for path in rust_files_under(framework_dir) {
        let source = std::fs::read_to_string(&path).expect("read framework source file");
        for forbidden in [
            "crate::fixtures",
            "fixtures::",
            "ALWAYS_SUCCESS",
            "deploy_test_lock",
            "deploy_always_success",
            "deploy_data2_script",
            "limit_order",
            "cobuild_otx_lock",
            "test-udt",
            "test-nft",
            "input-type-proxy-lock",
            "wrong-owner",
        ] {
            assert!(
                !source.contains(forbidden),
                "framework source {path:?} must not depend on fixture or named test-contract term `{forbidden}`"
            );
        }
    }
}

#[test]
fn fixtures_common_contracts_and_assets_are_usable() {
    let mut context = ckb_testtool::context::Context::default();

    let always_success_code =
        crate::fixtures::common::contracts::deploy_always_success_code(&mut context);
    let udt_code = crate::fixtures::common::contracts::deploy_test_udt_code(&mut context);
    let nft_code = crate::fixtures::common::contracts::deploy_test_nft_code(&mut context);
    let always_success = crate::fixtures::common::contracts::build_always_success_script(
        &mut context,
        &always_success_code,
        Vec::new(),
    );
    let udt = crate::fixtures::common::contracts::build_test_udt_script(
        &mut context,
        &udt_code,
        always_success.script_hash,
    );
    let nft =
        crate::fixtures::common::contracts::build_test_nft_script(&mut context, &nft_code, [7; 32]);

    assert_eq!(
        always_success.script_hash,
        crate::framework::scripts::script_hash(&always_success.script)
    );
    assert_eq!(
        udt.script.args().raw_data().as_ref(),
        always_success.script_hash.as_slice()
    );
    assert_eq!(nft.script.args().raw_data().as_ref(), &[7; 32]);
    assert_eq!(
        crate::fixtures::common::assets::udt_amount_data(30).as_ref(),
        &30u128.to_le_bytes()
    );
    let udt_asset = crate::fixtures::common::assets::TestUdt::from_deployed(&udt);
    let nft_asset = crate::fixtures::common::assets::TestNft::from_deployed(&nft);
    assert_eq!(udt_asset.script_hash, udt.script_hash);
    assert_eq!(udt_asset.script.as_slice(), udt.script.as_slice());
    assert_eq!(nft_asset.script_hash, nft.script_hash);
    assert_eq!(nft_asset.cell_dep.as_slice(), nft.cell_dep.as_slice());

    let personas = crate::fixtures::common::personas::Personas::default();
    assert_eq!(personas.owner.id.0, "owner");
    assert!(personas.owner.secret_key.is_some());
    assert_eq!(
        personas.owner.lock_hash,
        crate::framework::scripts::script_hash(&personas.owner.lock)
    );
    assert_eq!(personas.buyer.id.0, "buyer");
    assert_eq!(personas.wrong_owner.id.0, "wrong_owner");
    assert_eq!(personas.order_lock_owner.id.0, "order_lock_owner");

    assert_eq!(
        crate::fixtures::common::assets::nft_data(b"demo", [1, 2, 3, 4], 9).as_ref(),
        &[
            4, b'd', b'e', b'm', b'o', 1, 2, 3, 4, 9, 0, 0, 0, 0, 0, 0, 0
        ]
    );
}

#[test]
fn signing_hash_oracle_is_framework_owned() {
    let oracle_path = std::path::Path::new("src/framework/signing/oracle.rs");
    let oracle_source = if oracle_path.exists() {
        std::fs::read_to_string(oracle_path).expect("read signing hash oracle")
    } else {
        String::new()
    };

    assert!(
        oracle_source.contains("pub trait SigningHashOracle"),
        "signing hash oracle trait should be owned by tests/src/framework/signing/oracle.rs"
    );
    assert!(
        oracle_source.contains("pub struct TestSigningHashOracle"),
        "framework should expose a concrete test signing hash oracle"
    );
}

#[test]
fn expected_outcome_assertions_report_no_failed_tx_dump_delta() {
    let before = assertions::failed_txs_count();

    assertions::assert_no_failed_tx_dump_delta(before);
}
