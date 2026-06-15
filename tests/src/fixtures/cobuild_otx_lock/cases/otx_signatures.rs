use super::*;

#[derive(Clone, Copy, Debug)]
struct OtxCaseConfig {
    include_sighash_all: bool,
    corrupt_append_seal: bool,
    malformed_permissions: bool,
    include_full_preimage: bool,
    seal_shape: OtxSealShape,
    invalid_action_target: bool,
    include_outside_same_lock_without_tx_signature: bool,
    include_outside_other_lock_without_tx_signature: bool,
    mutate_signed_append_output: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OtxSealShape {
    Valid,
    MissingBase,
    MissingAppend,
    DuplicateBase,
    InvalidScope,
    WrongScriptHash,
}

pub(super) fn signed_otx_dual_scope_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_accepts_otx_base_and_append_signatures",
        OtxCaseConfig {
            include_sighash_all: false,
            corrupt_append_seal: false,
            malformed_permissions: false,
            include_full_preimage: false,
            seal_shape: OtxSealShape::Valid,
            invalid_action_target: false,
            include_outside_same_lock_without_tx_signature: false,
            include_outside_other_lock_without_tx_signature: false,
            mutate_signed_append_output: false,
        },
    )
}

pub(super) fn signed_otx_full_preimage_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_accepts_otx_signatures_covering_full_preimage_shape",
        OtxCaseConfig {
            include_sighash_all: false,
            corrupt_append_seal: false,
            malformed_permissions: false,
            include_full_preimage: true,
            seal_shape: OtxSealShape::Valid,
            invalid_action_target: false,
            include_outside_same_lock_without_tx_signature: false,
            include_outside_other_lock_without_tx_signature: false,
            mutate_signed_append_output: false,
        },
    )
}

pub(super) fn signed_otx_append_output_mutated_after_signing_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_signed_append_output_mutation",
        OtxCaseConfig {
            include_sighash_all: false,
            corrupt_append_seal: false,
            malformed_permissions: false,
            include_full_preimage: true,
            seal_shape: OtxSealShape::Valid,
            invalid_action_target: false,
            include_outside_same_lock_without_tx_signature: false,
            include_outside_other_lock_without_tx_signature: false,
            mutate_signed_append_output: true,
        },
    )
}

pub(super) fn otx_and_outside_same_lock_without_tx_level_signature_case() -> BuiltCobuildOtxLockCase
{
    signed_otx_case(
        "contract_rejects_otx_and_outside_same_lock_without_tx_level_signature",
        OtxCaseConfig {
            include_sighash_all: false,
            corrupt_append_seal: false,
            malformed_permissions: false,
            include_full_preimage: false,
            seal_shape: OtxSealShape::Valid,
            invalid_action_target: false,
            include_outside_same_lock_without_tx_signature: true,
            include_outside_other_lock_without_tx_signature: false,
            mutate_signed_append_output: false,
        },
    )
}

pub(super) fn otx_and_outside_other_lock_without_tx_level_signature_case() -> BuiltCobuildOtxLockCase
{
    signed_otx_case(
        "contract_accepts_other_lock_outside_otx_without_tx_level_signature",
        OtxCaseConfig {
            include_sighash_all: false,
            corrupt_append_seal: false,
            malformed_permissions: false,
            include_full_preimage: false,
            seal_shape: OtxSealShape::Valid,
            invalid_action_target: false,
            include_outside_same_lock_without_tx_signature: false,
            include_outside_other_lock_without_tx_signature: true,
            mutate_signed_append_output: false,
        },
    )
}

pub(super) fn signed_otx_missing_base_seal_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_otx_missing_base_seal",
        OtxCaseConfig {
            include_sighash_all: false,
            corrupt_append_seal: false,
            malformed_permissions: false,
            include_full_preimage: false,
            seal_shape: OtxSealShape::MissingBase,
            invalid_action_target: false,
            include_outside_same_lock_without_tx_signature: false,
            include_outside_other_lock_without_tx_signature: false,
            mutate_signed_append_output: false,
        },
    )
}

