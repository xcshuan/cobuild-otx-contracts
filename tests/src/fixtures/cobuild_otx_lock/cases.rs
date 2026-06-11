use ckb_testtool::ckb_types::{
    bytes::Bytes,
    packed::{CellOutput, Script},
    prelude::*,
};
use cobuild_types::entity::{
    core::{Otx, SealPairVec},
    witness::{WitnessLayout, WitnessLayoutUnion},
};
use secp256k1::SecretKey;

use crate::{
    fixtures::{
        cobuild_otx_lock::CobuildOtxLockError,
        common::{
            assets::udt_amount_data,
            contracts::{
                deploy_always_success, deploy_cobuild_otx_lock, deploy_cobuild_otx_lock_code,
                deploy_test_udt, rebuild_data2_script,
            },
        },
    },
    framework::{
        cells::{ResolvedInputFacts, TestCellOutput, live_resolved_facts, normal_output},
        cobuild::seal_pair,
        fixture::CobuildTestFixture,
        scenario::{ExpectedOutcome, ScriptLocation},
        scripts::script_hash,
        signing::{
            SignatureScope, SignerId, SigningFacts, TestSigningHashOracle, fixed_secret_key,
            public_key_hash20, sighash_all_only_witness, sign_scope,
        },
        tx::{
            BuiltTxShape, InputHandle, OtxHandle, OtxSegment, ProtocolMutation, TxShape,
            TxShapeMutation, WitnessHandle,
        },
    },
};

pub struct BuiltCobuildOtxLockCase {
    pub name: &'static str,
    pub fixture: CobuildTestFixture,
    pub built: BuiltTxShape,
    pub signing_facts: Vec<SigningFacts>,
    pub expected: ExpectedOutcome,
    pub two_udt_transfer_facts: Option<TwoUdtTransferFacts>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TwoUdtTransferFacts {
    pub fee_lock_hash: Option<[u8; 32]>,
    pub otx_a_lock_hash: [u8; 32],
    pub otx_b_lock_hash: [u8; 32],
}

#[derive(Clone, Copy, Debug)]
struct OtxCaseConfig {
    include_sighash_all: bool,
    corrupt_append_seal: bool,
    malformed_permissions: bool,
    include_full_preimage: bool,
    seal_shape: OtxSealShape,
    invalid_action_target: bool,
    include_outside_same_lock_without_tx_signature: bool,
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

pub fn cases() -> Vec<BuiltCobuildOtxLockCase> {
    vec![
        invalid_args_case(),
        no_relevant_signature_request_case(),
        signed_sighash_all_case(),
        signed_sighash_all_offset_lock_case(),
        signed_otx_dual_scope_case(),
        signed_otx_full_preimage_case(),
        otx_and_outside_same_lock_without_tx_level_signature_case(),
        signed_otx_missing_base_seal_case(),
        signed_otx_missing_append_seal_case(),
        signed_otx_duplicate_base_seal_case(),
        signed_otx_invalid_seal_scope_case(),
        signed_otx_wrong_script_hash_seal_case(),
        signed_otx_invalid_action_target_case(),
        two_udt_transfer_otxs_case(false),
        two_udt_transfer_otxs_case(true),
        mixed_sighash_all_and_otx_case(),
        bad_seal_case(),
        malformed_cobuild_witness_case(),
        malformed_otx_layout_case(),
    ]
}

fn invalid_args_case() -> BuiltCobuildOtxLockCase {
    unsigned_single_input_case(
        "contract_rejects_invalid_args",
        Bytes::from(vec![0u8]),
        CobuildOtxLockError::InvalidArgs,
    )
}

fn no_relevant_signature_request_case() -> BuiltCobuildOtxLockCase {
    let mut args = vec![0u8];
    args.extend_from_slice(&[1u8; 20]);
    unsigned_single_input_case(
        "contract_rejects_without_relevant_signature_request",
        Bytes::from(args),
        CobuildOtxLockError::NoRelevantSignatureRequest,
    )
}

fn signed_sighash_all_case() -> BuiltCobuildOtxLockCase {
    let secret_key = fixed_secret_key(1);
    let mut fixture = CobuildTestFixture::new();
    let contract =
        deploy_cobuild_otx_lock(fixture.context_mut(), 0, &public_key_hash20(&secret_key));
    let lock_input = resolved_lock_input(
        fixture.context_mut(),
        contract.script.clone(),
        100_000_000_000,
        Bytes::new(),
    );
    let output = always_success_output(fixture.context_mut(), 90_000_000_000, Bytes::new());

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(contract.cell_dep);
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

fn signed_sighash_all_offset_lock_case() -> BuiltCobuildOtxLockCase {
    let secret_key = fixed_secret_key(1);
    let mut fixture = CobuildTestFixture::new();
    let contract =
        deploy_cobuild_otx_lock(fixture.context_mut(), 0, &public_key_hash20(&secret_key));
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
    shape.push_prefix_cell_dep(contract.cell_dep);
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

fn signed_otx_dual_scope_case() -> BuiltCobuildOtxLockCase {
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
        },
    )
}

fn signed_otx_full_preimage_case() -> BuiltCobuildOtxLockCase {
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
        },
    )
}

