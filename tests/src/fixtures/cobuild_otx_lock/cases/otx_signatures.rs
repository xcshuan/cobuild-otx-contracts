use super::*;

#[derive(Clone, Copy, Debug)]
struct OtxCaseConfig {
    tx_level_shape: OtxTxLevelShape,
    preimage_shape: OtxPreimageShape,
    seal_shape: OtxSealShape,
    tamper: OtxTamper,
}

impl Default for OtxCaseConfig {
    fn default() -> Self {
        Self {
            tx_level_shape: OtxTxLevelShape::None,
            preimage_shape: OtxPreimageShape::Minimal,
            seal_shape: OtxSealShape::Valid,
            tamper: OtxTamper::None,
        }
    }
}

impl OtxCaseConfig {
    fn with_sighash_all(mut self) -> Self {
        self.tx_level_shape = OtxTxLevelShape::SignedSameLock;
        self
    }

    fn with_full_preimage(mut self) -> Self {
        self.preimage_shape = OtxPreimageShape::Full;
        self
    }

    fn with_seal_shape(mut self, seal_shape: OtxSealShape) -> Self {
        self.seal_shape = seal_shape;
        self
    }

    fn with_corrupt_append_seal(mut self) -> Self {
        self.tamper = OtxTamper::CorruptAppendSeal;
        self
    }

    fn with_corrupt_second_append_seal(mut self) -> Self {
        self.tamper = OtxTamper::CorruptSecondAppendSeal;
        self.seal_shape = OtxSealShape::TwoAppendSegments;
        self
    }

    fn with_malformed_permissions(mut self) -> Self {
        self.tamper = OtxTamper::MalformedPermissions;
        self
    }

    fn with_invalid_action_target(mut self) -> Self {
        self.tamper = OtxTamper::InvalidActionTarget;
        self
    }

    fn with_outside_same_lock_without_tx_signature(mut self) -> Self {
        self.tx_level_shape = OtxTxLevelShape::UnsignedOutsideSameLock;
        self
    }

    fn with_outside_other_lock_without_tx_signature(mut self) -> Self {
        self.tx_level_shape = OtxTxLevelShape::UnsignedOutsideOtherLock;
        self
    }

    fn with_signed_append_output_mutation(mut self) -> Self {
        self.preimage_shape = OtxPreimageShape::FullWithSignedAppendOutputMutation;
        self
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OtxTxLevelShape {
    None,
    SignedSameLock,
    UnsignedOutsideSameLock,
    UnsignedOutsideOtherLock,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OtxPreimageShape {
    Minimal,
    Full,
    FullWithSignedAppendOutputMutation,
}

impl OtxPreimageShape {
    fn includes_full_preimage(self) -> bool {
        matches!(
            self,
            OtxPreimageShape::Full | OtxPreimageShape::FullWithSignedAppendOutputMutation
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OtxSealShape {
    Valid,
    MissingBase,
    MissingAppend,
    DuplicateBase,
    DuplicateAppend,
    TwoAppendSegments,
    MissingSecondAppend,
    WrongScriptHash,
}

impl OtxSealShape {
    fn needs_second_append_segment(self) -> bool {
        matches!(
            self,
            OtxSealShape::TwoAppendSegments | OtxSealShape::MissingSecondAppend
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OtxTamper {
    None,
    CorruptAppendSeal,
    CorruptSecondAppendSeal,
    MalformedPermissions,
    InvalidActionTarget,
}

pub(super) fn signed_otx_dual_scope_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_accepts_otx_base_and_append_signatures",
        OtxCaseConfig::default(),
    )
}

pub(super) fn signed_otx_full_preimage_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_accepts_otx_signatures_covering_full_preimage_shape",
        OtxCaseConfig::default().with_full_preimage(),
    )
}

pub(super) fn signed_otx_append_output_mutated_after_signing_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_signed_append_output_mutation",
        OtxCaseConfig::default().with_signed_append_output_mutation(),
    )
}

pub(super) fn otx_and_outside_same_lock_without_tx_level_signature_case() -> BuiltCobuildOtxLockCase
{
    signed_otx_case(
        "contract_rejects_otx_and_outside_same_lock_without_tx_level_signature",
        OtxCaseConfig::default().with_outside_same_lock_without_tx_signature(),
    )
}

pub(super) fn otx_and_outside_other_lock_without_tx_level_signature_case() -> BuiltCobuildOtxLockCase
{
    signed_otx_case(
        "contract_accepts_other_lock_outside_otx_without_tx_level_signature",
        OtxCaseConfig::default().with_outside_other_lock_without_tx_signature(),
    )
}

pub(super) fn signed_otx_missing_base_seal_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_otx_missing_base_seal",
        OtxCaseConfig::default().with_seal_shape(OtxSealShape::MissingBase),
    )
}

pub(super) fn signed_otx_missing_append_seal_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_otx_missing_append_seal",
        OtxCaseConfig::default().with_seal_shape(OtxSealShape::MissingAppend),
    )
}

