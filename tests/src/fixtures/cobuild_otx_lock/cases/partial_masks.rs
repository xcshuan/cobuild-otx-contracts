use super::*;

pub(super) fn input_mask_accepts_uncovered_previous_output_mutation_case() -> BuiltCobuildOtxLockCase
{
    partial_mask_case(
        "contract_accepts_partial_input_mask_when_uncovered_previous_output_changes",
        PartialMaskConfig {
            base_input_masks: base_input_mask(1)
                .cover_field(0, BaseInputMaskField::Since)
                .bytes(),
            mutation: PartialMaskMutation::PreviousOutput,
            expected: PartialMaskExpected::Pass,
            ..Default::default()
        },
    )
}

pub(super) fn input_mask_rejects_covered_previous_output_mutation_case() -> BuiltCobuildOtxLockCase
{
    partial_mask_case(
        "contract_rejects_partial_input_mask_when_covered_previous_output_changes",
        PartialMaskConfig {
            base_input_masks: base_input_mask(1)
                .cover_field(0, BaseInputMaskField::PreviousOutput)
                .bytes(),
            mutation: PartialMaskMutation::PreviousOutput,
            expected: PartialMaskExpected::BadSeal,
            ..Default::default()
        },
    )
}

pub(super) fn output_mask_accepts_uncovered_lock_mutation_case() -> BuiltCobuildOtxLockCase {
    partial_mask_case(
        "contract_accepts_partial_output_mask_when_uncovered_lock_changes",
        PartialMaskConfig {
            base_output_masks: base_output_mask(1)
                .cover_field(0, BaseOutputMaskField::Capacity)
                .bytes(),
            mutation: PartialMaskMutation::OutputLock,
            expected: PartialMaskExpected::Pass,
            ..Default::default()
        },
    )
}

pub(super) fn output_mask_rejects_covered_lock_mutation_case() -> BuiltCobuildOtxLockCase {
    partial_mask_case(
        "contract_rejects_partial_output_mask_when_covered_lock_changes",
        PartialMaskConfig {
            base_output_masks: base_output_mask(1)
                .cover_field(0, BaseOutputMaskField::Lock)
                .bytes(),
            mutation: PartialMaskMutation::OutputLock,
            expected: PartialMaskExpected::BadSeal,
            ..Default::default()
        },
    )
}

pub(super) fn cell_dep_mask_accepts_uncovered_cell_dep_mutation_case() -> BuiltCobuildOtxLockCase {
    partial_mask_case(
        "contract_accepts_partial_cell_dep_mask_when_uncovered_cell_dep_changes",
        PartialMaskConfig {
            base_cell_dep_masks: base_cell_dep_item_mask(1).bytes(),
            mutation: PartialMaskMutation::CellDep,
            expected: PartialMaskExpected::Pass,
            ..Default::default()
        },
    )
}

pub(super) fn cell_dep_mask_rejects_covered_cell_dep_mutation_case() -> BuiltCobuildOtxLockCase {
    partial_mask_case(
        "contract_rejects_partial_cell_dep_mask_when_covered_cell_dep_changes",
        PartialMaskConfig {
            base_cell_dep_masks: base_cell_dep_item_mask(1).cover_item(0).bytes(),
            mutation: PartialMaskMutation::CellDep,
            expected: PartialMaskExpected::BadSeal,
            ..Default::default()
        },
    )
}

pub(super) fn header_dep_mask_accepts_uncovered_header_dep_mutation_case() -> BuiltCobuildOtxLockCase
{
    partial_mask_case(
        "contract_accepts_partial_header_dep_mask_when_uncovered_header_dep_changes",
        PartialMaskConfig {
            base_header_dep_masks: base_header_dep_item_mask(1).bytes(),
            mutation: PartialMaskMutation::HeaderDep,
            expected: PartialMaskExpected::Pass,
            ..Default::default()
        },
    )
}

pub(super) fn header_dep_mask_rejects_covered_header_dep_mutation_case() -> BuiltCobuildOtxLockCase
{
    partial_mask_case(
        "contract_rejects_partial_header_dep_mask_when_covered_header_dep_changes",
        PartialMaskConfig {
            base_header_dep_masks: base_header_dep_item_mask(1).cover_item(0).bytes(),
            mutation: PartialMaskMutation::HeaderDep,
            expected: PartialMaskExpected::BadSeal,
            ..Default::default()
        },
    )
}

#[derive(Clone, Debug)]
struct PartialMaskConfig {
    base_input_masks: Vec<u8>,
    base_output_masks: Vec<u8>,
    base_cell_dep_masks: Vec<u8>,
    base_header_dep_masks: Vec<u8>,
    mutation: PartialMaskMutation,
    expected: PartialMaskExpected,
}

