use crate::{
    TestEnv, default_test_env,
    fixtures::limit_order::{
        ActionSourceKind, BusinessMutation, FlowKind, LimitOrderAction, LimitOrderHappyPath,
        LimitOrderLockError, LimitOrderState, LimitOrderTypeError, OtxScopeKind, ScriptRoleKind,
        encode_action, order_data,
    },
    framework::{
        assertions,
        cells::{ResolvedInputFacts, TestCellOutput, normal_output},
        cobuild::{CobuildMessageBuilder, OtxBuilder, OtxStartSpec, seal_pair},
        scenario::{ExpectedOutcome, ScriptLocation},
        scripts::packed_hash_to_array,
        signing::{
            SignatureScope, SignerId, SigningHashOracle, TestSigningHashOracle,
            assert_hash_changed, fixed_secret_key, sign_scope, tx_without_message_hash_for_inputs,
        },
        tx::{
            BuiltTxShape, HeaderDepHandle, OtxSegment, ProtocolMutation, TxShape, TxShapeMutation,
            WitnessHandle,
        },
    },
};
use ckb_testtool::{
    ckb_script::ScriptError,
    ckb_types::{
        bytes::Bytes,
        packed::{CellDep, CellInput, CellOutput, OutPoint, Script},
        prelude::{Builder, Entity, Pack},
    },
};
use cobuild_types::entity::{
    blockchain::Uint32,
    core::Otx,
    witness::{WitnessLayout, WitnessLayoutUnion},
};

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
fn framework_otx_builder_defaults_to_neutral_layout() {
    let built = crate::framework::cobuild::OtxBuilder::new().build_with_layout();

    assert_eq!(built.base_input_cells, 0);
    assert_eq!(built.base_output_cells, 0);
    assert_eq!(built.base_cell_deps, 0);
    assert_eq!(built.base_header_deps, 0);
    assert_eq!(built.append_input_cells, 0);
    assert_eq!(built.append_output_cells, 0);
    assert_eq!(built.append_cell_deps, 0);
    assert_eq!(built.append_header_deps, 0);
}

#[test]
fn cobuild_protocol_builders_encode_raw_permissions_and_masks() {
    let built = crate::framework::cobuild::OtxBuilder::new()
        .append_permissions_raw(0x10)
        .base_input_masks_raw(vec![0xff])
        .build_with_layout();

    assert_eq!(built.otx.append_permissions().as_slice(), &[0x10]);
    assert_eq!(built.otx.base_input_masks().raw_data().as_ref(), &[0xff]);
}

#[test]
fn cobuild_protocol_builders_set_append_dep_permission_bits() {
    let built = crate::framework::cobuild::OtxBuilder::new()
        .allow_append_cell_deps()
        .allow_append_header_deps()
        .build_with_layout();

    assert_eq!(built.otx.append_permissions().as_slice(), &[0b1100]);
}

#[test]
fn cobuild_protocol_builders_size_semantic_base_masks() {
    let input_masks = crate::framework::cobuild::OtxBuilder::new()
        .base_input_cells(5)
        .build()
        .base_input_masks();
    assert_eq!(input_masks.len(), 2);

    let cell_dep_masks = crate::framework::cobuild::OtxBuilder::new()
        .base_cell_deps(9)
        .build()
        .base_cell_dep_masks();
    assert_eq!(cell_dep_masks.len(), 2);

    let header_dep_masks = crate::framework::cobuild::OtxBuilder::new()
        .base_header_deps(9)
        .build()
        .base_header_dep_masks();
    assert_eq!(header_dep_masks.len(), 2);
}

#[test]
fn cobuild_protocol_builders_preserve_malformed_raw_mask_lengths() {
    let cell_dep_masks = crate::framework::cobuild::OtxBuilder::new()
        .base_cell_deps(9)
        .base_cell_dep_masks_raw(vec![0xff])
        .build()
        .base_cell_dep_masks();

    assert_eq!(cell_dep_masks.len(), 1);
    assert_eq!(cell_dep_masks.raw_data().as_ref(), &[0xff]);
}

#[test]
fn cobuild_protocol_builders_cover_and_uncover_input_mask_bits() {
    let covered = crate::framework::cobuild::OtxBuilder::new()
        .base_input_cells(1)
        .cover_base_input_since(0)
        .cover_base_input_previous_output(0)
        .build_with_layout();
    assert_eq!(
        covered.otx.base_input_masks().raw_data().as_ref(),
        &[0b0011]
    );

    let uncovered = crate::framework::cobuild::OtxBuilder::new()
        .base_input_cells(1)
        .cover_base_input_since(0)
        .cover_base_input_previous_output(0)
        .uncover_base_input_since(0)
        .uncover_base_input_previous_output(0)
        .build_with_layout();
    assert_eq!(uncovered.otx.base_input_masks().raw_data().as_ref(), &[0]);
}

#[test]
fn cobuild_protocol_builders_cover_and_uncover_output_mask_bits() {
    let uncovered = crate::framework::cobuild::OtxBuilder::new()
        .base_output_cells(1)
        .uncover_base_output_capacity(0)
        .uncover_base_output_lock(0)
        .uncover_base_output_type(0)
        .uncover_base_output_data(0)
        .build_with_layout();
    assert_eq!(
        uncovered.otx.base_output_masks().raw_data().as_ref(),
        &[0b0000]
    );

    let covered = crate::framework::cobuild::OtxBuilder::new()
        .base_output_cells(1)
        .uncover_base_output_capacity(0)
        .uncover_base_output_lock(0)
        .uncover_base_output_type(0)
        .uncover_base_output_data(0)
        .cover_base_output_capacity(0)
        .cover_base_output_lock(0)
        .cover_base_output_type(0)
        .cover_base_output_data(0)
        .build_with_layout();
    assert_eq!(
        covered.otx.base_output_masks().raw_data().as_ref(),
        &[0b1111]
    );
}