pub(super) fn signed_otx_duplicate_base_seal_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_otx_duplicate_base_seal",
        OtxCaseConfig::default().with_seal_shape(OtxSealShape::DuplicateBase),
    )
}

pub(super) fn signed_otx_duplicate_append_seal_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_otx_duplicate_append_seal",
        OtxCaseConfig::default().with_seal_shape(OtxSealShape::DuplicateAppend),
    )
}

pub(super) fn signed_otx_two_append_segments_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_accepts_otx_two_append_segment_signatures",
        OtxCaseConfig::default().with_seal_shape(OtxSealShape::TwoAppendSegments),
    )
}

pub(super) fn signed_otx_missing_second_append_seal_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_otx_missing_second_append_seal",
        OtxCaseConfig::default().with_seal_shape(OtxSealShape::MissingSecondAppend),
    )
}

pub(super) fn signed_otx_wrong_script_hash_seal_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_otx_wrong_script_hash_seal",
        OtxCaseConfig::default().with_seal_shape(OtxSealShape::WrongScriptHash),
    )
}

pub(super) fn signed_otx_invalid_action_target_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_otx_action_target_missing",
        OtxCaseConfig::default().with_invalid_action_target(),
    )
}

pub(super) fn mixed_sighash_all_and_otx_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_accepts_mixed_sighash_all_and_otx_signature_requests",
        OtxCaseConfig::default().with_sighash_all(),
    )
}

pub(super) fn bad_seal_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_bad_seal",
        OtxCaseConfig::default().with_corrupt_append_seal(),
    )
}

pub(super) fn corrupt_second_append_seal_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_corrupt_second_append_seal",
        OtxCaseConfig::default().with_corrupt_second_append_seal(),
    )
}

pub(super) fn malformed_otx_layout_case() -> BuiltCobuildOtxLockCase {
    signed_otx_case(
        "contract_rejects_malformed_otx_layout",
        OtxCaseConfig::default().with_malformed_permissions(),
    )
}

