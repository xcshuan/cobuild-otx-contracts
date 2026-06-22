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
        cobuild::{
            BaseInputMaskField, BaseOutputMaskField, CobuildMessageBuilder, OtxStartSpec,
            RawOtxBuilder, base_cell_dep_item_mask, base_header_dep_item_mask, base_input_mask,
            base_output_mask, seal_pair,
        },
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
        packed::{CellDep, CellInput, CellOutput, OutPoint, Script, ScriptOpt},
        prelude::{Builder, Entity, Pack},
    },
};
use cobuild_types::entity::{
    blockchain::Uint32,
    core::Otx,
    witness::{WitnessLayout, WitnessLayoutUnion},
};

mod cobuild_protocol;
mod limit_order_actions;
mod meta;
mod signing_hash;
mod tx_mutations;

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

fn signing_resolved_input_with_since(
    tag: u8,
    since: u64,
    data: impl Into<Bytes>,
) -> ResolvedInputFacts {
    let mut facts = signing_resolved_input(tag, data);
    facts.input = facts.input.as_builder().since(since).build();
    facts
}

fn signing_resolved_input_with_previous_output_tag(
    base_tag: u8,
    previous_output_tag: u8,
    data: impl Into<Bytes>,
) -> ResolvedInputFacts {
    let mut facts = signing_resolved_input(base_tag, data);
    facts.input = facts
        .input
        .as_builder()
        .previous_output(OutPoint::new([previous_output_tag; 32].pack(), 0))
        .build();
    facts
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

fn signing_output_with_lock_tag(
    capacity_tag: u8,
    lock_tag: u8,
    data: impl Into<Bytes>,
) -> TestCellOutput {
    TestCellOutput::new(
        CellOutput::new_builder()
            .capacity(2_000 + u64::from(capacity_tag))
            .lock(signing_test_script(lock_tag))
            .build(),
        data,
    )
}

fn signing_typed_output(capacity_tag: u8, type_tag: u8, data: impl Into<Bytes>) -> TestCellOutput {
    TestCellOutput::new(
        CellOutput::new_builder()
            .capacity(2_000 + u64::from(capacity_tag))
            .lock(signing_test_script(capacity_tag))
            .type_(Some(signing_test_script(type_tag)).pack())
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
    let witness = witness_bytes(built, built.otx_witness(otx));
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
    let otx = RawOtxBuilder::new()
        .base_input_cells(1)
        .append_output_cells(append_output_cells)
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
    let otx = RawOtxBuilder::new()
        .message(message)
        .append_permissions(0x0b)
        .base_input_cells(1)
        .base_input_masks(vec![0x03])
        .base_output_cells(base_output_cells)
        .base_output_masks(vec![0xa5])
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