#[test]
fn cobuild_protocol_builders_encode_custom_otx_start_spec() {
    let witness = crate::framework::cobuild::OtxStartSpec {
        start_input_cell: 1,
        start_output_cell: 2,
        start_cell_deps: 3,
        start_header_deps: 4,
    }
    .encode();

    assert!(
        witness
            .windows(4)
            .any(|window| window == 1u32.to_le_bytes())
    );
    assert!(
        witness
            .windows(4)
            .any(|window| window == 2u32.to_le_bytes())
    );
    assert!(
        witness
            .windows(4)
            .any(|window| window == 3u32.to_le_bytes())
    );
    assert!(
        witness
            .windows(4)
            .any(|window| window == 4u32.to_le_bytes())
    );
}

#[test]
fn cobuild_protocol_builders_preserve_raw_cobuild_witness_bytes() {
    let raw = ckb_testtool::ckb_types::bytes::Bytes::from(vec![0xde, 0xad, 0xbe, 0xef]);
    let encoded = crate::framework::cobuild::WitnessSpec::RawCobuild(raw.clone()).encode();

    assert_eq!(encoded, raw);
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

    let always_success =
        crate::fixtures::common::contracts::deploy_always_success(&mut context, Vec::new());
    let udt = crate::fixtures::common::contracts::deploy_test_udt(
        &mut context,
        always_success.script_hash,
    );
    let nft = crate::fixtures::common::contracts::deploy_test_nft(&mut context, [7; 32]);

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

fn signing_test_script(tag: u8) -> Script {
    Script::new_builder()
        .args(Bytes::from(vec![tag]).pack())
        .build()
}

fn signing_resolved_input(tag: u8, data: impl Into<Bytes>) -> ResolvedInputFacts {
    let lock = signing_test_script(tag);
    let output = normal_output(lock.clone(), 1_000 + u64::from(tag));
    let input = CellInput::new_builder()
        .previous_output(OutPoint::new([tag; 32].pack(), 0))
        .build();

    ResolvedInputFacts {
        input,
        output,
        data: data.into(),
        lock_hash: [tag; 32],
        type_hash: None,
    }
}

fn signing_output(tag: u8, data: impl Into<Bytes>) -> TestCellOutput {
    TestCellOutput::new(
        CellOutput::new_builder()
            .capacity(2_000 + u64::from(tag))
            .lock(signing_test_script(tag))
            .build(),
        data,
    )
}

#[test]
fn limit_order_fill_action_encodes_payment_output_handle_tx_index() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, Bytes::new())],
        base_outputs: vec![signing_output(2, Bytes::new())],
        append_outputs: vec![
            signing_output(3, Bytes::new()),
            signing_output(4, Bytes::new()),
        ],
        ..Default::default()
    });
    let payment = shape.otx_append_output(otx, 1);
    let built = shape.build();

    let encoded = encode_action(
        &LimitOrderAction::Fill {
            payment,
            buyer_lock_hash: [0x42; 32],
        },
        &built,
    );

    assert_eq!(built.outputs.tx_index(payment), 2);
    assert_eq!(encoded[0], 2);
    assert_eq!(&encoded[1..5], &2u32.to_le_bytes());
    assert_eq!(&encoded[5..37], &[0x42; 32]);
}

#[test]
fn limit_order_create_action_encodes_order_state() {
    let built = TxShape::new().build();
    let order = LimitOrderState {
        owner_lock_hash: [1; 32],
        offered_nft_type_hash: [2; 32],
        requested_asset_id: [3; 32],
        requested_amount: 30,
    };

    let encoded = encode_action(&LimitOrderAction::Create { order }, &built);

    assert_eq!(encoded[0], 1);
    assert_eq!(&encoded[1..], order_data(order).as_ref());
}

#[test]
fn limit_order_unknown_action_uses_unknown_tag() {
    let built = TxShape::new().build();
    let encoded = encode_action(&LimitOrderAction::UnknownTag, &built);

    assert_ne!(encoded[0], 1);
    assert_ne!(encoded[0], 2);
}

#[test]
fn limit_order_duplicate_payment_is_expressed_by_reusing_the_same_output_handle() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, Bytes::new())],
        append_outputs: vec![signing_output(2, Bytes::new())],
        ..Default::default()
    });
    let shared_payment = shape.otx_append_output(otx, 0);
    let built = shape.build();

    let first = encode_action(
        &LimitOrderAction::Fill {
            payment: shared_payment,
            buyer_lock_hash: [0x11; 32],
        },
        &built,
    );
    let second = encode_action(
        &LimitOrderAction::Fill {
            payment: shared_payment,
            buyer_lock_hash: [0x22; 32],
        },
        &built,
    );

    assert_eq!(&first[1..5], &second[1..5]);
    assert_eq!(
        u32::from_le_bytes(first[1..5].try_into().expect("payment index")),
        built.outputs.tx_index(shared_payment) as u32
    );
}

