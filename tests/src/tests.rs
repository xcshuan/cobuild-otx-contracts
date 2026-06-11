use crate::{
    TestEnv, default_test_env,
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
        tx::{BuiltTxShape, OtxSegment, ProtocolMutation, TxShape, TxShapeMutation, WitnessHandle},
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

#[test]
fn framework_does_not_depend_on_fixtures_or_named_test_contracts() {
    let framework_dir = std::path::Path::new("src/framework");
    for path in rust_files_under(framework_dir) {
        let source = std::fs::read_to_string(&path).expect("read framework source file");
        for forbidden in [
            "crate::fixtures",
            "fixtures::",
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
    let witness = witness_bytes(built, WitnessHandle::from_raw(otx.0 + 1));
    match WitnessLayout::from_slice(witness.as_ref())
        .expect("parse witness layout")
        .to_enum()
    {
        WitnessLayoutUnion::Otx(otx) => otx,
        other => panic!("expected OTX witness, got {}", other.item_name()),
    }
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