fn otx_and_outside_same_lock_without_tx_level_signature_case() -> BuiltCobuildOtxLockCase {
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
        },
    )
}

fn signed_otx_missing_base_seal_case() -> BuiltCobuildOtxLockCase {
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
        },
    )
}

fn signed_otx_missing_append_seal_case() -> BuiltCobuildOtxLockCase {
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
        },
    )
}

fn signed_otx_duplicate_base_seal_case() -> BuiltCobuildOtxLockCase {
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
        },
    )
}

fn signed_otx_invalid_seal_scope_case() -> BuiltCobuildOtxLockCase {
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
        },
    )
}

fn signed_otx_wrong_script_hash_seal_case() -> BuiltCobuildOtxLockCase {
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
        },
    )
}

fn signed_otx_invalid_action_target_case() -> BuiltCobuildOtxLockCase {
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
        },
    )
}

fn mixed_sighash_all_and_otx_case() -> BuiltCobuildOtxLockCase {
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
        },
    )
}

fn bad_seal_case() -> BuiltCobuildOtxLockCase {
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
        },
    )
}

fn malformed_otx_layout_case() -> BuiltCobuildOtxLockCase {
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
        },
    )
}

fn malformed_cobuild_witness_case() -> BuiltCobuildOtxLockCase {
    let secret_key = fixed_secret_key(1);
    let mut fixture = CobuildTestFixture::new();
    let contract =
        deploy_cobuild_otx_lock(fixture.context_mut(), 0, &public_key_hash20(&secret_key));
    let lock_input = resolved_lock_input(
        fixture.context_mut(),
        contract.script,
        100_000_000_000,
        Bytes::new(),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(contract.cell_dep);
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

fn signed_otx_case(name: &'static str, config: OtxCaseConfig) -> BuiltCobuildOtxLockCase {
    let secret_key = fixed_secret_key(1);
    let mut fixture = CobuildTestFixture::new();
    let contract =
        deploy_cobuild_otx_lock(fixture.context_mut(), 0, &public_key_hash20(&secret_key));
    let lock_output = normal_output(contract.script.clone(), 100_000_000_000);
    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(contract.cell_dep.clone());

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
        base_input_masks: Some(vec![0b0000_0011]),
        base_cell_dep_masks: config.include_full_preimage.then_some(vec![0b0000_0001]),
        base_header_dep_masks: config.include_full_preimage.then_some(vec![0b0000_0001]),
        ..Default::default()
    });
    let base_input_handle = shape.otx_base_input(otx, 0);
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
    } else if config.corrupt_append_seal {
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

pub fn two_udt_transfer_otxs_case(include_fee_input: bool) -> BuiltCobuildOtxLockCase {
    let fee_secret_key = fixed_secret_key(1);
    let otx_a_secret_key = fixed_secret_key(2);
    let otx_b_secret_key = fixed_secret_key(3);
    let mut fixture = CobuildTestFixture::new();
    let lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut(), Vec::new());
    let fee_lock = rebuild_data2_script(
        fixture.context_mut(),
        &lock_code,
        lock_args_for_key(&fee_secret_key),
    );
    let otx_a_lock = rebuild_data2_script(
        fixture.context_mut(),
        &lock_code,
        lock_args_for_key(&otx_a_secret_key),
    );
    let otx_b_lock = rebuild_data2_script(
        fixture.context_mut(),
        &lock_code,
        lock_args_for_key(&otx_b_secret_key),
    );
    let fee_lock_hash = script_hash(&fee_lock);
    let otx_a_lock_hash = script_hash(&otx_a_lock);
    let otx_b_lock_hash = script_hash(&otx_b_lock);
    let issuer_lock = rebuild_data2_script(fixture.context_mut(), &lock_code, vec![0u8; 21]);
    let udt = deploy_test_udt(fixture.context_mut(), script_hash(&issuer_lock));

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(lock_code.cell_dep);
    shape.push_prefix_cell_dep(udt.cell_dep);
    if include_fee_input {
        shape.push_prefix_input(live_resolved_facts(
            fixture.context_mut(),
            normal_output(fee_lock, 100_000_000_000),
            Bytes::new(),
        ));
    }

    let otx_a = shape.push_otx(OtxSegment {
        base_inputs: vec![live_resolved_facts(
            fixture.context_mut(),
            typed_udt_cell(otx_a_lock.clone(), udt.script.clone(), 100),
            udt_amount_data(100),
        )],
        base_outputs: vec![
            udt_output(otx_a_lock.clone(), udt.script.clone(), 40),
            udt_output(otx_a_lock, udt.script.clone(), 60),
        ],
        base_input_masks: Some(vec![0b0000_0011]),
        base_output_masks: Some(full_output_masks(2)),
        ..Default::default()
    });
    let otx_b = shape.push_otx(OtxSegment {
        base_inputs: vec![live_resolved_facts(
            fixture.context_mut(),
            typed_udt_cell(otx_b_lock.clone(), udt.script.clone(), 300),
            udt_amount_data(300),
        )],
        base_outputs: vec![
            udt_output(otx_b_lock.clone(), udt.script.clone(), 100),
            udt_output(otx_b_lock.clone(), udt.script.clone(), 100),
            udt_output(otx_b_lock, udt.script, 100),
        ],
        base_input_masks: Some(vec![0b0000_0011]),
        base_output_masks: Some(full_output_masks(3)),
        ..Default::default()
    });

    let mut built = shape.build();
    let oracle = TestSigningHashOracle;
    let otx_a_facts = sign_scope(
        &built,
        &oracle,
        SignerId("otx-a"),
        &otx_a_secret_key,
        otx_a_lock_hash,
        built.otx_witness(otx_a),
        SignatureScope::OtxBase { otx: otx_a },
    );
    let otx_b_facts = sign_scope(
        &built,
        &oracle,
        SignerId("otx-b"),
        &otx_b_secret_key,
        otx_b_lock_hash,
        built.otx_witness(otx_b),
        SignatureScope::OtxBase { otx: otx_b },
    );
    fill_otx_seals(&mut built, otx_a, std::slice::from_ref(&otx_a_facts));
    fill_otx_seals(&mut built, otx_b, std::slice::from_ref(&otx_b_facts));
    let mut signing_facts = vec![otx_a_facts, otx_b_facts];

    if include_fee_input {
        let witness = insert_leading_witness_placeholders(&mut built, 1)[0];
        let fee_facts = sign_and_fill_sighash_all(
            &mut built,
            &fee_secret_key,
            fee_lock_hash,
            witness,
            SignerId("fee-payer"),
        );
        signing_facts.push(fee_facts);
    }

    BuiltCobuildOtxLockCase {
        name: if include_fee_input {
            "contract_accepts_two_udt_transfer_otxs_with_sighash_all_fee_input"
        } else {
            "contract_accepts_two_udt_transfer_otxs_in_one_transaction"
        },
        fixture,
        built,
        signing_facts,
        expected: ExpectedOutcome::Pass,
        two_udt_transfer_facts: Some(TwoUdtTransferFacts {
            fee_lock_hash: include_fee_input.then_some(fee_lock_hash),
            otx_a_lock_hash,
            otx_b_lock_hash,
        }),
    }
}