pub(super) fn malformed_otx_duplicate_start_case() -> BuiltCobuildOtxLockCase {
    let mut built = signed_otx_case(
        "contract_rejects_duplicate_otx_start",
        OtxCaseConfig::default(),
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
        &public_key_hash20(&secret_key),
    );
    let lock_output = normal_output(contract.script.clone(), 100_000_000_000);
    let mut shape = TxShape::new();
    shape.push_prefix_cell_dep(code.cell_dep.clone());

    let outside_same_lock_input =
        (config.tx_level_shape == OtxTxLevelShape::UnsignedOutsideSameLock).then(|| {
            shape.push_prefix_input(live_resolved_facts(
                fixture.context_mut(),
                lock_output.clone(),
                Bytes::new(),
            ))
        });
    if config.tx_level_shape == OtxTxLevelShape::UnsignedOutsideOtherLock {
        let other = deploy_always_success(fixture.context_mut(), Vec::new());
        shape.push_prefix_cell_dep(other.cell_dep);
        shape.push_prefix_input(live_resolved_facts(
            fixture.context_mut(),
            normal_output(other.script, 100_000_000_000),
            Bytes::new(),
        ));
    }

    let tx_input = (config.tx_level_shape == OtxTxLevelShape::SignedSameLock).then(|| {
        shape.push_prefix_input(live_resolved_facts(
            fixture.context_mut(),
            lock_output.clone(),
            Bytes::new(),
        ))
    });

    let base_input = live_resolved_facts(fixture.context_mut(), lock_output.clone(), Bytes::new());
    let append_input =
        live_resolved_facts(fixture.context_mut(), lock_output.clone(), Bytes::new());
    let second_append_input = config
        .seal_shape
        .needs_second_append_segment()
        .then(|| live_resolved_facts(fixture.context_mut(), lock_output, Bytes::new()));
    let (
        base_outputs,
        append_outputs,
        base_cell_deps,
        append_cell_deps,
        base_header_deps,
        append_header_deps,
    ) = if config.preimage_shape.includes_full_preimage() {
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

    let mut append_segments = vec![
        append_segment_spec(0x00)
            .with_inputs(vec![append_input])
            .with_outputs(append_outputs)
            .with_cell_deps(append_cell_deps)
            .with_header_deps(append_header_deps),
    ];
    if let Some(input) = second_append_input {
        append_segments.push(
            append_segment_spec(0x00)
                .with_inputs(vec![input])
                .with_outputs(vec![always_success_output(
                    fixture.context_mut(),
                    94_000_000_000,
                    Bytes::from(vec![0xa1, 0xa2]),
                )]),
        );
    }

    let otx = shape.push_otx(OtxSpec {
        message: (config.tamper == OtxTamper::InvalidActionTarget)
            .then(invalid_action_target_message),
        base_inputs: vec![base_input],
        base_outputs,
        base_cell_deps,
        base_header_deps,
        append_segments,
        base_input_masks: Some(full_base_input_masks(1)),
        base_cell_dep_masks: config
            .preimage_shape
            .includes_full_preimage()
            .then_some(full_base_cell_dep_masks(1)),
        base_header_dep_masks: config
            .preimage_shape
            .includes_full_preimage()
            .then_some(full_base_header_dep_masks(1)),
        ..Default::default()
    });
    let base_input_handle = shape.otx_base_input(otx, 0);
    let signed_append_output = (config.preimage_shape
        == OtxPreimageShape::FullWithSignedAppendOutputMutation)
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
        SignatureScope::OtxAppendSegment {
            otx,
            segment_index: 0,
        },
    );
    let second_append_facts = config.seal_shape.needs_second_append_segment().then(|| {
        sign_scope(
            &built,
            &oracle,
            SignerId("owner"),
            &secret_key,
            contract.script_hash,
            built.otx_witness(otx),
            SignatureScope::OtxAppendSegment {
                otx,
                segment_index: 1,
            },
        )
    });
    match config.seal_shape {
        OtxSealShape::Valid => {
            fill_otx_seals(&mut built, otx, &[base_facts.clone(), append_facts.clone()]);
        }
        OtxSealShape::TwoAppendSegments => {
            let second_append_facts = second_append_facts
                .as_ref()
                .expect("second append facts for two-segment case");
            fill_otx_seals(
                &mut built,
                otx,
                &[
                    base_facts.clone(),
                    append_facts.clone(),
                    second_append_facts.clone(),
                ],
            );
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
        OtxSealShape::DuplicateAppend => {
            fill_otx_seals(
                &mut built,
                otx,
                &[
                    base_facts.clone(),
                    append_facts.clone(),
                    append_facts.clone(),
                ],
            );
        }
        OtxSealShape::MissingSecondAppend => {
            fill_otx_seals(&mut built, otx, &[base_facts.clone(), append_facts.clone()]);
        }
        OtxSealShape::WrongScriptHash => {
            fill_otx_seals_with(
                &mut built,
                otx,
                &[base_facts.clone(), append_facts.clone()],
                Some([0x5au8; 32]),
            );
        }
    }
    let mut signing_facts = vec![base_facts, append_facts.clone()];
    if let Some(facts) = second_append_facts.clone() {
        signing_facts.push(facts);
    }

    if let Some(input) = tx_input {
        let tx_facts = sign_and_fill_tx_level_lock_group(
            &mut built,
            input,
            &secret_key,
            contract.script_hash,
            SignerId("owner"),
        );
        signing_facts.push(tx_facts);
    }

    if config.tamper == OtxTamper::CorruptAppendSeal {
        let mut bad_seal = append_facts.seal.clone();
        bad_seal[0] ^= 0x01;
        built.apply_protocol_mutation(ProtocolMutation::AppendSegmentSealRaw {
            otx,
            segment_index: 0,
            script_hash: contract.script_hash,
            seal: Some(bad_seal),
        });
    }
    if config.tamper == OtxTamper::CorruptSecondAppendSeal {
        let append_facts = second_append_facts
            .as_ref()
            .expect("second append facts for corrupt second append seal");
        let mut bad_seal = append_facts.seal.clone();
        bad_seal[0] ^= 0x01;
        built.apply_protocol_mutation(ProtocolMutation::AppendSegmentSealRaw {
            otx,
            segment_index: 1,
            script_hash: contract.script_hash,
            seal: Some(bad_seal),
        });
    }
    if config.tamper == OtxTamper::MalformedPermissions {
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

    let expected = if config.tx_level_shape == OtxTxLevelShape::UnsignedOutsideSameLock {
        lock_exit(
            outside_same_lock_input.expect("outside same-lock input handle"),
            CobuildOtxLockError::InvalidLockGroupWitness,
        )
    } else if config.tamper == OtxTamper::InvalidActionTarget {
        lock_exit(base_input_handle, CobuildOtxLockError::InvalidMessageTarget)
    } else if matches!(
        config.seal_shape,
        OtxSealShape::MissingBase
            | OtxSealShape::MissingAppend
            | OtxSealShape::WrongScriptHash
            | OtxSealShape::MissingSecondAppend
    ) {
        lock_exit(base_input_handle, CobuildOtxLockError::MissingLockSeal)
    } else if matches!(
        config.seal_shape,
        OtxSealShape::DuplicateBase | OtxSealShape::DuplicateAppend
    ) {
        lock_exit(base_input_handle, CobuildOtxLockError::DuplicateLockSeal)
    } else if config.tamper == OtxTamper::CorruptAppendSeal
        || config.tamper == OtxTamper::CorruptSecondAppendSeal
        || config.preimage_shape == OtxPreimageShape::FullWithSignedAppendOutputMutation
    {
        lock_exit(base_input_handle, CobuildOtxLockError::BadSeal)
    } else if config.tamper == OtxTamper::MalformedPermissions {
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
