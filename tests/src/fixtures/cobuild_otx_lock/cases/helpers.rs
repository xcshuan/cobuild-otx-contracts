use super::*;

pub(super) fn unsigned_single_input_case(
    name: &'static str,
    args: Bytes,
    error: CobuildOtxLockError,
) -> BuiltCobuildOtxLockCase {
    let mut fixture = CobuildTestFixture::new();
    let contract_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let contract = rebuild_data2_script(fixture.context_mut(), &contract_code, args.to_vec());
    let lock_input = resolved_lock_input(
        fixture.context_mut(),
        contract,
        100_000_000_000,
        Bytes::new(),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(contract_code.cell_dep);
    let input = shape.push_prefix_input(lock_input);
    shape.push_remainder_output(always_success_output(
        fixture.context_mut(),
        90_000_000_000,
        Bytes::new(),
    ));
    let mut built = shape.build();
    insert_leading_witness_placeholders(&mut built, 1);

    BuiltCobuildOtxLockCase {
        name,
        fixture,
        built,
        signing_facts: Vec::new(),
        expected: lock_exit(input, error),
        two_udt_transfer_facts: None,
    }
}

pub(super) fn sign_and_fill_sighash_all(
    built: &mut BuiltTxShape,
    secret_key: &SecretKey,
    script_hash: [u8; 32],
    witness: WitnessHandle,
    signer: SignerId,
) -> SigningFacts {
    let oracle = TestSigningHashOracle;
    let facts = sign_scope(
        built,
        &oracle,
        signer,
        secret_key,
        script_hash,
        witness,
        SignatureScope::TxWithoutMessage,
    );
    replace_witness_bytes(built, witness, sighash_all_only_witness(facts.seal.clone()));
    facts
}

pub(super) fn sign_and_fill_tx_level_lock_group(
    built: &mut BuiltTxShape,
    input: InputHandle,
    secret_key: &SecretKey,
    script_hash: [u8; 32],
    signer: SignerId,
) -> SigningFacts {
    let input_tx_index = built.inputs.tx_index(input);
    let witnesses = insert_leading_witness_placeholders(built, built.resolved_inputs.len());
    sign_and_fill_sighash_all(
        built,
        secret_key,
        script_hash,
        witnesses[input_tx_index],
        signer,
    )
}

pub(super) fn fill_otx_seals(built: &mut BuiltTxShape, otx: OtxHandle, facts: &[SigningFacts]) {
    fill_otx_seals_with(built, otx, facts, None);
}

pub(super) fn fill_otx_seals_with(
    built: &mut BuiltTxShape,
    otx: OtxHandle,
    facts: &[SigningFacts],
    script_hash_override: Option<[u8; 32]>,
) {
    let seals = facts
        .iter()
        .map(|facts| {
            seal_pair(
                script_hash_override.unwrap_or(facts.script_hash),
                seal_scope(facts.scope),
                facts.seal.clone(),
            )
        })
        .collect::<Vec<_>>();
    let updated = current_otx_witness(built, otx)
        .as_builder()
        .seals(SealPairVec::new_builder().extend(seals).build())
        .build();
    replace_otx_witness(built, otx, updated);
}

pub(super) fn current_otx_witness(built: &BuiltTxShape, otx: OtxHandle) -> Otx {
    let witness = built
        .tx
        .witnesses()
        .into_iter()
        .nth(built.witnesses.tx_index(built.otx_witness(otx)))
        .expect("OTX witness")
        .raw_data();
    match WitnessLayout::from_slice(witness.as_ref())
        .expect("parse witness layout")
        .to_enum()
    {
        WitnessLayoutUnion::Otx(otx) => otx,
        other => panic!("expected OTX witness, got {}", other.item_name()),
    }
}

pub(super) fn replace_otx_witness(built: &mut BuiltTxShape, otx: OtxHandle, otx_entity: Otx) {
    let witness = WitnessLayout::from(otx_entity);
    replace_witness_bytes(
        built,
        built.otx_witness(otx),
        Bytes::copy_from_slice(witness.as_slice()),
    );
}

pub(super) fn replace_witness_bytes(
    built: &mut BuiltTxShape,
    witness: WitnessHandle,
    replacement: Bytes,
) {
    let tx_index = built.witnesses.tx_index(witness);
    let mut witnesses: Vec<_> = built.tx.witnesses().into_iter().collect();
    witnesses[tx_index] = replacement.pack();
    built.tx = built
        .tx
        .as_advanced_builder()
        .set_witnesses(witnesses)
        .build();
}

pub(super) fn insert_leading_witness_placeholders(
    built: &mut BuiltTxShape,
    count: usize,
) -> Vec<WitnessHandle> {
    let mut witnesses = vec![Bytes::new().pack(); count];
    witnesses.extend(built.tx.witnesses());
    built.witnesses.remap_tx_indexes(|index| index + count);

    let handles = (0..count)
        .map(WitnessHandle::synthetic_input)
        .collect::<Vec<_>>();
    for (index, handle) in handles.iter().copied().enumerate() {
        built.witnesses.set_tx_index(handle, index);
    }
    built.tx = built
        .tx
        .as_advanced_builder()
        .set_witnesses(witnesses)
        .build();
    handles
}

pub(super) fn seal_scope(scope: SignatureScope) -> u8 {
    match scope {
        SignatureScope::OtxBase { .. } => 0,
        SignatureScope::OtxAppend { .. } => 1,
        SignatureScope::TxWithoutMessage | SignatureScope::TxWithMessage => {
            panic!("tx-level signature facts cannot be inserted into an OTX")
        }
    }
}

pub(super) fn resolved_lock_input(
    fixture: &mut ckb_testtool::context::Context,
    lock: Script,
    capacity: u64,
    data: Bytes,
) -> ResolvedInputFacts {
    live_resolved_facts(fixture, normal_output(lock, capacity), data)
}

pub(super) fn always_success_output(
    context: &mut ckb_testtool::context::Context,
    capacity: u64,
    data: Bytes,
) -> TestCellOutput {
    TestCellOutput::new(
        normal_output(deploy_always_success(context, Vec::new()).script, capacity),
        data,
    )
}

pub(super) fn deploy_dummy_dep(
    context: &mut ckb_testtool::context::Context,
    tag: u8,
) -> ckb_testtool::ckb_types::packed::CellDep {
    ckb_testtool::ckb_types::packed::CellDep::new_builder()
        .out_point(context.deploy_cell(Bytes::from(vec![tag])))
        .build()
}

pub(super) fn typed_udt_cell(lock: Script, type_script: Script) -> CellOutput {
    CellOutput::new_builder()
        .capacity(100_000_000_000u64)
        .lock(lock)
        .type_(Some(type_script).pack())
        .build()
}

pub(super) fn typed_asset_cell(lock: Script, type_script: Script, capacity: u64) -> CellOutput {
    CellOutput::new_builder()
        .capacity(capacity)
        .lock(lock)
        .type_(Some(type_script).pack())
        .build()
}

pub(super) fn udt_output(lock: Script, type_script: Script, amount: u128) -> TestCellOutput {
    TestCellOutput::new(
        CellOutput::new_builder()
            .capacity(90_000_000_000u64)
            .lock(lock)
            .type_(Some(type_script).pack())
            .build(),
        udt_amount_data(amount),
    )
}

pub(super) fn lock_exit(input: InputHandle, error: CobuildOtxLockError) -> ExpectedOutcome {
    ExpectedOutcome::ScriptExit {
        location: ScriptLocation::InputLock(input),
        code: error.code(),
    }
}

pub(super) fn invalid_action_target_message() -> cobuild_types::entity::core::Message {
    crate::framework::cobuild::MessageBuilder::new()
        .push_action(0, [0xabu8; 32], Vec::new())
        .build()
}

pub(super) fn malformed_sighash_all_only_witness() -> Bytes {
    Bytes::from(witness_union(0xff00_0002, &table(&[Vec::new()])))
}

pub(super) fn witness_union(item_id: u32, item: &[u8]) -> Vec<u8> {
    let mut witness = Vec::with_capacity(4 + item.len());
    witness.extend_from_slice(&item_id.to_le_bytes());
    witness.extend_from_slice(item);
    witness
}

pub(super) fn table(fields: &[Vec<u8>]) -> Vec<u8> {
    let header_size = 4 + fields.len() * 4;
    let total_size = header_size + fields.iter().map(Vec::len).sum::<usize>();
    let mut out = Vec::with_capacity(total_size);
    out.extend_from_slice(&(total_size as u32).to_le_bytes());
    let mut offset = header_size as u32;
    for field in fields {
        out.extend_from_slice(&offset.to_le_bytes());
        offset += field.len() as u32;
    }
    for field in fields {
        out.extend_from_slice(field);
    }
    out
}
