use super::*;

pub(super) fn signed_sighash_all_case() -> BuiltCobuildOtxLockCase {
    let secret_key = fixed_secret_key(1);
    let mut fixture = CobuildTestFixture::new();
    let code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let contract = build_cobuild_otx_lock(
        fixture.context_mut(),
        &code,
        0,
        &public_key_hash20(&secret_key),
    );
    let lock_input = resolved_lock_input(
        fixture.context_mut(),
        contract.script.clone(),
        100_000_000_000,
        Bytes::new(),
    );
    let output = always_success_output(fixture.context_mut(), 90_000_000_000, Bytes::new());

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(code.cell_dep);
    shape.push_prefix_input(lock_input);
    shape.push_remainder_output(output);
    let mut built = shape.build();
    let witness = insert_leading_witness_placeholders(&mut built, 1)[0];

    let facts = sign_and_fill_sighash_all(
        &mut built,
        &secret_key,
        contract.script_hash,
        witness,
        SignerId("owner"),
    );

    BuiltCobuildOtxLockCase {
        name: "contract_accepts_sighash_all_cobuild_signature",
        fixture,
        built,
        signing_facts: vec![facts],
        expected: ExpectedOutcome::Pass,
        two_udt_transfer_facts: None,
    }
}

pub(super) fn signed_sighash_all_offset_lock_case() -> BuiltCobuildOtxLockCase {
    let secret_key = fixed_secret_key(1);
    let mut fixture = CobuildTestFixture::new();
    let code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let contract = build_cobuild_otx_lock(
        fixture.context_mut(),
        &code,
        0,
        &public_key_hash20(&secret_key),
    );
    let other = deploy_always_success(fixture.context_mut(), Vec::new());
    let other_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(other.script, 100_000_000_000),
        Bytes::new(),
    );
    let lock_input = resolved_lock_input(
        fixture.context_mut(),
        contract.script.clone(),
        100_000_000_000,
        Bytes::new(),
    );
    let output = always_success_output(fixture.context_mut(), 90_000_000_000, Bytes::new());

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(code.cell_dep);
    shape.push_prefix_cell_dep(other.cell_dep);
    shape.push_prefix_input(other_input);
    shape.push_prefix_input(lock_input);
    shape.push_remainder_output(output);
    let mut built = shape.build();
    let witnesses = insert_leading_witness_placeholders(&mut built, 2);

    let facts = sign_and_fill_sighash_all(
        &mut built,
        &secret_key,
        contract.script_hash,
        witnesses[1],
        SignerId("owner"),
    );

    BuiltCobuildOtxLockCase {
        name: "contract_accepts_sighash_all_when_current_lock_starts_after_input_zero",
        fixture,
        built,
        signing_facts: vec![facts],
        expected: ExpectedOutcome::Pass,
        two_udt_transfer_facts: None,
    }
}