pub(super) fn signed_otx_missing_append_seal_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_otx_missing_append_seal",
        OtxCaseConfig {
            include_sighash_all: false,
            corrupt_append_seal: false,
            malformed_permissions: false,
            include_full_preimage: false,
            seal_shape: OtxSealShape::MissingAppend,
            invalid_action_target: false,
            include_outside_same_lock_without_tx_signature: false,
            include_outside_other_lock_without_tx_signature: false,
            mutate_signed_append_output: false,
        },
    )
}

pub(super) fn signed_otx_duplicate_base_seal_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_otx_duplicate_base_seal",
        OtxCaseConfig {
            include_sighash_all: false,
            corrupt_append_seal: false,
            malformed_permissions: false,
            include_full_preimage: false,
            seal_shape: OtxSealShape::DuplicateBase,
            invalid_action_target: false,
            include_outside_same_lock_without_tx_signature: false,
            include_outside_other_lock_without_tx_signature: false,
            mutate_signed_append_output: false,
        },
    )
}

pub(super) fn signed_otx_invalid_seal_scope_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_otx_invalid_seal_scope",
        OtxCaseConfig {
            include_sighash_all: false,
            corrupt_append_seal: false,
            malformed_permissions: false,
            include_full_preimage: false,
            seal_shape: OtxSealShape::InvalidScope,
            invalid_action_target: false,
            include_outside_same_lock_without_tx_signature: false,
            include_outside_other_lock_without_tx_signature: false,
            mutate_signed_append_output: false,
        },
    )
}

pub(super) fn signed_otx_wrong_script_hash_seal_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_otx_wrong_script_hash_seal",
        OtxCaseConfig {
            include_sighash_all: false,
            corrupt_append_seal: false,
            malformed_permissions: false,
            include_full_preimage: false,
            seal_shape: OtxSealShape::WrongScriptHash,
            invalid_action_target: false,
            include_outside_same_lock_without_tx_signature: false,
            include_outside_other_lock_without_tx_signature: false,
            mutate_signed_append_output: false,
        },
    )
}

pub(super) fn signed_otx_invalid_action_target_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_otx_action_target_missing",
        OtxCaseConfig {
            include_sighash_all: false,
            corrupt_append_seal: false,
            malformed_permissions: false,
            include_full_preimage: false,
            seal_shape: OtxSealShape::Valid,
            invalid_action_target: true,
            include_outside_same_lock_without_tx_signature: false,
            include_outside_other_lock_without_tx_signature: false,
            mutate_signed_append_output: false,
        },
    )
}

pub(super) fn mixed_sighash_all_and_otx_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_accepts_mixed_sighash_all_and_otx_signature_requests",
        OtxCaseConfig {
            include_sighash_all: true,
            corrupt_append_seal: false,
            malformed_permissions: false,
            include_full_preimage: false,
            seal_shape: OtxSealShape::Valid,
            invalid_action_target: false,
            include_outside_same_lock_without_tx_signature: false,
            include_outside_other_lock_without_tx_signature: false,
            mutate_signed_append_output: false,
        },
    )
}

pub(super) fn bad_seal_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_bad_seal",
        OtxCaseConfig {
            include_sighash_all: false,
            corrupt_append_seal: true,
            malformed_permissions: false,
            include_full_preimage: false,
            seal_shape: OtxSealShape::Valid,
            invalid_action_target: false,
            include_outside_same_lock_without_tx_signature: false,
            include_outside_other_lock_without_tx_signature: false,
            mutate_signed_append_output: false,
        },
    )
}

pub(super) fn malformed_otx_layout_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_malformed_otx_layout",
        OtxCaseConfig {
            include_sighash_all: false,
            corrupt_append_seal: false,
            malformed_permissions: true,
            include_full_preimage: false,
            seal_shape: OtxSealShape::Valid,
            invalid_action_target: false,
            include_outside_same_lock_without_tx_signature: false,
            include_outside_other_lock_without_tx_signature: false,
            mutate_signed_append_output: false,
        },
    )
}