fn unsigned_single_input_case(
    name: &'static str,
    args: Bytes,
    error: CobuildOtxLockError,
) -> BuiltCobuildOtxLockCase {
    let mut fixture = CobuildTestFixture::new();
    let contract = deploy_cobuild_otx_lock_code(fixture.context_mut(), args.to_vec());
    let lock_input = resolved_lock_input(
        fixture.context_mut(),
        contract.script,
        100_000_000_000,
        Bytes::new(),
    );

    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(contract.cell_dep);
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

fn sign_and_fill_sighash_all(
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

fn fill_otx_seals(built: &mut BuiltTxShape, otx: OtxHandle, facts: &[SigningFacts]) {
    let seals = facts
        .iter()
        .map(|facts| {
            seal_pair(
                facts.script_hash,
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

fn fill_otx_seals_with_script_hash(
    built: &mut BuiltTxShape,
    otx: OtxHandle,
    script_hash: [u8; 32],
    facts: &[SigningFacts],
) {
    let seals = facts
        .iter()
        .map(|facts| seal_pair(script_hash, seal_scope(facts.scope), facts.seal.clone()))
        .collect::<Vec<_>>();
    let updated = current_otx_witness(built, otx)
        .as_builder()
        .seals(SealPairVec::new_builder().extend(seals).build())
        .build();
    replace_otx_witness(built, otx, updated);
}

fn current_otx_witness(built: &BuiltTxShape, otx: OtxHandle) -> Otx {
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

fn replace_otx_witness(built: &mut BuiltTxShape, otx: OtxHandle, otx_entity: Otx) {
    let witness = WitnessLayout::from(otx_entity);
    replace_witness_bytes(
        built,
        built.otx_witness(otx),
        Bytes::copy_from_slice(witness.as_slice()),
    );
}

fn replace_witness_bytes(built: &mut BuiltTxShape, witness: WitnessHandle, replacement: Bytes) {
    let tx_index = built.witnesses.tx_index(witness);
    let mut witnesses: Vec<_> = built.tx.witnesses().into_iter().collect();
    witnesses[tx_index] = replacement.pack();
    built.tx = built
        .tx
        .as_advanced_builder()
        .set_witnesses(witnesses)
        .build();
}

fn insert_leading_witness_placeholders(
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

fn seal_scope(scope: SignatureScope) -> u8 {
    match scope {
        SignatureScope::OtxBase { .. } => 0,
        SignatureScope::OtxAppend { .. } => 1,
        SignatureScope::TxWithoutMessage | SignatureScope::TxWithMessage => {
            panic!("tx-level signature facts cannot be inserted into an OTX")
        }
    }
}

fn resolved_lock_input(
    fixture: &mut ckb_testtool::context::Context,
    lock: Script,
    capacity: u64,
    data: Bytes,
) -> ResolvedInputFacts {
    live_resolved_facts(fixture, normal_output(lock, capacity), data)
}

fn always_success_output(
    context: &mut ckb_testtool::context::Context,
    capacity: u64,
    data: Bytes,
) -> TestCellOutput {
    TestCellOutput::new(
        normal_output(deploy_always_success(context, Vec::new()).script, capacity),
        data,
    )
}

fn deploy_dummy_dep(
    context: &mut ckb_testtool::context::Context,
    tag: u8,
) -> ckb_testtool::ckb_types::packed::CellDep {
    ckb_testtool::ckb_types::packed::CellDep::new_builder()
        .out_point(context.deploy_cell(Bytes::from(vec![tag])))
        .build()
}

fn typed_udt_cell(lock: Script, type_script: Script, _amount: u128) -> CellOutput {
    CellOutput::new_builder()
        .capacity(100_000_000_000u64)
        .lock(lock)
        .type_(Some(type_script).pack())
        .build()
}

fn udt_output(lock: Script, type_script: Script, amount: u128) -> TestCellOutput {
    TestCellOutput::new(
        CellOutput::new_builder()
            .capacity(90_000_000_000u64)
            .lock(lock)
            .type_(Some(type_script).pack())
            .build(),
        udt_amount_data(amount),
    )
}

fn full_output_masks(output_count: usize) -> Vec<u8> {
    let bits = output_count * 4;
    let bytes = bits.div_ceil(8);
    let mut masks = vec![0xff; bytes];
    let extra_bits = bytes * 8 - bits;
    if extra_bits > 0 {
        let keep_bits = 8 - extra_bits;
        let last = masks.last_mut().expect("non-empty output mask");
        *last = (1u8 << keep_bits) - 1;
    }
    masks
}

fn lock_args_for_key(secret_key: &SecretKey) -> Vec<u8> {
    let mut args = vec![0u8];
    args.extend_from_slice(&public_key_hash20(secret_key));
    args
}

fn lock_exit(input: InputHandle, error: CobuildOtxLockError) -> ExpectedOutcome {
    ExpectedOutcome::ScriptExit {
        location: ScriptLocation::InputLock(input),
        code: error.code(),
    }
}

fn invalid_action_target_message() -> cobuild_types::entity::core::Message {
    crate::framework::cobuild::MessageBuilder::new()
        .push_action(0, [0xabu8; 32], Vec::new())
        .build()
}

fn malformed_sighash_all_only_witness() -> Bytes {
    Bytes::from(witness_union(0xff00_0002, &table(&[Vec::new()])))
}

fn witness_union(item_id: u32, item: &[u8]) -> Vec<u8> {
    let mut witness = Vec::with_capacity(4 + item.len());
    witness.extend_from_slice(&item_id.to_le_bytes());
    witness.extend_from_slice(item);
    witness
}

fn table(fields: &[Vec<u8>]) -> Vec<u8> {
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