#[test]
fn limit_order_payment_handles_can_point_outside_current_otx_output_range() {
    let mut shape = TxShape::new();
    let current_otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, Bytes::new())],
        append_outputs: vec![signing_output(2, Bytes::new())],
        ..Default::default()
    });
    let current_payment = shape.otx_append_output(current_otx, 0);
    let other_otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(3, Bytes::new())],
        append_outputs: vec![signing_output(4, Bytes::new())],
        ..Default::default()
    });
    let other_otx_payment = shape.otx_append_output(other_otx, 0);
    let remainder_payment = shape.push_remainder_output(signing_output(5, Bytes::new()));
    let built = shape.build();
    let current_range = &built.otx_ranges[0];

    assert!(
        current_range
            .append_outputs
            .contains(&built.outputs.tx_index(current_payment))
    );
    for payment in [other_otx_payment, remainder_payment] {
        let index = built.outputs.tx_index(payment);
        assert!(!current_range.base_outputs.contains(&index));
        assert!(!current_range.append_outputs.contains(&index));

        let encoded = encode_action(
            &LimitOrderAction::Fill {
                payment,
                buyer_lock_hash: [0x33; 32],
            },
            &built,
        );
        assert_eq!(
            u32::from_le_bytes(encoded[1..5].try_into().expect("payment index")),
            index as u32
        );
    }
}

#[test]
fn limit_order_error_mappings_match_contract_exit_codes() {
    assert_eq!(LimitOrderTypeError::InputAndOutputGroupShape.code(), 5);
    assert_eq!(LimitOrderTypeError::StateActionMismatch.code(), 10);
    assert_eq!(LimitOrderTypeError::InvalidPayment.code(), 11);
    assert_eq!(LimitOrderTypeError::InvalidAction.code(), 12);
    assert_eq!(LimitOrderTypeError::InvalidTypeId.code(), 14);

    assert_eq!(LimitOrderLockError::MalformedArgs.code(), 5);
    assert_eq!(LimitOrderLockError::MalformedAction.code(), 6);
    assert_eq!(LimitOrderLockError::UnknownActionTag.code(), 7);
    assert_eq!(LimitOrderLockError::WrongNftType.code(), 8);
    assert_eq!(LimitOrderLockError::InvalidPayment.code(), 10);
    assert_eq!(LimitOrderLockError::InvalidAction.code(), 12);
}

#[test]
fn limit_order_happy_path_coverage_has_full_tag_shape() {
    let tag = LimitOrderHappyPath::TwoTypeOrders
        .default_coverage()
        .with_mutation(BusinessMutation::ReusePaymentOutput);

    assert_eq!(tag.flow, FlowKind::OtxOnly);
    assert_eq!(tag.script_role, ScriptRoleKind::InputType);
    assert_eq!(tag.otx_scope, OtxScopeKind::BaseInput);
    assert_eq!(tag.action_source, ActionSourceKind::Duplicate);
    assert_eq!(tag.mutation, Some(BusinessMutation::ReusePaymentOutput));
}

fn signing_cell_dep(tag: u8) -> CellDep {
    CellDep::new_builder()
        .out_point(OutPoint::new([tag; 32].pack(), 0))
        .build()
}

fn witness_bytes(built: &BuiltTxShape, witness: WitnessHandle) -> Bytes {
    built
        .tx
        .witnesses()
        .into_iter()
        .nth(built.witnesses.tx_index(witness))
        .expect("witness by handle")
        .raw_data()
}

fn otx_witness(built: &BuiltTxShape, otx: crate::framework::tx::OtxHandle) -> Otx {
    let witness = witness_bytes(built, built.otx_witness(otx));
    match WitnessLayout::from_slice(witness.as_ref())
        .expect("parse witness layout")
        .to_enum()
    {
        WitnessLayoutUnion::Otx(otx) => otx,
        other => panic!("expected OTX witness, got {}", other.item_name()),
    }
}

#[test]
fn signing_hash_oracle_otx_uses_remapped_witness_handle() {
    let secret_key = fixed_secret_key(1);
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();

    built.apply_protocol_mutation(ProtocolMutation::NonContiguousOtxWitness);

    let facts = sign_scope(
        &built,
        &TestSigningHashOracle,
        SignerId("owner"),
        &secret_key,
        [9; 32],
        built.otx_witness(otx),
        SignatureScope::OtxBase { otx },
    );
    assert_eq!(facts.carrier, built.otx_witness(otx));
}

fn molecule_u32(value: Uint32) -> u32 {
    u32::from_le_bytes(value.as_slice().try_into().expect("uint32 bytes"))
}

fn signing_replace_otx_witness(mut built: BuiltTxShape, otx_witness: Bytes) -> BuiltTxShape {
    let mut witnesses: Vec<_> = built.tx.witnesses().into_iter().collect();
    witnesses[1] = otx_witness.pack();
    built.tx = built
        .tx
        .as_advanced_builder()
        .set_witnesses(witnesses)
        .build();
    built
}

fn signing_otx_witness_with_append_output_count(append_output_cells: u32) -> Bytes {
    let otx = OtxBuilder::new()
        .base_input_cells(1)
        .raw_append_output_cells(append_output_cells)
        .allow_append_outputs()
        .build();
    let witness = WitnessLayout::from(otx);
    Bytes::copy_from_slice(witness.as_slice())
}

fn signing_otx_witness_with_message_and_seal() -> (Bytes, Otx) {
    signing_otx_witness_with_message_seal_and_outputs(2, 2)
}