pub(super) fn malformed_otx_duplicate_start_case() -> BuiltCobuildOtxLockCase {
    let mut built = signed_otx_case(
        "contract_rejects_duplicate_otx_start",
        OtxCaseConfig {
            include_sighash_all: false,
            corrupt_append_seal: false,
            malformed_permissions: false,
            include_full_preimage: false,
            seal_shape: OtxSealShape::Valid,
            invalid_action_target: false,
            include_outside_same_lock_without_tx_signature: false,
            include_outside_other_lock_without_tx_signature: false,
            mutate_signed_append_output: false,
        },
    );
    built
        .built
        .apply_protocol_mutation(ProtocolMutation::DuplicateOtxStart);
    let base_input = built
        .built
        .inputs
        .handle_at_tx_index(built.built.otx_ranges[0].base_inputs.start)
        .expect("OTX base input handle");
    built.expected = lock_exit(base_input, CobuildOtxLockError::MalformedOtxLayout);
    built
}

fn signed_otx_case(name: &'static str, config: OtxCaseConfig) -> BuiltCobuildOtxLockCase {
    let secret_key = fixed_secret_key(1);
    let mut fixture = CobuildTestFixture::new();
    let code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let contract = build_cobuild_otx_lock(
        fixture.context_mut(),
        &code,
        0,
        &public_key_hash20(&secret_key),
    );
    let lock_output = normal_output(contract.script.clone(), 100_000_000_000);
    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(code.cell_dep.clone());

    let outside_same_lock_input =
        config
            .include_outside_same_lock_without_tx_signature
            .then(|| {
                shape.push_prefix_input(live_resolved_facts(
                    fixture.context_mut(),
                    lock_output.clone(),
                    Bytes::new(),
                ))
            });
    if config.include_outside_other_lock_without_tx_signature {
        let other = deploy_always_success(fixture.context_mut(), Vec::new());
        shape.push_prefix_cell_dep(other.cell_dep);
        shape.push_prefix_input(live_resolved_facts(
            fixture.context_mut(),
            normal_output(other.script, 100_000_000_000),
            Bytes::new(),
        ));
    }

    let tx_input = config.include_sighash_all.then(|| {
        shape.push_prefix_input(live_resolved_facts(
            fixture.context_mut(),
            lock_output.clone(),
            Bytes::new(),
        ))
    });

    let base_input = live_resolved_facts(fixture.context_mut(), lock_output.clone(), Bytes::new());
    let append_input = live_resolved_facts(fixture.context_mut(), lock_output, Bytes::new());
    let (
        base_outputs,
        append_outputs,
        base_cell_deps,
        append_cell_deps,
        base_header_deps,
        append_header_deps,
    ) = if config.include_full_preimage {
        let base_dep = deploy_dummy_dep(fixture.context_mut(), 0x51);
        let append_dep = deploy_dummy_dep(fixture.context_mut(), 0x52);
        (
            vec![always_success_output(
                fixture.context_mut(),
                91_000_000_000,
                Bytes::from(vec![0x71, 0x72]),
            )],
            vec![always_success_output(
                fixture.context_mut(),
                92_000_000_000,
                Bytes::from(vec![0x81, 0x82, 0x83]),
            )],
            vec![base_dep],
            vec![append_dep],
            vec![[0x61u8; 32]],
            vec![[0x62u8; 32]],
        )
    } else {
        (
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
        )
    };

    let otx = shape.push_otx(OtxSegment {
        message: config
            .invalid_action_target
            .then(invalid_action_target_message),
        base_inputs: vec![base_input],
        append_inputs: vec![append_input],
        base_outputs,
        append_outputs,
        base_cell_deps,
        append_cell_deps,
        base_header_deps,
        append_header_deps,
        base_input_masks: Some(full_base_input_masks(1)),
        base_cell_dep_masks: config
            .include_full_preimage
            .then_some(full_base_cell_dep_masks(1)),
        base_header_dep_masks: config
            .include_full_preimage
            .then_some(full_base_header_dep_masks(1)),
        ..Default::default()
    });
    let base_input_handle = shape.otx_base_input(otx, 0);
    let signed_append_output = config
        .mutate_signed_append_output
        .then(|| shape.otx_append_output(otx, 0));
    let mut built = shape.build();

    let oracle = TestSigningHashOracle;
    let base_facts = sign_scope(
        &built,
        &oracle,
        SignerId("owner"),
        &secret_key,
        contract.script_hash,
        built.otx_witness(otx),
        SignatureScope::OtxBase { otx },
    );
    let append_facts = sign_scope(
        &built,
        &oracle,
        SignerId("owner"),
        &secret_key,
        contract.script_hash,
        built.otx_witness(otx),
        SignatureScope::OtxAppend { otx },
    );
    match config.seal_shape {
        OtxSealShape::Valid => {
            fill_otx_seals(&mut built, otx, &[base_facts.clone(), append_facts.clone()]);
        }
        OtxSealShape::MissingBase => {
            fill_otx_seals(&mut built, otx, std::slice::from_ref(&append_facts));
        }
        OtxSealShape::MissingAppend => {
            fill_otx_seals(&mut built, otx, std::slice::from_ref(&base_facts));
        }
        OtxSealShape::DuplicateBase => {
            fill_otx_seals(
                &mut built,
                otx,
                &[base_facts.clone(), base_facts.clone(), append_facts.clone()],
            );
        }
        OtxSealShape::InvalidScope => {
            fill_otx_seals(&mut built, otx, &[base_facts.clone(), append_facts.clone()]);
            built.apply_protocol_mutation(ProtocolMutation::SealScopeRaw {
                otx,
                script_hash: contract.script_hash,
                scope: 9,
            });
        }
        OtxSealShape::WrongScriptHash => {
            fill_otx_seals_with_script_hash(
                &mut built,
                otx,
                [0x5au8; 32],
                &[base_facts.clone(), append_facts.clone()],
            );
        }
    }
    let mut signing_facts = vec![base_facts, append_facts.clone()];

    if let Some(_input) = tx_input {
        let input_count = built.resolved_inputs.len();
        let witnesses = insert_leading_witness_placeholders(&mut built, input_count);
        let tx_facts = sign_and_fill_sighash_all(
            &mut built,
            &secret_key,
            contract.script_hash,
            witnesses[0],
            SignerId("owner"),
        );
        signing_facts.push(tx_facts);
    }

    if config.corrupt_append_seal {
        let mut bad_seal = append_facts.seal.clone();
        bad_seal[0] ^= 0x01;
        built.apply_protocol_mutation(ProtocolMutation::SealRaw {
            otx,
            script_hash: contract.script_hash,
            scope: 1,
            seal: bad_seal,
        });
    }
    if config.malformed_permissions {
        built.apply_protocol_mutation(ProtocolMutation::OtxRawPermission {
            otx,
            permissions: 0x10,
        });
    }
    if let Some(output) = signed_append_output {
        built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
            output,
            replacement: always_success_output(
                fixture.context_mut(),
                93_000_000_000,
                Bytes::from(vec![0x91, 0x92]),
            ),
        });
    }

    let expected = if config.include_outside_same_lock_without_tx_signature {
        lock_exit(
            outside_same_lock_input.expect("outside same-lock input handle"),
            CobuildOtxLockError::InvalidLockGroupWitness,
        )
    } else if config.invalid_action_target {
        lock_exit(base_input_handle, CobuildOtxLockError::InvalidMessageTarget)
    } else if matches!(
        config.seal_shape,
        OtxSealShape::MissingBase | OtxSealShape::MissingAppend | OtxSealShape::WrongScriptHash
    ) {
        lock_exit(base_input_handle, CobuildOtxLockError::MissingSealPair)
    } else if config.seal_shape == OtxSealShape::DuplicateBase {
        lock_exit(base_input_handle, CobuildOtxLockError::DuplicateSealPair)
    } else if config.seal_shape == OtxSealShape::InvalidScope {
        lock_exit(base_input_handle, CobuildOtxLockError::InvalidSealScope)
    } else if config.corrupt_append_seal || config.mutate_signed_append_output {
        lock_exit(base_input_handle, CobuildOtxLockError::BadSeal)
    } else if config.malformed_permissions {
        lock_exit(base_input_handle, CobuildOtxLockError::MalformedOtxLayout)
    } else {
        ExpectedOutcome::Pass
    };

    BuiltCobuildOtxLockCase {
        name,
        fixture,
        built,
        signing_facts,
        expected,
        two_udt_transfer_facts: None,
    }
}
