use super::*;

pub(super) fn invalid_args_case() -> BuiltCobuildOtxLockCase {
    unsigned_single_input_case(
        "contract_rejects_invalid_args",
        Bytes::from(vec![0u8]),
        CobuildOtxLockError::InvalidArgs,
    )
}

pub(super) fn no_relevant_signature_request_case() -> BuiltCobuildOtxLockCase {
    unsigned_single_input_case(
        "contract_rejects_without_relevant_signature_request",
        Bytes::from(vec![1u8; 20]),
        CobuildOtxLockError::NoRelevantSignatureRequest,
    )
}

pub(super) fn malformed_cobuild_witness_case() -> BuiltCobuildOtxLockCase {
    let secret_key = fixed_secret_key(1);
    let mut fixture = CobuildTestFixture::new();
    let code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let contract = build_cobuild_otx_lock(
        fixture.context_mut(),
        &code,
        &public_key_hash20(&secret_key),
    );
    let lock_input = resolved_lock_input(
        fixture.context_mut(),
        contract.script,
        100_000_000_000,
        Bytes::new(),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(code.cell_dep);
    let input = shape.push_prefix_input(lock_input);
    shape.push_remainder_output(always_success_output(
        fixture.context_mut(),
        90_000_000_000,
        Bytes::new(),
    ));
    let mut built = shape.build();
    let witness = insert_leading_witness_placeholders(&mut built, 1)[0];
    built.apply_shape_mutation(TxShapeMutation::ReplaceWitness {
        witness,
        replacement: malformed_sighash_all_only_witness(),
    });

    BuiltCobuildOtxLockCase {
        name: "contract_rejects_malformed_cobuild_witness",
        fixture,
        built,
        signing_facts: Vec::new(),
        expected: lock_exit(input, CobuildOtxLockError::MalformedCobuildWitness),
        two_udt_transfer_facts: None,
    }
}