fn signing_otx_witness_with_message_seal_and_outputs(
    base_output_cells: u32,
    append_output_cells: u32,
) -> (Bytes, Otx) {
    let message = CobuildMessageBuilder::new()
        .input_lock_action([9; 32])
        .action_data(vec![1, 2, 3])
        .build();
    let seal = seal_pair([7; 32], 0x42, vec![0xaa, 0xbb, 0xcc]);
    let otx = OtxBuilder::new()
        .message(message)
        .append_permissions_raw(0x0b)
        .base_input_cells(1)
        .base_input_masks_raw(vec![0x03])
        .base_output_cells(base_output_cells)
        .base_output_masks_raw(vec![0xa5])
        .append_input_cells(1)
        .append_output_cells(append_output_cells)
        .seals(vec![seal])
        .build();
    let witness = WitnessLayout::from(otx.clone());

    (Bytes::copy_from_slice(witness.as_slice()), otx)
}

fn assert_same_message_seals_and_permissions(mutated: &Otx, original: &Otx) {
    assert_eq!(
        mutated.message().as_slice(),
        original.message().as_slice(),
        "message"
    );
    assert_eq!(
        mutated.seals().as_slice(),
        original.seals().as_slice(),
        "seals"
    );
    assert_eq!(
        mutated.append_permissions().as_slice(),
        original.append_permissions().as_slice(),
        "append permissions"
    );
}

fn assert_same_base_inputs(mutated: &Otx, original: &Otx) {
    assert_eq!(
        molecule_u32(mutated.base_input_cells()),
        molecule_u32(original.base_input_cells()),
        "base input cells"
    );
    assert_eq!(
        mutated.base_input_masks().raw_data().as_ref(),
        original.base_input_masks().raw_data().as_ref(),
        "base input masks"
    );
}

fn assert_same_base_outputs(mutated: &Otx, original: &Otx) {
    assert_eq!(
        molecule_u32(mutated.base_output_cells()),
        molecule_u32(original.base_output_cells()),
        "base output cells"
    );
    assert_eq!(
        mutated.base_output_masks().raw_data().as_ref(),
        original.base_output_masks().raw_data().as_ref(),
        "base output masks"
    );
}

fn assert_same_append_counts(mutated: &Otx, original: &Otx) {
    assert_eq!(
        molecule_u32(mutated.append_input_cells()),
        molecule_u32(original.append_input_cells()),
        "append input cells"
    );
    assert_eq!(
        molecule_u32(mutated.append_output_cells()),
        molecule_u32(original.append_output_cells()),
        "append output cells"
    );
    assert_eq!(
        molecule_u32(mutated.append_cell_deps()),
        molecule_u32(original.append_cell_deps()),
        "append cell deps"
    );
    assert_eq!(
        molecule_u32(mutated.append_header_deps()),
        molecule_u32(original.append_header_deps()),
        "append header deps"
    );
}

#[test]
fn mutation_move_output_to_remainder_keeps_output_handle_stable() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        append_outputs: vec![signing_output(3, vec![0xcc]), signing_output(4, vec![0xdd])],
        ..Default::default()
    });
    let moved_output = shape.otx_append_output(otx, 0);
    let mut built = shape.build();
    let old_index = built.outputs.tx_index(moved_output);
    assert_eq!(old_index, 1);

    assert_eq!(
        built.apply_shape_mutation(TxShapeMutation::MoveOutputToRemainder {
            output: moved_output,
        }),
        None
    );

    let new_index = built.outputs.tx_index(moved_output);
    assert_eq!(new_index, built.tx.outputs().len() - 1);
    assert_eq!(
        built.outputs.handle_at_tx_index(new_index),
        Some(moved_output)
    );
    assert!(!built.otx_ranges[0].append_outputs.contains(&new_index));
    assert_eq!(built.otx_ranges[0].append_outputs, 1..2);
}

#[test]
fn mutation_move_append_output_to_remainder_updates_otx_witness_count_for_signing_oracle() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_outputs: vec![signing_output(3, vec![0xcc]), signing_output(4, vec![0xdd])],
        ..Default::default()
    });
    let moved_output = shape.otx_append_output(otx, 0);
    let mut built = shape.build();

    built.apply_shape_mutation(TxShapeMutation::MoveOutputToRemainder {
        output: moved_output,
    });

    assert_eq!(
        molecule_u32(otx_witness(&built, otx).append_output_cells()),
        1
    );
    let oracle = TestSigningHashOracle;
    let actual_hash = oracle.otx_append(&built, otx, [3; 32]);
    let expected = signing_replace_otx_witness(
        built.clone(),
        signing_otx_witness_with_append_output_count(1),
    );
    assert_eq!(actual_hash, oracle.otx_append(&expected, otx, [3; 32]));
}

#[test]
fn mutation_move_base_output_to_remainder_updates_otx_witness_count_and_mask() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb]), signing_output(3, vec![0xcc])],
        ..Default::default()
    });
    let moved_output = shape.otx_base_output(otx, 0);
    let mut built = shape.build();

    built.apply_shape_mutation(TxShapeMutation::MoveOutputToRemainder {
        output: moved_output,
    });

    let mutated = otx_witness(&built, otx);
    assert_eq!(molecule_u32(mutated.base_output_cells()), 1);
    assert_eq!(mutated.base_output_masks().raw_data().as_ref(), &[0x0f]);
    TestSigningHashOracle.otx_base(&built, otx);
}

