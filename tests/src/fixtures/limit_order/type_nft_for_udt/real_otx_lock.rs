use super::*;

use crate::{
    fixtures::{
        cobuild_otx_lock::CobuildOtxLockError,
        common::contracts::{build_cobuild_otx_lock, deploy_cobuild_otx_lock_code},
    },
    framework::{
        scenario::{ExpectedOutcome, ScriptLocation},
        signing::{
            SignatureScope, SignerId, TestSigningHashOracle, fixed_secret_key, public_key_hash20,
            sign_scope,
        },
        tx::{ProtocolMutation, TxShapeMutation},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RealOtxLockCase {
    SignedBase,
    MissingBaseSeal,
    BadBaseSeal,
    TamperBaseOutput,
}

pub(super) fn real_otx_lock_cases() -> Vec<BuiltLimitOrderCase> {
    vec![
        real_otx_lock_case(RealOtxLockCase::SignedBase),
        real_otx_lock_case(RealOtxLockCase::MissingBaseSeal),
        real_otx_lock_case(RealOtxLockCase::BadBaseSeal),
        real_otx_lock_case(RealOtxLockCase::TamperBaseOutput),
    ]
}

fn real_otx_lock_case(case: RealOtxLockCase) -> BuiltLimitOrderCase {
    let secret_key = fixed_secret_key(78);
    let mut fixture = CobuildTestFixture::new();
    let limit_order = fixture.deploy_limit_order();
    let otx_lock_code = deploy_cobuild_otx_lock_code(fixture.context_mut());
    let otx_lock = build_cobuild_otx_lock(
        fixture.context_mut(),
        &otx_lock_code,
        &public_key_hash20(&secret_key),
    );
    let owner_success = deploy_always_success(fixture.context_mut(), b"owner".to_vec());
    let buyer_success = deploy_always_success(fixture.context_mut(), b"buyer".to_vec());
    let owner_lock = owner_success.script.clone();
    let buyer_lock = buyer_success.script.clone();
    let issuer_lock_hash = script_hash(&owner_success.script);
    let proxy_lock = deploy_input_type_proxy_lock(fixture.context_mut(), limit_order.script_hash);
    let nft = deploy_test_nft(fixture.context_mut(), NFT_TYPE_ARGS);
    let udt = deploy_test_udt(fixture.context_mut(), issuer_lock_hash);

    let otx_lock_input = live_resolved_facts(
        fixture.context_mut(),
        normal_output(otx_lock.script.clone(), 100_000_000_000),
        Bytes::new(),
    );
    let order_input = limit_order_input(
        &mut fixture,
        owner_lock.clone(),
        nft.script_hash,
        udt.script_hash,
        30,
        &limit_order.script,
    );
    let nft_payload = nft_data(b"type-real-otx-lock-order", [1, 2, 3, 4], 1_717_171_717);
    let nft_input = live_resolved_facts(
        fixture.context_mut(),
        typed_output(
            proxy_lock.script.clone(),
            nft.script.clone(),
            100_000_000_000,
        ),
        nft_payload.clone(),
    );
    let udt_input = live_resolved_facts(
        fixture.context_mut(),
        typed_output(buyer_lock.clone(), udt.script.clone(), 100_000_000_000),
        udt_amount_data(30),
    );
    let otx_lock_change = TestCellOutput::new(
        normal_output(otx_lock.script.clone(), 90_000_000_000),
        Bytes::new(),
    );
    let nft_output = TestCellOutput::new(
        typed_output(buyer_lock.clone(), nft.script.clone(), 90_000_000_000),
        nft_payload,
    );
    let payment_output = TestCellOutput::new(
        typed_output(owner_lock, udt.script.clone(), 90_000_000_000),
        udt_amount_data(30),
    );

    let mut shape = TxShape::new();
    push_deps(
        &mut shape,
        [
            &limit_order,
            &otx_lock_code,
            &owner_success,
            &buyer_success,
            &proxy_lock,
            &nft,
            &udt,
        ],
    );
    let otx = shape.push_otx(OtxSegment {
        base_inputs: vec![otx_lock_input, order_input, nft_input],
        base_outputs: vec![otx_lock_change, nft_output],
        append_segments: vec![
            append_segment_spec(0x00)
                .with_inputs(vec![udt_input])
                .with_outputs(vec![payment_output]),
        ],
        ..Default::default()
    });
    let otx_lock_input = shape.otx_base_input(otx, 0);
    let otx_lock_base_output = shape.otx_base_output(otx, 0);
    let payment = shape.otx_append_output(otx, 0);
    let mut built = shape.build();
    let message = fill_message(
        &fixture,
        limit_order.script_hash,
        payment,
        script_hash(&buyer_lock),
        &built,
    );
    replace_otx_message(&mut built, otx, message);

    let oracle = TestSigningHashOracle;
    let base_facts = sign_scope(
        &built,
        &oracle,
        SignerId("limit_order_type_real_otx_lock"),
        &secret_key,
        otx_lock.script_hash,
        built.otx_witness(otx),
        SignatureScope::OtxBase { otx },
    );
    if case != RealOtxLockCase::MissingBaseSeal {
        let mut seal = base_facts.seal.clone();
        if case == RealOtxLockCase::BadBaseSeal {
            seal[0] ^= 0x01;
        }
        built.apply_protocol_mutation(ProtocolMutation::BaseSealRaw {
            otx,
            script_hash: otx_lock.script_hash,
            seal: Some(seal),
        });
    }
    if case == RealOtxLockCase::TamperBaseOutput {
        built.apply_shape_mutation(TxShapeMutation::ReplaceOutput {
            output: otx_lock_base_output,
            replacement: TestCellOutput::new(
                normal_output(otx_lock.script.clone(), 90_000_000_001),
                Bytes::new(),
            ),
        });
    }

    let expected = match case {
        RealOtxLockCase::SignedBase => LimitOrderExpectedOutcome::Pass,
        RealOtxLockCase::MissingBaseSeal => {
            otx_lock_error(otx_lock_input, CobuildOtxLockError::MissingLockSeal)
        }
        RealOtxLockCase::BadBaseSeal | RealOtxLockCase::TamperBaseOutput => {
            otx_lock_error(otx_lock_input, CobuildOtxLockError::BadSeal)
        }
    };

    BuiltLimitOrderCase {
        name: format!("real_otx_lock::{case:?}"),
        fixture,
        built,
        signing_facts: vec![base_facts],
        expected,
        coverage: vec![coverage(
            FlowKind::OtxOnly,
            ScriptRoleKind::InputType,
            OtxScopeKind::BaseInput,
            super::ActionSourceKind::Otx,
            None,
        )],
    }
}

fn otx_lock_error(input: InputHandle, error: CobuildOtxLockError) -> LimitOrderExpectedOutcome {
    LimitOrderExpectedOutcome::Framework(ExpectedOutcome::ScriptExit {
        location: ScriptLocation::InputLock(input),
        code: error.code(),
    })
}