impl Default for PartialMaskConfig {
    fn default() -> Self {
        Self {
            base_input_masks: full_base_input_masks(1),
            base_output_masks: full_base_output_masks(1),
            base_cell_dep_masks: full_base_cell_dep_masks(1),
            base_header_dep_masks: full_base_header_dep_masks(1),
            mutation: PartialMaskMutation::PreviousOutput,
            expected: PartialMaskExpected::Pass,
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum PartialMaskMutation {
    PreviousOutput,
    OutputLock,
    CellDep,
    HeaderDep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PartialMaskExpected {
    Pass,
    BadSeal,
}

fn partial_mask_case(name: &'static str, config: PartialMaskConfig) -> BuiltCobuildOtxLockCase {
    let secret_key = fixed_secret_key(6);
    let mut fixture = CobuildTestFixture::new();
    let code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let contract = build_cobuild_otx_lock(
        fixture.context_mut(),
        &code,
        &public_key_hash20(&secret_key),
    );
    let base_lock = contract.script.clone();
    let base_output_cell = normal_output(base_lock.clone(), 100_000_000_000);
    let base_input = live_resolved_facts(
        fixture.context_mut(),
        base_output_cell.clone(),
        Bytes::from(vec![0x31]),
    );
    let base_output = TestCellOutput::new(
        normal_output(base_lock.clone(), 90_000_000_000),
        Bytes::from(vec![0x41]),
    );
    let base_cell_dep = deploy_dummy_dep(fixture.context_mut(), 0x61);
    let replacement_cell_dep = deploy_dummy_dep(fixture.context_mut(), 0x62);

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(code.cell_dep.clone());
    let otx = shape.push_otx(OtxSpec {
        base_inputs: vec![base_input],
        base_outputs: vec![base_output],
        base_cell_deps: vec![base_cell_dep],
        base_header_deps: vec![[0x71; 32]],
        base_input_masks: Some(config.base_input_masks),
        base_output_masks: Some(config.base_output_masks),
        base_cell_dep_masks: Some(config.base_cell_dep_masks),
        base_header_dep_masks: Some(config.base_header_dep_masks),
        ..Default::default()
    });
    let base_input_handle = shape.otx_base_input(otx, 0);
    let base_output_handle = shape.otx_base_output(otx, 0);
    let base_cell_dep_handle = shape.otx_base_cell_dep(otx, 0);
    let base_header_dep_handle = shape.otx_base_header_dep(otx, 0);
    let mut built = shape.build();

    let base_facts = sign_scope(
        &built,
        &TestSigningHashOracle,
        SignerId("partial-mask-owner"),
        &secret_key,
        contract.script_hash,
        built.otx_witness(otx),
        SignatureScope::OtxBase { otx },
    );
    fill_otx_seals(&mut built, otx, std::slice::from_ref(&base_facts));

    match config.mutation {
        PartialMaskMutation::PreviousOutput => {
            built.apply_shape_mutation(TxShapeMutation::ReplaceInput {
                input: base_input_handle,
                replacement: live_resolved_facts(
                    fixture.context_mut(),
                    base_output_cell,
                    Bytes::from(vec![0x31]),
                ),
            });
        }
        PartialMaskMutation::OutputLock => {
            let other_lock = deploy_always_success(fixture.context_mut(), b"partial-mask".to_vec());
            built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
                output: base_output_handle,
                replacement: TestCellOutput::new(
                    normal_output(other_lock.script, 90_000_000_000),
                    Bytes::from(vec![0x41]),
                ),
            });
        }
        PartialMaskMutation::CellDep => {
            built.apply_shape_mutation(TxShapeMutation::ReplaceCellDep {
                cell_dep: base_cell_dep_handle,
                replacement: replacement_cell_dep,
            });
        }
        PartialMaskMutation::HeaderDep => {
            built.apply_shape_mutation(TxShapeMutation::ReplaceHeaderDep {
                header_dep: base_header_dep_handle,
                replacement: [0x72; 32],
            });
        }
    }

    let expected = match config.expected {
        PartialMaskExpected::Pass => ExpectedOutcome::Pass,
        PartialMaskExpected::BadSeal => lock_exit(base_input_handle, CobuildOtxLockError::BadSeal),
    };

    BuiltCobuildOtxLockCase {
        name,
        fixture,
        built,
        signing_facts: vec![base_facts],
        expected,
        two_udt_transfer_facts: None,
    }
}