#[test]
fn mutation_move_append_output_to_remainder_preserves_non_target_otx_witness_fields() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_inputs: vec![signing_resolved_input(5, vec![0xee])],
        base_outputs: vec![signing_output(2, vec![0xbb]), signing_output(3, vec![0xcc])],
        append_outputs: vec![signing_output(4, vec![0xdd]), signing_output(6, vec![0xff])],
        ..Default::default()
    });
    let moved_output = shape.otx_append_output(otx, 0);
    let (witness, original_otx) = signing_otx_witness_with_message_seal_and_outputs(2, 2);
    let mut built = signing_replace_otx_witness(shape.build(), witness);

    built.apply_shape_mutation(TxShapeMutation::MoveOutputToRemainder {
        output: moved_output,
    });

    let mutated = otx_witness(&built, otx);
    assert_eq!(molecule_u32(mutated.append_output_cells()), 1);
    assert_same_message_seals_and_permissions(&mutated, &original_otx);
    assert_same_base_inputs(&mutated, &original_otx);
    assert_same_base_outputs(&mutated, &original_otx);
    assert_eq!(
        molecule_u32(mutated.append_input_cells()),
        molecule_u32(original_otx.append_input_cells()),
        "append input cells"
    );
    assert_eq!(
        molecule_u32(mutated.append_cell_deps()),
        molecule_u32(original_otx.append_cell_deps()),
        "append cell deps"
    );
    assert_eq!(
        molecule_u32(mutated.append_header_deps()),
        molecule_u32(original_otx.append_header_deps()),
        "append header deps"
    );
}

#[test]
fn mutation_move_base_output_to_remainder_preserves_non_target_otx_witness_fields() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_inputs: vec![signing_resolved_input(5, vec![0xee])],
        base_outputs: vec![signing_output(2, vec![0xbb]), signing_output(3, vec![0xcc])],
        append_outputs: vec![signing_output(4, vec![0xdd]), signing_output(6, vec![0xff])],
        ..Default::default()
    });
    let moved_output = shape.otx_base_output(otx, 0);
    let (witness, original_otx) = signing_otx_witness_with_message_seal_and_outputs(2, 2);
    let mut built = signing_replace_otx_witness(shape.build(), witness);

    built.apply_shape_mutation(TxShapeMutation::MoveOutputToRemainder {
        output: moved_output,
    });

    let mutated = otx_witness(&built, otx);
    assert_eq!(molecule_u32(mutated.base_output_cells()), 1);
    assert_eq!(mutated.base_output_masks().raw_data().as_ref(), &[0x0a]);
    assert_same_message_seals_and_permissions(&mutated, &original_otx);
    assert_same_base_inputs(&mutated, &original_otx);
    assert_same_append_counts(&mutated, &original_otx);
}

#[test]
fn expected_outcome_output_type_resolves_moved_output_after_mutation() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        append_outputs: vec![signing_output(3, vec![0xcc]), signing_output(4, vec![0xdd])],
        ..Default::default()
    });
    let moved_output = shape.otx_append_output(otx, 0);
    let mut built = shape.build();
    built.apply_shape_mutation(TxShapeMutation::MoveOutputToRemainder {
        output: moved_output,
    });
    let current_index = built.outputs.tx_index(moved_output);
    let error = ScriptError::ValidationFailure("by convention".to_owned(), 14)
        .output_type_script(current_index)
        .into();

    ExpectedOutcome::ScriptExit {
        location: ScriptLocation::OutputType(moved_output),
        code: 14,
    }
    .assert_result(Err(error), &built);
}

#[test]
fn mutation_replace_witness_updates_bytes_through_witness_handle() {
    let mut shape = TxShape::new();
    shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();
    let otx_witness = WitnessHandle::from_raw(1);
    let replacement = Bytes::from(vec![0xde, 0xad, 0xbe, 0xef]);

    built.apply_shape_mutation(TxShapeMutation::ReplaceWitness {
        witness: otx_witness,
        replacement: replacement.clone(),
    });

    assert_eq!(witness_bytes(&built, otx_witness), replacement);
}

#[test]
fn mutation_otx_start_raw_replaces_start_witness_bytes() {
    let mut shape = TxShape::new();
    shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();
    let start_witness = WitnessHandle::from_raw(0);
    let replacement = OtxStartSpec {
        start_input_cell: 1,
        start_output_cell: 2,
        start_cell_deps: 3,
        start_header_deps: 4,
    }
    .encode();

    built.apply_protocol_mutation(ProtocolMutation::OtxStartRaw(OtxStartSpec {
        start_input_cell: 1,
        start_output_cell: 2,
        start_cell_deps: 3,
        start_header_deps: 4,
    }));

    assert_eq!(witness_bytes(&built, start_witness), replacement);
}

#[test]
fn mutation_otx_start_raw_uses_start_handle_after_tx_level_witness() {
    let mut shape = TxShape::new();
    shape.tx_level_message(
        CobuildMessageBuilder::new()
            .input_lock_action([1; 32])
            .action_data(vec![1])
            .build(),
    );
    shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();
    let tx_level_witness = WitnessHandle::from_raw(0);
    let tx_level_before = witness_bytes(&built, tx_level_witness);
    let start_witness = built.otx_start_witness();
    let replacement = OtxStartSpec {
        start_input_cell: 9,
        start_output_cell: 8,
        start_cell_deps: 7,
        start_header_deps: 6,
    }
    .encode();

    built.apply_protocol_mutation(ProtocolMutation::OtxStartRaw(OtxStartSpec {
        start_input_cell: 9,
        start_output_cell: 8,
        start_cell_deps: 7,
        start_header_deps: 6,
    }));

    assert_eq!(witness_bytes(&built, tx_level_witness), tx_level_before);
    assert_eq!(witness_bytes(&built, start_witness), replacement);
}

#[test]
fn mutation_duplicate_sighash_all_inserts_two_sighash_witnesses() {
    let mut shape = TxShape::new();
    shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();

    built.apply_protocol_mutation(ProtocolMutation::DuplicateSighashAll);

    assert_eq!(built.tx.witnesses().len(), 4);
    for index in 0..2 {
        let witness = built
            .tx
            .witnesses()
            .into_iter()
            .nth(index)
            .expect("inserted sighash witness");
        match WitnessLayout::from_slice(witness.raw_data().as_ref())
            .expect("parse sighash witness")
            .to_enum()
        {
            WitnessLayoutUnion::SighashAll(_) => {}
            other => panic!("expected SighashAll witness, got {}", other.item_name()),
        }
    }
    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(0)), 2);
    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(1)), 3);
}

#[test]
fn mutation_duplicate_otx_start_inserts_second_start_before_original() {
    let mut shape = TxShape::new();
    shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();

    built.apply_protocol_mutation(ProtocolMutation::DuplicateOtxStart);

    assert_eq!(built.tx.witnesses().len(), 3);
    for index in 0..2 {
        let witness = built
            .tx
            .witnesses()
            .into_iter()
            .nth(index)
            .expect("OTX start witness");
        match WitnessLayout::from_slice(witness.raw_data().as_ref())
            .expect("parse OTX start witness")
            .to_enum()
        {
            WitnessLayoutUnion::OtxStart(_) => {}
            other => panic!("expected OtxStart witness, got {}", other.item_name()),
        }
    }
    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(0)), 1);
    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(1)), 2);
}

#[test]
fn mutation_non_contiguous_otx_witness_inserts_gap_after_start() {
    let mut shape = TxShape::new();
    shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();

    built.apply_protocol_mutation(ProtocolMutation::NonContiguousOtxWitness);

    assert_eq!(built.tx.witnesses().len(), 3);
    let gap = built
        .tx
        .witnesses()
        .into_iter()
        .nth(1)
        .expect("inserted witness gap");
    match WitnessLayout::from_slice(gap.raw_data().as_ref())
        .expect("parse witness gap")
        .to_enum()
    {
        WitnessLayoutUnion::SighashAllOnly(_) => {}
        other => panic!("expected SighashAllOnly witness, got {}", other.item_name()),
    }
    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(0)), 0);
    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(1)), 2);
}

#[test]
fn mutation_otx_before_otx_start_swaps_witness_handles() {
    let mut shape = TxShape::new();
    shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();

    built.apply_protocol_mutation(ProtocolMutation::OtxBeforeOtxStart);

    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(0)), 1);
    assert_eq!(built.witnesses.tx_index(WitnessHandle::from_raw(1)), 0);
    match WitnessLayout::from_slice(
        built
            .tx
            .witnesses()
            .into_iter()
            .next()
            .expect("first witness")
            .raw_data()
            .as_ref(),
    )
    .expect("parse first witness")
    .to_enum()
    {
        WitnessLayoutUnion::Otx(_) => {}
        other => panic!("expected OTX witness, got {}", other.item_name()),
    }
}

#[test]
fn mutation_seal_scope_raw_patches_matching_otx_seal() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let (witness, original_otx) = signing_otx_witness_with_message_and_seal();
    let mut built = signing_replace_otx_witness(shape.build(), witness);

    built.apply_protocol_mutation(ProtocolMutation::SealScopeRaw {
        otx,
        script_hash: [7; 32],
        scope: 0xfe,
    });

    let mutated = otx_witness(&built, otx);
    assert_ne!(mutated.seals().as_slice(), original_otx.seals().as_slice());
    assert_eq!(
        mutated.message().as_slice(),
        original_otx.message().as_slice()
    );
    assert_eq!(
        mutated.append_permissions().as_slice(),
        original_otx.append_permissions().as_slice()
    );
}

#[test]
fn mutation_seal_raw_patches_matching_otx_seal_bytes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let (witness, original_otx) = signing_otx_witness_with_message_and_seal();
    let mut built = signing_replace_otx_witness(shape.build(), witness);

    built.apply_protocol_mutation(ProtocolMutation::SealRaw {
        otx,
        script_hash: [7; 32],
        scope: 0x42,
        seal: vec![0xde, 0xad],
    });

    let mutated = otx_witness(&built, otx);
    assert_ne!(mutated.seals().as_slice(), original_otx.seals().as_slice());
    assert!(
        mutated
            .seals()
            .into_iter()
            .any(|seal| seal.seal().raw_data().as_ref() == [0xde, 0xad])
    );
}

#[test]
fn mutation_otx_raw_permission_changes_witness_bytes_and_signing_hash() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();
    let otx_witness = WitnessHandle::from_raw(1);
    let before_witness = witness_bytes(&built, otx_witness);
    let oracle = TestSigningHashOracle;
    let before_hash = oracle.otx_base(&built, otx);

    built.apply_protocol_mutation(ProtocolMutation::OtxRawPermission {
        otx,
        permissions: 0xf0,
    });

    assert_ne!(witness_bytes(&built, otx_witness), before_witness);
    assert_hash_changed(before_hash, oracle.otx_base(&built, otx));
}

#[test]
fn mutation_otx_raw_permission_preserves_existing_message_and_seals() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        append_outputs: vec![signing_output(3, vec![0xcc])],
        ..Default::default()
    });
    let (witness, original_otx) = signing_otx_witness_with_message_and_seal();
    let mut built = signing_replace_otx_witness(shape.build(), witness);

    built.apply_protocol_mutation(ProtocolMutation::OtxRawPermission {
        otx,
        permissions: 0xf0,
    });

    let mutated = otx_witness(&built, otx);
    assert_eq!(mutated.append_permissions().as_slice(), &[0xf0]);
    assert_eq!(
        mutated.message().as_slice(),
        original_otx.message().as_slice()
    );
    assert_eq!(mutated.seals().as_slice(), original_otx.seals().as_slice());
    assert_same_base_inputs(&mutated, &original_otx);
    assert_same_base_outputs(&mutated, &original_otx);
    assert_same_append_counts(&mutated, &original_otx);
}

#[test]
fn mutation_otx_raw_base_input_masks_changes_witness_bytes_and_signing_hash() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        ..Default::default()
    });
    let mut built = shape.build();
    let otx_witness = WitnessHandle::from_raw(1);
    let before_witness = witness_bytes(&built, otx_witness);
    let oracle = TestSigningHashOracle;
    let before_hash = oracle.otx_base(&built, otx);

    built.apply_protocol_mutation(ProtocolMutation::OtxRawBaseInputMasks {
        otx,
        masks: vec![0xff],
    });

    assert_ne!(witness_bytes(&built, otx_witness), before_witness);
    assert_hash_changed(before_hash, oracle.otx_base(&built, otx));
}

#[test]
fn mutation_otx_raw_base_input_masks_preserves_existing_message_and_seals() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        append_outputs: vec![signing_output(3, vec![0xcc])],
        ..Default::default()
    });
    let (witness, original_otx) = signing_otx_witness_with_message_and_seal();
    let mut built = signing_replace_otx_witness(shape.build(), witness);

    built.apply_protocol_mutation(ProtocolMutation::OtxRawBaseInputMasks {
        otx,
        masks: vec![0xff],
    });

    let mutated = otx_witness(&built, otx);
    assert_eq!(mutated.base_input_masks().raw_data().as_ref(), &[0xff]);
    assert_eq!(
        mutated.message().as_slice(),
        original_otx.message().as_slice()
    );
    assert_eq!(mutated.seals().as_slice(), original_otx.seals().as_slice());
    assert_eq!(
        mutated.append_permissions().as_slice(),
        original_otx.append_permissions().as_slice(),
        "append permissions"
    );
    assert_eq!(
        molecule_u32(mutated.base_input_cells()),
        molecule_u32(original_otx.base_input_cells()),
        "base input cells"
    );
    assert_same_base_outputs(&mutated, &original_otx);
    assert_same_append_counts(&mutated, &original_otx);
}

#[test]
fn expected_outcome_assertions_report_no_failed_tx_dump_delta() {
    let before = assertions::failed_txs_count();

    assertions::assert_no_failed_tx_dump_delta(before);
}

#[test]
fn signing_hash_oracle_tx_without_message_uses_resolved_facts() {
    let mut shape = TxShape::new();
    shape.push_prefix_input(signing_resolved_input(1, vec![0xaa, 0xbb]));
    let built = shape.build();
    let oracle = TestSigningHashOracle;

    let inputs: Vec<_> = built
        .resolved_inputs
        .iter()
        .map(|input| (input.output.as_slice(), input.data.as_ref()))
        .collect();
    let witnesses: Vec<_> = built
        .tx
        .witnesses()
        .into_iter()
        .map(|witness| witness.raw_data().to_vec())
        .collect();
    let expected = tx_without_message_hash_for_inputs(
        packed_hash_to_array(built.tx.hash()),
        &inputs,
        &witnesses,
    );

    assert_eq!(oracle.tx_without_message(&built), expected);
}

#[test]
fn signing_hash_oracle_tx_with_message_changes_when_message_changes() {
    let mut shape = TxShape::new();
    shape.push_prefix_input(signing_resolved_input(1, vec![0xaa]));
    let built = shape.build();
    let oracle = TestSigningHashOracle;
    let first = CobuildMessageBuilder::new()
        .input_lock_action([1; 32])
        .action_data(vec![1])
        .build();
    let second = CobuildMessageBuilder::new()
        .input_lock_action([1; 32])
        .action_data(vec![2])
        .build();

    assert_hash_changed(
        oracle.tx_with_message(&built, &first),
        oracle.tx_with_message(&built, &second),
    );
}

#[test]
fn signing_hash_oracle_otx_base_changes_when_resolved_input_data_changes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_outputs: vec![signing_output(2, vec![0xbb])],
        base_cell_deps: vec![signing_cell_dep(3)],
        base_header_deps: vec![[4; 32]],
        ..Default::default()
    });
    let built = shape.build();
    let mut changed = built.clone();
    changed.resolved_inputs[0].data = Bytes::from(vec![0xcc]);
    let oracle = TestSigningHashOracle;

    assert_hash_changed(oracle.otx_base(&built, otx), oracle.otx_base(&changed, otx));
}

#[test]
fn signing_hash_oracle_otx_base_changes_when_covered_previous_output_changes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        base_input_masks: Some(vec![0b0000_0010]),
        ..Default::default()
    });
    let input = shape.otx_base_input(otx, 0);
    let mut built = shape.build();
    let before = TestSigningHashOracle.otx_base(&built, otx);

    built.apply_shape_mutation(TxShapeMutation::ReplaceInput {
        input,
        replacement: signing_resolved_input(9, vec![0xaa]),
    });

    assert_hash_changed(before, TestSigningHashOracle.otx_base(&built, otx));
}

#[test]
fn signing_hash_oracle_otx_append_changes_when_append_input_changes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_inputs: vec![signing_resolved_input(2, vec![0xbb])],
        ..Default::default()
    });
    let input = shape.otx_append_input(otx, 0);
    let mut built = shape.build();
    let base_hash = TestSigningHashOracle.otx_base(&built, otx);
    let before = TestSigningHashOracle.otx_append(&built, otx, base_hash);

    built.apply_shape_mutation(TxShapeMutation::ReplaceInput {
        input,
        replacement: signing_resolved_input(9, vec![0xbb]),
    });

    assert_hash_changed(
        before,
        TestSigningHashOracle.otx_append(&built, otx, base_hash),
    );
}

#[test]
fn signing_hash_oracle_otx_append_changes_when_append_output_changes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_outputs: vec![signing_output(2, vec![0xbb])],
        ..Default::default()
    });
    let output = shape.otx_append_output(otx, 0);
    let mut built = shape.build();
    let base_hash = TestSigningHashOracle.otx_base(&built, otx);
    let before = TestSigningHashOracle.otx_append(&built, otx, base_hash);

    built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
        output,
        replacement: signing_output(9, vec![0xbb]),
    });

    assert_hash_changed(
        before,
        TestSigningHashOracle.otx_append(&built, otx, base_hash),
    );
}

#[test]
fn signing_hash_oracle_otx_append_changes_when_append_output_order_changes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_outputs: vec![signing_output(2, vec![0xbb]), signing_output(3, vec![0xcc])],
        ..Default::default()
    });
    let first = shape.otx_append_output(otx, 0);
    let second = shape.otx_append_output(otx, 1);
    let mut built = shape.build();
    let base_hash = TestSigningHashOracle.otx_base(&built, otx);
    let before = TestSigningHashOracle.otx_append(&built, otx, base_hash);

    built.apply_shape_mutation(TxShapeMutation::SwapOutputs {
        left: first,
        right: second,
    });

    assert_hash_changed(
        before,
        TestSigningHashOracle.otx_append(&built, otx, base_hash),
    );
}

#[test]
fn signing_hash_oracle_otx_append_changes_when_append_cell_dep_changes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_cell_deps: vec![signing_cell_dep(2)],
        ..Default::default()
    });
    let cell_dep = shape.otx_append_cell_dep(otx, 0);
    let mut built = shape.build();
    let base_hash = TestSigningHashOracle.otx_base(&built, otx);
    let before = TestSigningHashOracle.otx_append(&built, otx, base_hash);

    built.apply_shape_mutation(TxShapeMutation::ReplaceCellDep {
        cell_dep,
        replacement: signing_cell_dep(9),
    });

    assert_hash_changed(
        before,
        TestSigningHashOracle.otx_append(&built, otx, base_hash),
    );
}

#[test]
fn signing_hash_oracle_otx_append_changes_when_append_header_dep_changes() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_header_deps: vec![[2; 32]],
        ..Default::default()
    });
    let header_dep: HeaderDepHandle = shape.otx_append_header_dep(otx, 0);
    let mut built = shape.build();
    let base_hash = TestSigningHashOracle.otx_base(&built, otx);
    let before = TestSigningHashOracle.otx_append(&built, otx, base_hash);

    built.apply_shape_mutation(TxShapeMutation::ReplaceHeaderDep {
        header_dep,
        replacement: [9; 32],
    });

    assert_hash_changed(
        before,
        TestSigningHashOracle.otx_append(&built, otx, base_hash),
    );
}

#[test]
fn signing_hash_oracle_otx_append_binds_base_hash() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_inputs: vec![signing_resolved_input(2, vec![0xbb])],
        base_outputs: vec![signing_output(3, vec![0xcc])],
        append_outputs: vec![signing_output(4, vec![0xdd])],
        append_cell_deps: vec![signing_cell_dep(5)],
        append_header_deps: vec![[6; 32]],
        ..Default::default()
    });
    let built = shape.build();
    let oracle = TestSigningHashOracle;

    assert_hash_changed(
        oracle.otx_append(&built, otx, [1; 32]),
        oracle.otx_append(&built, otx, [2; 32]),
    );
}

#[test]
fn signing_hash_oracle_otx_append_count_comes_from_witness() {
    let mut shape = TxShape::new();
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![signing_resolved_input(1, vec![0xaa])],
        append_outputs: vec![signing_output(2, vec![0xbb])],
        ..Default::default()
    });
    let built = shape.build();
    let changed = signing_replace_otx_witness(
        built.clone(),
        signing_otx_witness_with_append_output_count(0),
    );
    let oracle = TestSigningHashOracle;
    let base_hash = [3; 32];

    assert_hash_changed(
        oracle.otx_append(&built, otx, base_hash),
        oracle.otx_append(&changed, otx, base_hash),
    );
}

#[test]
fn signing_hash_oracle_sign_scope_records_facts() {
    let mut shape = TxShape::new();
    shape.push_prefix_input(signing_resolved_input(1, vec![0xaa]));
    let built = shape.build();
    let oracle = TestSigningHashOracle;
    let secret_key = fixed_secret_key(7);
    let script_hash = [8; 32];
    let scope = SignatureScope::TxWithoutMessage;

    let facts = sign_scope(
        &built,
        &oracle,
        SignerId("alice"),
        &secret_key,
        script_hash,
        WitnessHandle::from_raw(0),
        scope,
    );

    assert_eq!(facts.signer, SignerId("alice"));
    assert_eq!(facts.scope, scope);
    assert_eq!(facts.carrier, WitnessHandle::from_raw(0));
    assert_eq!(facts.script_hash, script_hash);
    assert_eq!(facts.signing_hash, oracle.tx_without_message(&built));
    assert_eq!(facts.seal.len(), 65);
}
